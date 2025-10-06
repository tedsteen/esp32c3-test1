use embedded_storage::{ReadStorage, Storage};
use esp_storage::FlashStorage;
use log::{debug, info};

const FLASH_ADDR: u32 = 0x9000;
const HEADER: &[u8; 5] = b"m3rra";

pub struct HighScore {
    flash_storage: FlashStorage,
    score: Option<u32>,
}

impl HighScore {
    pub fn get(&mut self) -> u32 {
        if self.score.is_none() {
            let buffer = &mut [0_u8; HEADER.len()];
            self.flash_storage
                .read(FLASH_ADDR, buffer)
                .expect("to be able to read header");

            let correct_header = buffer.iter().zip(HEADER.iter()).all(|(a, b)| a == b);
            if !correct_header {
                info!("No previous high score, creating a new one");
                self.flash_storage
                    .write(FLASH_ADDR, HEADER)
                    .expect("a header to be written");
                self.score = Some(0);
            } else {
                let high_score = &mut [0_u8; size_of::<u32>()];
                self.flash_storage
                    .read(FLASH_ADDR + HEADER.len() as u32, high_score)
                    .expect("bytes to be highscore");
                self.score = Some(u32::from_be_bytes(*high_score));
            }
        };

        self.score.expect("a highscore")
    }

    pub fn set(&mut self, score: u32) {
        if self.get() != score {
            debug!("Writing new score to flash ({} -> {})", self.get(), score);
            self.flash_storage
                .write(FLASH_ADDR + HEADER.len() as u32, &score.to_be_bytes())
                .expect("a highscore to be written");
            self.score = Some(score);
        }
    }
}

impl Default for HighScore {
    fn default() -> Self {
        let flash_storage = FlashStorage::new();

        info!("Flash size = {}", flash_storage.capacity());

        Self {
            flash_storage,
            score: None,
        }
    }
}
