// sfx.rs
#![allow(dead_code)]
use core::f32::consts::TAU;
use embedded_io_async::Write;
use libm::{asinf, floorf, sinf};
use log::warn;

use crate::audio::{mixer::AudioProducerChannel, BYTES_PER_FRAME, CH, SAMPLE_RATE};

#[inline(always)]
fn as_bytes_i16(slice: &[i16]) -> &[u8] {
    unsafe { core::slice::from_raw_parts(slice.as_ptr() as *const u8, slice.len() * 2) }
}

#[inline(always)]
fn sat_i16(x: i32) -> i16 {
    x.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

#[derive(Clone, Copy)]
pub enum Wave {
    Sine,
    Square,
    Saw,
    Tri,
    Noise,
}

#[derive(Clone, Copy)]
pub struct Adsrs {
    pub a_ms: u32,      // attack
    pub d_ms: u32,      // decay
    pub s_lvl_q15: u16, // sustain level
    pub r_ms: u32,      // release
}
impl Adsrs {
    pub const fn beep() -> Self {
        Self {
            a_ms: 4,
            d_ms: 30,
            s_lvl_q15: 0x6000,
            r_ms: 60,
        }
    }
    pub const fn pluck() -> Self {
        Self {
            a_ms: 1,
            d_ms: 80,
            s_lvl_q15: 0x2000,
            r_ms: 40,
        }
    }
}

// simple LCG (fast) for noise
#[derive(Clone)]
struct Lcg(u32);
impl Lcg {
    fn new(seed: u32) -> Self {
        Self(seed | 1)
    }
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0
    }
}

// pan in Q1.15: -1.0 = left, 0 = center, +1.0 = right
#[inline(always)]
fn pan_gains_q15(pan_q15: i16) -> (u16, u16) {
    // constant-power-ish: cos/sin half-angle approx using linear mix (cheap)
    // map [-32768..32767] -> [0..1]
    let p = (pan_q15 as i32 + 32768) as u32; // 0..65535
    let l = 65535u32.saturating_sub(p);
    let r = p;
    ((l >> 1) as u16, (r >> 1) as u16)
}

/// Stream a waveform with ADSR + pan into a channel for duration_ms.
/// gain_q15 multiplies amplitude pre-pan (0..0x7FFF)
pub async fn play_tone(
    mut tx: AudioProducerChannel,
    wave: Wave,
    freq_hz: f32,
    duration_ms: u32,
    gain_q15: u16,
    pan_q15: i16,
    env: Option<Adsrs>,
) {
    // buffer = small frame chunk (multiple of BYTES_PER_FRAME)
    const FRAMES: usize = 32 * BYTES_PER_FRAME;
    let mut buf: [i16; FRAMES * CH] = [0; FRAMES * CH];

    let (gl, gr) = pan_gains_q15(pan_q15);
    let gain = gain_q15 as i32;

    let total_frames = ((duration_ms as u64 * SAMPLE_RATE as u64) / 1000) as usize;
    let mut done = 0usize;

    // phase accum (float, bc SR is low and CPU is fine)
    let mut phase = 0.0f32;
    let inc = freq_hz * (TAU / SAMPLE_RATE as f32);

    // noise source
    let mut noise = Lcg::new(0xC0FFEE);

    // envelope helpers
    let (a, d, s, r) = env
        .map(|e| (e.a_ms, e.d_ms, e.s_lvl_q15, e.r_ms))
        .unwrap_or((0, 0, 0x7FFF, 0));
    let atk_frames = (a as u64 * SAMPLE_RATE as u64 / 1000) as usize;
    let dec_frames = (d as u64 * SAMPLE_RATE as u64 / 1000) as usize;
    let rel_frames = (r as u64 * SAMPLE_RATE as u64 / 1000) as usize;

    while done < total_frames {
        let n = core::cmp::min(FRAMES, total_frames - done);

        for i in 0..n {
            // 1) oscillator
            let x = match wave {
                Wave::Sine => sinf(phase),
                Wave::Square => {
                    if sinf(phase) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Wave::Saw => 2.0 * (phase / TAU - floorf(phase / TAU)) - 1.0,
                Wave::Tri => {
                    let t = 2.0 * (phase / TAU - floorf(phase / TAU)) - 1.0;
                    (2.0 / core::f32::consts::PI) * asinf(t)
                }
                Wave::Noise => {
                    let v = (noise.next() >> 9) as i16; // ~7-bit-ish
                    (v as f32) / 32768.0
                }
            };
            phase += inc;
            if phase >= TAU {
                phase -= TAU;
            }

            // 2) envelope (Q15)
            let idx = done + i;
            let env_q15: u16 = if env.is_none() {
                0x7FFF
            } else if idx < atk_frames {
                // linear attack 0..1
                ((idx as u64 * 0x7FFFu64) / atk_frames.max(1) as u64) as u16
            } else if idx < atk_frames + dec_frames {
                let t = idx - atk_frames;
                let from = 0x7FFFu32;
                let to = s as u32;
                let val = from - ((from - to) * (t as u32) / dec_frames.max(1) as u32);
                val as u16
            } else if idx < total_frames.saturating_sub(rel_frames) {
                s
            } else {
                // release to 0
                let t = idx.saturating_sub(total_frames.saturating_sub(rel_frames));
                let from = s as u32;
                let val = from.saturating_sub((from * t as u32) / rel_frames.max(1) as u32);
                val as u16
            };

            // 3) scale -> pan -> write interleaved
            let s32 = ((((x * 32767.0) as i32 * gain) >> 15) * (env_q15 as i32)) >> 15;
            let l = ((s32 * (gl as i32)) >> 15) as i32;
            let r = ((s32 * (gr as i32)) >> 15) as i32;

            let o = i * CH;
            buf[o] = sat_i16(l);
            buf[o + 1] = sat_i16(r);
        }

        // 4) send
        let bytes = &as_bytes_i16(&buf[..n * CH]);
        if let Err(e) = tx.write_all(bytes).await {
            warn!("Problem writing tone {e:?}");
        };

        done += n;
    }
}

pub async fn beep(tx: AudioProducerChannel, hz: f32, ms: u32, gain_q15: u16, pan_q15: i16) {
    play_tone(
        tx,
        Wave::Sine,
        hz,
        ms,
        gain_q15,
        pan_q15,
        Some(Adsrs::beep()),
    )
    .await
}

/// short blip (square)
pub async fn blip(tx: AudioProducerChannel, hz: f32, ms: u32, gain_q15: u16, pan_q15: i16) {
    play_tone(
        tx,
        Wave::Square,
        hz,
        ms,
        gain_q15,
        pan_q15,
        Some(Adsrs::pluck()),
    )
    .await
}

pub async fn noise_burst(tx: AudioProducerChannel, ms: u32, gain_q15: u16, pan_q15: i16) {
    play_tone(
        tx,
        Wave::Noise,
        0.0,
        ms,
        gain_q15,
        pan_q15,
        Some(Adsrs {
            a_ms: 1,
            d_ms: 30,
            s_lvl_q15: 0x3000,
            r_ms: 60,
        }),
    )
    .await
}

/// arpeggiate notes (Hz list) with step_ms per note
pub async fn arpeggio<const BYTES: usize>(
    tx: AudioProducerChannel,
    notes_hz: &[f32],
    step_ms: u32,
    gain_q15: u16,
    pan_q15: i16,
) {
    for &f in notes_hz {
        play_tone(
            tx,
            Wave::Sine,
            f,
            step_ms,
            gain_q15,
            pan_q15,
            Some(Adsrs::pluck()),
        )
        .await;
    }
}

/// run a simple “song” concurrently: returns when pattern done
pub async fn pattern_demo(music_tx: AudioProducerChannel, sfx_tx: AudioProducerChannel) {
    // fire-and-forget SFX overlaps with background pad
    // background pad
    let volume = 0x1000;
    let music = async {
        play_tone(
            music_tx,
            Wave::Tri,
            220.0,
            1200,
            volume,
            0,
            Some(Adsrs {
                a_ms: 80,
                d_ms: 600,
                s_lvl_q15: 0x0000,
                r_ms: 100,
            }),
        )
        .await;
        play_tone(
            music_tx,
            Wave::Tri,
            247.0,
            1200,
            volume,
            0,
            Some(Adsrs {
                a_ms: 80,
                d_ms: 600,
                s_lvl_q15: 0x0000,
                r_ms: 300,
            }),
        )
        .await;
        play_tone(
            music_tx,
            Wave::Tri,
            262.0,
            1200,
            volume,
            0,
            Some(Adsrs {
                a_ms: 80,
                d_ms: 600,
                s_lvl_q15: 0x0000,
                r_ms: 300,
            }),
        )
        .await;
    }
    .await;
    // //sfx on top
    // let fx = async {
    //     Timer::after(Duration::from_millis(180)).await;
    //     blip(sfx_tx, 880.0, 90, 0x7FFF, 0).await;
    //     Timer::after(Duration::from_millis(220)).await;
    //     noise_burst(sfx_tx, 120, 0x6FFF, 0).await;
    //     Timer::after(Duration::from_millis(300)).await;
    //     beep(sfx_tx, 660.0, 240, 0x6FFF, 000).await;
    // };
    // join(music, fx).await;
}
