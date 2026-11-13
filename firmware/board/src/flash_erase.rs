//! Locked R05 ALL-DATA NOR erasure. Consumes the sole identified flash owner;
//! erases every sector, then independently reads every byte a second time.
//! Progress is volatile: reset/recovery MUST restart at zero while a separate
//! protected durable erase intent keeps all normal interfaces unavailable.
//! No resume offset, host-selected range or unauthenticated "already erased".
use crate::{
    Error,
    flash::{CAPACITY, Flash, PAGE, SECTOR},
};
use embedded_hal::{delay::DelayNs, spi::SpiDevice};
use zeroize::Zeroizing;

/// Root must already have durably latched ALL-DATA erasure, revoked both PINs,
/// disabled normal requests and cleared private RAM/display. Check the same
/// generation, hardware state and maintenance watchdog before/after each step.
/// This is not a host flag, ordinary PIN lease or request to renew that lease.
pub trait Checkpoint {
    fn check(&mut self) -> Result<(), Error>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    Erasing { sectors_done: u16 },
    Verifying { pages_done: u32 },
    Complete,
}
enum Phase {
    Erase,
    Verify,
    Done,
}

/// Not a certificate of SE/Root destruction. The receipt exclusively retains
/// the same flash owner so a caller cannot keep writing through this API while
/// claiming an earlier completed wipe. Root consumes it during commissioning.
/// Physical replacement/probing, shared GPIO/DMA and stale journals need the
/// independent hardware/boot boundary and full reset recovery tests.
pub struct ErasedFlash<S> {
    flash: Flash<S>,
}
impl<S> ErasedFlash<S> {
    pub fn into_flash(self) -> Flash<S> {
        self.flash
    }
}
pub struct Eraser<S> {
    flash: Flash<S>,
    phase: Phase,
    next: u32,
    faulted: bool,
}
impl<S: SpiDevice> Eraser<S> {
    pub fn begin(flash: Flash<S>, checkpoint: &mut impl Checkpoint) -> Result<Self, Error> {
        checkpoint.check()?;
        flash.check_erase_owner()?;
        Ok(Self {
            flash,
            phase: Phase::Erase,
            next: 0,
            faulted: false,
        })
    }
    pub fn progress(&self) -> Result<Progress, Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        Ok(match self.phase {
            Phase::Erase => Progress::Erasing {
                sectors_done: (self.next / SECTOR) as u16,
            },
            Phase::Verify => Progress::Verifying {
                pages_done: self.next / PAGE as u32,
            },
            Phase::Done => Progress::Complete,
        })
    }
    /// At most one sector erase (including its full readback) OR one256-byte
    /// verification read. Flash has bounded BUSY polls; Root must qualify the
    /// real worst-case duration and service the watchdog between calls. This
    /// maintenance can outlast a user's30s lease; it never authorizes private use.
    pub fn step(
        &mut self,
        delay: &mut impl DelayNs,
        checkpoint: &mut impl Checkpoint,
    ) -> Result<Progress, Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        let result = self.step_inner(delay, checkpoint);
        if result.is_err() {
            self.faulted = true;
        }
        result
    }
    fn step_inner(
        &mut self,
        delay: &mut impl DelayNs,
        checkpoint: &mut impl Checkpoint,
    ) -> Result<Progress, Error> {
        checkpoint.check()?;
        match self.phase {
            Phase::Erase => {
                self.flash.erase_sector(self.next, delay)?;
                checkpoint.check()?;
                self.next = self.next.checked_add(SECTOR).ok_or(Error::InvalidInput)?;
                if self.next == CAPACITY {
                    self.phase = Phase::Verify;
                    self.next = 0;
                }
            }
            Phase::Verify => {
                let mut bytes = Zeroizing::new([0; PAGE]);
                self.flash.read(self.next, &mut bytes[..], delay)?;
                checkpoint.check()?;
                if bytes.iter().any(|&b| b != 0xff) {
                    return Err(Error::Verify);
                }
                self.next = self
                    .next
                    .checked_add(PAGE as u32)
                    .ok_or(Error::InvalidInput)?;
                if self.next == CAPACITY {
                    self.phase = Phase::Done;
                }
            }
            Phase::Done => {}
        }
        self.progress()
    }
    /// A single-use receipt only after the complete independent pass. The
    /// durable Root journal must still verify SE and Root-secret destruction;
    /// this function cannot mark the whole device blank or clear an erase latch.
    pub fn finish(self, checkpoint: &mut impl Checkpoint) -> Result<ErasedFlash<S>, Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        if !matches!(self.phase, Phase::Done) {
            return Err(Error::Verify);
        }
        checkpoint.check()?;
        Ok(ErasedFlash { flash: self.flash })
    }
}
