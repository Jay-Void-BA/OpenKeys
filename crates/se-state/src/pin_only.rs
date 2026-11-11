//! R05 commissioned PIN namespace, independent of R04 biometric counters.
//! Actual SE050 APDUs over the sealed authenticated Exchange. No runtime
//! creation, default PIN, counter reset, host PIN endpoint or key export.
//! The verified root must exclusively own this service and the SCP03 guard.
use crate::{Error, Exchange};
use device_policy::{
    Request,
    pin::{Error as PinError, ProtectedPin},
    pin_only::PIN_DIGITS,
};
use openkeys_scp03::Command;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

// "More than ten" means the eleventh incorrect/ambiguous completed guess.
pub const MAX_FAILURES: u64 = 11;
pub const SPENT_ID: u32 = 0x4f4b_0501;
pub const CREDIT_ID: u32 = 0x4f4b_0502;
pub const SALT_ID: u32 = 0x4f4b_0503;
pub const VERIFIER_ID: u32 = 0x4f4b_0504;
pub const PEPPER_ID: u32 = 0x4f4b_0505;
pub const MARKER_ID: u32 = 0x4f4b_0506;
pub const ERASE_ID: u32 = 0x4f4b_0507;
pub const KDF_INFO: &[u8] = b"OpenKeys/R05/PIN-check/v2";

pub fn marker() -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"OpenKeys/R05/PIN-state/v2");
    h.update([PIN_DIGITS as u8]);
    h.update(MAX_FAILURES.to_be_bytes());
    h.update(KDF_INFO);
    h.update(b"HKDF-SHA256/salt32/info-domain-decimal6");
    for id in [
        SPENT_ID,
        CREDIT_ID,
        SALT_ID,
        VERIFIER_ID,
        PEPPER_ID,
        MARKER_ID,
        ERASE_ID,
    ] {
        h.update(id.to_be_bytes());
    }
    h.finalize().into()
}
struct Pending {
    request: Request,
    spent: u64,
    credit: u64,
}
pub struct PinState<X: Exchange> {
    io: X,
    pending: Option<Pending>,
    last_spent: u64,
    last_credit: u64,
    salt: Zeroizing<[u8; 32]>,
    expected: Zeroizing<[u8; 32]>,
    faulted: bool,
}
impl<X: Exchange> PinState<X> {
    /// Opens ONLY an existing, reviewed, per-device commissioned namespace.
    /// Attribute checks are not proof of the object's ACL; commissioning must
    /// separately enforce SCP mandate, no export and no unauthorized rewrite.
    pub fn open(io: X) -> Result<Self, Error> {
        let mut s = Self {
            io,
            pending: None,
            last_spent: 0,
            last_credit: 0,
            salt: Zeroizing::new([0; 32]),
            expected: Zeroizing::new([0; 32]),
            faulted: false,
        };
        let result = (|| {
            for id in [SPENT_ID, CREDIT_ID, ERASE_ID] {
                s.schema(id, 0x0d, 8)?;
            }
            if s.counter(ERASE_ID)? != 0 {
                return Err(Error::EraseRequired);
            }
            for id in [SALT_ID, VERIFIER_ID, MARKER_ID] {
                s.schema(id, 0x0b, 32)?;
            }
            s.schema(PEPPER_ID, 0x11, 32)?; // HMAC key, persistent and non-exporting
            let found = s.file(MARKER_ID)?;
            if !bool::from(found.ct_eq(&marker())) {
                return Err(Error::Schema);
            }
            s.salt = s.file(SALT_ID)?;
            s.expected = s.file(VERIFIER_ID)?;
            if s.salt.iter().all(|&b| b == 0) || s.expected.iter().all(|&b| b == 0) {
                return Err(Error::Schema);
            }
            let (spent, credit) = s.counters()?;
            if spent - credit == MAX_FAILURES {
                s.latch_erase()?;
                return Err(Error::EraseRequired);
            }
            Ok(())
        })();
        if result.is_err() {
            s.close();
        }
        result.map(|_| s)
    }
    pub fn close(&mut self) {
        self.faulted = true;
        self.pending = None;
        self.salt.fill(0);
        self.expected.fill(0);
        self.io.close();
    }
    fn id(id: u32) -> [u8; 6] {
        let mut out = [0x41, 4, 0, 0, 0, 0];
        out[2..].copy_from_slice(&id.to_be_bytes());
        out
    }
    fn call(
        &mut self,
        ins: u8,
        p1: u8,
        p2: u8,
        data: &[u8],
        out: &mut [u8],
    ) -> Result<usize, Error> {
        out.fill(0);
        if self.faulted {
            return Err(Error::Closed);
        }
        let result = self.io.exchange(
            &Command {
                class: 0x80,
                instruction: ins,
                p1,
                p2,
                data,
                le: Some(0),
            },
            out,
        );
        match result {
            Ok(n) if n <= out.len() => Ok(n),
            result => {
                self.close();
                out.fill(0);
                Err(result.err().unwrap_or(Error::Schema))
            }
        }
    }
    fn schema(&mut self, id: u32, kind: u8, size: u8) -> Result<(), Error> {
        let mut out = [0; 16];
        let data = Self::id(id);
        let n = self.call(2, 0, 0x26, &data, &mut out)?;
        if out[..n] != [0x41, 1, kind, 0x42, 1, 1] {
            return Err(Error::Schema);
        }
        let n = self.call(2, 0, 7, &data, &mut out)?;
        if out[..n] != [0x41, 2, 0, size] {
            return Err(Error::Schema);
        }
        Ok(())
    }
    fn file(&mut self, id: u32) -> Result<Zeroizing<[u8; 32]>, Error> {
        let mut data = [0; 14];
        data[..6].copy_from_slice(&Self::id(id));
        data[6..].copy_from_slice(&[0x42, 2, 0, 0, 0x43, 2, 0, 32]);
        let mut out = Zeroizing::new([0; 40]);
        let n = self.call(2, 0, 0, &data, &mut out[..])?;
        if n != 34 || out[..2] != [0x41, 32] {
            return Err(Error::Schema);
        }
        let mut value = Zeroizing::new([0; 32]);
        value.copy_from_slice(&out[2..34]);
        Ok(value)
    }
    fn counter(&mut self, id: u32) -> Result<u64, Error> {
        let mut out = [0; 16];
        let n = self.call(2, 0, 0, &Self::id(id), &mut out)?;
        if n != 10 || out[..2] != [0x41, 8] {
            return Err(Error::Schema);
        }
        Ok(u64::from_be_bytes(
            out[2..10].try_into().map_err(|_| Error::Schema)?,
        ))
    }
    fn counters(&mut self) -> Result<(u64, u64), Error> {
        let spent = self.counter(SPENT_ID)?;
        let credit = self.counter(CREDIT_ID)?;
        if spent < self.last_spent
            || credit < self.last_credit
            || credit > spent
            || spent - credit > MAX_FAILURES
        {
            return Err(Error::Counter);
        }
        self.last_spent = spent;
        self.last_credit = credit;
        Ok((spent, credit))
    }
    fn reserve(&mut self, r: &Request) -> Result<(), Error> {
        self.pending = None;
        let (spent, credit) = self.counters()?;
        if spent - credit >= MAX_FAILURES {
            self.latch_erase()?;
            return Err(Error::EraseRequired);
        }
        if spent == u64::MAX {
            return Err(Error::Exhausted);
        }
        let mut out = [0; 4];
        let n = self.call(1, 8, 0, &Self::id(SPENT_ID), &mut out)?;
        if n != 0 {
            return Err(Error::Schema);
        }
        let (after, actual_credit) = self.counters()?;
        if after != spent + 1 || actual_credit != credit {
            return Err(Error::Counter);
        }
        self.pending = Some(Pending {
            request: *r,
            spent: after,
            credit,
        });
        Ok(())
    }
    fn candidate(&mut self, digits: &[u8; PIN_DIGITS]) -> Result<Zeroizing<[u8; 32]>, Error> {
        if digits.iter().any(|&d| d > 9) {
            return Err(Error::Schema);
        }
        // RFC5869 HKDF-SHA256 inside the SE: secret IKM=per-device pepper,
        // salt=commissioned random32, info=profile-domain || six digits.
        // PIN bytes only enter Zeroizing APDU storage, not an MCU hash context.
        let mut data = Zeroizing::new([0; 96]);
        data[..6].copy_from_slice(&Self::id(PEPPER_ID));
        data[6..11].copy_from_slice(&[0x42, 1, 4, 0x43, 32]);
        data[11..43].copy_from_slice(&self.salt[..]);
        data[43..45].copy_from_slice(&[0x44, (KDF_INFO.len() + PIN_DIGITS) as u8]);
        let middle = 45 + KDF_INFO.len();
        data[45..middle].copy_from_slice(KDF_INFO);
        data[middle..middle + PIN_DIGITS].copy_from_slice(digits);
        let end = middle + PIN_DIGITS;
        data[end..end + 4].copy_from_slice(&[0x45, 2, 0, 32]);
        let mut out = Zeroizing::new([0; 40]);
        let n = self.call(3, 0, 0x2d, &data[..end + 4], &mut out[..])?;
        if n != 34 || out[..2] != [0x41, 32] {
            return Err(Error::Schema);
        }
        let mut value = Zeroizing::new([0; 32]);
        value.copy_from_slice(&out[2..34]);
        Ok(value)
    }
    fn verify_inner(&mut self, r: &Request, digits: &[u8; PIN_DIGITS]) -> Result<bool, Error> {
        let pending = self.pending.take().ok_or(Error::Counter)?;
        if pending.request != *r || self.counters()? != (pending.spent, pending.credit) {
            return Err(Error::Counter);
        }
        let candidate = self.candidate(digits)?;
        if self.counters()? != (pending.spent, pending.credit) {
            return Err(Error::Counter);
        }
        if !bool::from(candidate[..].ct_eq(&self.expected[..])) {
            if pending.spent - pending.credit == MAX_FAILURES {
                self.latch_erase()?;
                return Err(Error::EraseRequired);
            }
            return Ok(false);
        }
        // Only a verified correct PIN can credit the exact precharged attempt.
        // Interrupted/uncertain writes close the service; no blind retry.
        let mut data = [0; 16];
        data[..6].copy_from_slice(&Self::id(CREDIT_ID));
        data[6..8].copy_from_slice(&[0x43, 8]);
        data[8..].copy_from_slice(&pending.spent.to_be_bytes());
        let mut out = [0; 4];
        let n = self.call(1, 8, 0, &data, &mut out)?;
        if n != 0 || self.counters()? != (pending.spent, pending.spent) {
            return Err(Error::Counter);
        }
        Ok(true)
    }
    fn latch_erase(&mut self) -> Result<(), Error> {
        let before = self.counter(ERASE_ID)?;
        if before != 0 {
            return Err(Error::EraseRequired);
        }
        let mut out = [0; 4];
        let n = self.call(1, 8, 0, &Self::id(ERASE_ID), &mut out)?;
        if n != 0 || self.counter(ERASE_ID)? != 1 {
            return Err(Error::Counter);
        }
        Ok(())
    }
    /// First persist an irreversible erase request. This does NOT claim data
    /// deletion: the production root must destroy all FIDO/OTP/vault/session
    /// keys, verify cleanup and commission a fresh profile before any access.
    /// A pending latch is refused on EVERY subsequent open, including reboot.
    pub fn latch_pin_change_erase(
        &mut self,
        auth: device_policy::Authorization,
        now: u64,
    ) -> Result<(), Error> {
        if auth.request().operation != device_policy::Operation::ChangePin
            || now < auth.issued_ms()
            || now >= auth.expires_ms()
        {
            return Err(Error::Authorization);
        }
        let result = self.latch_erase();
        self.close();
        result
    }
}
impl<X: Exchange> ProtectedPin<PIN_DIGITS> for PinState<X> {
    fn reserve_attempt(&mut self, r: &Request) -> Result<(), PinError> {
        if self.faulted {
            return Err(PinError::Storage);
        }
        if self.reserve(r).is_err() {
            self.close();
            return Err(PinError::Storage);
        }
        Ok(())
    }
    fn verify(&mut self, r: &Request, digits: &[u8; PIN_DIGITS]) -> Result<(), PinError> {
        if self.faulted {
            return Err(PinError::Storage);
        }
        match self.verify_inner(r, digits) {
            Ok(true) => Ok(()),
            Ok(false) => Err(PinError::Incorrect),
            Err(_) => {
                self.close();
                Err(PinError::Storage)
            }
        }
    }
}
impl<X: Exchange> Drop for PinState<X> {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
#[path = "pin_only_tests.rs"]
mod tests;
