#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    gpio::{Input, Level, Output, Pull},
    rng::Rng,
    timer::timg::TimerGroup,
};
use esp_println::logger::init_logger_from_env;
use log::info;

type LedMutex<'a> = Mutex<CriticalSectionRawMutex, Option<Output<'a>>>;
static LED: LedMutex = Mutex::new(None);

#[embassy_executor::task]
async fn update_led(mut rng: Rng) {
    loop {
        if let Some(led) = LED.lock().await.as_mut() {
            led.set_high();
        };
        let delay = 200 + ((rng.random() as f32 / u32::MAX as f32) * 500.0) as u64;

        Timer::after(Duration::from_millis(delay)).await;

        if let Some(led) = LED.lock().await.as_mut() {
            led.set_low();
        }
        let delay = 1000 + ((rng.random() as f32 / u32::MAX as f32) * 3000.0) as u64;
        Timer::after(Duration::from_millis(delay)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default());

    // See this: https://dev.to/theembeddedrustacean/esp32-standard-library-embedded-rust-spi-with-the-max7219-led-dot-matrix-1ge0
    // and this: https://dev.to/theembeddedrustacean/esp32-embedded-rust-at-the-hal-spi-communication-30a4
    // use esp_hal::spi::SpiMode;
    // let sclk = Input::new(peripherals.GPIO0, Pull::Down);
    // let mosi = Input::new(peripherals.GPIO2, Pull::Down);
    // let cs = Input::new(peripherals.GPIO3, Pull::Down);

    // let mut spi = esp_hal::spi::master::Spi::new_with_config(
    //     peripherals.SPI2,
    //     esp_hal::spi::master::Config {
    //         frequency: fugit::HertzU32::MHz(2),
    //         mode: SpiMode::Mode0,
    //         read_bit_order: esp_hal::spi::SpiBitOrder::LSBFirst,
    //         write_bit_order: esp_hal::spi::SpiBitOrder::LSBFirst,
    //     },
    // )
    // .with_sck(sclk)
    // .with_mosi(mosi)
    // .with_cs(cs);

    // spi.write_byte(3).expect("a byte to be written");

    *LED.lock().await = Some(Output::new(peripherals.GPIO7, Level::Low));

    let rng = Rng::new(peripherals.RNG);

    let mut button = Input::new(peripherals.GPIO9, Pull::Down);
    info!("Init!");

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    spawner.spawn(update_led(rng)).ok();

    let mut points = 0;
    loop {
        let _ = button.wait_for_falling_edge().await;
        if let Some(led) = LED.lock().await.as_ref() {
            if led.is_set_high() {
                points += 1;
            } else {
                info!("FAIL!");
                points = 0;
            }
            info!("Points: {}", points);
        }
    }
}
