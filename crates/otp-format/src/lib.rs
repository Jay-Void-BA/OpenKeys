#![no_std]
#![forbid(unsafe_code)]
//! Canonical OTP provisioning input. A seed exists only during encrypted import
//! into an SE HMAC object. It must never enter an ordinary record/export path.
#[cfg(any(feature = "alloc", test))]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use sha2::{Digest, Sha256};
#[cfg(feature = "alloc")]
use zeroize::Zeroizing;

pub const MAX_LABEL: usize = 80;
pub const MAX_IMPORT: usize = 15 + 2 * MAX_LABEL + 64;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Format,
    Label,
    Seed,
    Counter,
    Unsupported,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Algorithm {
    Sha1 = 0x18,
    Sha256 = 0x19,
    Sha512 = 0x1b,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Totp = 1,
    Hotp = 2,
}

/// Borrowed from a zeroizing, AEAD-authenticated buffer. No Debug/Clone/serde.
pub struct Import<'a> {
    kind: Kind,
    algorithm: Algorithm,
    issuer: &'a str,
    account: &'a str,
    initial_counter: u64,
    seed: &'a [u8],
}
fn label(s: &str, empty: bool) -> bool {
    (empty || !s.is_empty())
        && s.len() <= MAX_LABEL
        && s.bytes().all(|c| (0x20..=0x7e).contains(&c))
        && s.trim() == s
}
impl<'a> Import<'a> {
    pub const fn kind(&self) -> Kind {
        self.kind
    }
    pub const fn algorithm(&self) -> Algorithm {
        self.algorithm
    }
    pub const fn issuer(&self) -> &'a str {
        self.issuer
    }
    pub const fn account(&self) -> &'a str {
        self.account
    }
    pub const fn initial_counter(&self) -> u64 {
        self.initial_counter
    }
    pub fn new(
        kind: Kind,
        algorithm: Algorithm,
        issuer: &'a str,
        account: &'a str,
        initial_counter: u64,
        seed: &'a [u8],
    ) -> Result<Self, Error> {
        if !label(issuer, true) || !label(account, false) {
            return Err(Error::Label);
        }
        if !(16..=64).contains(&seed.len()) {
            return Err(Error::Seed);
        }
        if (kind == Kind::Totp && initial_counter != 0) || initial_counter == u64::MAX {
            return Err(Error::Counter);
        }
        Ok(Self {
            kind,
            algorithm,
            issuer,
            account,
            initial_counter,
            seed,
        })
    }
    /// Root-only provisioning input. There is no counterpart for reading an
    /// installed seed. The SE HMAC object's ACL must forbid read/export.
    pub fn seed_for_provisioning(&self) -> &[u8] {
        self.seed
    }
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() < 12 || bytes.len() > MAX_IMPORT || bytes[0] != 1 {
            return Err(Error::Format);
        }
        let kind = match bytes[1] {
            1 => Kind::Totp,
            2 => Kind::Hotp,
            _ => return Err(Error::Unsupported),
        };
        let algorithm = match bytes[2] {
            0x18 => Algorithm::Sha1,
            0x19 => Algorithm::Sha256,
            0x1b => Algorithm::Sha512,
            _ => return Err(Error::Unsupported),
        };
        // Fixed 6 digits, 30-second TOTP. No permissive digits/period coercion.
        if bytes[3] != 6 {
            return Err(Error::Unsupported);
        }
        let counter = u64::from_be_bytes(bytes[4..12].try_into().map_err(|_| Error::Format)?);
        let mut cursor = &bytes[12..];
        fn field<'b>(cursor: &mut &'b [u8]) -> Result<&'b [u8], Error> {
            let n = *cursor.first().ok_or(Error::Format)? as usize;
            let value = cursor.get(1..1 + n).ok_or(Error::Format)?;
            *cursor = &cursor[1 + n..];
            Ok(value)
        }
        let issuer = core::str::from_utf8(field(&mut cursor)?).map_err(|_| Error::Label)?;
        let account = core::str::from_utf8(field(&mut cursor)?).map_err(|_| Error::Label)?;
        let seed = field(&mut cursor)?;
        if !cursor.is_empty() {
            return Err(Error::Format);
        }
        Self::new(kind, algorithm, issuer, account, counter, seed)
    }
    #[cfg(feature = "alloc")]
    pub fn encode(&self) -> Zeroizing<Vec<u8>> {
        let mut out = Zeroizing::new(Vec::with_capacity(
            15 + self.issuer.len() + self.account.len() + self.seed.len(),
        ));
        self.canonical(|part| out.extend_from_slice(part));
        out
    }
    fn canonical(&self, mut output: impl FnMut(&[u8])) {
        output(&[1, self.kind as u8, self.algorithm as u8, 6]);
        output(&self.initial_counter.to_be_bytes());
        for field in [self.issuer.as_bytes(), self.account.as_bytes(), self.seed] {
            output(&[field.len() as u8]);
            output(field);
        }
    }
    /// Binding to exact content prevents swapping an account/seed/counter
    /// after the person confirms the identity on the LCD. Do not log this
    /// digest: it is not a PIN proof and need not become public metadata.
    pub fn digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"OpenKeys/R05/OTP-import/v1");
        self.canonical(|part| h.update(part));
        h.finalize().into()
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    #[test]
    fn seed_label_counter_and_algorithm_are_one_canonical_approved_input() {
        let source = Import::new(
            Kind::Hotp,
            Algorithm::Sha256,
            "Example",
            "owner",
            47,
            &[0xaa; 32],
        )
        .unwrap();
        let bytes = source.encode();
        let parsed = Import::parse(&bytes).unwrap();
        assert_eq!(source.digest(), parsed.digest());
        assert_eq!(parsed.initial_counter, 47);
        for n in 0..bytes.len() {
            assert!(Import::parse(&bytes[..n]).is_err());
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(Import::parse(&extra).is_err());
        let mut other = bytes.clone();
        *other.last_mut().unwrap() ^= 1;
        assert_ne!(Import::parse(&other).unwrap().digest(), source.digest());
    }
    #[test]
    fn unicode_controls_short_seeds_and_unrepresentable_counters_are_rejected() {
        for issuer in ["\u{202e}bank", " bank", "bank ", "bank\n", "银行"] {
            assert!(
                Import::new(Kind::Totp, Algorithm::Sha1, issuer, "owner", 0, &[1; 20]).is_err()
            );
        }
        for n in [0, 10, 15, 65] {
            assert!(
                Import::new(
                    Kind::Totp,
                    Algorithm::Sha1,
                    "",
                    "owner",
                    0,
                    &alloc::vec![1; n]
                )
                .is_err()
            );
        }
        assert!(Import::new(Kind::Totp, Algorithm::Sha1, "", "owner", 1, &[1; 20]).is_err());
        assert!(Import::new(Kind::Hotp, Algorithm::Sha1, "", "owner", u64::MAX, &[1; 20]).is_err());
    }
    #[test]
    fn maximum_valid_import_fits_the_parser_bound() {
        let issuer = "I".repeat(MAX_LABEL);
        let account = "A".repeat(MAX_LABEL);
        let import = Import::new(
            Kind::Totp,
            Algorithm::Sha512,
            &issuer,
            &account,
            0,
            &[0x55; 64],
        )
        .unwrap();
        let bytes = import.encode();
        assert_eq!(bytes.len(), MAX_IMPORT);
        assert_eq!(Import::parse(&bytes).unwrap().digest(), import.digest());
    }
}
