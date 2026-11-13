//! One AES/RNG owner for SCP03, vault and fingerprint adapters. Borrowing a
//! service does not retain a peripheral lock over an entire SE transaction.
use crate::{
    Direction, Error, MaskedAes, Registers,
    entropy::{CheckedRng, Source},
};
use core::cell::{Cell, RefCell};

pub struct Service<P: Registers, S: Source> {
    aes: RefCell<MaskedAes<P>>,
    rng: RefCell<CheckedRng<S>>,
    busy: Cell<bool>,
    poisoned: Cell<bool>,
}
struct Active<'a> {
    busy: &'a Cell<bool>,
}
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.busy.set(false);
    }
}
impl<P: Registers, S: Source> Service<P, S> {
    pub fn new(aes: MaskedAes<P>, rng: CheckedRng<S>) -> Self {
        Self {
            aes: RefCell::new(aes),
            rng: RefCell::new(rng),
            busy: Cell::new(false),
            poisoned: Cell::new(false),
        }
    }
    /// One-way component revocation. The root must also revoke operation
    /// grants, blank LCD and disable interfaces; this is not that root service.
    pub fn revoke(&self) {
        self.poisoned.set(true);
    }
    pub fn faulted(&self) -> bool {
        self.poisoned.get()
    }
    fn enter(&self) -> Result<Active<'_>, Error> {
        if self.poisoned.get() {
            return Err(Error::Faulted);
        }
        if self.busy.replace(true) {
            self.revoke();
            return Err(Error::Faulted);
        }
        Ok(Active { busy: &self.busy })
    }
    fn finish<T>(&self, result: Result<T, Error>) -> Result<T, Error> {
        if result.is_err() {
            self.revoke();
        }
        if self.poisoned.get() {
            return Err(Error::Faulted);
        }
        result
    }
    fn with_aes<T>(
        &self,
        f: impl FnOnce(&mut MaskedAes<P>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let _active = self.enter()?;
        let result = match self.aes.try_borrow_mut() {
            Ok(mut aes) => f(&mut aes),
            Err(_) => Err(Error::Faulted),
        };
        self.finish(result)
    }
    pub fn random(&self, out: &mut [u8]) -> Result<(), Error> {
        out.fill(0);
        let result = (|| {
            let _active = self.enter()?;
            let result = match self.rng.try_borrow_mut() {
                Ok(mut rng) => rng.fill(out),
                Err(_) => Err(Error::Faulted),
            };
            self.finish(result)
        })();
        if result.is_err() {
            out.fill(0);
        }
        result
    }
    pub fn ecb<const N: usize>(
        &self,
        key: &[u8; N],
        direction: Direction,
        input: &[u8; 16],
        out: &mut [u8; 16],
    ) -> Result<(), Error> {
        out.fill(0);
        let result = self.with_aes(|aes| aes.ecb(key, direction, input, out));
        if result.is_err() {
            out.fill(0);
        }
        result
    }
    pub fn cmac<const N: usize>(
        &self,
        key: &[u8; N],
        message: &[u8],
        out: &mut [u8; 16],
    ) -> Result<(), Error> {
        out.fill(0);
        let result = self.with_aes(|aes| aes.cmac(key, message, out));
        if result.is_err() {
            out.fill(0);
        }
        result
    }
    pub fn dhuk_cmac(&self, message: &[u8], out: &mut [u8; 16]) -> Result<(), Error> {
        out.fill(0);
        let result = self.with_aes(|aes| aes.dhuk_cmac(message, out));
        if result.is_err() {
            out.fill(0);
        }
        result
    }
    pub fn cbc<const N: usize>(
        &self,
        key: &[u8; N],
        direction: Direction,
        iv: &[u8; 16],
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        let result = self.with_aes(|aes| aes.cbc(key, direction, iv, bytes));
        if result.is_err() {
            bytes.fill(0);
        }
        result
    }
    pub fn gcm<const N: usize>(
        &self,
        key: &[u8; N],
        direction: Direction,
        nonce: &[u8; 12],
        aad: &[u8],
        bytes: &mut [u8],
    ) -> Result<[u8; 16], Error> {
        let result = self.with_aes(|aes| aes.gcm(key, direction, nonce, aad, bytes));
        if result.is_err() {
            bytes.fill(0);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Entropy {
        word: u32,
        fail: bool,
    }
    impl Source for Entropy {
        fn poll(&mut self) -> Result<Option<u32>, Error> {
            self.word += 1;
            if self.fail && self.word > 2 {
                Err(Error::Hardware)
            } else {
                Ok(Some(self.word))
            }
        }
    }
    fn service(fail: bool) -> Service<crate::tests::Port, Entropy> {
        let (aes, _) = crate::tests::engine();
        Service::new(aes, CheckedRng::new(Entropy { word: 0, fail }).unwrap())
    }
    #[test]
    fn shared_crypto_uses_one_engine_and_never_keeps_a_long_lived_borrow() {
        let service = service(false);
        let mut tag = [0; 16];
        service.cmac(&[0; 16], b"public fixture", &mut tag).unwrap();
        let mut bytes = [0x31; 12];
        let expected = bytes;
        let tag = service
            .gcm(
                &[0; 32],
                Direction::Encrypt,
                &[0; 12],
                b"eightAAD",
                &mut bytes,
            )
            .unwrap();
        let got = service
            .gcm(
                &[0; 32],
                Direction::Decrypt,
                &[0; 12],
                b"eightAAD",
                &mut bytes,
            )
            .unwrap();
        assert_eq!(got, tag);
        assert_eq!(bytes, expected);
        let mut random = [0; 8];
        service.random(&mut random).unwrap();
        assert_eq!(random, [2, 0, 0, 0, 3, 0, 0, 0]);
        assert!(!service.faulted());
    }
    #[test]
    fn rng_fault_poison_covers_aes_and_erases_partial_random_output() {
        let service = service(true);
        let mut random = [0x55; 8];
        assert!(service.random(&mut random).is_err());
        assert_eq!(random, [0; 8]);
        let mut out = [0x55; 16];
        assert_eq!(
            service.ecb(&[0; 16], Direction::Encrypt, &[0; 16], &mut out),
            Err(Error::Faulted)
        );
        assert_eq!(out, [0; 16]);
    }
    #[test]
    fn missing_hardware_key_context_revokes_every_purpose_without_software_fallback() {
        let service = service(false);
        let mut tag = [0x55; 16];
        assert!(service.dhuk_cmac(b"journal", &mut tag).is_err());
        assert_eq!(tag, [0; 16]);
        assert!(service.faulted());
        assert_eq!(
            service.cmac(&[0; 16], b"SCP", &mut tag),
            Err(Error::Faulted)
        );
        assert_eq!(tag, [0; 16]);
        let mut random = [0x55; 8];
        assert_eq!(service.random(&mut random), Err(Error::Faulted));
        assert_eq!(random, [0; 8]);
    }
    #[test]
    fn reentry_poison_cannot_release_the_outer_success_or_be_reset_by_a_new_adapter() {
        let service = service(false);
        let result = service.with_aes(|_| {
            let mut random = [0x55; 8];
            assert_eq!(service.random(&mut random), Err(Error::Faulted));
            assert_eq!(random, [0; 8]);
            Ok([0x55u8; 16])
        });
        assert_eq!(result, Err(Error::Faulted));
        assert!(service.faulted());
        let mut bytes = [0x55; 16];
        assert_eq!(
            service.cbc(&[0; 16], Direction::Encrypt, &[0; 16], &mut bytes),
            Err(Error::Faulted)
        );
        assert_eq!(bytes, [0; 16]);
    }
    #[test]
    fn bad_cipher_input_revokes_all_purposes_and_never_leaves_partial_plaintext() {
        let service = service(false);
        let mut bytes = [0x55; 15];
        assert!(
            service
                .cbc(&[0; 16], Direction::Encrypt, &[0; 16], &mut bytes)
                .is_err()
        );
        assert_eq!(bytes, [0; 15]);
        let mut random = [0x55; 8];
        assert_eq!(service.random(&mut random), Err(Error::Faulted));
        assert_eq!(random, [0; 8]);
    }
}
