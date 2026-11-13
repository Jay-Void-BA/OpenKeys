//! First-boot local six-digit PIN setup. Never inferred from a missing file.
//! The protected platform must establish a qualified factory-blank lifecycle
//! and atomically commission a fresh namespace. No production implementation
//! of that service is supplied by this policy module.
use crate::{
    lcd::Frame,
    pin::Press,
    pin_only::{Digit, PIN_DIGITS},
};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub trait ProtectedCommissioning {
    /// Verify boot/debug/SE identity and deliberate factory-blank lifecycle.
    /// Missing/corrupt/partial owner state MUST fail; it is not a blank device.
    fn qualified_factory_blank(&mut self) -> Result<(), Error>;
    /// Install the locally entered native PIN and qualified namespace, verify
    /// the exact written state and write the validity marker LAST. Legacy
    /// HKDF pepper/salt verification is not the R05 native-PIN implementation.
    /// Any uncertain reply poisons setup; never blindly retry or set a default.
    fn install_owner_pin(&mut self, pin: &[u8; PIN_DIGITS]) -> Result<(), Error>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NotBlank,
    Expired,
    Cancelled,
    Persistence,
    Mismatch,
    Ended,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    Entering,
    Repeat,
    Installed,
}

pub struct Setup {
    first: Zeroizing<[u8; PIN_DIGITS]>,
    second: Zeroizing<[u8; PIN_DIGITS]>,
    len: usize,
    repeat: bool,
    expires: u64,
    last: u64,
    ended: bool,
}
impl Setup {
    pub fn begin(
        platform: &mut impl ProtectedCommissioning,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Self, Error> {
        let start = clock();
        let expires = start.checked_add(120_000).ok_or(Error::Expired)?;
        platform
            .qualified_factory_blank()
            .map_err(|_| Error::NotBlank)?;
        let now = clock();
        if now < start || now >= expires {
            return Err(Error::Expired);
        }
        Ok(Self {
            first: Zeroizing::new([0; PIN_DIGITS]),
            second: Zeroizing::new([0; PIN_DIGITS]),
            len: 0,
            repeat: false,
            expires,
            last: now,
            ended: false,
        })
    }
    fn clear(&mut self) {
        self.first.fill(0);
        self.second.fill(0);
        self.len = 0;
        self.ended = true;
    }
    pub fn tick(&mut self, now: u64) -> Result<(), Error> {
        if self.ended {
            return Err(Error::Ended);
        }
        if now < self.last || now >= self.expires {
            self.clear();
            return Err(Error::Expired);
        }
        self.last = now;
        Ok(())
    }
    pub fn frame(&mut self, now: u64) -> Result<Frame, Error> {
        self.tick(now)?;
        let mut f = [
            if self.repeat {
                *b"Repeat PIN      "
            } else {
                *b"Set device PIN  "
            },
            *b"______          ",
        ];
        f[1][..self.len].fill(b'*');
        Ok(f)
    }
    /// Called only for a new debounced local release. Discard pending keys on
    /// the Repeat transition; never let the host submit either PIN value.
    pub fn press(
        &mut self,
        digit: Digit,
        kind: Press,
        platform: &mut impl ProtectedCommissioning,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Progress, Error> {
        self.tick(clock())?;
        match (digit, kind) {
            (Digit::Zero, Press::Long) => {
                self.clear();
                return Err(Error::Cancelled);
            }
            (Digit::Nine, Press::Long) => {
                self.len = self.len.saturating_sub(1);
                if self.repeat {
                    self.second[self.len] = 0
                } else {
                    self.first[self.len] = 0
                }
                return Ok(Progress::Entering);
            }
            (_, Press::Long) => return Ok(Progress::Entering),
            _ => {}
        }
        if self.repeat {
            self.second[self.len] = digit.value()
        } else {
            self.first[self.len] = digit.value()
        }
        self.len += 1;
        if self.len < PIN_DIGITS {
            return Ok(Progress::Entering);
        }
        if !self.repeat {
            self.repeat = true;
            self.len = 0;
            return Ok(Progress::Repeat);
        }
        if !bool::from(self.first[..].ct_eq(&self.second[..])) {
            self.clear();
            return Err(Error::Mismatch);
        }
        platform.qualified_factory_blank().map_err(|_| {
            self.clear();
            Error::NotBlank
        })?;
        self.tick(clock())?;
        if platform.install_owner_pin(&self.first).is_err() {
            self.clear();
            return Err(Error::Persistence);
        }
        let checked = self.tick(clock());
        self.clear();
        checked?;
        Ok(Progress::Installed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Platform {
        not_blank: bool,
        fail_write: bool,
        writes: usize,
    }
    impl ProtectedCommissioning for Platform {
        fn qualified_factory_blank(&mut self) -> Result<(), Error> {
            if self.not_blank {
                Err(Error::NotBlank)
            } else {
                Ok(())
            }
        }
        fn install_owner_pin(&mut self, pin: &[u8; PIN_DIGITS]) -> Result<(), Error> {
            assert_eq!(*pin, [0, 1, 2, 3, 4, 5]);
            self.writes += 1;
            // Commissioning is durably in progress before any owner write.
            // An uncertain reply must never make the device factory-blank.
            self.not_blank = true;
            if self.fail_write {
                Err(Error::Persistence)
            } else {
                Ok(())
            }
        }
    }
    fn digits() -> [Digit; 6] {
        [
            Digit::Zero,
            Digit::One,
            Digit::Two,
            Digit::Three,
            Digit::Four,
            Digit::Five,
        ]
    }
    #[test]
    fn only_matching_local_six_digits_install_once() {
        let mut p = Platform::default();
        let mut s = Setup::begin(&mut p, || 0).unwrap();
        for (i, d) in digits().into_iter().enumerate() {
            let r = s.press(d, Press::Short, &mut p, || 1).unwrap();
            assert_eq!(
                r,
                if i == 5 {
                    Progress::Repeat
                } else {
                    Progress::Entering
                }
            );
        }
        assert_eq!(p.writes, 0);
        assert_eq!(s.frame(1).unwrap()[1], *b"______          ");
        for (i, d) in digits().into_iter().enumerate() {
            let r = s.press(d, Press::Short, &mut p, || 2).unwrap();
            assert_eq!(
                r,
                if i == 5 {
                    Progress::Installed
                } else {
                    Progress::Entering
                }
            );
        }
        assert_eq!(p.writes, 1);
        assert!(s.first.iter().all(|&x| x == 0));
        assert_eq!(s.frame(3), Err(Error::Ended));
        assert!(Setup::begin(&mut p, || 4).is_err());
    }
    #[test]
    fn mismatch_cancel_timeout_or_partial_state_cannot_install() {
        let mut p = Platform::default();
        let mut s = Setup::begin(&mut p, || 0).unwrap();
        for d in digits() {
            s.press(d, Press::Short, &mut p, || 1).unwrap();
        }
        for _ in 0..5 {
            s.press(Digit::Zero, Press::Short, &mut p, || 2).unwrap();
        }
        assert_eq!(
            s.press(Digit::Zero, Press::Short, &mut p, || 2),
            Err(Error::Mismatch)
        );
        assert_eq!(p.writes, 0);
        let mut s = Setup::begin(&mut p, || 0).unwrap();
        s.press(Digit::One, Press::Short, &mut p, || 1).unwrap();
        assert_eq!(
            s.press(Digit::Zero, Press::Long, &mut p, || 2),
            Err(Error::Cancelled)
        );
        assert!(s.first.iter().all(|&x| x == 0));
        let mut s = Setup::begin(&mut p, || 10).unwrap();
        assert_eq!(s.frame(9), Err(Error::Expired));
        p.not_blank = true;
        assert!(Setup::begin(&mut p, || 0).is_err());
    }
    #[test]
    fn uncertain_install_is_terminal_and_never_retried() {
        let mut p = Platform {
            fail_write: true,
            ..Default::default()
        };
        let mut s = Setup::begin(&mut p, || 0).unwrap();
        for d in digits() {
            s.press(d, Press::Short, &mut p, || 1).unwrap();
        }
        for d in digits().into_iter().take(5) {
            s.press(d, Press::Short, &mut p, || 2).unwrap();
        }
        assert_eq!(
            s.press(Digit::Five, Press::Short, &mut p, || 3),
            Err(Error::Persistence)
        );
        assert_eq!(p.writes, 1);
        assert_eq!(
            s.press(Digit::Five, Press::Short, &mut p, || 4),
            Err(Error::Ended)
        );
        assert_eq!(p.writes, 1);
        assert!(Setup::begin(&mut p, || 5).is_err());
    }
}
