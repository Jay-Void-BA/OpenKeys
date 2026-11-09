//! Vault adapter over the single bounded SAES owner, never a host oracle.
//! Requires verified boot/attribution, independent per-device vault key and
//! durable SE epoch ledger. No default/UID-derived key or diagnostic unlock.
use device_vault::{Aead, Error, PAYLOAD_BYTES};
use openkeys_h573_crypto::{Direction, Registers, Service, entropy::Source};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

pub struct HardwareVaultAead<'a, P: Registers, S: Source> {
    engine: &'a Service<P, S>,
    key: Zeroizing<[u8; 32]>,
    faulted: bool,
}
impl<'a, P: Registers, S: Source> HardwareVaultAead<'a, P, S> {
    pub fn new(engine: &'a Service<P, S>, key: Zeroizing<[u8; 32]>) -> Self {
        Self {
            engine,
            key,
            faulted: false,
        }
    }
    fn crypt(
        &mut self,
        direction: Direction,
        nonce: &[u8; 12],
        aad: &[u8; 48],
        bytes: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<[u8; 16], Error> {
        if self.faulted {
            bytes.fill(0);
            return Err(Error::Faulted);
        }
        self.engine
            .gcm(&self.key, direction, nonce, aad, bytes)
            .map_err(|_| {
                self.faulted = true;
                self.key.zeroize();
                bytes.fill(0);
                Error::Crypto
            })
    }
}
impl<P: Registers, S: Source> Aead for HardwareVaultAead<'_, P, S> {
    fn seal(
        &mut self,
        nonce: &[u8; 12],
        aad: &[u8; 48],
        bytes: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<[u8; 16], Error> {
        self.crypt(Direction::Encrypt, nonce, aad, bytes)
    }
    fn open(
        &mut self,
        nonce: &[u8; 12],
        aad: &[u8; 48],
        tag: &[u8; 16],
        bytes: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<(), Error> {
        let mut output = Zeroizing::new(*bytes);
        if let Ok(calculated) = self.crypt(Direction::Decrypt, nonce, aad, &mut output)
            && bool::from(calculated.ct_eq(tag))
        {
            bytes.copy_from_slice(&output[..]);
            return Ok(());
        }
        bytes.fill(0);
        self.faulted = true;
        self.engine.revoke();
        self.key.zeroize();
        Err(Error::Crypto)
    }
}
