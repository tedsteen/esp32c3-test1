use log::info;

use crate::dot_matrix::DotMatrix;

#[derive(Debug, Clone)]
pub enum PadPosition {
    Left(f32),
    Right(f32),
    Top(f32),
    Bottom(f32),
}

impl PadPosition {
    pub fn next(&mut self) {
        *self = match self {
            PadPosition::Left(_) => PadPosition::Top(0.0),
            PadPosition::Right(_) => PadPosition::Bottom(0.0),
            PadPosition::Top(_) => PadPosition::Right(0.0),
            PadPosition::Bottom(_) => PadPosition::Left(0.0),
        }
    }

    fn to_pixels(slide_amount: f32) -> u8 {
        (7.0 * (slide_amount * slide_amount * slide_amount)) as u8
    }

    fn draw(&self, dot_matrix: &mut DotMatrix) {
        match self {
            PadPosition::Top(slide_amount) => {
                let pixels = Self::to_pixels(*slide_amount);
                dot_matrix.set_row(0, 0b11111111 << (7 - pixels));
                for y in (0..=7 - pixels).rev() {
                    dot_matrix.put(0, y);
                }
            }
            PadPosition::Right(slide_amount) => {
                let pixels = Self::to_pixels(*slide_amount);
                dot_matrix.set_row(0, 0b11111111 >> pixels);
                for y in 0..=pixels {
                    dot_matrix.put(7, y);
                }
            }

            PadPosition::Bottom(slide_amount) => {
                let pixels = Self::to_pixels(*slide_amount);
                dot_matrix.set_row(7, 0b11111111 >> (8 - pixels - 1));
                for y in pixels..=7 {
                    dot_matrix.put(7, y);
                }
            }
            PadPosition::Left(slide_amount) => {
                let pixels = Self::to_pixels(*slide_amount);
                for y in (7 - pixels..=7).rev() {
                    dot_matrix.put(0, y);
                }
                dot_matrix.set_row(7, 0b11111111 << pixels);
            }
        }
    }

    fn update(&mut self, delta_time_ms: u64) {
        match self {
            PadPosition::Left(slide_amount)
            | PadPosition::Right(slide_amount)
            | PadPosition::Top(slide_amount)
            | PadPosition::Bottom(slide_amount) => {
                *slide_amount = f32::min(*slide_amount + delta_time_ms as f32 * 0.009, 1.0);
            }
        }
    }
}
const MAX_HEALTH: u8 = 4;

#[derive(Clone, Debug)]
pub enum PadState {
    Normal,
    Hurting(i64),
    Dying(i64),
}

#[derive(Clone, Debug)]
pub enum Pad {
    Alive {
        state: PadState,
        position: PadPosition,
        health: u8,
    },

    Dead,
}

impl Pad {
    pub const fn new(initial_position: PadPosition) -> Self {
        Self::Alive {
            state: PadState::Normal,
            position: initial_position,
            health: MAX_HEALTH,
        }
    }

    pub fn take_damage(&mut self) {
        if let Pad::Alive {
            health,
            state: alive_state,
            ..
        } = self
        {
            *health -= 1;
            info!("Health: {}", health);
            if *health == 0 {
                info!("YOU DED!");
                *alive_state = PadState::Dying(16 * 70)
            } else {
                *alive_state = PadState::Hurting(16 * 10);
            }
        }
    }

    pub fn update(&mut self, delta_time_ms: u64) {
        match self {
            Pad::Alive {
                state, position, ..
            } => {
                match state {
                    PadState::Hurting(countdown) => {
                        *countdown -= delta_time_ms as i64;
                        if *countdown <= 0_i64 {
                            *state = PadState::Normal
                        }
                    }
                    PadState::Dying(countdown) => {
                        *countdown -= delta_time_ms as i64;
                        if *countdown <= 0_i64 {
                            *self = Pad::Dead;
                        }
                    }
                    PadState::Normal => {
                        position.update(delta_time_ms);
                    }
                };
            }
            Pad::Dead => {}
        }
    }

    pub fn draw(&self, dot_matrix: &mut DotMatrix) {
        if let Pad::Alive {
            state: alive_state,
            position,
            ..
        } = self
        {
            match &alive_state {
                PadState::Normal => position.draw(dot_matrix),
                PadState::Hurting(countdown) | PadState::Dying(countdown) => {
                    if countdown % (16 * 5) > 16 * 3 {
                        dot_matrix.fill();
                    }
                }
            }
        }
    }
}
