//! DFRobot DFR0555, 16x2 blue LCD, on the R05 latched-power circuit.
//! This is NOT compatible with the R04 NHD footprint/reset-only circuit.
//! The protected root owns I2C and both control pins. No raw host display API.
use crate::Error;
use device_policy::lcd::Frame;
use embedded_hal::{delay::DelayNs, digital::OutputPin, i2c::I2c};
use zeroize::Zeroizing;

pub const LCD_ADDRESS: u8 = 0x3e;
pub const OFF_HOLD_MS: u32 = 500;
/// TPS3808 fixed300ms mode can hold clear for420ms after MR release.
/// Keep CLK low for500ms, then create a NEW edge; an earlier held-high
/// level would never clock the asynchronously cleared latch.
pub const RESET_RELEASE_MS: u32 = 500;
pub const POWER_UP_MS: u32 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Revision {
    V1_0,
    V1_1,
}
impl Revision {
    pub const fn backlight_address(self) -> u8 {
        match self {
            Self::V1_0 => 0x60,
            Self::V1_1 => 0x6b,
        }
    }
}

pub struct Lcd<I: I2c, R: OutputPin, A: OutputPin, D: DelayNs> {
    bus: I,
    release: R,
    arm: A,
    delay: D,
    revision: Revision,
    initialized: bool,
    faulted: bool,
}
impl<I: I2c, R: OutputPin, A: OutputPin, D: DelayNs> Lcd<I, R, A, D> {
    /// PC3/release MUST be open-drain; PC4/arm is a local rising-edge clock.
    /// Revision comes from the reviewed physical BOM, never a host message.
    pub fn new(bus: I, release: R, arm: A, delay: D, revision: Revision) -> Result<Self, Error> {
        let mut lcd = Self {
            bus,
            release,
            arm,
            delay,
            revision,
            initialized: false,
            faulted: false,
        };
        lcd.lock()?;
        Ok(lcd)
    }
    fn power_off(&mut self) -> bool {
        self.initialized = false;
        // Try both even when one pin fails. No I2C is needed to cut power.
        let arm = self.arm.set_low();
        let release = self.release.set_low();
        self.delay.delay_ms(OFF_HOLD_MS);
        arm.is_ok() && release.is_ok()
    }
    fn fault(&mut self) -> Error {
        self.faulted = true;
        self.power_off();
        Error::Io
    }
    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Error> {
        self.bus.write(address, bytes).map_err(|_| self.fault())
    }
    fn command(&mut self, byte: u8) -> Result<(), Error> {
        self.write(LCD_ADDRESS, &[0x80, byte])?;
        self.delay
            .delay_us(if matches!(byte, 1 | 2) { 2000 } else { 50 });
        Ok(())
    }
    fn register(&mut self, register: u8, value: u8) -> Result<(), Error> {
        self.write(self.revision.backlight_address(), &[register, value])
    }
    fn brightness(&mut self, value: u8) -> Result<(), Error> {
        match self.revision {
            Revision::V1_0 => {
                for register in [0x04, 0x03, 0x02] {
                    self.register(register, value)?;
                }
            }
            Revision::V1_1 => {
                // Cut the global gate before modifying per-channel PWM.
                self.register(0x07, 0)?;
                for register in [0x06, 0x05, 0x04] {
                    self.register(register, value)?;
                }
                if value != 0 {
                    self.register(0x07, 0xff)?;
                }
            }
        }
        Ok(())
    }
    pub fn initialize(&mut self) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        self.lock()?;
        self.release.set_high().map_err(|_| self.fault())?;
        self.delay.delay_ms(RESET_RELEASE_MS);
        self.arm.set_high().map_err(|_| self.fault())?;
        self.delay.delay_ms(POWER_UP_MS); // load rise, RC bus isolation and LCD POR
        for _ in 0..3 {
            self.command(0x28)?;
            self.delay.delay_ms(5);
        }
        self.command(0x08)?; // remain dark until a complete frame is loaded
        self.command(0x01)?;
        self.command(0x06)?;
        match self.revision {
            Revision::V1_0 => {
                self.register(0x00, 0)?;
                self.brightness(0)?;
                self.register(0x08, 0xff)?;
                self.register(0x01, 0x20)?;
            }
            Revision::V1_1 => {
                for (register, value) in [(0x2f, 0), (0x00, 0x20), (0x01, 0), (0x02, 1), (0x03, 4)]
                {
                    self.register(register, value)?;
                }
                self.brightness(0)?;
            }
        }
        self.initialized = true;
        Ok(())
    }
    /// Replaces both rows, including spaces. Per-character timing avoids
    /// relying on undocumented multi-byte FIFO or a particular HAL bus rate.
    /// Success means ACK/timing only, not optical attestation of what is shown.
    pub fn show(&mut self, frame: &Frame) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        if !self.initialized {
            return Err(Error::InvalidInput);
        }
        if !frame.iter().flatten().all(|c| matches!(c, 0x20..=0x7e)) {
            self.fault();
            return Err(Error::InvalidInput);
        }
        self.brightness(0)?;
        self.command(0x08)?;
        for (address, row) in [0x80, 0xc0].into_iter().zip(frame) {
            self.command(address)?;
            for &byte in row {
                let packet = Zeroizing::new([0x40, byte]);
                self.write(LCD_ADDRESS, &packet[..])?;
                self.delay.delay_us(50);
            }
        }
        self.command(0x0c)?;
        self.brightness(0xff)?;
        Ok(())
    }
    /// Also use on cancel, expiry, unplug, suspend, scene changes and faults.
    /// The RC/discharge bound is a hardware qualification condition; this
    /// software delay is not an independent physical clearing measurement.
    pub fn lock(&mut self) -> Result<(), Error> {
        if !self.power_off() {
            self.faulted = true;
            return Err(Error::Io);
        }
        Ok(())
    }
}
impl<I: I2c, R: OutputPin, A: OutputPin, D: DelayNs> Drop for Lcd<I, R, A, D> {
    fn drop(&mut self) {
        self.power_off();
    }
}
