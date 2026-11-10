//! English-only 16x2 trusted review model. It never renders passwords or PINs.
//! The secure platform owns the LCD bus; no host raw-display command is allowed.
use crate::{Operation, Request, biometric::VerifiedRequest};

pub const COLUMNS: usize = 16;
pub type Frame = [[u8; COLUMNS]; 2];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidIdentity,
    NotReviewed,
    Expired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Pending,
    Confirmed(Request),
    Cancelled,
}

pub const fn locked() -> Frame {
    [*b"OpenKeys        ", *b"Use fingerprint "]
}
pub const fn locked_pin() -> Frame {
    [*b"OpenKeys R05    ", *b"Enter device PIN"]
}
pub fn pin_progress(filled: usize) -> Frame {
    let mut frame = [*b"Enter device PIN", *b"____________    "];
    frame[1][..filled.min(12)].fill(b'*');
    frame
}
pub fn pin_only_progress(filled: usize) -> Frame {
    let mut frame = [*b"Enter device PIN", *b"______          "];
    frame[1][..filled.min(crate::pin_only::PIN_DIGITS)].fill(b'*');
    frame
}
fn operation_label(op: Operation) -> &'static [u8; COLUMNS] {
    match op {
        Operation::ListPrivateMetadata => b"View vault      ",
        Operation::RevealNote => b"Read note       ",
        Operation::RevealRecoveryCode => b"Recovery code   ",
        Operation::Import => b"Import records  ",
        Operation::Delete => b"Delete record   ",
        Operation::ChangePin => b"Change PIN      ",
        Operation::EnrollFingerprint => b"Add fingerprint ",
        Operation::DeleteFingerprint => b"Delete finger   ",
        Operation::PairHost => b"Pair computer   ",
        Operation::FirmwareUpdate => b"Firmware update ",
        Operation::FactoryReset => b"Erase all data  ",
        Operation::FidoRegister => b"Create passkey  ",
        Operation::FidoSign => b"Sign in         ",
        Operation::DeriveHmacSecret => b"Derive secret   ",
        Operation::ShowTotp => b"Show TOTP       ",
        Operation::ShowHotp => b"Show HOTP       ",
        Operation::ImportOtp => b"Import OTP seed ",
        Operation::DeleteOtp => b"Delete OTP seed ",
        Operation::SetTime => b"Set device time ",
        Operation::OpenPgp => b"OpenPGP action  ",
        Operation::Piv => b"PIV operation   ",
        Operation::ProtocolPin => b"Protocol PIN    ",
        Operation::GenerateKey => b"Generate key    ",
        Operation::ImportKey => b"Import key      ",
        Operation::DeleteKey => b"Delete key      ",
        Operation::UnlockSession => b"Unlock session  ",
    }
}
/// Identity must be derived by secure code from the exact canonical request:
/// RP ID for FIDO, or an authenticated title/ID for a vault action. This model
/// does not establish a browser origin and cannot validate what a host claims.
pub struct Review<'a> {
    request: Request,
    identity: &'a [u8],
    page: usize,
    seen: u32,
    ended: bool,
    expires: u64,
    last_time: u64,
    awaiting_ack: bool,
    locked_frame: Frame,
    pin_change_erases: bool,
}
impl<'a> Review<'a> {
    /// ASCII display identity only. Use validated ASCII/Punycode RP IDs, or an
    /// explicit ASCII record identifier for titles this LCD cannot represent.
    /// Never silently replace/truncate Unicode or suppress the domain suffix.
    pub fn new(fingerprint: &VerifiedRequest, identity: &'a [u8], now: u64) -> Result<Self, Error> {
        if now >= fingerprint.expires {
            return Err(Error::Expired);
        }
        if identity.is_empty()
            || identity.len() > 253
            || !identity.iter().all(|c| matches!(c, 0x21..=0x7e))
        {
            return Err(Error::InvalidIdentity);
        }
        Ok(Self {
            request: *fingerprint.request(),
            identity,
            page: 0,
            seen: 0,
            ended: false,
            expires: fingerprint.expires,
            last_time: now,
            awaiting_ack: false,
            locked_frame: locked(),
            pin_change_erases: false,
        })
    }
    /// R05 entry point: a fresh verified local decimal PIN replaces biometrics.
    pub fn for_pin(
        pin: &crate::pin_only::VerifiedPinRequest,
        identity: &'a [u8],
        now: u64,
    ) -> Result<Self, Error> {
        if now >= pin.expires_ms() {
            return Err(Error::Expired);
        }
        if identity.is_empty()
            || identity.len() > 253
            || !identity.iter().all(|c| matches!(c, 0x21..=0x7e))
        {
            return Err(Error::InvalidIdentity);
        }
        Ok(Self {
            request: *pin.request(),
            identity,
            page: 0,
            seen: 0,
            ended: false,
            expires: pin.expires_ms(),
            last_time: now,
            awaiting_ack: false,
            locked_frame: locked_pin(),
            pin_change_erases: true,
        })
    }
    fn pages(&self) -> usize {
        self.identity.len().div_ceil(COLUMNS)
    }
    /// Only mark a page reviewed AFTER the secure display adapter acknowledges
    /// this exact frame was written. The caller must not pre-render future pages.
    pub fn frame(&mut self, now: u64) -> Frame {
        self.observe_time(now);
        if self.ended {
            return self.locked_frame;
        }
        self.awaiting_ack = true;
        let label = if self.pin_change_erases && self.request.operation == Operation::ChangePin {
            *b"Erase+change PIN"
        } else {
            *operation_label(self.request.operation)
        };
        let mut frame = [label, [b' '; COLUMNS]];
        let start = self.page * COLUMNS;
        let end = (start + COLUMNS).min(self.identity.len());
        frame[1][..end - start].copy_from_slice(&self.identity[start..end]);
        frame
    }
    pub fn displayed(&mut self) {
        if !self.ended && self.awaiting_ack {
            self.awaiting_ack = false;
            self.seen |= 1 << self.page;
        }
    }
    pub fn next_page(&mut self) {
        if !self.ended {
            self.awaiting_ack = false;
            self.page = (self.page + 1) % self.pages();
        }
    }
    pub fn previous_page(&mut self) {
        if !self.ended {
            self.awaiting_ack = false;
            self.page = (self.page + self.pages() - 1) % self.pages();
        }
    }
    /// R05 contextual mapping, using new debounced device RELEASE events.
    /// The caller discards held keys on entry to this scene. Digits entered
    /// for the PIN must never carry over into a confirmation.
    pub fn press_digit(
        &mut self,
        digit: crate::pin_only::Digit,
        press: crate::pin::Press,
        now: u64,
    ) -> Result<Decision, Error> {
        use crate::{pin::Press, pin_only::Digit};
        self.observe_time(now);
        if self.ended {
            return Ok(Decision::Cancelled);
        }
        match (digit, press) {
            (Digit::Zero, _) => Ok(self.cancel()),
            (_, Press::Long) => Ok(Decision::Pending),
            (Digit::Two, Press::Short) => {
                self.previous_page();
                Ok(Decision::Pending)
            }
            (Digit::Eight, Press::Short) => {
                self.next_page();
                Ok(Decision::Pending)
            }
            (Digit::Five, Press::Short) => self.confirm(now),
            _ => Ok(Decision::Pending),
        }
    }
    fn observe_time(&mut self, now: u64) {
        if now < self.last_time || now >= self.expires {
            self.ended = true;
        }
        self.last_time = now;
    }
    /// Caller supplies live secure monotonic time, never a host timestamp.
    pub fn confirm(&mut self, now: u64) -> Result<Decision, Error> {
        self.observe_time(now);
        if self.ended {
            return Ok(Decision::Cancelled);
        }
        let all = (1u32 << self.pages()) - 1;
        if self.seen != all {
            return Err(Error::NotReviewed);
        }
        self.ended = true;
        Ok(Decision::Confirmed(self.request))
    }
    pub fn cancel(&mut self) -> Decision {
        self.ended = true;
        Decision::Cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Request {
        Request {
            operation: Operation::FidoSign,
            digest: [1; 32],
            channel: [2; 32],
        }
    }
    fn proof() -> VerifiedRequest {
        VerifiedRequest {
            request: request(),
            expires: 15_000,
        }
    }
    #[test]
    fn long_identity_cannot_be_confirmed_when_only_prefix_seen() {
        let mut r = Review::new(&proof(), b"accounts.example.attacker.test", 0).unwrap();
        r.frame(0);
        r.displayed();
        assert_eq!(r.confirm(0), Err(Error::NotReviewed));
        r.next_page();
        r.frame(0);
        r.displayed();
        assert_eq!(r.confirm(0), Ok(Decision::Confirmed(request())));
        assert_eq!(r.confirm(0), Ok(Decision::Cancelled));
    }
    #[test]
    fn control_unicode_empty_and_oversized_identities_are_rejected() {
        for id in [b"".as_slice(), b"a\nb", b"a\x1bb", b"\xff", &[b'a'; 254]] {
            assert!(matches!(
                Review::new(&proof(), id, 0),
                Err(Error::InvalidIdentity)
            ));
        }
        assert!(!locked().iter().flatten().any(|c| *c == b'*'));
        assert_eq!(pin_progress(6)[1], *b"******______    ");
    }
    #[test]
    fn expired_and_rolled_back_review_clear_identity() {
        let mut r = Review::new(&proof(), b"private.example", 10).unwrap();
        assert_ne!(r.frame(11), locked());
        assert_eq!(r.frame(10), locked());
        assert_eq!(r.confirm(12), Ok(Decision::Cancelled));
        let mut r = Review::new(&proof(), b"private.example", 0).unwrap();
        assert_eq!(r.frame(15_000), locked());
        assert!(matches!(
            Review::new(&proof(), b"example.test", 15_000),
            Err(Error::Expired)
        ));
    }
    #[test]
    fn cancelled_review_is_never_approved() {
        let mut r = Review::new(&proof(), b"example.test", 0).unwrap();
        r.frame(0);
        r.displayed();
        r.cancel();
        assert_eq!(r.confirm(0), Ok(Decision::Cancelled));
        assert_eq!(r.frame(0), locked());
    }
}
