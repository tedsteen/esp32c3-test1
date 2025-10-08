use esp_hal::{
    gpio::interconnect::PeripheralOutput,
    spi::{
        master::{Config, ConfigError, Instance, Spi},
        Mode,
    },
    time::Rate,
    Async,
};
use log::{debug, trace};

pub struct DotMatrix<'a> {
    buffer: [u8; 8],
    intensity: u8,
    spi: Spi<'a, Async>,
}

type Result<T> = core::result::Result<T, DotMatrixError>;

#[derive(Debug)]
pub enum DotMatrixError {
    SpiInitFailed(ConfigError),
    TransferFailed(esp_hal::spi::Error),
}

impl<'a> DotMatrix<'a> {
    pub async fn new(
        spi: impl Instance + 'a,
        clk: impl PeripheralOutput<'a>,
        cs: impl PeripheralOutput<'a>,
        din: impl PeripheralOutput<'a>,
    ) -> Result<Self> {
        let mut spi = Spi::new(
            spi,
            Config::default()
                .with_frequency(Rate::from_mhz(2))
                .with_mode(Mode::_0),
        )
        .map_err(DotMatrixError::SpiInitFailed)?
        .with_cs(cs)
        .with_mosi(din)
        .with_sck(clk)
        .into_async();

        let initial_intensity = 0x0F;

        initialise_spi(&mut spi, initial_intensity)
            .await
            .map_err(DotMatrixError::TransferFailed)?;

        Ok(Self {
            spi,
            intensity: initial_intensity,
            buffer: [0; 8],
        })
    }

    // NOTE: Max intensity is 0x0F
    pub async fn set_intensity(&mut self, intensity: u8) -> Result<()> {
        if self.intensity != intensity {
            self.intensity = intensity;
            debug!("Write intensity: 0x{:01x}", intensity);
            self.spi
                .transfer_in_place_async(&mut [0x0A, intensity])
                .await
                .map_err(DotMatrixError::TransferFailed)?;
        }
        Ok(())
    }

    pub fn fill(&mut self) {
        trace!("Fill");
        for addr in 1..=8 {
            self.buffer[addr - 1] = 0xff;
        }
    }

    pub fn clear(&mut self) {
        trace!("Clear");
        for addr in 1..=8 {
            self.buffer[addr - 1] = 0;
        }
    }

    pub fn put(&mut self, x: u8, y: u8) {
        self.buffer[y as usize] |= (0b10000000 >> x) as u8;
    }

    pub fn set_row(&mut self, row: u8, row_data: u8) {
        self.buffer[row as usize] = row_data;
    }

    pub async fn flush_buffer_to_spi(&mut self) -> Result<()> {
        for i in 0..8 {
            self.spi
                .transfer_in_place_async(&mut [i + 1, self.buffer[i as usize]])
                .await
                .map_err(DotMatrixError::TransferFailed)?;
        }
        Ok(())
    }

    pub fn draw<const ROWS: usize>(&mut self, bitmap: &[u8; ROWS]) {
        self.buffer[0..ROWS].copy_from_slice(&bitmap[0..ROWS]);
    }

    pub fn shift(&mut self, x: u8, y: u8) {
        for r in 0..8 {
            self.buffer[r] >>= x;
        }
        self.buffer.rotate_right(y as usize);
    }
}

async fn initialise_spi(
    spi: &mut Spi<'_, Async>,
    initial_intensity: u8,
) -> core::result::Result<(), esp_hal::spi::Error> {
    // Zero out all registers
    for cmd in 0..16 {
        spi.transfer_in_place_async(&mut [cmd, 0x00]).await?;
    }
    // Power Up Device
    spi.transfer_in_place_async(&mut [0x0C, 0x01]).await?;
    // Set up Decode Mode to work with the MAX2719
    spi.transfer_in_place_async(&mut [0x09, 0x00]).await?;
    //Configure Scan Limit to work with the MAX2719
    spi.transfer_in_place_async(&mut [0x0b, 0x07]).await?;

    //Set initial intensity
    spi.transfer_in_place_async(&mut [0x0A, initial_intensity])
        .await?;
    Ok(())
}
