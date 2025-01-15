use esp_hal::{
    gpio::interconnect::PeripheralOutput,
    peripheral::Peripheral,
    spi::master::{Config, Instance, Spi},
    Blocking,
};
use log::{debug, trace};

pub struct DotMatrix<'a> {
    buffer: [u8; 8],
    intensity: u8,
    spi: Spi<'a, Blocking>,
}

impl<'a> DotMatrix<'a> {
    pub fn new<SCK: PeripheralOutput>(
        mosi: impl Peripheral<P = SCK> + 'a,
        cs: impl Peripheral<P = SCK> + 'a,
        sclk: impl Peripheral<P = SCK> + 'a,
        spi: impl Peripheral<P = impl Instance> + 'a,
    ) -> Self {
        let mut spi = esp_hal::spi::master::Spi::new(
            spi,
            Config::default().with_frequency(fugit::HertzU32::MHz(2)),
        )
        .expect("an spi")
        .with_sck(sclk)
        .with_mosi(mosi)
        .with_cs(cs);

        // Zero out all registers
        for cmd in 0..16 {
            spi.write_bytes(&[cmd, 0x00]).expect("bytes to be written");
        }

        // Power Up Device
        spi.write_bytes(&[0x0C, 0x01]).expect("bytes to be written");

        // Set up Decode Mode to work with the MAX2719
        spi.write_bytes(&[0x09, 0x00]).expect("bytes to be written");

        //Configure Scan Limit to work with the MAX2719
        spi.write_bytes(&[0x0b, 0x07]).expect("bytes to be written");

        let mut s = Self {
            spi,
            intensity: 0xFF,
            buffer: [0; 8],
        };
        s.set_intensity(0x0F);
        s
    }

    // NOTE: Max intensity is 0x0F
    pub fn set_intensity(&mut self, intensity: u8) {
        if self.intensity != intensity {
            self.intensity = intensity;
            debug!("Write intensity: 0x{:01x}", intensity);
            self.spi
                .write_bytes(&[0x0a, intensity])
                .expect("bytes to be written");
        }
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

    pub fn flush_buffer_to_spi(&mut self) {
        for i in 0..8 {
            self.spi
                .write_bytes(&[i + 1, self.buffer[i as usize]])
                .expect("buffer to be written to spi");
        }
    }

    pub fn draw<const ROWS: usize>(&mut self, bitmap: &[u8; ROWS]) {
        self.buffer[0..ROWS].copy_from_slice(&bitmap[0..ROWS]);
    }

    pub(crate) fn shift(&mut self, x: u8, y: u8) {
        for r in 0..8 {
            self.buffer[r] >>= x;
        }
        self.buffer.rotate_right(y as usize);
    }
}
