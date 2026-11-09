//! H573 I2C-v2 master for the fixed SE050 address. A retryable address NACK
//! requires zero data-register progress, an empty RX/TX state and confirmed
//! STOP/bus-idle. Data/unknown errors are terminal, never guessed as busy.
use openkeys_se_transport::{
    MAX_FRAME,
    transport::{Clock, Io, IoError},
};

pub const PE: u32 = 1;
pub const TXE: u32 = 1;
pub const TXIS: u32 = 1 << 1;
pub const RXNE: u32 = 1 << 2;
pub const NACK: u32 = 1 << 4;
pub const STOP: u32 = 1 << 5;
pub const TC: u32 = 1 << 6;
pub const TCR: u32 = 1 << 7;
pub const FAULTS: u32 = 0x3f00; // BERR,ARLO,OVR,PECERR,TIMEOUT,ALERT
pub const BUSY: u32 = 1 << 15;
const IRQ_DMA: u32 = 0xfe | (3 << 14);
const START: u32 = 1 << 13;
const RELOAD: u32 = 1 << 24;
const AUTOEND: u32 = 1 << 25;
const READ: u32 = 1 << 10;
const ADDRESS: u32 = 0x48 << 1;
const IO_US: u64 = 100_000;
const POLLS: u32 = 2_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Hardware,
    Timeout,
    Clock,
    Frame,
    Progress,
    Closed,
    AddressBusy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Register {
    Cr1,
    Cr2,
    Isr,
    Icr,
    Rxdr,
    Txdr,
    Timing,
}
/// Only the fixed Secure-alias target port is a production implementation.
/// No arbitrary addresses, transactions, DMA or peripheral reset are exposed.
pub trait Registers {
    fn read(&mut self, reg: Register) -> Result<u32, Error>;
    fn write(&mut self, reg: Register, value: u32) -> Result<(), Error>;
}
struct Budget {
    until: u64,
    last: u64,
    polls: u32,
}
pub struct Master<R: Registers, C: Clock> {
    registers: R,
    clock: C,
    profile: u32,
    timing: u32,
    last_time: u64,
    closed: bool,
}
impl<R: Registers, C: Clock> Master<R, C> {
    /// Peripheral/pins/timing must already be configured in the protected
    /// root. Never resets a peripheral to conceal an earlier transfer/fault.
    pub fn new(mut registers: R, mut clock: C) -> Result<Self, Error> {
        let profile = registers.read(Register::Cr1)?;
        let timing = registers.read(Register::Timing)?;
        let status = registers.read(Register::Isr)?;
        if profile & PE == 0
            || profile & IRQ_DMA != 0
            || timing == 0
            || status & (BUSY | FAULTS | NACK | STOP | RXNE | TXIS | TC | TCR) != 0
            || status & TXE == 0
        {
            let _ = registers.write(Register::Cr1, profile & !PE);
            return Err(Error::Hardware);
        }
        let last_time = clock.now_us();
        Ok(Self {
            registers,
            clock,
            profile,
            timing,
            last_time,
            closed: false,
        })
    }
    fn begin(&mut self, len: usize) -> Result<Budget, Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        if len == 0 || len > MAX_FRAME {
            return Err(Error::Frame);
        }
        let now = self.clock.now_us();
        if now < self.last_time {
            return Err(Error::Clock);
        }
        self.last_time = now;
        let mut b = Budget {
            until: now.checked_add(IO_US).ok_or(Error::Clock)?,
            last: now,
            polls: POLLS,
        };
        let status = self.status(&mut b)?;
        if status & (BUSY | NACK | STOP | RXNE | TXIS | TC | TCR) != 0 || status & TXE == 0 {
            return Err(Error::Progress);
        }
        Ok(b)
    }
    fn status(&mut self, b: &mut Budget) -> Result<u32, Error> {
        if b.polls == 0 {
            return Err(Error::Timeout);
        }
        b.polls -= 1;
        let now = self.clock.now_us();
        if now < b.last || now < self.last_time {
            return Err(Error::Clock);
        }
        b.last = now;
        self.last_time = now;
        if now >= b.until {
            return Err(Error::Timeout);
        }
        if self.registers.read(Register::Cr1)? != self.profile
            || self.registers.read(Register::Timing)? != self.timing
        {
            return Err(Error::Hardware);
        }
        let status = self.registers.read(Register::Isr)?;
        if status & FAULTS != 0 {
            return Err(Error::Hardware);
        }
        Ok(status)
    }
    fn chunk(&mut self, left: usize, read: bool, start: bool) -> Result<usize, Error> {
        let count = left.min(255);
        let end = if left > 255 { RELOAD } else { AUTOEND };
        let cr2 = ADDRESS
            | ((count as u32) << 16)
            | end
            | if read { READ } else { 0 }
            | if start { START } else { 0 };
        self.registers.write(Register::Cr2, cr2)?;
        Ok(count)
    }
    fn address_busy(&mut self, b: &mut Budget, progress: usize, status: u32) -> Result<(), Error> {
        if progress != 0 || status & (RXNE | TXIS | TC | TCR) != 0 || status & TXE == 0 {
            return Err(Error::Progress);
        }
        // RM0481 controller mode sends STOP automatically on received NACK.
        // Confirm it; do not clear a NACK and optimistically resend data.
        loop {
            let status = self.status(b)?;
            if status & (RXNE | TXIS | TC | TCR) != 0 || status & TXE == 0 {
                return Err(Error::Progress);
            }
            if status & STOP != 0 && status & BUSY == 0 {
                self.registers.write(Register::Icr, NACK | STOP)?;
                let clean = self.status(b)?;
                if clean & (BUSY | NACK | STOP | RXNE | TXIS | TC | TCR) != 0 || clean & TXE == 0 {
                    return Err(Error::Progress);
                }
                return Err(Error::AddressBusy);
            }
        }
    }
    fn wait(
        &mut self,
        b: &mut Budget,
        wanted: u32,
        progress: usize,
        read: bool,
    ) -> Result<u32, Error> {
        loop {
            let status = self.status(b)?;
            if status & NACK != 0 {
                self.address_busy(b, progress, status)?;
            }
            if (!read && status & RXNE != 0) || (read && status & TXIS != 0) {
                return Err(Error::Progress);
            }
            if status & wanted != 0 {
                return Ok(status);
            }
            if status & (STOP | TC | TCR) != 0 {
                return Err(Error::Progress);
            }
        }
    }
    fn complete(&mut self, b: &mut Budget, progress: usize, read: bool) -> Result<(), Error> {
        self.wait(b, STOP, progress, read)?;
        let status = self.status(b)?;
        if status & (BUSY | RXNE | TXIS | TC | TCR) != 0 || status & TXE == 0 {
            return Err(Error::Progress);
        }
        self.registers.write(Register::Icr, STOP)?;
        let clean = self.status(b)?;
        if clean & (BUSY | NACK | STOP | RXNE | TXIS | TC | TCR) != 0 || clean & TXE == 0 {
            return Err(Error::Progress);
        }
        Ok(())
    }
    fn transfer(&mut self, bytes: &mut [u8], read: bool) -> Result<(), Error> {
        let mut b = self.begin(bytes.len())?;
        let mut chunk = self.chunk(bytes.len(), read, true)?;
        for index in 0..bytes.len() {
            if chunk == 0 {
                self.wait(&mut b, TCR, index, read)?;
                chunk = self.chunk(bytes.len() - index, read, false)?;
            }
            self.wait(&mut b, if read { RXNE } else { TXIS }, index, read)?;
            if read {
                bytes[index] = self.registers.read(Register::Rxdr)? as u8;
            } else {
                self.registers
                    .write(Register::Txdr, u32::from(bytes[index]))?;
            }
            chunk -= 1;
        }
        self.complete(&mut b, bytes.len(), read)
    }
    fn finish(&mut self, result: Result<(), Error>) -> Result<(), IoError> {
        match result {
            Ok(()) => Ok(()),
            Err(Error::AddressBusy) => Err(IoError::AddressNotReady),
            Err(_) => {
                self.close();
                Err(IoError::Fault)
            }
        }
    }
    pub fn close(&mut self) {
        self.closed = true;
        let _ = self.registers.write(Register::Cr1, self.profile & !PE);
    }
}
impl<R: Registers, C: Clock> Io for Master<R, C> {
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        bytes.fill(0);
        let result = self.transfer(bytes, true);
        let result = self.finish(result);
        if result.is_err() {
            bytes.fill(0);
        }
        result
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), IoError> {
        if bytes.len() > MAX_FRAME {
            self.close();
            return Err(IoError::Fault);
        }
        let mut copy = [0; MAX_FRAME];
        copy[..bytes.len()].copy_from_slice(bytes);
        let result = self.transfer(&mut copy[..bytes.len()], false);
        copy.fill(0);
        self.finish(result)
    }
}
impl<R: Registers, C: Clock> Drop for Master<R, C> {
    fn drop(&mut self) {
        self.close();
    }
}
