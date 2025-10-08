use core::f32::consts::PI;
use esp_hal::{
    dma::DmaChannelFor,
    dma_circular_buffers,
    gpio::interconnect::PeripheralOutput,
    i2s::{
        master::{DataFormat, I2s, I2sTx, Instance, Standard},
        AnyI2s,
    },
    time::Rate,
    Async,
};
use libm::sin;
use log::warn;
type Sample = i16;

pub const SAMPLE_RATE: u32 = 44_100;
const BYTES_PER_FRAME: usize = 2 * core::mem::size_of::<Sample>(); // 2ch * size_of_sample_size

/// A minimal stereo I2S sink that pulls samples from a user callback and
/// shoves them into a circular DMA buffer.
pub struct StereoSink<'d> {
    tx_buf: &'d [u8; BUF_BYTES],
    tx: I2sTx<'d, Async>,
}

// Tunable..
pub const FRAMES_PER_BUF: usize = 1024 * 3;
pub const BUF_BYTES: usize = FRAMES_PER_BUF * BYTES_PER_FRAME;

impl<'d> StereoSink<'d> {
    pub fn new(
        dma_ch: impl DmaChannelFor<AnyI2s<'d>>,
        i2s_periph: impl Instance + 'd,
        bck: impl PeripheralOutput<'d>,
        lck: impl PeripheralOutput<'d>,
        din: impl PeripheralOutput<'d>,
    ) -> Self {
        // rx is unused; descriptors are owned by HAL. tx_buf is a &[u8; BUF_BYTES].
        let (_rx_buf, _rx_desc, tx_buf, tx_desc) = dma_circular_buffers!(0, BUF_BYTES);

        let i2s = I2s::new(
            i2s_periph,
            Standard::Philips,
            DataFormat::Data16Channel16,
            Rate::from_hz(SAMPLE_RATE),
            dma_ch,
        )
        .into_async();

        let tx = i2s
            .i2s_tx
            .with_bclk(bck)
            .with_ws(lck)
            .with_dout(din)
            .build(tx_desc);

        Self { tx_buf, tx }
    }

    /// Start streaming; pulls frames from `provider` forever.
    /// `provider` should be fast and non-blocking.
    pub async fn run(self, mut provider: impl FnMut() -> (Sample, Sample)) -> ! {
        let mut txn = self.tx.write_dma_circular_async(self.tx_buf).expect("dma");

        loop {
            match txn
                .push_with(|buf| {
                    // ensure whole frames
                    let full_bytes = buf.len() - (buf.len() % BYTES_PER_FRAME);
                    if full_bytes == 0 {
                        return 0;
                    }
                    let frames = full_bytes / BYTES_PER_FRAME;

                    // write exactly full_bytes
                    let mut p = 0;
                    for _ in 0..frames {
                        let (l, r) = provider();
                        let lb = l.to_ne_bytes();
                        let rb = r.to_ne_bytes();
                        buf[p + 0] = lb[0];
                        buf[p + 1] = lb[1];
                        buf[p + 2] = rb[0];
                        buf[p + 3] = rb[1];
                        p += BYTES_PER_FRAME;
                    }

                    // zero any tail (rare, but be safe)
                    for b in &mut buf[p..] {
                        *b = 0;
                    }

                    full_bytes
                })
                .await
            {
                Ok(_) => {}
                Err(e) => warn!("i2s push_with: {e:?}"),
            }
        }
    }
}

/// Returns a provider producing a 440 Hz anti-phase stereo tone at `volume` (0..1).
pub fn demo_tone_provider(volume: f32) -> impl FnMut() -> (Sample, Sample) {
    let vol = volume.clamp(0.0, 1.0);
    let mut phase = 0.0f32;
    let inc = 2.0 * PI * 440.0 / SAMPLE_RATE as f32;

    move || {
        phase += inc;
        if phase >= 2.0 * PI {
            phase -= 2.0 * PI;
        }
        let x = (sin(phase as f64) as f32 * vol * Sample::MAX as f32)
            .clamp(Sample::MIN as f32, Sample::MAX as f32) as Sample;
        (x, -x)
    }
}
