use core::f32::consts::PI;

use esp_backtrace as _;
use esp_hal::{
    dma::DmaChannelFor,
    dma_circular_buffers,
    gpio::interconnect::PeripheralOutput,
    i2s::master::{AnyI2s, DataFormat, I2s, RegisterAccess, Standard},
    peripheral::Peripheral,
};
use libm::sin;

const SAMPLE_RATE: u32 = 44100;
const NUM_CHANNELS: usize = 2;
const NUM_SAMPLES: usize = 1024 * 4;

fn as_u8_slice(slice: &[i16]) -> &[u8] {
    let ptr = slice.as_ptr().cast::<u8>();
    let len = core::mem::size_of_val(slice);
    unsafe { core::slice::from_raw_parts(ptr, len) }
}

pub struct Audio {}

impl Audio {
    pub fn new<'d>(
        dma_channel: impl Peripheral<P = impl DmaChannelFor<AnyI2s>> + 'd,
        i2s: impl Peripheral<P = impl RegisterAccess> + 'd,
        bclk: impl Peripheral<P = impl PeripheralOutput> + 'd,
        ws: impl Peripheral<P = impl PeripheralOutput> + 'd,
        dout: impl Peripheral<P = impl PeripheralOutput> + 'd,
    ) -> Self {
        let (_rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) =
            dma_circular_buffers!(0, NUM_SAMPLES * NUM_CHANNELS * core::mem::size_of::<i16>());

        let i2s = I2s::new(
            i2s,
            Standard::Philips,
            DataFormat::Data16Channel16,
            fugit::HertzU32::Hz(SAMPLE_RATE),
            dma_channel,
            rx_descriptors,
            tx_descriptors,
        );

        let mut i2s_tx = i2s
            .i2s_tx
            .with_bclk(bclk)
            .with_ws(ws)
            .with_dout(dout)
            .build();

        let mut sample_clock = 0u32;

        let mut sin_sample = || {
            sample_clock = (sample_clock + 1) % SAMPLE_RATE;
            let smpl_f32 =
                sin((2.0 * PI * 440.0 * sample_clock as f32 / SAMPLE_RATE as f32) as f64) as f32;

            (smpl_f32 * i16::MAX as f32) as i16
        };

        log::info!("DMA buffer: {} bytes", tx_buffer.len(),);
        let mut transaction = i2s_tx
            .write_dma_circular(tx_buffer)
            .expect("dma transaction");

        loop {
            match transaction.push_with(|samples| {
                if samples.len() > 4 {
                    let volume = 0.1;
                    for s in samples.chunks_exact_mut(4) {
                        let sample = (sin_sample() as f32 * volume) as i16;
                        let (left, right) = (sample, -sample);
                        s[0] = left.to_le_bytes()[0];
                        s[1] = left.to_le_bytes()[1];
                        s[2] = right.to_le_bytes()[0];
                        s[3] = right.to_le_bytes()[1];
                    }
                    return samples.len();
                }
                0
            }) {
                Ok(sent) => {
                    if sent > 0 {
                        log::info!("Wrote {sent} bytes");
                    }
                }
                Err(error) => {
                    log::warn!("Problem : {error:?}");
                }
            }
        }
        Self {}
    }
}
