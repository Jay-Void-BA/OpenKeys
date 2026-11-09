//! R04 NHD-C0216CiZ-FSW-FBW-3V3,7-bit0x3e,3.0V logic.
//! Reset MUST be an open-drain GPIO (PC3), not3.3V push-pull.
//! The bus is owned by the protected domain; no host raw-display API exists.
use crate::Error;
use device_policy::lcd::Frame;
use embedded_hal::{delay::DelayNs, digital::OutputPin, i2c::I2c};

pub const ADDRESS: u8 = 0x3e;

pub struct Lcd<I: I2c, R: OutputPin, B: OutputPin, D: DelayNs> {
    bus: I,
    delay: D,
    reset: R,
    backlight: B,
    initialized: bool,
    faulted: bool,
}

impl<I: I2c, R: OutputPin, B: OutputPin, D: DelayNs> Lcd<I, R, B, D> {
    pub fn new(bus: I, reset: R, backlight: B, delay: D) -> Result<Self, Error> {
        let mut lcd = Self {
            bus,
            delay,
            reset,
            backlight,
            initialized: false,
            faulted: false,
        };
        lcd.lock()?;
        Ok(lcd)
    }

    fn fault(&mut self) -> Error {
        self.initialized = false;
        self.faulted = true;
        // Attempt both even when one GPIO reports an error. R04 U15 also
        // asserts the physical reset independently when NRST goes low.
        let _ = self.backlight.set_low();
        let _ = self.reset.set_low();
        self.delay.delay_ms(2);
        Error::Io
    }

    fn command(&mut self, value: u8) -> Result<(), Error> {
        self.bus
            .write(ADDRESS, &[0x00, value])
            .map_err(|_| self.fault())?;
        self.delay.delay_us(if value == 0x01 || value == 0x02 {
            2000
        } else {
            50
        });
        Ok(())
    }

    /// Mirrors the vendor's initialization sequence with conservative delays,
    /// but keeps display and backlight OFF until both complete rows are loaded.
    /// The current vendor PDF names AiP31032; its example retains a legacy
    /// ST7032i heading. Real modules/revisions require qualification.
    pub fn initialize(&mut self) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        self.lock()?;
        self.delay.delay_ms(2);
        self.reset.set_high().map_err(|_| self.fault())?;
        self.delay.delay_ms(50);
        for cmd in [0x38, 0x39] {
            self.command(cmd)?;
            self.delay.delay_ms(10);
        }
        for cmd in [0x14, 0x78, 0x5e, 0x6d] {
            self.command(cmd)?;
        }
        self.delay.delay_ms(200);
        for cmd in [0x38, 0x08, 0x01, 0x06] {
            self.command(cmd)?;
        }
        self.initialized = true;
        Ok(())
    }

    /// Complete-row replacement avoids stale trailing characters. Success is
    /// bus acknowledgement and command timing, not optical proof of the text.
    /// Call Review::displayed only after this method returns Ok.
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
        self.backlight.set_low().map_err(|_| self.fault())?;
        self.command(0x08)?;
        for (address, row) in [0x80, 0xc0].into_iter().zip(frame) {
            self.command(address)?;
            let mut bytes = [0x40; 17];
            bytes[1..].copy_from_slice(row);
            self.bus.write(ADDRESS, &bytes).map_err(|_| self.fault())?;
            self.delay.delay_us(50);
        }
        self.command(0x0c)?;
        self.backlight.set_high().map_err(|_| self.fault())?;
        Ok(())
    }

    /// Reset clears controller state as well as extinguishing its backlight.
    /// Use on lock, timeout, cancellation, disconnect, suspend and any fault.
    pub fn lock(&mut self) -> Result<(), Error> {
        self.initialized = false;
        let light = self.backlight.set_low();
        let reset = self.reset.set_low();
        self.delay.delay_ms(2);
        if light.is_err() || reset.is_err() {
            return Err(self.fault());
        }
        Ok(())
    }
}

// Clear before GPIO destructors return their pins to floating state. Merely
// dropping a backlight pin would leave readable transflective characters.
impl<I: I2c, R: OutputPin, B: OutputPin, D: DelayNs> Drop for Lcd<I, R, B, D> {
    fn drop(&mut self) {
        let _ = self.backlight.set_low();
        let _ = self.reset.set_low();
        self.delay.delay_ms(2);
    }
}
