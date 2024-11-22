use log::info;

use crate::dot_matrix::DotMatrix;

#[derive(Debug, Clone)]
pub enum PadPosition {
    Left,
    Right,
    Top,
    Bottom,
}

impl PadPosition {
    pub fn next(&mut self) {
        *self = match self {
            PadPosition::Left => PadPosition::Top,
            PadPosition::Right => PadPosition::Bottom,
            PadPosition::Top => PadPosition::Right,
            PadPosition::Bottom => PadPosition::Left,
        }
    }

    fn draw(&self, dot_matrix: &mut DotMatrix) {
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
                *alive_state = PadState::Dying(1000)
            } else {
                *alive_state = PadState::Hurting(200);
            }
        }
    }

    pub fn update(&mut self, delta_time_ms: u64) {
        match self {
            Pad::Alive { state, .. } => {
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
                    PadState::Normal => {}
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
                PadState::Hurting(countdown) => {
                    if countdown % 100 < 50 {
                        dot_matrix.fill();
                    }
                }
                PadState::Dying(countdown) => {
                    if countdown % 100 < 50 {
                        dot_matrix.fill();
                    }
                }
            }
        }
    }
}
