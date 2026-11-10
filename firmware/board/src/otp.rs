//! Device-only TOTP/HOTP display. No seed/code export or USB keyboard output.
//! The protected root must supply the real SE, RTC, counter and LCD services.
//! This module is not a complete commissioned production OTP application.
use crate::Error;
use device_policy::{
    Authorization, Operation, Request,
    execution::{Completed, Execution, State},
    lcd::Frame,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Algorithm {
    Sha1 = 0x18,
    Sha256 = 0x19,
    Sha512 = 0x1b,
}
impl Algorithm {
    pub const fn bytes(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha512 => 64,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Totp,
    Hotp,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot(u8);
impl Slot {
    pub fn new(index: u8) -> Result<Self, Error> {
        if index < 32 {
            Ok(Self(index))
        } else {
            Err(Error::InvalidInput)
        }
    }
    pub const fn index(self) -> u8 {
        self.0
    }
}
/// Uses the root's live authenticated connection binding, never a host label.
/// Slot/algorithm must be loaded from the authenticated device catalog before
/// displaying the canonical account identity and collecting the local PIN.
pub fn request(mode: Mode, slot: Slot, algorithm: Algorithm, channel: [u8; 32]) -> Request {
    let mut h = Sha256::new();
    h.update(b"OpenKeys/OTP/display/v1");
    h.update([
        match mode {
            Mode::Totp => 1,
            Mode::Hotp => 2,
        },
        slot.0,
        algorithm as u8,
    ]);
    Request {
        operation: match mode {
            Mode::Totp => Operation::ShowTotp,
            Mode::Hotp => Operation::ShowHotp,
        },
        digest: h.finalize().into(),
        channel,
    }
}
pub trait ProtectedOtp: State {
    /// Real backup RTC milliseconds, with invalid/reset/rollback state denied.
    /// Setting it is a separate authenticated, locally approved operation.
    fn utc_millis(&mut self) -> Result<u64, Error>;
    /// Durable preincrement + readback, returning the consumed OLD counter.
    /// Lost acknowledgement must return Err with no retry/refund/reuse.
    fn reserve_hotp(
        &mut self,
        execution: &Execution,
        slot: Slot,
        algorithm: Algorithm,
    ) -> Result<u64, Error>;
    /// SE-internal HMAC with a non-exportable OTP key in a separate namespace.
    /// No arbitrary-key identifier or raw MAC API may be exposed to a host.
    fn hmac(
        &mut self,
        execution: &Execution,
        slot: Slot,
        algorithm: Algorithm,
        counter: &[u8; 8],
        out: &mut [u8; 64],
    ) -> Result<usize, Error>;
}
pub trait TrustedScreen {
    fn write_private_frame(&mut self, frame: &Frame) -> Result<(), Error>;
    fn clear_private_frame(&mut self) -> Result<(), Error>;
}
impl<
    I: embedded_hal::i2c::I2c,
    R: embedded_hal::digital::OutputPin,
    A: embedded_hal::digital::OutputPin,
    D: embedded_hal::delay::DelayNs,
> TrustedScreen for crate::dfr0555::Lcd<I, R, A, D>
{
    fn write_private_frame(&mut self, frame: &Frame) -> Result<(), Error> {
        self.show(frame)
    }
    fn clear_private_frame(&mut self) -> Result<(), Error> {
        self.lock()
    }
}
pub struct Display<B: ProtectedOtp, S: TrustedScreen> {
    backend: B,
    screen: S,
    visible: Option<(u64, u64, Option<u64>)>,
    last: u64,
    faulted: bool,
}
impl<B: ProtectedOtp, S: TrustedScreen> Display<B, S> {
    pub const fn new(backend: B, screen: S, now: u64) -> Self {
        Self {
            backend,
            screen,
            visible: None,
            last: now,
            faulted: false,
        }
    }
    fn time(&mut self, now: u64) -> Result<(), Error> {
        if self.faulted || now < self.last {
            self.faulted = true;
            self.revoke()?;
            return Err(Error::Faulted);
        }
        self.last = now;
        Ok(())
    }
    pub fn revoke(&mut self) -> Result<(), Error> {
        self.visible = None;
        self.screen
            .clear_private_frame()
            .inspect_err(|_| self.faulted = true)
    }
    /// Call frequently even with no host/keyboard event. No automatic code
    /// regeneration is allowed; each new display consumes a locally approved
    /// grant within the authenticated hardware-PIN session.
    pub fn tick(&mut self, now: u64) -> Result<(), Error> {
        self.time(now)?;
        if let Some((until, generation, step)) = self.visible {
            let step_valid = step.is_none_or(|step| {
                self.backend
                    .utc_millis()
                    .is_ok_and(|utc| utc / 30_000 == step)
            });
            if now >= until
                || !step_valid
                || self.backend.generation() != generation
                || self.backend.check_security_state().is_err()
            {
                self.revoke()?;
            }
        }
        Ok(())
    }
    /// Returns only an opaque completion capability. Six digits exist only in
    /// zeroizing device frame storage and the trusted LCD, never in a response.
    pub fn show_once(
        &mut self,
        auth: Authorization,
        mode: Mode,
        slot: Slot,
        algorithm: Algorithm,
        channel: [u8; 32],
        mut clock: impl FnMut() -> u64,
    ) -> Result<Completed, Error> {
        let result = (|| {
            self.time(clock())?;
            let expected = request(mode, slot, algorithm, channel);
            let mut execution = auth
                .activate(&expected, &mut self.backend, &mut clock)
                .map_err(|_| Error::Verify)?;
            let generation = execution.authorization().generation();
            let measured = clock();
            let (counter, until) = match mode {
                Mode::Totp => {
                    let utc = self.backend.utc_millis()?;
                    let until = measured
                        .checked_add(30_000 - utc % 30_000)
                        .ok_or(Error::Timeout)?;
                    (utc / 30_000, until)
                }
                Mode::Hotp => {
                    let counter = self.backend.reserve_hotp(&execution, slot, algorithm)?;
                    (counter, measured.checked_add(30_000).ok_or(Error::Timeout)?)
                }
            };
            execution
                .check(&expected, &mut self.backend, &mut clock)
                .map_err(|_| Error::Verify)?;
            let mut mac = Zeroizing::new([0; 64]);
            let n = self.backend.hmac(
                &execution,
                slot,
                algorithm,
                &counter.to_be_bytes(),
                &mut mac,
            )?;
            if n != algorithm.bytes() {
                return Err(Error::Verify);
            }
            execution
                .check(&expected, &mut self.backend, &mut clock)
                .map_err(|_| Error::Verify)?;
            if mode == Mode::Totp && self.backend.utc_millis()? / 30_000 != counter {
                return Err(Error::Timeout);
            }
            let now = clock();
            self.time(now)?;
            if now >= until {
                return Err(Error::Timeout);
            }
            let offset = usize::from(mac[n - 1] & 15);
            let value = (u32::from(mac[offset] & 0x7f) << 24)
                | (u32::from(mac[offset + 1]) << 16)
                | (u32::from(mac[offset + 2]) << 8)
                | u32::from(mac[offset + 3]);
            let mut code = Zeroizing::new(value % 1_000_000);
            let mut frame = Zeroizing::new([
                match mode {
                    Mode::Totp => *b"TOTP            ",
                    Mode::Hotp => *b"HOTP            ",
                },
                *b"000000          ",
            ]);
            for index in (0..6).rev() {
                frame[1][index] = b'0' + (*code % 10) as u8;
                *code /= 10;
            }
            self.screen.write_private_frame(&frame)?;
            execution
                .check(&expected, &mut self.backend, &mut clock)
                .map_err(|_| Error::Verify)?;
            let now = clock();
            self.time(now)?;
            if now >= until
                || (mode == Mode::Totp && self.backend.utc_millis()? / 30_000 != counter)
            {
                return Err(Error::Timeout);
            }
            self.visible = Some((
                until,
                generation,
                if mode == Mode::Totp {
                    Some(counter)
                } else {
                    None
                },
            ));
            execution
                .finish(&expected, &mut self.backend, &mut clock)
                .map_err(|_| Error::Verify)
        })();
        if result.is_err() {
            self.revoke()?;
        }
        result
    }
}
impl<B: ProtectedOtp, S: TrustedScreen> Drop for Display<B, S> {
    fn drop(&mut self) {
        let _ = self.revoke();
    }
}
