#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use esp32c3_test1::game_state::GameState;
use esp32c3_test1::highscore::HighScore;

use core::sync::atomic::AtomicBool;
use embassy_time::{Duration, Instant, Timer};
use esp32c3_test1::dot_matrix::DotMatrix;
use esp32c3_test1::text_ticker::TextTicker;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Pull};
use esp_hal::timer::systimer::SystemTimer;
use heapless::{format, String};
use log::{error, info};
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static BTN_DOWN: AtomicBool = AtomicBool::new(false);

#[embassy_executor::task]
async fn game_loop(
    mut dot_matrix: DotMatrix<'static>,
    mut highscore: HighScore,
    intro_message_override: Option<&'static str>,
) {
    info!("Starting game loop!");
    let mut last_tick = Instant::now();

    let mut game_state = GameState::Intro(TextTicker::new(
        intro_message_override
            .map(|s| String::try_from(s).expect("a string"))
            .unwrap_or_else(|| format!(" Highscore:{}", highscore.get()).expect("a string")),
        0.008,
    ));
    loop {
        let now = Instant::now();
        let delta_time_ms = now.duration_since(last_tick).as_millis();

        if BTN_DOWN.load(core::sync::atomic::Ordering::Relaxed) {
            game_state.button_click();
            BTN_DOWN.store(false, core::sync::atomic::Ordering::Relaxed);
        }

        match game_state.tick(delta_time_ms, &mut highscore, &mut dot_matrix) {
            Ok(_) => {
                last_tick = now;
            }
            Err(e) => error!("Failed to advance game state: {e:?}"),
        }

        Timer::after(Duration::from_millis(2)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    match DotMatrix::new(
        peripherals.SPI2,
        peripherals.GPIO0,
        peripherals.GPIO1,
        peripherals.GPIO2,
    ) {
        Ok(dot_matrix) => {
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
            let mut intro_text = None;
            if button.is_low() {
                info!("Resetting highscore");
                intro_text = Some(" RESET HIGHSCORE");
                highscore.set(0);
            }

            spawner
                .spawn(game_loop(dot_matrix, highscore, intro_text))
                .ok();

            button.wait_for_high().await; // If highscore reset then wait for the button to be released

            info!("Starting main loop!");
            loop {
                button.wait_for_low().await;
                BTN_DOWN.store(true, core::sync::atomic::Ordering::Relaxed);
                Timer::after_millis(50).await;
                button.wait_for_high().await;
                Timer::after_millis(50).await;
            }
        }
        Err(e) => {
            panic!("Failed to setup dot matrix display: {e:?}");
        }
    }
}
