use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    pipe::{Pipe, Reader, Writer},
};

use crate::audio::{BYTES_PER_FRAME, DMA_TOTAL};

#[inline(always)]
fn as_i16_mut(b: &mut [u8]) -> &mut [i16] {
    unsafe { core::slice::from_raw_parts_mut(b.as_mut_ptr() as *mut i16, b.len() / 2) }
}
#[inline(always)]
fn as_i16(b: &[u8]) -> &[i16] {
    unsafe { core::slice::from_raw_parts(b.as_ptr() as *const i16, b.len() / 2) }
}

const MAX_CHANS: usize = 8;
pub const PIPE_BYTES: usize = DMA_TOTAL * 30;

// one global bank
static mut PIPES: [Pipe<CriticalSectionRawMutex, PIPE_BYTES>; MAX_CHANS] =
    [const { Pipe::new() }; MAX_CHANS];

pub type AudioProducerChannel = Writer<'static, CriticalSectionRawMutex, PIPE_BYTES>;
pub struct Mixer<const N: usize> {
    readers: [Reader<'static, CriticalSectionRawMutex, PIPE_BYTES>; N],
    pub writers: [AudioProducerChannel; N],
    gains_q15: [u16; N],
}

impl<const N: usize> Mixer<N> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        assert!(N > 0 && N <= MAX_CHANS);
        let mut readers = [(); N].map(|_| None);
        let mut writers = [(); N].map(|_| None);
        for i in 0..N {
            let (rx, tx) = unsafe { PIPES[i].split() };
            readers[i] = Some(rx);
            writers[i] = Some(tx);
        }
        Self {
            readers: readers.map(Option::unwrap),
            writers: writers.map(Option::unwrap),
            gains_q15: [0x7FFF; N],
        }
    }

    #[inline(always)]
    pub fn set_gain_q15(&mut self, idx: usize, q15: u16) {
        self.gains_q15[idx] = q15;
    }

    /// Mix N channels into `dst` (bytes, multiple of frame size).
    /// Strategy: zero `dst` once, then for each channel, stream-read in small chunks
    /// and accumulate (scaled) into `dst`. Underrun == add silence.
    pub fn mix_into(&mut self, dst: &mut [u8]) {
        debug_assert!(dst.len() % BYTES_PER_FRAME == 0);
        // zero output once
        as_i16_mut(dst).fill(0);

        // small stack scratch to bound IRQ latency & stack use
        const SCRATCH: usize = 128 * BYTES_PER_FRAME;
        let mut tmp = [0u8; SCRATCH];

        for (i, rx) in self.readers.iter_mut().enumerate() {
            let gain = self.gains_q15[i] as i32;
            let mut off = 0;
            while off < dst.len() {
                let want = core::cmp::min(SCRATCH, dst.len() - off);

                // read up to `want` bytes from this channel
                let got = rx.try_read(&mut tmp[..want]).unwrap_or(0);

                // add the part we actually got
                if got != 0 {
                    let out = &mut as_i16_mut(&mut dst[off..off + got])[..];
                    let inp = as_i16(&tmp[..got]);
                    for (o, &x) in out.iter_mut().zip(inp.iter()) {
                        // Q15 scale: (x * gain) >> 15
                        let scaled = ((x as i32) * gain) >> 15;
                        *o = o.saturating_add(scaled as i16);
                    }
                }
                // any shortfall in this window == silence (no-op add)

                off += want;
            }
        }
    }
}
