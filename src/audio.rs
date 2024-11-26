use core::f32::consts::PI;

use esp_backtrace as _;
use esp_hal::{
    dma::{Dma, DmaPriority},
    dma_circular_buffers,
    gpio::interconnect::PeripheralOutput,
    i2s::master::{DataFormat, I2s, RegisterAccess, Standard},
    peripheral::Peripheral,
    peripherals::DMA,
};
use libm::sin;

const SAMPLE_RATE: u32 = 44100;
const NUM_CHANNELS: usize = 2;
const NUM_SAMPLES: usize = 4096;

fn as_u8_slice(slice: &[i16]) -> &[u8] {
    let ptr = slice.as_ptr().cast::<u8>();
    let len = core::mem::size_of_val(slice);
    unsafe { core::slice::from_raw_parts(ptr, len) }
}

pub struct Audio {}

impl<'d> Audio {
    pub fn new(
        dma: impl Peripheral<P = DMA> + 'd,
        i2s: impl Peripheral<P = impl RegisterAccess> + 'd,
        bclk: impl Peripheral<P = impl PeripheralOutput> + 'd,
        ws: impl Peripheral<P = impl PeripheralOutput> + 'd,
        dout: impl Peripheral<P = impl PeripheralOutput> + 'd,
    ) -> Self {
        let dma = Dma::new(dma);
        let dma_channel = dma.channel0;

        let (tx_buffer, tx_descriptors, _, rx_descriptors) =
            dma_circular_buffers!(NUM_SAMPLES * NUM_CHANNELS * core::mem::size_of::<i16>(), 0);

        let i2s = I2s::new(
            i2s,
            Standard::Philips,
            DataFormat::Data16Channel16,
            fugit::HertzU32::Hz(SAMPLE_RATE),
            dma_channel.configure(false, DmaPriority::Priority0),
            tx_descriptors,
            rx_descriptors,
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

        let mut filler = [0i16; NUM_SAMPLES];

        log::info!(
            "DMA buffer: {} bytes, filler: {} channel samples ({} bytes)",
            tx_buffer.len(),
            filler.len(),
            size_of_val(&filler)
        );

        let mut transaction = i2s_tx
            .write_dma_circular(tx_buffer)
            .expect("dma transaction");

        for i in (0..filler.len()).step_by(NUM_CHANNELS) {
            let sample = sin_sample();
            let (left, right) = (sample, -sample);
            filler[i] = left;
            filler[i + 1] = right;
        }

        let avail = transaction.available().unwrap_or(0);
        let bytes_written = transaction.push(as_u8_slice(&filler)).unwrap();
        log::info!("written bytes: {bytes_written} (available: {avail})");

        Self {}
    }
}
