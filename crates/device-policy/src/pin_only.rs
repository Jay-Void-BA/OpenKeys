//! R05 local decimal PIN policy. This profile never accepts biometric proofs,
//! a host PIN string, an OS unlock result, or a transport-supplied Boolean.
//! Real protected PIN storage, SE retry accounting and isolated GPIO are still
//! platform obligations; an abstract verifier is not a production deployment.
use crate::{Authorization, Error, Operation, Request, pin::ProtectedPin};
use zeroize::Zeroizing;

pub const PIN_DIGITS: usize = 6;
pub const ENTRY_MS: u64 = 60_000;
pub const REVIEW_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Digit {
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
}
impl Digit {
    pub const fn value(self) -> u8 {
        match self {
            Self::Zero => 0,
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
            Self::Four => 4,
            Self::Five => 5,
            Self::Six => 6,
            Self::Seven => 7,
            Self::Eight => 8,
            Self::Nine => 9,
        }
    }
    /// R05 electrical bit order: 1,2,3,4,5,6,7,8,9,0.
    pub const fn from_gpio_index(index: u32) -> Option<Self> {
        match index {
            0 => Some(Self::One),
            1 => Some(Self::Two),
            2 => Some(Self::Three),
            3 => Some(Self::Four),
            4 => Some(Self::Five),
            5 => Some(Self::Six),
            6 => Some(Self::Seven),
            7 => Some(Self::Eight),
            8 => Some(Self::Nine),
            9 => Some(Self::Zero),
            _ => None,
        }
    }
}

/// Can only be minted by a completed local entry and protected verification.
/// No public constructor, serialization, Clone or host Boolean conversion.
pub struct VerifiedPinRequest {
    request: Request,
    expires: u64,
}
impl VerifiedPinRequest {
    pub const fn request(&self) -> &Request {
        &self.request
    }
    pub const fn expires_ms(&self) -> u64 {
        self.expires
    }
}

pub struct PinEntry {
    digits: Zeroizing<[u8; PIN_DIGITS]>,
    len: usize,
    request: Request,
    expires: u64,
    last: u64,
    cancelled: bool,
}
impl PinEntry {
    pub fn begin(
        request: Request,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Self, crate::pin::Error> {
        let start = clock();
        let expires = start
            .checked_add(ENTRY_MS)
            .ok_or(crate::pin::Error::Expired)?;
        // Editing/cancelling an incomplete entry is not an incorrect guess.
        // The completed guess is durably charged immediately before verify.
        let now = clock();
        if now < start || now >= expires {
            return Err(crate::pin::Error::Expired);
        }
        Ok(Self {
            digits: Zeroizing::new([0; PIN_DIGITS]),
            len: 0,
            request,
            expires,
            last: now,
            cancelled: false,
        })
    }
    fn time(&mut self, now: u64) -> Result<(), crate::pin::Error> {
        if self.cancelled {
            return Err(crate::pin::Error::Cancelled);
        }
        if now < self.last || now >= self.expires {
            self.cancel();
            return Err(crate::pin::Error::Expired);
        }
        self.last = now;
        Ok(())
    }
    pub const fn filled(&self) -> usize {
        self.len
    }
    /// Call even when no key event occurs, so expired input is cleared promptly.
    pub fn tick(&mut self, now: u64) -> Result<(), crate::pin::Error> {
        self.time(now)
    }
    /// Secure debounced RELEASE only: short0..9 appends; hold9 erases;
    /// hold0 cancels. A long hold never also enters its digit.
    pub fn press(
        &mut self,
        digit: Digit,
        press: crate::pin::Press,
        now: u64,
    ) -> Result<usize, crate::pin::Error> {
        self.time(now)?;
        match (digit, press) {
            (Digit::Zero, crate::pin::Press::Long) => {
                self.cancel();
                return Err(crate::pin::Error::Cancelled);
            }
            (Digit::Nine, crate::pin::Press::Long) => {
                self.len = self.len.saturating_sub(1);
                self.digits[self.len] = 0;
            }
            (_, crate::pin::Press::Long) => {}
            (digit, crate::pin::Press::Short) => {
                if self.len == PIN_DIGITS {
                    return Err(crate::pin::Error::Incomplete);
                }
                self.digits[self.len] = digit.value();
                self.len += 1;
            }
        }
        Ok(self.len)
    }
    /// Submit exactly six digits. Inputs are zeroized on every return/drop.
    pub fn submit(
        mut self,
        pin: &mut impl ProtectedPin<PIN_DIGITS>,
        mut clock: impl FnMut() -> u64,
    ) -> Result<VerifiedPinRequest, crate::pin::Error> {
        self.time(clock())?;
        if self.len != PIN_DIGITS {
            return Err(crate::pin::Error::Incomplete);
        }
        pin.reserve_attempt(&self.request)?;
        self.time(clock())?;
        pin.verify(&self.request, &self.digits)?;
        let now = clock();
        self.time(now)?;
        let expires = now
            .checked_add(REVIEW_MS)
            .ok_or(crate::pin::Error::Expired)?;
        Ok(VerifiedPinRequest {
            request: self.request,
            expires,
        })
    }
    pub fn cancel(&mut self) {
        self.digits.fill(0);
        self.len = 0;
        self.cancelled = true;
    }
}

pub trait SecurePinVerifier {
    fn check_security_state(&mut self) -> Result<(), Error>;
    /// Collect a NEW local entry with persistent retry credit for this request.
    fn verify_device_pin(&mut self, request: &Request) -> Result<VerifiedPinRequest, Error>;
    /// Show the complete exact identity after PIN, then require a new release.
    fn confirm_on_device(&mut self, pin: &VerifiedPinRequest) -> Result<(), Error>;
}

struct Grant {
    request: Request,
    expires: u64,
    generation: u64,
}
pub struct Gate<V> {
    verifier: V,
    pending: Option<Grant>,
    last: u64,
    generation: u64,
    faulted: bool,
}
impl<V: SecurePinVerifier> Gate<V> {
    pub const fn new(verifier: V) -> Self {
        Self {
            verifier,
            pending: None,
            last: 0,
            generation: 0,
            faulted: false,
        }
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    fn time(&mut self, now: u64) -> Result<(), Error> {
        if self.faulted || self.generation == u64::MAX {
            return Err(Error::HardwareFault);
        }
        if now < self.last {
            self.lock();
            self.faulted = true;
            return Err(Error::TimeWentBackwards);
        }
        self.last = now;
        Ok(())
    }
    fn checkpoint(&mut self, clock: &mut impl FnMut() -> u64, expires: u64) -> Result<(), Error> {
        self.time(clock())?;
        if self.last >= expires {
            return Err(Error::Expired);
        }
        if self.verifier.check_security_state().is_err() {
            self.lock();
            self.faulted = true;
            return Err(Error::HardwareFault);
        }
        self.time(clock())?;
        if self.last >= expires {
            return Err(Error::Expired);
        }
        Ok(())
    }
    pub fn approve(
        &mut self,
        request: Request,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(), Error> {
        self.pending = None;
        // Fingerprint-management commands are invalid in the R05 profile.
        if matches!(
            request.operation,
            Operation::EnrollFingerprint | Operation::DeleteFingerprint
        ) {
            return Err(Error::Rejected);
        }
        self.time(clock())?;
        let limit = self
            .last
            .checked_add(ENTRY_MS + REVIEW_MS)
            .ok_or(Error::CounterExhausted)?;
        self.checkpoint(&mut clock, limit)?;
        let pin = self.verifier.verify_device_pin(&request)?;
        if pin.request != request {
            return Err(Error::WrongRequest);
        }
        let expires = limit.min(pin.expires);
        self.checkpoint(&mut clock, expires)?;
        self.verifier.confirm_on_device(&pin)?;
        self.checkpoint(&mut clock, expires)?;
        self.pending = Some(Grant {
            request,
            expires,
            generation: self.generation,
        });
        Ok(())
    }
    pub fn consume_authorized(
        &mut self,
        request: &Request,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Authorization, Error> {
        let grant = self.pending.take();
        self.time(clock())?;
        let grant = grant.ok_or(Error::Locked)?;
        self.checkpoint(&mut clock, grant.expires)?;
        if grant.generation != self.generation || grant.request != *request {
            return Err(Error::WrongRequest);
        }
        Ok(Authorization {
            request: grant.request,
            issued: self.last,
            expires: grant.expires,
            generation: grant.generation,
            execution_deadline: None,
        })
    }
    pub fn lock(&mut self) {
        self.pending = None;
        match self.generation.checked_add(1) {
            Some(next) if next < u64::MAX => self.generation = next,
            _ => {
                self.generation = u64::MAX;
                self.faulted = true;
            }
        }
    }
}

pub struct Unprovisioned;
impl SecurePinVerifier for Unprovisioned {
    fn check_security_state(&mut self) -> Result<(), Error> {
        Err(Error::Rejected)
    }
    fn verify_device_pin(&mut self, _: &Request) -> Result<VerifiedPinRequest, Error> {
        Err(Error::Rejected)
    }
    fn confirm_on_device(&mut self, _: &VerifiedPinRequest) -> Result<(), Error> {
        Err(Error::Rejected)
    }
}

#[cfg(test)]
#[path = "pin_only_tests.rs"]
mod tests;
