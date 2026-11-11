//! Small fixed-suite adapter. The upstream automatic RNG API cannot report
//! hardware errors, so the wrapper always supplies a fresh ephemeral itself.
use noise_protocol::{DH, Hash, U8Array};
use noise_rust_crypto::sensitive::Sensitive;
use sha2_secret::Digest;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

/// Both HMAC pads and chaining/hash outputs are zeroizing, including the hash
/// core/buffer. Upstream's Sha256 adapter uses ordinary [u8;64] pads and a
/// non-zeroizing SHA2 state. No rekey/raw split/clone API is exposed by us.
#[derive(Default)]
pub(crate) struct SecretHash(sha2_secret::Sha256);
impl Hash for SecretHash {
    type Block = Sensitive<[u8; 64]>;
    type Output = Sensitive<[u8; 32]>;
    fn name() -> &'static str {
        "SHA256"
    }
    fn input(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    fn result(&mut self) -> Self::Output {
        let mut result = self.0.finalize_reset();
        let output = Self::Output::from_slice(&result);
        result.as_mut_slice().zeroize();
        output
    }
}

pub(crate) enum Dh {}
pub(crate) fn public(key: &[u8; 32]) -> [u8; 32] {
    *PublicKey::from(&StaticSecret::from(*key)).as_bytes()
}
impl DH for Dh {
    type Key = Sensitive<[u8; 32]>;
    type Pubkey = [u8; 32];
    type Output = Sensitive<[u8; 32]>;
    fn name() -> &'static str {
        "25519"
    }
    fn genkey() -> Self::Key {
        // Not reachable through Handshake: every constructor supplies Some(e),
        // and no raw Noise state is exposed. A future integration bug must
        // terminate rather than substitute a fixed/weak ephemeral key.
        panic!("OpenKeys requires fallible, explicit hardware entropy")
    }
    fn pubkey(key: &Self::Key) -> Self::Pubkey {
        public(key)
    }
    fn dh(key: &Self::Key, peer: &Self::Pubkey) -> Result<Self::Output, ()> {
        let shared = StaticSecret::from(**key).diffie_hellman(&PublicKey::from(*peer));
        // RFC7748: reject non-contributory / low-order peers. The upstream
        // noise-rust-crypto X25519 adapter does not perform this check.
        if !shared.was_contributory() {
            return Err(());
        }
        Ok(Self::Output::from_slice(shared.as_bytes()))
    }
}
