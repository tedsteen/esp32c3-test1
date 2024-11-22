#![no_std]
#![no_main]

use core::{borrow::BorrowMut, ops::DerefMut};

use ball::Ball;
use dot_matrix::DotMatrix;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    gpio::{Input, Pull},
    time::now,
    timer::timg::TimerGroup,
};
use esp_println::logger::init_logger_from_env;
use log::info;
use pad::{PadAliveState, PadState};

mod ball;
mod dot_matrix;
mod pad;

static DOT_MATRIX: Mutex<CriticalSectionRawMutex, Option<DotMatrix<'_>>> = Mutex::new(None);
static GAME_STATE: Mutex<CriticalSectionRawMutex, GameState> = Mutex::new(GameState::new());

enum GameState {
    Playing { ball: Ball, pad: PadState },
    GameOver,
}

impl GameState {
    const fn new() -> Self {
        Self::Playing {
            ball: Ball::new(3, 3),
            pad: PadState::new(),
        }
    }
    async fn tick(&mut self, delta_time_ms: u64) {
        match self {
            GameState::Playing { ball, pad } => {
                let pad_update_result = pad.update(delta_time_ms);
                let ball_update_result = ball.update(pad, delta_time_ms);

                if let Some(dot_matrix) = DOT_MATRIX.lock().await.as_mut() {
                    dot_matrix.clear();
                    match pad_update_result.pad_state {
                        PadState::Alive {
                            position,
                            state: alive_state,
                            ..
                        } => match &alive_state {
                            PadAliveState::Normal => {
                                position.draw(dot_matrix);
                            }
                            PadAliveState::Hurting(countdown) => {
                                if countdown % 100 < 50 {
                                    dot_matrix.fill();
                                }
                            }
                            PadAliveState::Dying(countdown) => {
                                if countdown % 100 < 50 {
                                    dot_matrix.fill();
                                }
                            }
                        },

                        PadState::Dead => *self = GameState::GameOver,
                    }
                    dot_matrix.put(ball_update_result.x, ball_update_result.y);
                    dot_matrix.flush_buffer_to_spi();
                }
            }
            GameState::GameOver => {
                // TODO: Show score
            }
        }
    }
}

#[embassy_executor::task]
async fn game_loop() {
    info!("Starting game loop!");
    let mut last_tick = now().duration_since_epoch().to_millis();
    loop {
        let now = now().duration_since_epoch().to_millis();
        let delta_time_ms = now - last_tick;
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
        match GAME_STATE.lock().await.deref_mut() {
            GameState::Playing { pad, .. } => {
                if let PadState::Alive { position, .. } = pad {
                    position.next();
                }
            }
            GameState::GameOver => {
                // Restart game?
            }
        }
    }
}
