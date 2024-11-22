use crate::{
    dot_matrix::DotMatrix,
    pad::{Pad, PadPosition, PadState},
};

#[derive(Clone)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    x_speed: f32,
    y_speed: f32,
}
impl Ball {
    pub const fn new(initial_x: u8, initial_y: u8) -> Self {
        Self {
            x: initial_x as f32,
            y: initial_y as f32,
            x_speed: 0.0054,
            y_speed: -0.004,
        }
    }

    pub fn update(&mut self, pad: &mut Pad, delta_time_ms: u64) {
        let mut pad_hit = false;
        if let Pad::Alive {
            position,
            state: PadState::Normal,
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
    }

    pub fn draw(&self, dot_matrix: &mut DotMatrix) {
        dot_matrix.put(self.x as u8, self.y as u8);
    }
}
