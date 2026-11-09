//! Non-exporting SE050 P-256 signing and hmac-secret root derivation.
//!
//! APDUs follow AN12413 Rev2.12 and pinned NXP nano 26d55b4. No private EC
//! scalar or hmac-secret master is imported into or read by MCU code. These
//! operations belong ONLY to the protected root; they are not a host APDU API.
//! The SCP03 guard must check the SAME authorization and its live cancellation,
//! generation, clock and hardware state before/after every exchange.
use crate::{Error, Exchange};
use device_policy::{Authorization, Operation};
use openkeys_scp03::Command;
use p256::ecdsa::{Signature, VerifyingKey, signature::hazmat::PrehashVerifier};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Separately commissioned 32-byte HMAC roots, never the vault/SCP/FP key.
pub const HMAC_NO_UV_ID: u32 = 0x4f4b_0200;
pub const HMAC_UV_ID: u32 = 0x4f4b_0201;
const SLOT_BASE: u32 = 0x4f4b_1000;
pub const KEY_SLOTS: u16 = 256;
// SIGN | public READ | DELETE | REQUIRE_SM. No update, regeneration,
// import/export, key agreement or private-key read permission.
const SIGN_POLICY: u32 = 0x1026_0000;

/// Public, root-owned catalog metadata; must be stored inside the authenticated
/// vault/credential wrapping. Never deserialize a host-selected ID as a handle.
pub struct SigningKey {
    id: u32,
    public: [u8; 65],
}
impl SigningKey {
    pub fn public_key(&self) -> &[u8; 65] {
        &self.public
    }
    pub fn slot(&self) -> u16 {
        (self.id - SLOT_BASE) as u16
    }
}

/// No software implementation of the authenticated Exchange is available in
/// production. Ambiguous I/O, invalid objects or malformed replies close it.
pub struct Keys<X: Exchange> {
    io: X,
    faulted: bool,
}
impl<X: Exchange> Keys<X> {
    pub fn new(io: X) -> Self {
        Self { io, faulted: false }
    }
    pub fn close(&mut self) {
        self.faulted = true;
        self.io.close();
    }
    fn require(auth: &Authorization, allowed: &[Operation]) -> Result<(), Error> {
        if allowed.contains(&auth.request().operation) {
            Ok(())
        } else {
            Err(Error::Authorization)
        }
    }
    fn id(slot: u16) -> Result<u32, Error> {
        if slot < KEY_SLOTS {
            Ok(SLOT_BASE + u32::from(slot))
        } else {
            Err(Error::Schema)
        }
    }
    fn id_tlv(id: u32) -> [u8; 6] {
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
    fn schema(&mut self, id: u32, kind: u8, size: u16) -> Result<(), Error> {
        let data = Self::id_tlv(id);
        let mut out = [0; 16];
        let n = self.call(2, 0, 0x26, &data, &mut out)?;
        if out[..n] != [0x41, 1, kind, 0x42, 1, 1] {
            self.close();
            return Err(Error::Schema);
        }
        let n = self.call(2, 0, 7, &data, &mut out)?;
        if out[..n] != [0x41, 2, (size >> 8) as u8, size as u8] {
            self.close();
            return Err(Error::Schema);
        }
        Ok(())
    }
    fn read_public(&mut self, id: u32) -> Result<[u8; 65], Error> {
        // Curve-specific ReadType is mandatory. Generic EC types are not
        // accepted as evidence of P-256. The physical personality needs a KAT.
        self.schema(id, 0x29, 32)?;
        let mut out = [0; 80];
        let n = self.call(2, 0, 0, &Self::id_tlv(id), &mut out)?;
        if n != 67 || out[..3] != [0x41, 65, 4] {
            self.close();
            return Err(Error::Schema);
        }
        let public: [u8; 65] = out[2..67].try_into().map_err(|_| Error::Schema)?;
        if VerifyingKey::from_sec1_bytes(&public).is_err() {
            self.close();
            return Err(Error::Record);
        }
        Ok(public)
    }
    /// The caller first durably reserves a NEVER-REUSED catalog slot in the SE
    /// ledger. A power cut after generation can orphan it; do not retry, reclaim
    /// or overwrite it automatically. Capacity is bounded, not infinite U2F.
    pub fn generate(&mut self, auth: &Authorization, slot: u16) -> Result<SigningKey, Error> {
        Self::require(auth, &[Operation::FidoRegister])?;
        let id = Self::id(slot)?;
        let mut out = [0; 16];
        let n = self.call(4, 0, 0x27, &Self::id_tlv(id), &mut out)?;
        // SE05x_Result_FAILURE is 02, SUCCESS is 01. Only authenticated
        // explicit absence allows creation; a bare error never means absent.
        if out[..n] != [0x41, 1, 2] {
            self.close();
            return Err(Error::Record);
        }
        let mut data = [0; 20];
        data[..3].copy_from_slice(&[0x11, 9, 8]);
        // Auth object ID=0: all other users, restricted to protected SCP access.
        data[7..11].copy_from_slice(&SIGN_POLICY.to_be_bytes());
        data[11..17].copy_from_slice(&Self::id_tlv(id));
        data[17..].copy_from_slice(&[0x42, 1, 3]); // NIST P-256, no private TLV.
        let n = self.call(1, 0x61, 0, &data, &mut out)?;
        if n != 0 {
            self.close();
            return Err(Error::Schema);
        }
        let public = self.read_public(id)?;
        Ok(SigningKey { id, public })
    }
    /// Restore only metadata already authenticated by the protected catalog.
    /// This reads PUBLIC key bytes and refuses a changed key under the same ID.
    pub fn restore(
        &mut self,
        auth: &Authorization,
        slot: u16,
        expected_public: &[u8; 65],
    ) -> Result<SigningKey, Error> {
        Self::require(
            auth,
            &[
                Operation::FidoRegister,
                Operation::FidoSign,
                Operation::DeriveHmacSecret,
            ],
        )?;
        let id = Self::id(slot)?;
        let public = self.read_public(id)?;
        if public != *expected_public {
            self.close();
            return Err(Error::Record);
        }
        Ok(SigningKey { id, public })
    }
    /// The SE signs exactly SHA256(message), not a second hash. Independently
    /// verify its DER signature against the bound public key before release.
    pub fn sign(
        &mut self,
        auth: &Authorization,
        key: &SigningKey,
        message: &[u8],
    ) -> Result<Signature, Error> {
        Self::require(
            auth,
            &[
                Operation::FidoRegister,
                Operation::FidoSign,
                Operation::DeriveHmacSecret,
            ],
        )?;
        let digest: [u8; 32] = Sha256::digest(message).into();
        let mut data = [0; 43];
        data[..6].copy_from_slice(&Self::id_tlv(key.id));
        data[6..11].copy_from_slice(&[0x42, 1, 0x21, 0x43, 32]);
        data[11..].copy_from_slice(&digest);
        let mut out = [0; 80];
        let n = self.call(3, 0x0c, 9, &data, &mut out)?;
        let result = (|| {
            if !(10..=74).contains(&n) || out[0] != 0x41 || usize::from(out[1]) + 2 != n {
                return Err(Error::Schema);
            }
            let signature = Signature::from_der(&out[2..n]).map_err(|_| Error::Record)?;
            let public = VerifyingKey::from_sec1_bytes(&key.public).map_err(|_| Error::Record)?;
            public
                .verify_prehash(&digest, &signature)
                .map_err(|_| Error::Record)?;
            Ok(signature)
        })();
        if result.is_err() {
            self.close();
        }
        result
    }
    /// RFC5869 Extract-and-Expand, SHA256, salt=per-credential random salt,
    /// info="credRandom", L=32, exactly matching OpenSK's helper.
    /// The non-UV root is retained for format compatibility, but OpenKeys
    /// requests always have mandatory fingerprint UV.
    pub fn derive_cred_random(
        &mut self,
        auth: &Authorization,
        has_uv: bool,
        salt: &[u8; 32],
    ) -> Result<Zeroizing<[u8; 32]>, Error> {
        Self::require(
            auth,
            &[
                Operation::FidoRegister,
                Operation::FidoSign,
                Operation::DeriveHmacSecret,
            ],
        )?;
        let id = if has_uv { HMAC_UV_ID } else { HMAC_NO_UV_ID };
        self.schema(id, 0x11, 32)?;
        let mut data = Zeroizing::new([0; 59]);
        data[..6].copy_from_slice(&Self::id_tlv(id));
        data[6..11].copy_from_slice(&[0x42, 1, 4, 0x43, 32]); // DIGEST_SHA256=04.
        data[11..43].copy_from_slice(salt);
        data[43..45].copy_from_slice(&[0x44, 10]);
        data[45..55].copy_from_slice(b"credRandom");
        data[55..59].copy_from_slice(&[0x45, 2, 0, 32]);
        let mut out = Zeroizing::new([0; 40]);
        let n = self.call(3, 0, 0x2d, &data[..], &mut out[..])?;
        if n != 34 || out[..2] != [0x41, 32] {
            self.close();
            return Err(Error::Schema);
        }
        let mut secret = Zeroizing::new([0; 32]);
        secret.copy_from_slice(&out[2..34]);
        Ok(secret)
    }
}
impl<X: Exchange> Drop for Keys<X> {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests;
