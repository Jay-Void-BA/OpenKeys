//! Shared checked hardware service: one SAES/RNG owner for SCP, vault and FP.
//! No long-lived peripheral borrow across SE I/O and no software fallback.
use openkeys_h573_crypto::{Direction, Registers, Service, entropy::Source};
use openkeys_scp03::{Crypto, Error};

pub struct HardwareScpCrypto<'a, P: Registers, S: Source> {
    service: &'a Service<P, S>,
}
impl<'a, P: Registers, S: Source> HardwareScpCrypto<'a, P, S> {
    pub fn new(service: &'a Service<P, S>) -> Self {
        Self { service }
    }
}
impl<P: Registers, S: Source, const N: usize> Crypto<N> for HardwareScpCrypto<'_, P, S> {
    fn random_challenge(&mut self, out: &mut [u8; 8]) -> Result<(), Error> {
        self.service.random(out).map_err(|_| Error::Crypto)
    }
    fn cmac(&mut self, key: &[u8; N], message: &[u8], out: &mut [u8; 16]) -> Result<(), Error> {
        self.service
            .cmac(key, message, out)
            .map_err(|_| Error::Crypto)
    }
    fn encrypt_block(
        &mut self,
        key: &[u8; N],
        input: &[u8; 16],
        out: &mut [u8; 16],
    ) -> Result<(), Error> {
        self.service
            .ecb(key, Direction::Encrypt, input, out)
            .map_err(|_| Error::Crypto)
    }
    fn encrypt_cbc(&mut self, key: &[u8; N], iv: &[u8; 16], bytes: &mut [u8]) -> Result<(), Error> {
        self.service
            .cbc(key, Direction::Encrypt, iv, bytes)
            .map_err(|_| Error::Crypto)
    }
    fn decrypt_cbc(&mut self, key: &[u8; N], iv: &[u8; 16], bytes: &mut [u8]) -> Result<(), Error> {
        self.service
            .cbc(key, Direction::Decrypt, iv, bytes)
            .map_err(|_| Error::Crypto)
    }
}
