//! R04 W25Q64JVSSIQ,8MiB, SPI mode0. Store authenticated ciphertext only.
//! Bounded transfers, page/sector checks, WEL verification and readback. This
//! driver is NOT an atomic vault or an anti-rollback store.
use crate::Error;
use embedded_hal::{
    delay::DelayNs,
    spi::{Operation, SpiDevice},
};

pub const CAPACITY: u32 = 8 * 1024 * 1024;
pub const PAGE: usize = 256;
pub const SECTOR: u32 = 4096;

pub struct Flash<S> {
    spi: S,
    identified: bool,
    faulted: bool,
}

impl<S: SpiDevice> Flash<S> {
    pub const fn new(spi: S) -> Self {
        Self {
            spi,
            identified: false,
            faulted: false,
        }
    }

    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        self.spi.transaction(operations).map_err(|_| {
            self.faulted = true;
            Error::Io
        })
    }

    pub fn identify(&mut self, delay: &mut impl DelayNs) -> Result<(), Error> {
        // Release deep power-down; do not send a reset that could terminate an
        // in-progress write after an MCU-only reset.
        self.transaction(&mut [Operation::Write(&[0xab])])?;
        delay.delay_us(10);
        self.ready(delay, 100)?;
        let mut id = [0; 3];
        self.transaction(&mut [Operation::Write(&[0x9f]), Operation::Read(&mut id)])?;
        if id != [0xef, 0x40, 0x17] {
            self.faulted = true;
            return Err(Error::WrongPart);
        }
        self.identified = true;
        Ok(())
    }

    fn check_range(&self, address: u32, length: usize) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        if !self.identified {
            return Err(Error::WrongPart);
        }
        let length = u32::try_from(length).map_err(|_| Error::InvalidInput)?;
        if length == 0 || address.checked_add(length).is_none_or(|end| end > CAPACITY) {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }

    fn header(opcode: u8, address: u32) -> [u8; 4] {
        let bytes = address.to_be_bytes();
        [opcode, bytes[1], bytes[2], bytes[3]]
    }

    fn status(&mut self) -> Result<u8, Error> {
        let mut byte = [0];
        self.transaction(&mut [Operation::Write(&[0x05]), Operation::Read(&mut byte)])?;
        // A floating/high MISO bus is never accepted as a valid ready status.
        if byte[0] == 0xff {
            self.faulted = true;
            return Err(Error::Io);
        }
        Ok(byte[0])
    }

    fn ready(&mut self, delay: &mut impl DelayNs, polls: u32) -> Result<(), Error> {
        for _ in 0..polls {
            if self.status()? & 1 == 0 {
                return Ok(());
            }
            delay.delay_us(100);
        }
        self.faulted = true;
        Err(Error::Timeout)
    }

    fn enable_write(&mut self, delay: &mut impl DelayNs) -> Result<(), Error> {
        self.ready(delay, 100)?;
        self.transaction(&mut [Operation::Write(&[0x06])])?;
        if self.status()? & 3 != 2 {
            self.faulted = true;
            return Err(Error::Verify);
        }
        Ok(())
    }

    /// Maximum one page per call keeps the scheduler's work bounded.
    pub fn read(
        &mut self,
        address: u32,
        output: &mut [u8],
        delay: &mut impl DelayNs,
    ) -> Result<(), Error> {
        self.check_range(address, output.len())?;
        if output.len() > PAGE {
            return Err(Error::InvalidInput);
        }
        self.ready(delay, 100)?;
        self.transaction(&mut [
            Operation::Write(&Self::header(0x03, address)),
            Operation::Read(output),
        ])
    }

    pub fn program_page(
        &mut self,
        address: u32,
        input: &[u8],
        delay: &mut impl DelayNs,
    ) -> Result<(), Error> {
        self.check_range(address, input.len())?;
        if input.len() > PAGE || address as usize % PAGE + input.len() > PAGE {
            return Err(Error::InvalidInput);
        }
        self.enable_write(delay)?;
        self.transaction(&mut [
            Operation::Write(&Self::header(0x02, address)),
            Operation::Write(input),
        ])?;
        self.ready(delay, 100)?;
        let mut actual = [0; PAGE];
        self.read(address, &mut actual[..input.len()], delay)?;
        let mismatch = input
            .iter()
            .zip(actual)
            .fold(0, |sum, (a, b)| sum | (a ^ b));
        if mismatch != 0 {
            self.faulted = true;
            return Err(Error::Verify);
        }
        Ok(())
    }

    pub fn erase_sector(&mut self, address: u32, delay: &mut impl DelayNs) -> Result<(), Error> {
        self.check_range(address, SECTOR as usize)?;
        if !address.is_multiple_of(SECTOR) {
            return Err(Error::InvalidInput);
        }
        self.enable_write(delay)?;
        self.transaction(&mut [Operation::Write(&Self::header(0x20, address))])?;
        self.ready(delay, 10_000)?;
        let mut block = [0; PAGE];
        let mut mismatch = 0;
        for offset in (0..SECTOR).step_by(PAGE) {
            self.read(address + offset, &mut block, delay)?;
            for byte in block {
                mismatch |= byte ^ 0xff;
            }
        }
        if mismatch != 0 {
            self.faulted = true;
            return Err(Error::Verify);
        }
        Ok(())
    }
}
