use heapless::format;
use log::info;

use crate::{
    ball::Ball,
    dot_matrix::{DotMatrix, DotMatrixError},
    font,
    highscore::HighScore,
    pad::{Pad, PadPosition},
    text_ticker::TextTicker,
};
type Result<T> = core::result::Result<T, GameStateError>;
#[derive(Debug)]
pub enum GameStateError {
    AdvanceFailed(DotMatrixError),
}
pub enum GameState {
    Intro(TextTicker<100>),
    Countdown(i64),
    Playing { ball: Ball, pad: Pad, score: u32 },
    GameOver(TextTicker<100>),
}

impl GameState {
    pub fn button_click(&mut self) {
        match self {
            GameState::Intro(_) | GameState::GameOver(_) => {
                *self = GameState::Countdown(3000);
            }
            GameState::Playing {
                pad: Pad::Alive { position, .. },
                ..
            } => {
                position.next();
            }
            _ => {}
        }
    }

    pub fn advance(
        &mut self,
        delta_time_ms: u64,
        highscore: &mut HighScore,
        dot_matrix: &mut DotMatrix<'_>,
    ) -> Result<()> {
        dot_matrix.clear();
        match self {
            GameState::Intro(text) | GameState::GameOver(text) => {
                text.update(delta_time_ms);
                text.draw(dot_matrix);
            }
            GameState::Countdown(countdown) => {
                *countdown -= delta_time_ms as i64;
                let countdown_as_secs = 1 + (*countdown / 1000);
                let countdown_as_bitmap =
                    *font::get_font_data(&((b'0' + countdown_as_secs as u8) as char))
                        .expect("a font for a number");

                dot_matrix.draw(&countdown_as_bitmap);
                dot_matrix.shift(2, 1);

                if *countdown <= 0 {
                    *self = Self::Playing {
                        ball: Ball::new(3, 3),
                        pad: Pad::new(PadPosition::Bottom(1.0)),
                        score: 0,
                    }
                }
            }
            GameState::Playing { ball, pad, score } => match pad {
                Pad::Alive { .. } => {
                    pad.update(delta_time_ms);
                    ball.update(pad, delta_time_ms, score);

                    pad.draw(dot_matrix);
                    ball.draw(dot_matrix);
                }
                Pad::Dead => {
                    let message = if *score > highscore.get() {
                        highscore.set(*score);
                        "New highscore!"
                    } else {
                        "Score"
                    };

                    info!("Result: {message} {score}");

                    *self = GameState::GameOver(TextTicker::new(
                        format!(" {message} {score}").expect("A string"),
                        0.014,
                    ));
                }
            },
        }
        dot_matrix
            .flush_buffer_to_spi()
            .map_err(GameStateError::AdvanceFailed)?;
        Ok(())
    }
}
