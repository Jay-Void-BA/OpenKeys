//! Non-exporting OTP-key HMAC and persistent HOTP precharge over full SCP03.
//! APDU facts: NXP plug-and-trust 7102413e0457d3dfa4fc4be686ca8ec50bd9f5e2,
//! MACOneShot_G / P1_MAC=0x0d / P2_GENERATE_ONESHOT=0x45.
//! Actual E2 support, policies, provisioning and power-loss semantics require
//! silicon qualification. There is no runtime creation/default/seed export.
use crate::{Error, PinBoundExchange};
use device_policy::{Operation, Request, execution::Execution};
use openkeys_otp_format::Import;
use openkeys_scp03::Command;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub const SEED_BASE: u32 = 0x4f4b_0600;
pub const HOTP_BASE: u32 = 0x4f4b_0700;
pub fn import_request(
    slot: u8,
    generation: u64,
    input: &Import<'_>,
    channel: [u8; 32],
) -> Result<Request, Error> {
    if slot >= 32 || generation == 0 {
        return Err(Error::Schema);
    }
    let mut h = Sha256::new();
    h.update(b"OpenKeys/OTP/import/v1");
    h.update([slot]);
    h.update(generation.to_be_bytes());
    h.update(input.digest());
    Ok(Request {
        operation: Operation::ImportOtp,
        digest: h.finalize().into(),
        channel,
    })
}
pub fn delete_request(slot: u8, generation: u64, channel: [u8; 32]) -> Result<Request, Error> {
    if slot >= 32 || generation == 0 {
        return Err(Error::Schema);
    }
    let mut h = Sha256::new();
    h.update(b"OpenKeys/OTP/delete/v1");
    h.update([slot]);
    h.update(generation.to_be_bytes());
    Ok(Request {
        operation: Operation::DeleteOtp,
        digest: h.finalize().into(),
        channel,
    })
}
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
pub struct OtpKeys<X: PinBoundExchange> {
    io: X,
    closed: bool,
    pending: Option<(Request, u8, Algorithm, u64)>,
}
impl<X: PinBoundExchange> OtpKeys<X> {
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
    fn exists(&mut self, id: u32) -> Result<bool, Error> {
        let mut out = [0; 8];
        let n = self.call(4, 0, 0x27, &Self::id(id), &mut out)?;
        match &out[..n] {
            [0x41, 1, 1] => Ok(true),
            [0x41, 1, 2] => Ok(false),
            _ => Err(Error::Schema),
        }
    }
    /// The root first durably reserves slot+generation and a pending catalog
    /// intent. It publishes metadata ONLY after this succeeds and the final
    /// live scope check. An uncertain import is never retried/overwritten: a
    /// reboot hides the pending entry and requires authorized cleanup.
    /// No installed seed can be read back; only type/size/counter are checked.
    pub fn install(
        &mut self,
        scope: &Execution,
        slot: u8,
        generation: u64,
        input: &Import<'_>,
    ) -> Result<(), Error> {
        let result = (|| {
            let expected = import_request(slot, generation, input, scope.request().channel)?;
            if scope.request() != &expected || scope.checked_at_ms() >= scope.expires_ms() {
                return Err(Error::Authorization);
            }
            let seed_id = SEED_BASE + u32::from(slot);
            let counter_id = HOTP_BASE + u32::from(slot);
            if self.exists(seed_id)? || self.exists(counter_id)? {
                return Err(Error::Record);
            }
            // Counter is created first for both modes. A cut can leave an
            // orphan, never an active catalog record with no persistent count.
            let mut data = Zeroizing::new([0; 36]);
            data[..20].copy_from_slice(&crate::native_pin::owner_policy(0x0034_0000)); // read/write/delete
            data[20..26].copy_from_slice(&Self::id(counter_id));
            // TAG_3 supplies the initial eight-byte value/width; TAG_2 is
            // conditional on TAG_3 being absent (AN12413 table99).
            data[26..28].copy_from_slice(&[0x43, 8]);
            data[28..36].copy_from_slice(&input.initial_counter().to_be_bytes());
            let mut out = [0; 8];
            if self.call(1, 8, 0, &*data, &mut out)? != 0 {
                return Err(Error::Schema);
            }
            if self.schema(counter_id, 0x0d)? != 8
                || self.counter(counter_id)? != input.initial_counter()
            {
                return Err(Error::Counter);
            }
            let seed = input.seed_for_provisioning();
            let mut data = Zeroizing::new([0; 92]);
            data[..20].copy_from_slice(&crate::native_pin::owner_policy(0x1004_0000)); // HMAC sign + delete; no read/export/update
            data[20..26].copy_from_slice(&Self::id(seed_id));
            data[26..28].copy_from_slice(&[0x43, seed.len() as u8]);
            data[28..28 + seed.len()].copy_from_slice(seed);
            if self.call(1, 5, 0, &data[..28 + seed.len()], &mut out)? != 0
                || self.schema(seed_id, 0x11)? != seed.len()
            {
                return Err(Error::Schema);
            }
            Ok(())
        })();
        if result.is_err() {
            self.close();
        }
        result
    }
    /// First persist a tombstone that hides the account and invalidates every
    /// pending operation for its generation. Delete seed BEFORE its counter;
    /// any cut stays tombstoned. Success does not itself commit a vault catalog.
    pub fn delete(&mut self, scope: &Execution, slot: u8, generation: u64) -> Result<(), Error> {
        let result = (|| {
            let expected = delete_request(slot, generation, scope.request().channel)?;
            if scope.request() != &expected || scope.checked_at_ms() >= scope.expires_ms() {
                return Err(Error::Authorization);
            }
            for id in [SEED_BASE + u32::from(slot), HOTP_BASE + u32::from(slot)] {
                // Explicit authenticated absence is safe on resuming a
                // tombstoned deletion; an error is never interpreted as absent.
                if self.exists(id)? {
                    let mut out = [0; 8];
                    if self.call(4, 0, 0x28, &Self::id(id), &mut out)? != 0 || self.exists(id)? {
                        return Err(Error::Record);
                    }
                }
            }
            Ok(())
        })();
        if result.is_err() {
            self.close();
        }
        result
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
impl<X: PinBoundExchange> Drop for OtpKeys<X> {
    fn drop(&mut self) {
        self.pending = None;
        self.io.release();
    }
}

#[cfg(test)]
#[path = "otp_tests.rs"]
mod tests;
