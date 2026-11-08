#![no_std]
#![forbid(unsafe_code)]

//! Device-side authorization model. Hardware implementations must verify UV,
//! secure monotonic time and authenticated channel bindings themselves.
//! This library does not authenticate hardware or implement cryptography.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Fingerprint,
    DevicePin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    ListPrivateMetadata,
    RevealOtp,
    RevealNote,
    RevealRecoveryCode,
    Import,
    Delete,
    ChangeTime,
    ChangePin,
    EnrollFingerprint,
    FidoSign,
    DeriveHmacSecret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub operation: Operation,
    /// Hash of the complete canonical request including item ID, revision and payload.
    pub digest: [u8; 32],
    /// Bound to a device-authenticated encrypted connection, never a host-provided label.
    pub channel: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Locked,
    Expired,
    TimeWentBackwards,
    WrongRequest,
    Rejected,
    CounterExhausted,
}

/// Implement ONLY inside the protected device firmware. A host Boolean or UART
/// match result is not an implementation of this trust boundary.
pub trait SecureVerifier {
    /// Includes persistent PIN/biometric retry limits, authenticated sensor
    /// results and a physical confirmation bound to this exact request.
    fn verify_and_confirm(&mut self, method: Method, request: &Request) -> Result<(), Error>;
}

#[derive(Clone, Copy)]
struct Grant {
    request: Request,
    expires: u64,
    generation: u64,
}

pub struct Gate<V> {
    verifier: V,
    pending: Option<Grant>,
    last_time: u64,
    generation: u64,
    exhausted: bool,
}

impl<V: SecureVerifier> Gate<V> {
    pub const fn new(verifier: V) -> Self {
        Self {
            verifier,
            pending: None,
            last_time: 0,
            generation: 0,
            exhausted: false,
        }
    }

    fn observe_time(&mut self, now: u64) -> Result<(), Error> {
        if self.exhausted {
            return Err(Error::CounterExhausted);
        }
        if now < self.last_time {
            self.lock();
            return Err(Error::TimeWentBackwards);
        }
        self.last_time = now;
        Ok(())
    }

    /// At most one command can be approved. Rejection clears any earlier grant.
    pub fn approve(&mut self, method: Method, request: Request, now: u64) -> Result<(), Error> {
        self.pending = None;
        self.observe_time(now)?;
        self.verifier.verify_and_confirm(method, &request)?;
        // The secure adapter must also enforce a deadline during blocking UV.
        let expires = now.checked_add(15_000).ok_or(Error::CounterExhausted)?;
        self.pending = Some(Grant {
            request,
            expires,
            generation: self.generation,
        });
        Ok(())
    }

    /// Consumes the grant on both success and failure; a different command,
    /// connection or digest cannot reuse a successful fingerprint/PIN operation.
    pub fn consume(&mut self, request: &Request, now: u64) -> Result<(), Error> {
        let grant = self.pending.take();
        self.observe_time(now)?;
        let grant = grant.ok_or(Error::Locked)?;
        if now >= grant.expires {
            return Err(Error::Expired);
        }
        if grant.generation != self.generation || grant.request != *request {
            return Err(Error::WrongRequest);
        }
        Ok(())
    }

    /// Call on disconnect, suspend, reset, sensor error, user cancellation and lock.
    pub fn lock(&mut self) {
        self.pending = None;
        match self.generation.checked_add(1) {
            Some(next) => self.generation = next,
            None => self.exhausted = true,
        }
    }
}

pub struct Unprovisioned;
impl SecureVerifier for Unprovisioned {
    fn verify_and_confirm(&mut self, _: Method, _: &Request) -> Result<(), Error> {
        Err(Error::Rejected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestHardware(bool);
    impl SecureVerifier for TestHardware {
        fn verify_and_confirm(&mut self, _: Method, _: &Request) -> Result<(), Error> {
            if self.0 { Ok(()) } else { Err(Error::Rejected) }
        }
    }
    fn request() -> Request {
        Request {
            operation: Operation::RevealRecoveryCode,
            digest: [1; 32],
            channel: [2; 32],
        }
    }
    #[test]
    fn default_backend_cannot_unlock() {
        let mut gate = Gate::new(Unprovisioned);
        assert_eq!(
            gate.approve(Method::Fingerprint, request(), 0),
            Err(Error::Rejected)
        );
        assert_eq!(gate.consume(&request(), 0), Err(Error::Locked));
    }
    #[test]
    fn commands_need_verification_and_are_single_use() {
        let mut gate = Gate::new(TestHardware(true));
        assert_eq!(gate.consume(&request(), 0), Err(Error::Locked));
        gate.approve(Method::DevicePin, request(), 1).unwrap();
        assert_eq!(gate.consume(&request(), 2), Ok(()));
        assert_eq!(gate.consume(&request(), 3), Err(Error::Locked));
    }
    #[test]
    fn request_and_connection_substitution_fail() {
        for which in 0..3 {
            let mut gate = Gate::new(TestHardware(true));
            gate.approve(Method::Fingerprint, request(), 0).unwrap();
            let mut changed = request();
            match which {
                0 => changed.channel[0] ^= 1,
                1 => changed.digest[0] ^= 1,
                _ => changed.operation = Operation::Delete,
            }
            assert_eq!(gate.consume(&changed, 1), Err(Error::WrongRequest));
            assert_eq!(gate.consume(&request(), 2), Err(Error::Locked));
        }
    }
    #[test]
    fn expiry_and_disconnect_revoke_access() {
        let mut gate = Gate::new(TestHardware(true));
        gate.approve(Method::Fingerprint, request(), 1).unwrap();
        assert_eq!(gate.consume(&request(), 15_001), Err(Error::Expired));
        gate.approve(Method::Fingerprint, request(), 15_002)
            .unwrap();
        gate.lock();
        assert_eq!(gate.consume(&request(), 15_003), Err(Error::Locked));
    }
    #[test]
    fn clock_rollback_fails_closed() {
        let mut gate = Gate::new(TestHardware(true));
        gate.approve(Method::DevicePin, request(), 100).unwrap();
        assert_eq!(gate.consume(&request(), 99), Err(Error::TimeWentBackwards));
        assert_eq!(gate.consume(&request(), 100), Err(Error::Locked));
    }
    #[test]
    fn rejected_new_request_clears_old_grant() {
        let mut gate = Gate::new(TestHardware(true));
        gate.approve(Method::Fingerprint, request(), 1).unwrap();
        gate.verifier.0 = false;
        assert_eq!(
            gate.approve(Method::Fingerprint, request(), 2),
            Err(Error::Rejected)
        );
        assert_eq!(gate.consume(&request(), 3), Err(Error::Locked));
    }
}
