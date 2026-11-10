//! Non-exporting OTP-key HMAC and persistent HOTP precharge over full SCP03.
//! APDU facts: NXP plug-and-trust 7102413e0457d3dfa4fc4be686ca8ec50bd9f5e2,
//! MACOneShot_G / P1_MAC=0x0d / P2_GENERATE_ONESHOT=0x45.
//! Actual E2 support, policies, provisioning and power-loss semantics require
//! silicon qualification. There is no runtime creation/default/seed export.
use crate::{Error, Exchange};
use device_policy::{Operation, Request, execution::Execution};
use openkeys_scp03::Command;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub const SEED_BASE: u32 = 0x4f4b_0600;
pub const HOTP_BASE: u32 = 0x4f4b_0700;
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
pub struct OtpKeys<X: Exchange> {
    io: X,
    closed: bool,
    pending: Option<(Request, u8, Algorithm, u64)>,
}
impl<X: Exchange> OtpKeys<X> {
    pub const fn new(io: X) -> Self {
        Self {
            io,
            closed: false,
            pending: None,
        }
    }
    pub fn close(&mut self) {
        self.closed = true;
        self.pending = None;
        self.io.close();
    }
    fn check(scope: &Execution, slot: u8, alg: Algorithm, hotp: bool) -> Result<(), Error> {
        if slot >= 32 {
            return Err(Error::Authorization);
        }
        let operation = if hotp {
            Operation::ShowHotp
        } else {
            Operation::ShowTotp
        };
        let mut h = Sha256::new();
        h.update(b"OpenKeys/OTP/display/v1");
        h.update([if hotp { 2 } else { 1 }, slot, alg as u8]);
        let expected = Request {
            operation,
            digest: h.finalize().into(),
            channel: scope.request().channel,
        };
        if scope.request() != &expected || scope.checked_at_ms() >= scope.expires_ms() {
            return Err(Error::Authorization);
        }
        Ok(())
    }
    fn id(id: u32) -> [u8; 6] {
        let mut d = [0x41, 4, 0, 0, 0, 0];
        d[2..].copy_from_slice(&id.to_be_bytes());
        d
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
        if self.closed {
            return Err(Error::Closed);
        }
        match self.io.exchange(
            &Command {
                class: 0x80,
                instruction: ins,
                p1,
                p2,
                data,
                le: Some(0),
            },
            out,
        ) {
            Ok(n) if n <= out.len() => Ok(n),
            result => {
                out.fill(0);
                self.close();
                Err(result.err().unwrap_or(Error::Schema))
            }
        }
    }
    fn schema(&mut self, id: u32, kind: u8) -> Result<usize, Error> {
        let mut out = [0; 16];
        let id = Self::id(id);
        let n = self.call(2, 0, 0x26, &id, &mut out)?;
        if out[..n] != [0x41, 1, kind, 0x42, 1, 1] {
            return Err(Error::Schema);
        }
        let n = self.call(2, 0, 7, &id, &mut out)?;
        if n != 4 || out[..2] != [0x41, 2] {
            return Err(Error::Schema);
        }
        Ok(usize::from(u16::from_be_bytes([out[2], out[3]])))
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
    /// The protected root checks the live execution generation/time before and
    /// after this call. It returns a consumed OLD counter, never a guess at an
    /// uncertain write. No response can be shown if later HMAC/display fails.
    pub fn reserve_hotp(
        &mut self,
        scope: &Execution,
        slot: u8,
        alg: Algorithm,
    ) -> Result<u64, Error> {
        let result = (|| {
            Self::check(scope, slot, alg, true)?;
            let id = HOTP_BASE + u32::from(slot);
            if self.schema(id, 0x0d)? != 8 {
                return Err(Error::Schema);
            }
            let old = self.counter(id)?;
            let expected = old.checked_add(1).ok_or(Error::Exhausted)?;
            let mut out = [0; 4];
            let n = self.call(1, 8, 0, &Self::id(id), &mut out)?;
            if n != 0 || self.counter(id)? != expected {
                return Err(Error::Counter);
            }
            self.pending = Some((*scope.request(), slot, alg, old));
            Ok(old)
        })();
        if result.is_err() {
            self.close();
        }
        result
    }
    /// HMAC digest is confined to protected firmware, then immediately
    /// truncated to a six-digit LCD frame. Never register this as a host APDU.
    pub fn hmac(
        &mut self,
        scope: &Execution,
        slot: u8,
        alg: Algorithm,
        counter: u64,
    ) -> Result<Zeroizing<[u8; 64]>, Error> {
        let result = (|| {
            let hotp = scope.request().operation == Operation::ShowHotp;
            Self::check(scope, slot, alg, hotp)?;
            if hotp && self.pending.take() != Some((*scope.request(), slot, alg, counter)) {
                return Err(Error::Authorization);
            }
            let id = SEED_BASE + u32::from(slot);
            if !(16..=64).contains(&self.schema(id, 0x11)?) {
                return Err(Error::Schema);
            }
            let mut data = [0; 19];
            data[..6].copy_from_slice(&Self::id(id));
            data[6..11].copy_from_slice(&[0x42, 1, alg as u8, 0x43, 8]);
            data[11..].copy_from_slice(&counter.to_be_bytes());
            let mut out = Zeroizing::new([0; 70]);
            let n = self.call(3, 0x0d, 0x45, &data, &mut out[..])?;
            let length = alg.bytes();
            if n != length + 2 || out[..2] != [0x41, length as u8] {
                return Err(Error::Schema);
            }
            let mut mac = Zeroizing::new([0; 64]);
            mac[..length].copy_from_slice(&out[2..n]);
            Ok(mac)
        })();
        if result.is_err() {
            self.close();
        }
        result
    }
}
impl<X: Exchange> Drop for OtpKeys<X> {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
#[path = "otp_tests.rs"]
mod tests;
