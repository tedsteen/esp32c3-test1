#![no_std]
#![no_main]

use core::borrow::BorrowMut;

use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    gpio::{Input, Pull},
    spi::master::Spi,
    time::now,
    timer::timg::TimerGroup,
    Blocking,
};
use esp_println::logger::init_logger_from_env;
use log::info;

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
        //TODO
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

struct DotMatrix<'a> {
    buffer: [u8; 8],
    spi: Spi<'a, Blocking>,
}

impl<'a> DotMatrix<'a> {
    fn new(spi: Spi<'a, Blocking>) -> Self {
        Self {
            buffer: [0; 8],
            spi,
        }
    }

    fn clear(&mut self) {
        for addr in 1..=8 {
            self.buffer[addr - 1] = 0;
        }
    }

    fn put(&mut self, x: u8, y: u8) {
        assert!((0..8).contains(&x), "x must be between 0 and 7");
        assert!((0..8).contains(&y), "y must be between 0 and 7");
        let column = (0b10000000 >> x) as u8;
        let row = y;
        self.buffer[row as usize] |= column;
    }

    fn set_row(&mut self, row: u8, row_data: u8) {
        self.buffer[row as usize] = row_data;
    }

    fn flush_buffer_to_spi(&mut self) {
        for i in 0..8 {
            self.spi
                .write_bytes(&[i + 1, self.buffer[i as usize]])
                .expect("buffer to be written to spi");
            self.buffer[i as usize] = 0;
        }
    }
}

type DotMatrixMutex<'a> = Mutex<CriticalSectionRawMutex, Option<DotMatrix<'a>>>;
static DOT_MATRIX: DotMatrixMutex = Mutex::new(None);

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
            y_speed: 0.004,
        }
    }

    fn game_over(&mut self) {
        info!("YOU DED!");
        //self.x_speed = 0.0;
        //self.y_speed = 0.0;
    }

    fn update(&mut self, pad_position: &PadPosition, delta_time_ms: u64) {
        let min_x = if matches!(pad_position, PadPosition::Left) {
            1.5
        } else {
            0.5
        };
        let min_y = if matches!(pad_position, PadPosition::Top) {
            1.5
        } else {
            0.5
        };
        let max_x = if matches!(pad_position, PadPosition::Right) {
            6.5
        } else {
            7.5
        };
        let max_y = if matches!(pad_position, PadPosition::Bottom) {
            6.5
        } else {
            7.5
        };

        if self.x >= max_x {
            if !matches!(pad_position, PadPosition::Right) {
                self.game_over();
            }
            self.x = max_x;
            self.x_speed *= -1.0;
        }
        if self.x < min_x {
            if !matches!(pad_position, PadPosition::Left) {
                self.game_over();
            }
            self.x = min_x;
            self.x_speed *= -1.0;
        }
        if self.y >= max_y {
            if !matches!(pad_position, PadPosition::Bottom) {
                self.game_over();
            }
            self.y = max_y;
            self.y_speed *= -1.0;
        }
        if self.y < min_y {
            if !matches!(pad_position, PadPosition::Top) {
                self.game_over();
            }
            self.y = min_y;
            self.y_speed *= -1.0;
        }
        self.x += self.x_speed * delta_time_ms as f32;
        self.y += self.y_speed * delta_time_ms as f32;
    }

    fn draw(&self, dot_matrix: &mut DotMatrix) {
        dot_matrix.put(self.x as u8, self.y as u8);
    }
}

static GAME_STATE: Mutex<CriticalSectionRawMutex, GameState> = Mutex::new(GameState::new());
struct GameState {
    ball: Ball,
    pad_position: PadPosition,
}

impl GameState {
    const fn new() -> Self {
        Self {
            ball: Ball::new(3, 3),
            pad_position: PadPosition::Bottom,
        }
    }
    fn update(&mut self, delta_time_ms: u64) -> (Ball, PadPosition) {
        self.ball.update(&self.pad_position, delta_time_ms);

        (self.ball.clone(), self.pad_position.clone())
    }
}

#[embassy_executor::task]
async fn game_loop() {
    let mut last_tick = now().duration_since_epoch().to_millis();
    loop {
        let now = now().duration_since_epoch().to_millis();
        let delta_time_ms = now - last_tick;
        last_tick = now;
        let (ball, mut pad_position) = GAME_STATE.lock().await.borrow_mut().update(delta_time_ms);
        if let Some(dot_matrix) = DOT_MATRIX.lock().await.as_mut() {
            dot_matrix.clear();

            ball.draw(dot_matrix);
            pad_position.draw(dot_matrix);
            dot_matrix.flush_buffer_to_spi();
        }
        Timer::after(Duration::from_millis(16)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default());

    // See this: https://dev.to/theembeddedrustacean/esp32-standard-library-embedded-rust-spi-with-the-max7219-led-dot-matrix-1ge0
    // and this: https://dev.to/theembeddedrustacean/esp32-embedded-rust-at-the-hal-spi-communication-30a4
    use esp_hal::spi::SpiMode;
    let mosi = Input::new(peripherals.GPIO0, Pull::Down); //DIN
    let cs = Input::new(peripherals.GPIO1, Pull::Down); //CS
    let sclk = Input::new(peripherals.GPIO2, Pull::Down); //CLK

    let mut spi = esp_hal::spi::master::Spi::new_with_config(
        peripherals.SPI2,
        esp_hal::spi::master::Config {
            frequency: fugit::HertzU32::MHz(2),
            mode: SpiMode::Mode0,
            read_bit_order: esp_hal::spi::SpiBitOrder::MSBFirst,
            write_bit_order: esp_hal::spi::SpiBitOrder::MSBFirst,
        },
    )
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_cs(cs);

    // Power Up Device
    spi.write_bytes(&[0x0C, 0x01]).expect("bytes to be written");

    // Set up Decode Mode
    spi.write_bytes(&[0x09, 0x00]).expect("bytes to be written");

    //Configure Scan Limit
    spi.write_bytes(&[0x0b, 0x07]).expect("bytes to be written");

    //Configure Intensity
    spi.write_bytes(&[0x0a, 0x9a]).expect("bytes to be written");

    *DOT_MATRIX.lock().await = Some(DotMatrix::new(spi));

    //let mut rng = Rng::new(peripherals.RNG);

    let mut button = Input::new(peripherals.GPIO9, Pull::Down);
    info!("Init!");

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    spawner.spawn(game_loop()).ok();

    loop {
        let _ = button.wait_for_falling_edge().await;
        GAME_STATE.lock().await.pad_position.next();
    }
}
