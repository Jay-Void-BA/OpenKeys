//! Exact R04 I2C1 PB6/PB7 binding. This does not enable SE_EN, provision keys,
//! authenticate a card or authorize a fingerprint-gated operation.
use embassy_stm32::{
    Peri,
    i2c::{Config, I2c},
    peripherals,
};
use openkeys_board::se_i2c::Error;
use openkeys_se_transport::transport::{Clock, Transport};
use openkeys_secure_mmio::{SecureSeI2c, initialize_se_i2c};

pub fn new_link<'d, C: Clock, D: Clock>(
    peripheral: Peri<'d, peripherals::I2C1>,
    scl: Peri<'d, peripherals::PB6>,
    sda: Peri<'d, peripherals::PB7>,
    bus_clock: C,
    transport_clock: D,
) -> Result<Transport<SecureSeI2c<'d, C>, D>, Error> {
    // Default100kHz, external4.7k pullups; no3.3V push-pull bus override.
    let bus = I2c::new_blocking(peripheral, scl, sda, Config::default());
    Ok(Transport::new(
        initialize_se_i2c(bus, bus_clock)?,
        transport_clock,
    ))
}
