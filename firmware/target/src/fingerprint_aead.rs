//! Private FPC secure-frame AES256-GCM verifier. A validated payload is not
//! a match/grant until device-policy checks its semantics and persists credit.
use device_policy::biometric::{Error, ProtectedGcm};
use openkeys_h573_crypto::{Direction, Registers, Service, entropy::Source};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

pub struct HardwareFingerprintGcm<'a, P: Registers, S: Source> {
    service: &'a Service<P, S>,
    key: Zeroizing<[u8; 32]>,
    faulted: bool,
}
impl<'a, P: Registers, S: Source> HardwareFingerprintGcm<'a, P, S> {
    /// Only a uniquely commissioned module key, unwrapped inside the verified
    /// protected root. No public UID/default key or host-provided key path.
    pub fn new(service: &'a Service<P, S>, key: Zeroizing<[u8; 32]>) -> Self {
        Self {
            service,
            key,
            faulted: false,
        }
    }
}
impl<P: Registers, S: Source> ProtectedGcm for HardwareFingerprintGcm<'_, P, S> {
    fn decrypt_identify_payload(
        &mut self,
        header: &[u8; 8],
        iv: &[u8; 12],
        tag: &[u8; 16],
        payload: &mut [u8; 12],
    ) -> Result<(), Error> {
        if self.faulted {
            payload.fill(0);
            return Err(Error::Unauthenticated);
        }
        let mut tentative = Zeroizing::new(*payload);
        if let Ok(computed) = self.service.gcm(
            &self.key,
            Direction::Decrypt,
            iv,
            header,
            &mut tentative[..],
        ) && bool::from(computed.ct_eq(tag))
        {
            payload.copy_from_slice(&tentative[..]);
            return Ok(());
        }
        payload.fill(0);
        self.key.zeroize();
        self.faulted = true;
        self.service.revoke();
        Err(Error::Unauthenticated)
    }
}
