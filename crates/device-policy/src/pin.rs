//! Legacy R04 four-symbol PIN entry, supplementary to a fingerprint.
//! R05 uses pin_only; ProtectedPin is the shared isolated persistence interface.
//! Secure GPIO debouncing and persistent SE verification are platform adapters.
use crate::{Request, biometric::VerifiedRequest};
use zeroize::Zeroizing;

pub const PIN_SYMBOLS: usize = 12;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Button {
    One,
    Two,
    Three,
    Four,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Short,
    Long,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Locked,
    Expired,
    Cancelled,
    Storage,
    Incorrect,
    Incomplete,
}

pub trait ProtectedPin<const N: usize = PIN_SYMBOLS> {
    /// Persistently debit BEFORE verifying a completed guess. R04 calls this
    /// at input start; R05 calls after six digits, immediately before verify.
    /// An uncertain write denies access. Reset/power loss cannot restore credit.
    fn reserve_attempt(&mut self, request: &Request) -> Result<(), Error>;
    /// R04 uses twelve symbols 1..4; R05 uses six decimal digits 0..9.
    /// Compare inside the protected boundary with constant-time verification.
    /// Reset retry credit only after durable confirmed success. Never log PINs.
    fn verify(&mut self, request: &Request, symbols: &[u8; N]) -> Result<(), Error>;
}

pub struct PinEntry {
    symbols: Zeroizing<[u8; PIN_SYMBOLS]>,
    len: usize,
    request: Request,
    expires: u64,
    last_time: u64,
    cancelled: bool,
}
impl PinEntry {
    pub fn begin(
        fingerprint: &VerifiedRequest,
        pin: &mut impl ProtectedPin,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Self, Error> {
        let start = clock();
        if start >= fingerprint.expires {
            return Err(Error::Expired);
        }
        pin.reserve_attempt(fingerprint.request())?;
        let now = clock();
        if now < start || now >= fingerprint.expires {
            return Err(Error::Expired);
        }
        Ok(Self {
            symbols: Zeroizing::new([0; PIN_SYMBOLS]),
            len: 0,
            request: *fingerprint.request(),
            expires: fingerprint.expires,
            last_time: now,
            cancelled: false,
        })
    }
    fn time(&mut self, now: u64) -> Result<(), Error> {
        if self.cancelled {
            return Err(Error::Cancelled);
        }
        if now < self.last_time || now >= self.expires {
            self.cancel();
            return Err(Error::Expired);
        }
        self.last_time = now;
        Ok(())
    }
    /// Called only for a debounced RELEASE from secure device GPIO. A long
    /// press must not also generate a short press. Short 1..4 enter symbols;
    /// hold 4 erases one; hold 2 cancels. The LCD shows only the filled count.
    pub fn press(&mut self, button: Button, press: Press, now: u64) -> Result<usize, Error> {
        self.time(now)?;
        match (button, press) {
            (Button::Two, Press::Long) => {
                self.cancel();
                return Err(Error::Cancelled);
            }
            (Button::Four, Press::Long) => {
                self.len = self.len.saturating_sub(1);
                self.symbols[self.len] = 0;
            }
            (_, Press::Long) => {}
            (button, Press::Short) => {
                if self.len == PIN_SYMBOLS {
                    return Err(Error::Incomplete);
                }
                self.symbols[self.len] = match button {
                    Button::One => 1,
                    Button::Two => 2,
                    Button::Three => 3,
                    Button::Four => 4,
                };
                self.len += 1;
            }
        }
        Ok(self.len)
    }
    /// Automatically submit at 12 symbols; this consumes and clears the input.
    pub fn submit(
        mut self,
        pin: &mut impl ProtectedPin,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(), Error> {
        self.time(clock())?;
        if self.len != PIN_SYMBOLS {
            return Err(Error::Incomplete);
        }
        pin.verify(&self.request, &self.symbols)?;
        self.time(clock())
    }
    pub fn cancel(&mut self) {
        self.symbols.fill(0);
        self.len = 0;
        self.cancelled = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Operation;
    struct Pin {
        attempts: u8,
        writes_fail: bool,
    }
    impl ProtectedPin for Pin {
        fn reserve_attempt(&mut self, _: &Request) -> Result<(), Error> {
            if self.writes_fail || self.attempts == 3 {
                return Err(Error::Storage);
            }
            self.attempts += 1;
            Ok(())
        }
        fn verify(&mut self, _: &Request, s: &[u8; PIN_SYMBOLS]) -> Result<(), Error> {
            if *s == [1; PIN_SYMBOLS] {
                Ok(())
            } else {
                Err(Error::Incorrect)
            }
        }
    }
    fn proof() -> VerifiedRequest {
        VerifiedRequest {
            request: Request {
                operation: Operation::Import,
                digest: [1; 32],
                channel: [2; 32],
            },
            expires: 15_000,
        }
    }
    #[test]
    fn pin_cannot_start_without_durable_attempt_credit() {
        let mut pin = Pin {
            attempts: 0,
            writes_fail: true,
        };
        assert!(matches!(
            PinEntry::begin(&proof(), &mut pin, || 0),
            Err(Error::Storage)
        ));
    }
    #[test]
    fn four_symbols_erase_and_fixed_length_are_enforced() {
        let mut pin = Pin {
            attempts: 0,
            writes_fail: false,
        };
        let mut e = PinEntry::begin(&proof(), &mut pin, || 0).unwrap();
        e.press(Button::Four, Press::Short, 1).unwrap();
        assert_eq!(e.press(Button::Four, Press::Long, 2), Ok(0));
        for _ in 0..PIN_SYMBOLS {
            e.press(Button::One, Press::Short, 3).unwrap();
        }
        assert_eq!(e.submit(&mut pin, || 4), Ok(()));
        assert_eq!(pin.attempts, 1);
    }
    #[test]
    fn cancellation_expiry_and_power_cycle_do_not_refund_attempts() {
        let mut pin = Pin {
            attempts: 0,
            writes_fail: false,
        };
        for _ in 0..3 {
            let mut e = PinEntry::begin(&proof(), &mut pin, || 0).unwrap();
            assert_eq!(e.press(Button::Two, Press::Long, 1), Err(Error::Cancelled));
            assert_eq!(e.submit(&mut pin, || 2), Err(Error::Cancelled));
        }
        assert!(matches!(
            PinEntry::begin(&proof(), &mut pin, || 0),
            Err(Error::Storage)
        ));
        pin.attempts = 0;
        let mut e = PinEntry::begin(&proof(), &mut pin, || 10).unwrap();
        assert_eq!(e.press(Button::One, Press::Short, 9), Err(Error::Expired));
        assert!(e.symbols.iter().all(|s| *s == 0));
    }
}
