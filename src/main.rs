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
    *LED.lock().await = Some(Output::new(peripherals.GPIO7, Level::Low));

    let rng = Rng::new(peripherals.RNG);

    let mut button = Input::new(peripherals.GPIO9, Pull::Down);
    log::info!("Init!");

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
                log::info!("FAIL!");
                points = 0;
            }
            log::info!("Points: {}", points);
        }
    }
}
