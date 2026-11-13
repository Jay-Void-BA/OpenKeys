#![no_std]
#![forbid(unsafe_code)]
//! H573 secure-flash controller for two fixed journal sectors, not a general
//! flash programmer, updater or factory eraser. RM0481 Rev5 chapter7.
use openkeys_h573_crypto::device_key::DeviceKeyContext;
use openkeys_root_journal::{Error, PHRASE_BYTES, SECTOR_BYTES, Storage};
use zeroize::Zeroizing;

pub const SECURE_BASE: u32 = 0x0c1f_c000;
pub const NONSECURE_BASE: u32 = 0x081f_c000;
pub const FIRST_SECTOR: u8 = 126;
const LOCK: u32 = 1;
const PG: u32 = 1 << 1;
const SER: u32 = 1 << 2;
const START: u32 = 1 << 5;
const BANK2: u32 = 1 << 31;
const ACTIVE: u32 = 0b1011; // BSY, WBNE and DBNE, not only BSY.
const ERRORS: u32 = 0x007e_0000;
const EOP: u32 = 1 << 16;
const MAX_POLLS: usize = 500_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Register {
    Status,
    Control,
    Clear,
    Key,
    Correction,
    Detection,
}

/// Actual fresh readbacks. The MCU secure watermark must include BOTH last
/// bank2 sectors, with no bank swap/EDATA remap. Caches are conservatively
/// required off; a future cache-coherent adapter needs independent review.
/// This cannot prove the signed boot/OTA layout excludes these sectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub options: u32,
    pub secure_watermark: u32,
    pub secure_tail: u32,
    pub privileged_tail: u32,
    pub edata: u32,
    pub icache_control: u32,
    pub icache_status: u32,
    pub dcache_control: u32,
    pub dcache_status: u32,
}
impl Layout {
    fn qualify(&self) -> Result<(), Error> {
        if self.options & (1 << 31) != 0
            || self.secure_watermark & 0x7f > u32::from(FIRST_SECTOR)
            || (self.secure_watermark >> 16) & 0x7f != 127
            || self.secure_tail & (3 << 30) != 3 << 30
            || self.privileged_tail & (3 << 30) != 3 << 30
            || self.edata & (1 << 15) != 0
            || self.icache_control & 1 != 0
            || self.icache_status & 5 != 0
            || self.dcache_control & 1 != 0
            || self.dcache_status & 13 != 0
        {
            return Err(Error::Authorization);
        }
        Ok(())
    }
}

/// The real port owns FLASH and accepts memory accesses ONLY within the two
/// constants above. It must perform four ordered32-bit stores as one phrase,
/// with barriers, no interruption/other write-buffer user and no DMA. No
/// arbitrary address, option key, bank/mass erase, forced write or ECC clear.
pub trait Port {
    fn read(&mut self, register: Register) -> Result<u32, Error>;
    fn write(&mut self, register: Register, value: u32) -> Result<(), Error>;
    fn context(&mut self) -> Result<DeviceKeyContext, Error>;
    fn layout(&mut self) -> Result<Layout, Error>;
    fn read_bytes(&mut self, slot: u8, offset: usize, out: &mut [u8]) -> Result<(), Error>;
    fn program_phrase(
        &mut self,
        slot: u8,
        offset: usize,
        data: &[u8; PHRASE_BYTES],
    ) -> Result<(), Error>;
    fn emergency_reset(&mut self) -> !;
}

pub struct Flash<P: Port> {
    port: P,
    context: Option<DeviceKeyContext>,
    layout: Option<Layout>,
    closed: bool,
}
impl<P: Port> Flash<P> {
    pub fn new(port: P) -> Result<Self, Error> {
        let mut this = Self {
            port,
            context: None,
            layout: None,
            closed: false,
        };
        let result = (|| {
            this.check()?;
            if this.status()? & ACTIVE != 0 || this.port.read(Register::Control)? != LOCK {
                return Err(Error::Io);
            }
            Ok(())
        })();
        this.finish(result)?;
        Ok(this)
    }
    fn range(slot: u8, offset: usize, length: usize) -> Result<(), Error> {
        if slot > 1 || offset > SECTOR_BYTES || length > SECTOR_BYTES - offset {
            return Err(Error::Input);
        }
        Ok(())
    }
    fn check(&mut self) -> Result<(), Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        let layout = self.port.layout()?;
        layout.qualify()?;
        let context = self.port.context().map_err(|_| Error::Authorization)?;
        if self.context.is_some_and(|previous| previous != context)
            || self.layout.is_some_and(|previous| previous != layout)
        {
            return Err(Error::Authorization);
        }
        self.context = Some(context);
        self.layout = Some(layout);
        Ok(())
    }
    fn status(&mut self) -> Result<u32, Error> {
        let status = self.port.read(Register::Status)?;
        if status & ERRORS != 0
            || self.port.read(Register::Correction)? & (1 << 30) != 0
            || self.port.read(Register::Detection)? & (1 << 31) != 0
        {
            return Err(Error::Io);
        }
        Ok(status)
    }
    fn wait(&mut self, completion: bool) -> Result<(), Error> {
        for _ in 0..MAX_POLLS {
            self.check()?;
            let status = self.status()?;
            if status & ACTIVE == 0 {
                return if !completion || status & EOP != 0 {
                    Ok(())
                } else {
                    Err(Error::Io)
                };
            }
        }
        // Returning while hardware could still program a phrase/sector is
        // unsafe. No force-write, retry, buffer fill or guessed completion.
        self.port.emergency_reset()
    }
    fn unlock(&mut self) -> Result<(), Error> {
        self.check()?;
        self.wait(false)?;
        if self.port.read(Register::Control)? != LOCK {
            return Err(Error::Io);
        }
        self.port.write(Register::Key, 0x4567_0123)?;
        self.port.write(Register::Key, 0xcdef_89ab)?;
        if self.port.read(Register::Control)? != 0 {
            return Err(Error::Io);
        }
        // A fresh completion must be observed, never reuse an old EOP.
        // All hardware errors are already checked and are NEVER cleared.
        self.port.write(Register::Clear, EOP)?;
        if self.status()? & (EOP | ACTIVE) != 0 {
            return Err(Error::Io);
        }
        self.check()
    }
    fn lock(&mut self) {
        // Must run even after a context/error fault. Do not change control
        // while BUSY/write/data buffers remain active. A stuck state resets.
        for _ in 0..MAX_POLLS {
            match self.port.read(Register::Status) {
                Ok(status) if status & ACTIVE == 0 => {
                    match self.port.read(Register::Control) {
                        Ok(LOCK) => return,
                        Ok(_) => {}
                        Err(_) => self.port.emergency_reset(),
                    }
                    if self.port.write(Register::Control, 0).is_err()
                        || self.port.write(Register::Control, LOCK).is_err()
                        || self.port.read(Register::Control) != Ok(LOCK)
                    {
                        self.port.emergency_reset()
                    }
                    return;
                }
                Ok(_) => {}
                Err(_) => self.port.emergency_reset(),
            }
        }
        self.port.emergency_reset()
    }
    fn finish<T>(&mut self, result: Result<T, Error>) -> Result<T, Error> {
        // All successful paths already locked explicitly or never unlocked.
        // Do not silently repair an unexpected control change into success.
        let control = match self.port.read(Register::Control) {
            Ok(control) => control,
            Err(_) => self.port.emergency_reset(),
        };
        let result = result.and_then(|value| {
            if control != LOCK {
                Err(Error::Io)
            } else {
                Ok(value)
            }
        });
        self.lock();
        let result = result.and_then(|value| {
            self.status()?;
            self.check()?;
            Ok(value)
        });
        if result.is_err() {
            self.closed = true;
        }
        result
    }
}
impl<P: Port> Storage for Flash<P> {
    fn read(&mut self, slot: u8, offset: usize, out: &mut [u8]) -> Result<(), Error> {
        out.fill(0);
        let result = (|| {
            Self::range(slot, offset, out.len())?;
            self.check()?;
            self.wait(false)?;
            if self.port.read(Register::Control)? != LOCK {
                return Err(Error::Io);
            }
            self.port.read_bytes(slot, offset, out)?;
            self.check()?;
            self.status()?;
            Ok(())
        })();
        let result = self.finish(result);
        if result.is_err() {
            out.fill(0);
        }
        result
    }
    fn program(&mut self, slot: u8, offset: usize, data: &[u8; PHRASE_BYTES]) -> Result<(), Error> {
        let result = (|| {
            Self::range(slot, offset, PHRASE_BYTES)?;
            if !offset.is_multiple_of(PHRASE_BYTES) {
                return Err(Error::Input);
            }
            self.check()?;
            self.wait(false)?;
            if self.port.read(Register::Control)? != LOCK {
                return Err(Error::Io);
            }
            let mut before = Zeroizing::new([0; PHRASE_BYTES]);
            self.port.read_bytes(slot, offset, before.as_mut())?;
            self.status()?;
            if *before != [0xff; PHRASE_BYTES] {
                return Err(Error::Integrity);
            }
            self.check()?;
            // Do not consume ECC on an all-ones/no-change phrase.
            if *data != [0xff; PHRASE_BYTES] {
                self.unlock()?;
                self.port.write(Register::Control, PG)?;
                if self.port.read(Register::Control)? != PG {
                    return Err(Error::Io);
                }
                self.check()?;
                self.port.program_phrase(slot, offset, data)?;
                self.wait(true)?;
                self.lock();
            }
            let mut after = Zeroizing::new([0; PHRASE_BYTES]);
            self.port.read_bytes(slot, offset, after.as_mut())?;
            self.check()?;
            self.status()?;
            if *after != *data {
                return Err(Error::Integrity);
            }
            Ok(())
        })();
        self.finish(result)
    }
    fn erase(&mut self, slot: u8) -> Result<(), Error> {
        let result = (|| {
            Self::range(slot, 0, SECTOR_BYTES)?;
            self.unlock()?;
            let control = BANK2 | SER | (u32::from(FIRST_SECTOR + slot) << 6);
            self.port.write(Register::Control, control)?;
            if self.port.read(Register::Control)? != control {
                return Err(Error::Io);
            }
            self.check()?;
            self.port.write(Register::Control, control | START)?;
            self.wait(true)?;
            self.lock();
            // Full-sector readback, not a sampled word or controller EOP.
            for offset in (0..SECTOR_BYTES).step_by(256) {
                self.check()?;
                let mut bytes = Zeroizing::new([0; 256]);
                self.port.read_bytes(slot, offset, bytes.as_mut())?;
                self.status()?;
                if bytes.iter().any(|&b| b != 0xff) {
                    return Err(Error::Integrity);
                }
                self.check()?;
            }
            Ok(())
        })();
        self.finish(result)
    }
}
impl<P: Port> Drop for Flash<P> {
    fn drop(&mut self) {
        self.lock();
    }
}

#[cfg(test)]
mod tests;
