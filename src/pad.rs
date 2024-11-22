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

    pub fn draw(&mut self, dot_matrix: &mut DotMatrix) {
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
pub enum PadAliveState {
    Normal,
    Hurting(i64),
    Dying(i64),
}

#[derive(Clone, Debug)]
pub enum PadState {
    Alive {
        state: PadAliveState,
        position: PadPosition,
        health: u8,
    },
    Dead,
}

impl PadState {
    pub const fn new() -> Self {
        Self::Alive {
            state: PadAliveState::Normal,
            position: PadPosition::Bottom,
            health: MAX_HEALTH,
        }
    }

    pub fn take_damage(&mut self) {
        if let PadState::Alive {
            health,
            state: alive_state,
            ..
        } = self
        {
            *health -= 1;
            info!("Health: {}", health);
            if *health == 0 {
                info!("YOU DED!");
                *alive_state = PadAliveState::Dying(1000)
            } else {
                *alive_state = PadAliveState::Hurting(200);
            }
        }
    }

    pub fn update(&mut self, delta_time_ms: u64) -> PadUpdateResult {
        match self {
            PadState::Alive {
                state: alive_state, ..
            } => {
                match alive_state {
                    PadAliveState::Normal => {}
                    PadAliveState::Hurting(countdown) => {
                        *countdown -= delta_time_ms as i64;
                        if *countdown <= 0_i64 {
                            *alive_state = PadAliveState::Normal
                        }
                    }
                    PadAliveState::Dying(countdown) => {
                        *countdown -= delta_time_ms as i64;
                        if *countdown <= 0_i64 {
                            *self = PadState::Dead;
                        }
                    }
                };
            }
            PadState::Dead => {}
        }
        PadUpdateResult {
            pad_state: self.clone(),
        }
    }
}

#[derive(Debug)]
pub struct PadUpdateResult {
    pub pad_state: PadState,
}
