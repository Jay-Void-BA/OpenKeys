//! Journal-only MAC adapter. The non-exportable device key never enters RAM,
//! an APDU, a host request or the manager. Closed boot and actual protected
//! flash/Root integration are separate mandatory components.
use openkeys_h573_crypto::{Registers, Service, entropy::Source};
use openkeys_root_journal::{BODY_BYTES, Context, ContinuityGuard, Error, Guard, Mac, State};
use zeroize::Zeroizing;

const DOMAIN: &[u8] = b"OpenKeys/R05/RootJournal/DHUK/v1";

/// Production continuity adapter for the actual H573 service. The caller still
/// supplies the protected boot/maintenance/identity guard; this wrapper cannot
/// establish trusted boot by itself. No backup write, reset or reseed API.
pub struct HardwareRootGuard<'a, G: Guard, P: Registers, S: Source> {
    guard: G,
    service: &'a Service<P, S>,
    closed: bool,
}
impl<'a, G: Guard, P: Registers, S: Source> HardwareRootGuard<'a, G, P, S> {
    pub fn new(guard: G, service: &'a Service<P, S>) -> Self {
        Self {
            guard,
            service,
            closed: false,
        }
    }
    fn finish<T>(&mut self, result: Result<T, Error>) -> Result<T, Error> {
        let result = result.and_then(|value| {
            self.check()?;
            Ok(value)
        });
        if result.is_err() {
            self.closed = true;
        }
        result
    }
}
impl<G: Guard, P: Registers, S: Source> Guard for HardwareRootGuard<'_, G, P, S> {
    fn check(&mut self) -> Result<(), Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        let result = self.guard.check();
        if result.is_err() {
            self.closed = true;
        }
        result
    }
    fn fresh_owner_nonce(&mut self) -> Result<[u8; 32], Error> {
        self.check()?;
        let result = self.guard.fresh_owner_nonce().and_then(|owner| {
            if owner == [0; 32] || owner == [0xff; 32] {
                Err(Error::Crypto)
            } else {
                Ok(owner)
            }
        });
        self.finish(result)
    }
    fn authorize_transition(&mut self, current: &State, target: &State) -> Result<(), Error> {
        self.check()?;
        let result = self.guard.authorize_transition(current, target);
        self.finish(result)
    }
    fn authorize_factory_initialize(&mut self, context: &Context) -> Result<(), Error> {
        self.check()?;
        let result = self.guard.authorize_factory_initialize(context);
        self.finish(result)
    }
}
impl<G: Guard, P: Registers, S: Source> ContinuityGuard for HardwareRootGuard<'_, G, P, S> {
    fn verify_backup_owner(&mut self, owner: &[u8; 32]) -> Result<(), Error> {
        self.check()?;
        let result = self
            .service
            .verify_backup_owner(owner)
            .map_err(|_| Error::Crypto);
        self.finish(result)
    }
}

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
