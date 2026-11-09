//! Exact R04 I2C1 PB6/PB7 binding. This does not enable SE_EN, provision keys,
//! authenticate a card or authorize a fingerprint-gated operation.
use embassy_stm32::{
    Peri,
    i2c::{Config, I2c, mode::Master},
    mode::Blocking,
    peripherals,
};
use openkeys_board::se_io::SeIo;
use openkeys_se_transport::transport::{Clock, Transport};

pub fn new_link<'d, C: Clock>(
    peripheral: Peri<'d, peripherals::I2C1>,
    scl: Peri<'d, peripherals::PB6>,
    sda: Peri<'d, peripherals::PB7>,
    clock: C,
) -> Transport<SeIo<I2c<'d, Blocking, Master>>, C> {
    // Default100kHz, external4.7k pullups; no3.3V push-pull bus override.
    let bus = I2c::new_blocking(peripheral, scl, sda, Config::default());
    Transport::new(SeIo::new(bus), clock)
}
