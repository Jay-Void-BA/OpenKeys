//! Temporal exclusive borrow of the shared masked SAES and checked RNG.
//! SCP03 session keys/state live in its private Channel, not in this adapter.
//! Releasing a borrow allows the same engine to service the vault/FP; it never
//! creates a second SAES/RNG owner or a software fallback.
use openkeys_h573_crypto::{
    Direction, MaskedAes, Registers,
    entropy::{CheckedRng, Source},
};
use openkeys_scp03::{Crypto, Error};

pub struct HardwareScpCrypto<'a, P: Registers, S: Source> {
    aes: &'a mut MaskedAes<P>,
    rng: &'a mut CheckedRng<S>,
    faulted: bool,
}
impl<'a, P: Registers, S: Source> HardwareScpCrypto<'a, P, S> {
    pub fn new(aes: &'a mut MaskedAes<P>, rng: &'a mut CheckedRng<S>) -> Self {
        Self {
            aes,
            rng,
            faulted: false,
        }
    }
    fn check(&self) -> Result<(), Error> {
        if self.faulted {
            Err(Error::Crypto)
        } else {
            Ok(())
        }
    }
    fn record(&mut self, result: Result<(), openkeys_h573_crypto::Error>) -> Result<(), Error> {
        result.map_err(|_| {
            self.faulted = true;
            Error::Crypto
        })
    }
}
impl<P: Registers, S: Source, const N: usize> Crypto<N> for HardwareScpCrypto<'_, P, S> {
    fn random_challenge(&mut self, out: &mut [u8; 8]) -> Result<(), Error> {
        out.fill(0);
        self.check()?;
        let result = self.rng.fill(out);
        self.record(result)
    }
    fn cmac(&mut self, key: &[u8; N], message: &[u8], out: &mut [u8; 16]) -> Result<(), Error> {
        out.fill(0);
        self.check()?;
        let result = self.aes.cmac(key, message, out);
        self.record(result)
    }
    fn encrypt_block(
        &mut self,
        key: &[u8; N],
        input: &[u8; 16],
        out: &mut [u8; 16],
    ) -> Result<(), Error> {
        out.fill(0);
        self.check()?;
        let result = self.aes.ecb(key, Direction::Encrypt, input, out);
        self.record(result)
    }
    fn encrypt_cbc(&mut self, key: &[u8; N], iv: &[u8; 16], bytes: &mut [u8]) -> Result<(), Error> {
        if self.check().is_err() {
            bytes.fill(0);
            return Err(Error::Crypto);
        }
        let result = self.aes.cbc(key, Direction::Encrypt, iv, bytes);
        self.record(result)
    }
    fn decrypt_cbc(&mut self, key: &[u8; N], iv: &[u8; 16], bytes: &mut [u8]) -> Result<(), Error> {
        if self.check().is_err() {
            bytes.fill(0);
            return Err(Error::Crypto);
        }
        let result = self.aes.cbc(key, Direction::Decrypt, iv, bytes);
        self.record(result)
    }
}
