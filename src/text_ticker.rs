use font8x8::UnicodeFonts;
use heapless::String;
use log::debug;

use crate::dot_matrix::DotMatrix;

pub struct TextTicker<const N: usize> {
    text: String<N>,
    scroll_position: f32,
    scroll_speed: f32,
}

impl<const N: usize> TextTicker<N> {
    pub fn new(text: String<N>, scroll_speed: f32) -> Self {
        Self {
            text,
            scroll_position: 0.0,
            scroll_speed,
        }
    }

    pub fn update(&mut self, delta_time_ms: u64) {
        self.scroll_position += delta_time_ms as f32 * self.scroll_speed;
    }

    fn get_font_data(str: &str, idx: usize) -> [u8; 8] {
        let char = str.chars().nth(idx % str.len()).unwrap();
        let font = font8x8::BASIC_FONTS.get_font(char).unwrap();
        font.1
    }

    pub fn draw(&self, dot_matrix: &mut DotMatrix) {
        let i = self.scroll_position as usize;

        let text_idx = i / 8;
        let char_offs = i % 8;
        let font_data = Self::get_font_data(self.text.as_str(), text_idx);
        let next_font_data = Self::get_font_data(self.text.as_str(), text_idx + 1);

        for r in 0..8 {
            let mut row_data = font_data[r as usize].reverse_bits() << char_offs;
            if char_offs != 0 {
                let next_row_data = next_font_data[r as usize].reverse_bits() >> (8 - char_offs);
                debug!("next_row_data {next_row_data:#010b}");
                row_data |= next_row_data;
            }

            dot_matrix.set_row(r, row_data);
        }
    }
}
