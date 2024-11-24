#![no_std]
#![no_main]

use core::{borrow::BorrowMut, fmt::Write, ops::DerefMut};

use ball::Ball;
use dot_matrix::DotMatrix;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};
use esp_backtrace as _;
use esp_hal::{
    gpio::{Input, Pull},
    timer::timg::TimerGroup,
};
use esp_println::logger::init_logger_from_env;
use font8x8::UnicodeFonts;
use heapless::String;
use log::{error, info};
use pad::{Pad, PadPosition};
use text_ticker::TextTicker;

mod ball;
mod dot_matrix;
mod pad;
mod text_ticker;

static DOT_MATRIX: Mutex<CriticalSectionRawMutex, Option<DotMatrix<'_>>> = Mutex::new(None);
static GAME_STATE: Mutex<CriticalSectionRawMutex, GameState> = Mutex::new(GameState::new());

enum GameState {
    Intro(i64),
    Playing {
        ball: Ball,
        pad: Pad,
        start_time: Instant,
    },
    GameOver(TextTicker<100>),
}

impl GameState {
    const fn new() -> Self {
        Self::Intro(3000)
    }

    fn start_new_game(&mut self) {
        *self = Self::Playing {
            ball: Ball::new(3, 3),
            pad: Pad::new(PadPosition::Bottom),
            start_time: Instant::now(),
        }
    }

    async fn tick(&mut self, delta_time_ms: u64) {
        if let GameState::Playing {
            pad: Pad::Dead,
            start_time,
            ..
        } = self
        {
            let play_time = Instant::now().duration_since(*start_time);
            info!("Score: {}", play_time.as_secs());
            let mut ticker_text = String::<100>::new();
            if write!(ticker_text, "Points - {} - ", play_time.as_secs()).is_err() {
                error!("Failed to write string");
            };

            *self = GameState::GameOver(TextTicker::new(ticker_text, 0.014));
        }

        match self {
            GameState::Playing { ball, pad, .. } => {
                pad.update(delta_time_ms);
                ball.update(pad, delta_time_ms);

                if let Some(dot_matrix) = DOT_MATRIX.lock().await.as_mut() {
                    dot_matrix.clear();

                    pad.draw(dot_matrix);
                    ball.draw(dot_matrix);
                    dot_matrix.flush_buffer_to_spi();
                }
            }
            GameState::GameOver(text_ticker) => {
                text_ticker.update(delta_time_ms);
                if let Some(dot_matrix) = DOT_MATRIX.lock().await.as_mut() {
                    text_ticker.draw(dot_matrix);
                    dot_matrix.flush_buffer_to_spi();
                }
            }
            GameState::Intro(countdown) => {
                *countdown -= delta_time_ms as i64;
                if let Some(dot_matrix) = DOT_MATRIX.lock().await.as_mut() {
                    let countdown_as_secs = 1 + (*countdown / 1000);
                    let mut countdown_as_bitmap = font8x8::BASIC_FONTS
                        .get_font((b'0' + countdown_as_secs as u8) as char)
                        .unwrap()
                        .1;

                    for row in &mut countdown_as_bitmap {
                        *row = row.reverse_bits() >> 1;
                    }

                    dot_matrix.draw(countdown_as_bitmap);
                    dot_matrix.flush_buffer_to_spi();
                }

                if *countdown <= 0 {
                    self.start_new_game();
                }
            }
        }
    }
}

#[embassy_executor::task]
async fn game_loop() {
    info!("Starting game loop!");
    let mut last_tick = Instant::now();
    loop {
        let now = Instant::now();
        let delta_time_ms = now.duration_since(last_tick).as_millis();
        last_tick = now;
        GAME_STATE
            .lock()
            .await
            .borrow_mut()
            .tick(delta_time_ms)
            .await;

        Timer::after(Duration::from_millis(16)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let mosi = Input::new(peripherals.GPIO0, Pull::Down); //DIN
    let cs = Input::new(peripherals.GPIO1, Pull::Down); //CS
    let sclk = Input::new(peripherals.GPIO2, Pull::Down); //CLK

    *DOT_MATRIX.lock().await = Some(DotMatrix::new(mosi, cs, sclk, peripherals.SPI2));

    //let mut rng = Rng::new(peripherals.RNG);

    let mut button = Input::new(peripherals.GPIO9, Pull::Down);

    spawner.spawn(game_loop()).ok();

    info!("Starting main loop!");
    loop {
        let _ = button.wait_for_falling_edge().await;
        let mut game_state = GAME_STATE.lock().await;
        let game_state = game_state.deref_mut();
        match game_state {
            GameState::Playing { pad, .. } => {
                if let Pad::Alive { position, .. } = pad {
                    position.next();
                }
            }
            GameState::GameOver(_) => {
                *game_state = GameState::new();
            }
            GameState::Intro(_) => {}
        }
    }
}
