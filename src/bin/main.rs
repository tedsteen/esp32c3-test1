#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use esp32c3_test1::ball::Ball;
use esp32c3_test1::font;
use esp32c3_test1::highscore::HighScore;
use esp32c3_test1::pad::{Pad, PadPosition};

use core::fmt::Write;
use core::sync::atomic::AtomicBool;
use embassy_time::{Duration, Instant, Timer};
use esp32c3_test1::dot_matrix::DotMatrix;
use esp32c3_test1::text_ticker::TextTicker;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Pull};
use esp_hal::timer::systimer::SystemTimer;
use heapless::String;
use log::{error, info};
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static BTN: AtomicBool = AtomicBool::new(false);

enum GameState {
    New(),
    Intro(TextTicker<100>),
    Countdown(i64),
    Playing { ball: Ball, pad: Pad, score: u32 },
    GameOver(TextTicker<100>),
}

impl GameState {
    fn start_new_game(&mut self) {
        *self = Self::Playing {
            ball: Ball::new(3, 3),
            pad: Pad::new(PadPosition::Bottom(1.0)),
            score: 0,
        }
    }

    async fn tick(
        &mut self,
        delta_time_ms: u64,
        highscore: &mut HighScore,
        dot_matrix: &mut DotMatrix<'_>,
    ) {
        if let GameState::Playing {
            pad: Pad::Dead,
            score,
            ..
        } = self
        {
            let message = if *score > highscore.get() {
                highscore.set(*score);
                "New highscore!"
            } else {
                "Score"
            };

            info!("Score: {message} {score}");
            let mut game_over_ticker_text = String::<100>::new();
            if write!(game_over_ticker_text, "{message} {score} ").is_err() {
                error!("Failed to write game over ticker string");
            };

            *self = GameState::GameOver(TextTicker::new(game_over_ticker_text, 0.014));
        }

        match self {
            GameState::New() => {
                let mut highscore_ticker_text = String::<100>::new();
                if write!(highscore_ticker_text, "Highscore:{} ", highscore.get()).is_err() {
                    error!("Failed to write to highscore ticker string");
                }
                *self = GameState::Intro(TextTicker::new(highscore_ticker_text, 0.014));
            }
            GameState::Intro(highscore) => {
                highscore.update(delta_time_ms);
                highscore.draw(dot_matrix);
                dot_matrix.flush_buffer_to_spi();
            }
            GameState::Countdown(countdown) => {
                *countdown -= delta_time_ms as i64;
                let countdown_as_secs = 1 + (*countdown / 1000);
                let countdown_as_bitmap =
                    *font::get_font_data(&((b'0' + countdown_as_secs as u8) as char))
                        .expect("a font for a number");

                dot_matrix.draw(&countdown_as_bitmap);
                dot_matrix.shift(2, 1);
                dot_matrix.flush_buffer_to_spi();
                dot_matrix.clear();

                if *countdown <= 0 {
                    self.start_new_game();
                }
            }
            GameState::Playing { ball, pad, score } => {
                pad.update(delta_time_ms);
                ball.update(pad, delta_time_ms, score);

                dot_matrix.clear();

                pad.draw(dot_matrix);
                ball.draw(dot_matrix);
                dot_matrix.flush_buffer_to_spi();
            }
            GameState::GameOver(text_ticker) => {
                text_ticker.update(delta_time_ms);
                text_ticker.draw(dot_matrix);
                dot_matrix.flush_buffer_to_spi();
            }
        }
    }
}

#[embassy_executor::task]
async fn game_loop(mut dot_matrix: DotMatrix<'static>, mut highscore: HighScore) {
    info!("Starting game loop!");
    let mut last_tick = Instant::now();
    let mut game_state = GameState::New();
    loop {
        let now = Instant::now();
        let delta_time_ms = now.duration_since(last_tick).as_millis();
        last_tick = now;
        if BTN.load(core::sync::atomic::Ordering::Relaxed) {
            BTN.store(false, core::sync::atomic::Ordering::Relaxed);
            match &mut game_state {
                GameState::New() => {}
                GameState::Intro(_) | GameState::GameOver(_) => {
                    game_state = GameState::Countdown(3000);
                }
                GameState::Countdown(_) => {}
                GameState::Playing { pad, .. } => {
                    if let Pad::Alive { position, .. } = pad {
                        position.next();
                    }
                }
            }
        }

        game_state
            .tick(delta_time_ms, &mut highscore, &mut dot_matrix)
            .await;

        Timer::after(Duration::from_millis(16)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    let dot_matrix = DotMatrix::new(
        peripherals.SPI2,
        peripherals.GPIO0,
        peripherals.GPIO1,
        peripherals.GPIO2,
    );

    // let _audio = audio::Audio::new(
    //     peripherals.DMA_CH2,
    //     peripherals.I2S0,
    //     peripherals.GPIO3,
    //     peripherals.GPIO4,
    //     peripherals.GPIO5,
    // );

    let mut button = Input::new(
        peripherals.GPIO9,
        InputConfig::default().with_pull(Pull::Up),
    );
    let mut highscore = HighScore::default();
    if button.is_low() {
        info!("Resetting highscore");
        highscore.set(0);
    }

    spawner.spawn(game_loop(dot_matrix, highscore)).ok();

    info!("Starting main loop!");
    loop {
        button.wait_for_low().await;
        BTN.store(true, core::sync::atomic::Ordering::Relaxed);
        Timer::after_millis(50).await;
        button.wait_for_high().await;
        Timer::after_millis(50).await;
    }
}
