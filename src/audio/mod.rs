use core::mem::size_of;

use esp_hal::{
    dma_circular_buffers_chunk_size,
    gpio::interconnect::PeripheralOutput,
    i2s::{
        master::{DataFormat, I2s, Instance, Standard},
        AnyI2s,
    },
    time::Rate,
};
use log::warn;

use crate::audio::mixer::Mixer;

pub mod mixer;

pub type Sample = i16;
pub const SAMPLE_RATE: u32 = 8000;
const CH: usize = 2;
const BYTES_PER_FRAME: usize = CH * size_of::<Sample>(); // i16 stereo -> 4 bytes/frame
const DMA_CHUNK: usize = BYTES_PER_FRAME * 32 * 2; // one half of the ring
const DMA_TOTAL: usize = DMA_CHUNK * 2; // full ring = 2 halves

pub struct AudioEngine<const N: usize> {
    pub mixer: Mixer<N>,
}

impl<const N: usize> AudioEngine<N> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            mixer: Mixer::new(),
        }
    }

    pub async fn start<Dma, Inst, Bck, Lrck, Dout>(
        self,
        dma_ch: Dma,
        i2s_periph: Inst,
        bck: Bck,
        lrck: Lrck,
        dout: Dout,
    ) -> !
    where
        Dma: esp_hal::dma::DmaChannelFor<AnyI2s<'static>>,
        Inst: Instance + 'static,
        Bck: PeripheralOutput<'static>,
        Lrck: PeripheralOutput<'static>,
        Dout: PeripheralOutput<'static>,
    {
        // Build async I2S
        let i2s = I2s::new(
            i2s_periph,
            Standard::Philips,
            DataFormat::Data16Channel16,
            Rate::from_hz(SAMPLE_RATE),
            dma_ch,
        )
        .into_async();

        // Circular DMA ring: TOTAL = 2 * CHUNK
        #[allow(clippy::manual_div_ceil)]
        let (_rx_buf, _rx_desc, tx_buf, tx_desc) =
            dma_circular_buffers_chunk_size!(0, DMA_TOTAL, DMA_CHUNK);

        let i2s_tx = i2s
            .i2s_tx
            .with_bclk(bck)
            .with_ws(lrck)
            .with_dout(dout)
            .build(tx_desc);

        let mut xfer = i2s_tx.write_dma_circular_async(tx_buf).unwrap();
        let mut rx = self.mixer;
        loop {
            // wait for writable window
            //let _ = xfer.available().await;
            //log::info!("TICK");
            match xfer
                .push_with(|dst| {
                    rx.mix_into(dst);
                    dst.len()
                })
                .await
            {
                Ok(_b) => {}
                Err(e) => warn!("Error when pushing to dma {e:?}"),
            }
        }
    }
}
