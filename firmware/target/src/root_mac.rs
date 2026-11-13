//! Journal-only MAC adapter. The non-exportable device key never enters RAM,
//! an APDU, a host request or the manager. Closed boot and actual protected
//! flash/Root integration are separate mandatory components.
use openkeys_h573_crypto::{Registers, Service, entropy::Source};
use openkeys_root_journal::{BODY_BYTES, Error, Mac};
use zeroize::Zeroizing;

const DOMAIN: &[u8] = b"OpenKeys/R05/RootJournal/DHUK/v1";

pub struct HardwareRootMac<'a, P: Registers, S: Source> {
    service: &'a Service<P, S>,
}
impl<'a, P: Registers, S: Source> HardwareRootMac<'a, P, S> {
    pub fn new(service: &'a Service<P, S>) -> Self {
        Self { service }
    }
}
impl<P: Registers, S: Source> Mac for HardwareRootMac<'_, P, S> {
    fn tag(&mut self, body: &[u8; BODY_BYTES]) -> Result<[u8; 16], Error> {
        let mut input = Zeroizing::new([0; DOMAIN.len() + BODY_BYTES]);
        input[..DOMAIN.len()].copy_from_slice(DOMAIN);
        input[DOMAIN.len()..].copy_from_slice(body);
        let mut out = Zeroizing::new([0; 16]);
        self.service
            .dhuk_cmac(input.as_ref(), &mut out)
            .map_err(|_| Error::Crypto)?;
        Ok(*out)
    }
}
