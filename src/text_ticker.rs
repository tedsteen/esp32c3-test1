use heapless::String;
use libm::ceilf;

use crate::{
    dot_matrix::DotMatrix,
    font::{get_font_data, FONT_HEIGHT, FONT_WIDTH},
};

pub struct TextTicker<const N: usize> {
    text: String<N>,
    scroll_position: f32,
    scroll_speed: f32,
}

impl<const N: usize> TextTicker<N> {
    pub const fn new(text: String<N>, scroll_speed: f32) -> Self {
        Self {
            text,
            scroll_position: 0.0,
            scroll_speed,
        }
    }

    pub fn update(&mut self, delta_time_ms: u64) {
        self.scroll_position += delta_time_ms as f32 * self.scroll_speed;
    }

    fn get_font_data(str: &str, idx: usize) -> [u8; FONT_HEIGHT] {
        let char = str.chars().nth(idx % str.len()).unwrap();
        let font = get_font_data(&char); //font8x8::BASIC_FONTS.get_font(char).unwrap();
        font.copied().unwrap_or_default()
    }

    pub fn draw(&self, dot_matrix: &mut DotMatrix) {
        if self.text.len() == 0 {
            return;
        }

        let width = FONT_WIDTH + 1;

        let text_idx = self.scroll_position as usize / width as usize;
        let x_offs = self.scroll_position as u32 % width as u32;
        let y_offs = (8 - FONT_HEIGHT) / 2;
        let max_chars = ceilf((8 + width) as f32 / width as f32) as u8;

        //println!("max_chars: {max_chars}");
        let mut screen = [0_u8; 8];
        for char in 0..max_chars {
            let font_data = Self::get_font_data(self.text.as_str(), text_idx + char as usize);
            let shift = width as i8 * char as i8 - x_offs as i8;
            //println!("shift: {shift}");
            for y in 0..FONT_HEIGHT {
                if shift < 0 {
                    screen[y + y_offs] |= font_data[y] << shift.abs_diff(0)
                } else if (0..8).contains(&shift) {
                    screen[y + y_offs] |= font_data[y] >> shift
                }
            }
        }
        dot_matrix.draw(&screen);
    }
}
