#![no_std]
#![forbid(unsafe_code)]
//! One fixed Noise XX suite, pinned peers, native-link epochs and ordered AEAD
//! records. This supplies cryptographic channel binding, NEVER user presence,
//! a hardware-PIN proof, protocol-PIN verification or an operation approval.
extern crate alloc;

pub mod apdu;
mod crypto;
mod handshake;
mod record;
pub mod wire;

pub use handshake::{Handshake, ManagerPairingReview, PairingCandidate, PeerStore, Role};
pub use record::{Channel, Context, MAX_PLAINTEXT, MAX_RECORD, NativeLink};
use zeroize::Zeroizing;

pub const SUITE: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
pub const HANDSHAKE_MS: u64 = 5_000;
pub const CHANNEL_MS: u64 = 600_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Closed,
    Entropy,
    State,
    Bounds,
    Crypto,
    Peer,
    Link,
    Expired,
    Time,
    Replay,
    Pairing,
    Persistence,
    Authorization,
    Format,
}

/// On the key, implement using the checked hardware RNG in the protected root.
/// On a manager, use the OS CSPRNG. There is no deterministic fallback.
pub trait Entropy {
    fn fill(&mut self, output: &mut [u8; 32]) -> Result<(), Error>;
}

/// No Debug/Clone/serialization. Store only wrapped by the root's independent
/// device key / the host OS credential store, never in a config file or Git.
pub struct Identity(Zeroizing<[u8; 32]>);
impl Identity {
    pub fn generate(entropy: &mut impl Entropy) -> Result<Self, Error> {
        let mut key = Zeroizing::new([0; 32]);
        entropy.fill(&mut key)?;
        if *key == [0; 32] {
            return Err(Error::Entropy);
        }
        Ok(Self(key))
    }
    /// Root/OS-keystore boundary only. `wrapped` storage must be authenticated;
    /// this method does not make plaintext filesystem storage acceptable.
    pub fn from_protected_storage(key: Zeroizing<[u8; 32]>) -> Result<Self, Error> {
        if *key == [0; 32] {
            return Err(Error::Entropy);
        }
        Ok(Self(key))
    }
    pub fn public_key(&self) -> [u8; 32] {
        crypto::public(&self.0)
    }
    pub fn seal_for_storage<T>(&self, seal: impl FnOnce(&[u8; 32]) -> T) -> T {
        seal(&self.0)
    }
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;
