#![no_std]
#![no_main]

use core::borrow::BorrowMut;

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

mod dot_matrix;

static DOT_MATRIX: Mutex<CriticalSectionRawMutex, Option<DotMatrix<'_>>> = Mutex::new(None);

#[derive(Debug, Clone)]
enum PadPosition {
    Left,
    Right,
    Top,
    Bottom,
}

impl PadPosition {
    fn next(&mut self) {
        *self = match self {
            PadPosition::Left => PadPosition::Top,
            PadPosition::Right => PadPosition::Bottom,
            PadPosition::Top => PadPosition::Right,
            PadPosition::Bottom => PadPosition::Left,
        }
    }

    fn draw(&mut self, dot_matrix: &mut DotMatrix) {
        match self {
            PadPosition::Left => {
                for y in 0..8 {
                    dot_matrix.put(0, y);
                }
            }
            PadPosition::Right => {
                for y in 0..8 {
                    dot_matrix.put(7, y);
                }
            }
            PadPosition::Top => {
                dot_matrix.set_row(0, 0b11111111);
            }
            PadPosition::Bottom => {
                dot_matrix.set_row(7, 0b11111111);
            }
        }
    }
}
const MAX_HEALTH: u8 = 4;

#[derive(Clone, Debug)]
enum PadAliveState {
    Normal,
    Hurting(i64),
    Dying(i64),
}

#[derive(Clone, Debug)]
enum PadState {
    Alive {
        state: PadAliveState,
        position: PadPosition,
        health: u8,
    },
    Dead,
}

impl PadState {
    const fn new() -> Self {
        Self::Alive {
            state: PadAliveState::Normal,
            position: PadPosition::Bottom,
            health: MAX_HEALTH,
        }
    }

    fn take_damage(&mut self) {
        if let PadState::Alive {
            health,
            state: alive_state,
            ..
        } = self
        {
            *health -= 1;
            info!("Health: {}", health);
            if *health == 0 {
                info!("YOU DED!");
                *alive_state = PadAliveState::Dying(1000)
            } else {
                *alive_state = PadAliveState::Hurting(200);
            }
        }
    }

    fn update(&mut self, delta_time_ms: u64) -> PadUpdateResult {
        match self {
            PadState::Alive {
                state: alive_state, ..
            } => {
                match alive_state {
                    PadAliveState::Normal => {}
                    PadAliveState::Hurting(countdown) => {
                        *countdown -= delta_time_ms as i64;
                        if *countdown <= 0_i64 {
                            *alive_state = PadAliveState::Normal
                        }
                    }
                    PadAliveState::Dying(countdown) => {
                        *countdown -= delta_time_ms as i64;
                        if *countdown <= 0_i64 {
                            *self = PadState::Dead;
                        }
                    }
                };
            }
            PadState::Dead => {}
        }
        PadUpdateResult {
            pad_state: self.clone(),
        }
    }
}

#[derive(Debug)]
struct PadUpdateResult {
    pad_state: PadState,
}

struct BallUpdateResult {
    x: u8,
    y: u8,
}

#[derive(Clone)]
struct Ball {
    x: f32,
    y: f32,
    x_speed: f32,
    y_speed: f32,
}
impl Ball {
    const fn new(initial_x: u8, initial_y: u8) -> Self {
        Self {
            x: initial_x as f32,
            y: initial_y as f32,
            x_speed: 0.0054,
            y_speed: -0.004,
        }
    }

    fn update(&mut self, pad: &mut PadState, delta_time_ms: u64) -> BallUpdateResult {
        let mut pad_hit = false;
        if let PadState::Alive {
            position,
            state: PadAliveState::Normal,
            ..
        } = &pad
        {
            // Check X collision
            if self.x_speed < 0.0 {
                let min_x = if matches!(position, PadPosition::Left) {
                    1.5
                } else {
                    0.5
                };
                if self.x < min_x {
                    pad_hit = matches!(position, PadPosition::Left);
                    self.x = min_x;
                    self.x_speed *= -1.0;
                }
            } else {
                let max_x = if matches!(position, PadPosition::Right) {
                    6.5
                } else {
                    7.5
                };

                if self.x >= max_x {
                    pad_hit = matches!(position, PadPosition::Right);
                    self.x = max_x;
                    self.x_speed *= -1.0;
                }
            }

            // Check Y collision
            if self.y_speed < 0.0 {
                let min_y = if matches!(position, PadPosition::Top) {
                    1.5
                } else {
                    0.5
                };
                if self.y < min_y {
                    pad_hit = matches!(position, PadPosition::Top);
                    self.y = min_y;
                    self.y_speed *= -1.0;
                }
            } else {
                let max_y = if matches!(position, PadPosition::Bottom) {
                    6.5
                } else {
                    7.5
                };
                if self.y >= max_y {
                    pad_hit = matches!(position, PadPosition::Bottom);
                    self.y = max_y;
                    self.y_speed *= -1.0;
                }
            }
            if pad_hit {
                pad.take_damage();
            }
            self.x += self.x_speed * delta_time_ms as f32;
            self.y += self.y_speed * delta_time_ms as f32;
        }
        BallUpdateResult {
            x: self.x as u8,
            y: self.y as u8,
        }
    }
}

static GAME_STATE: Mutex<CriticalSectionRawMutex, GameState> = Mutex::new(GameState::new());
struct GameState {
    ball: Ball,
    pad: PadState,
}

impl GameState {
    const fn new() -> Self {
        Self {
            ball: Ball::new(3, 3),
            pad: PadState::new(),
        }
    }
    fn update(&mut self, delta_time_ms: u64) -> (PadUpdateResult, BallUpdateResult) {
        (
            self.pad.update(delta_time_ms),
            self.ball.update(&mut self.pad, delta_time_ms),
        )
    }
}

#[embassy_executor::task]
async fn game_loop() {
    let mut last_tick = now().duration_since_epoch().to_millis();
    loop {
        let now = now().duration_since_epoch().to_millis();
        let delta_time_ms = now - last_tick;
        last_tick = now;
        let (mut pad_update_result, ball_update_result) =
            GAME_STATE.lock().await.borrow_mut().update(delta_time_ms);

        if let Some(dot_matrix) = DOT_MATRIX.lock().await.as_mut() {
            dot_matrix.clear();
            match &mut pad_update_result.pad_state {
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

                PadState::Dead => {
                    // TODO: Show score
                }
            }
            dot_matrix.put(ball_update_result.x, ball_update_result.y);
            dot_matrix.flush_buffer_to_spi();
        }
        Timer::after(Duration::from_millis(16)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let mosi = Input::new(peripherals.GPIO0, Pull::Down); //DIN
    let cs = Input::new(peripherals.GPIO1, Pull::Down); //CS
    let sclk = Input::new(peripherals.GPIO2, Pull::Down); //CLK

    *DOT_MATRIX.lock().await = Some(DotMatrix::new(mosi, cs, sclk, peripherals.SPI2));

    //let mut rng = Rng::new(peripherals.RNG);

    let mut button = Input::new(peripherals.GPIO9, Pull::Down);
    info!("Init!");

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    spawner.spawn(game_loop()).ok();

    loop {
        let _ = button.wait_for_falling_edge().await;

        if let PadState::Alive { position, .. } = &mut GAME_STATE.lock().await.borrow_mut().pad {
            position.next();
        }
    }
}
