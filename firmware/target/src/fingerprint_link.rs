//! R04 FPC2534 SPI3 binding. Only a verified protected runtime may release reset.
//! Diagnostics never instantiate this port or enable the sensor.
use embassy_stm32::{
    Peri,
    gpio::{Input, Level, Output, Pull, Speed},
    mode::Blocking,
    peripherals,
    spi::{BitOrder, Config, MODE_0, Spi, mode::Master},
    time::Hertz,
};
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use openkeys_board::fingerprint::{Error, Transport};

pub type Link<'d, D> =
    Transport<ExclusiveDevice<Spi<'d, Blocking, Master>, Output<'d>, D>, Input<'d>, Output<'d>>;

pub struct Pins<'d> {
    pub peripheral: Peri<'d, peripherals::SPI3>,
    pub sck: Peri<'d, peripherals::PC10>,
    pub mosi: Peri<'d, peripherals::PC12>,
    pub miso: Peri<'d, peripherals::PC11>,
    pub cs: Peri<'d, peripherals::PA15>,
    pub irq: Peri<'d, peripherals::PC6>,
    pub reset: Peri<'d, peripherals::PC7>,
}

pub fn new_link<'d, D: DelayNs>(pins: Pins<'d>, delay: D) -> Result<Link<'d, D>, Error> {
    let reset = Output::new(pins.reset, Level::Low, Speed::Low);
    let cs = Output::new(pins.cs, Level::High, Speed::Low);
    let irq = Input::new(pins.irq, Pull::None);
    let mut config = Config::default();
    config.mode = MODE_0;
    config.bit_order = BitOrder::MsbFirst;
    config.frequency = Hertz(1_000_000); // 10 MHz part maximum; qualify 1 MHz first.
    config.gpio_speed = Speed::Low;
    let spi = Spi::new_blocking(pins.peripheral, pins.sck, pins.mosi, pins.miso, config);
    let device = ExclusiveDevice::new(spi, cs, delay).map_err(|_| Error::Io)?;
    Transport::new(device, irq, reset)
}
