#![no_std]
#![forbid(unsafe_code)]

//! Protected-device policy, not a deployed authenticator. TrustZone attribution,
//! immutable boot, authenticated SE access and the physical I/O adapters remain
//! integration requirements. Host code must never execute this trust boundary.

pub mod biometric;
pub mod lcd;
pub mod pin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    ListPrivateMetadata,
    RevealNote,
    RevealRecoveryCode,
    Import,
    Delete,
    ChangePin,
    EnrollFingerprint,
    DeleteFingerprint,
    PairHost,
    FirmwareUpdate,
    FactoryReset,
    FidoRegister,
    FidoSign,
    DeriveHmacSecret,
}
impl Operation {
    /// This decision is device policy; a host cannot downgrade the requirement.
    pub const fn requires_additional_pin(self) -> bool {
        matches!(
            self,
            Self::RevealRecoveryCode
                | Self::Import
                | Self::Delete
                | Self::ChangePin
                | Self::EnrollFingerprint
                | Self::DeleteFingerprint
                | Self::PairHost
                | Self::FirmwareUpdate
                | Self::FactoryReset
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub operation: Operation,
    /// Hash of the canonical operation, item ID, revision and complete payload.
    pub digest: [u8; 32],
    /// Derived from the authenticated encrypted connection, not a host label.
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
    HardwareFault,
}

/// Implement only within the protected secure device domain. There is no
/// PIN-only entry point. A biometric receipt can only originate from the
/// authenticated sensor Session, with durable attempt/tag accounting.
///
/// Hardware must restrict FP/SE buses, LCD, buttons, timers, DMA and these calls
/// to the secure domain. A compromised nonsecure transport must not reach raw
/// crypto, arbitrary display writes, PIN/button injection or SE commands.
pub trait SecureVerifier {
    /// Check latched tamper, independent UV/OV, internal clock/voltage monitors,
    /// lifecycle and authenticated SE state. Do not clear faults automatically.
    fn check_security_state(&mut self) -> Result<(), Error>;
    fn verify_fingerprint(
        &mut self,
        request: &Request,
    ) -> Result<biometric::VerifiedRequest, Error>;
    /// Invoked only after fingerprint proof. Reserve a persistent attempt BEFORE
    /// collecting a new PIN using secure physical buttons. No host PIN input.
    fn verify_additional_pin(
        &mut self,
        fingerprint: &biometric::VerifiedRequest,
    ) -> Result<(), Error>;
    /// Show the complete request on the secure LCD, then require a fresh physical
    /// confirmation. No cached touch, automatic approval or truncated identity.
    fn confirm_on_device(&mut self, fingerprint: &biometric::VerifiedRequest) -> Result<(), Error>;
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
            // A clock rollback requires trusted recovery, not a later host time.
            self.exhausted = true;
            return Err(Error::TimeWentBackwards);
        }
        self.last_time = now;
        Ok(())
    }
    fn checkpoint(&mut self, clock: &mut impl FnMut() -> u64, expires: u64) -> Result<(), Error> {
        let now = clock();
        self.observe_time(now)?;
        if now >= expires {
            return Err(Error::Expired);
        }
        if self.verifier.check_security_state().is_err() {
            self.lock();
            self.exhausted = true;
            return Err(Error::HardwareFault);
        }
        let after_check = clock();
        self.observe_time(after_check)?;
        if after_check >= expires {
            return Err(Error::Expired);
        }
        Ok(())
    }
    /// Each request needs a fresh fingerprint, optional additional PIN, and a
    /// new device confirmation. All blocking steps share the original deadline.
    /// The callback must read trusted live monotonic time after every step.
    pub fn approve(
        &mut self,
        request: Request,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(), Error> {
        self.pending = None;
        let start = clock();
        self.observe_time(start)?;
        let mut expires = start.checked_add(15_000).ok_or(Error::CounterExhausted)?;
        self.checkpoint(&mut clock, expires)?;
        let fingerprint = self.verifier.verify_fingerprint(&request)?;
        if fingerprint.request != request {
            return Err(Error::WrongRequest);
        }
        expires = expires.min(fingerprint.expires);
        self.checkpoint(&mut clock, expires)?;
        if request.operation.requires_additional_pin() {
            self.verifier.verify_additional_pin(&fingerprint)?;
            self.checkpoint(&mut clock, expires)?;
        }
        self.verifier.confirm_on_device(&fingerprint)?;
        self.checkpoint(&mut clock, expires)?;
        self.pending = Some(Grant {
            request,
            expires,
            generation: self.generation,
        });
        Ok(())
    }
    /// Consumes the grant on success OR failure. The secure runtime must execute
    /// the matching key operation without returning to untrusted code between
    /// this check and use. This model alone cannot enforce peripheral isolation.
    pub fn consume(
        &mut self,
        request: &Request,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(), Error> {
        let grant = self.pending.take();
        self.observe_time(clock())?;
        let grant = grant.ok_or(Error::Locked)?;
        self.checkpoint(&mut clock, grant.expires)?;
        if grant.generation != self.generation || grant.request != *request {
            return Err(Error::WrongRequest);
        }
        Ok(())
    }
    /// Invoke on disconnect, suspend, reset, cancellation, error or lock.
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
    fn check_security_state(&mut self) -> Result<(), Error> {
        Err(Error::Rejected)
    }
    fn verify_fingerprint(&mut self, _: &Request) -> Result<biometric::VerifiedRequest, Error> {
        Err(Error::Rejected)
    }
    fn verify_additional_pin(&mut self, _: &biometric::VerifiedRequest) -> Result<(), Error> {
        Err(Error::Rejected)
    }
    fn confirm_on_device(&mut self, _: &biometric::VerifiedRequest) -> Result<(), Error> {
        Err(Error::Rejected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    struct Hardware<'a> {
        fingerprint: bool,
        pin: bool,
        confirmed: bool,
        fault: bool,
        pin_calls: u8,
        confirm_calls: u8,
        clock: &'a Cell<u64>,
        delay: u64,
    }
    impl SecureVerifier for Hardware<'_> {
        fn check_security_state(&mut self) -> Result<(), Error> {
            if self.fault {
                Err(Error::HardwareFault)
            } else {
                Ok(())
            }
        }
        fn verify_fingerprint(&mut self, r: &Request) -> Result<biometric::VerifiedRequest, Error> {
            self.clock.set(self.clock.get() + self.delay);
            if !self.fingerprint {
                return Err(Error::Rejected);
            }
            // Test-only construction is inside this crate. Production adapters
            // must obtain the private receipt from biometric::Session.
            Ok(biometric::VerifiedRequest {
                request: *r,
                expires: 15_001,
            })
        }
        fn verify_additional_pin(&mut self, _: &biometric::VerifiedRequest) -> Result<(), Error> {
            self.pin_calls += 1;
            if self.pin {
                Ok(())
            } else {
                Err(Error::Rejected)
            }
        }
        fn confirm_on_device(&mut self, _: &biometric::VerifiedRequest) -> Result<(), Error> {
            self.confirm_calls += 1;
            if self.confirmed {
                Ok(())
            } else {
                Err(Error::Rejected)
            }
        }
    }
    fn hardware(clock: &Cell<u64>) -> Hardware<'_> {
        Hardware {
            fingerprint: true,
            pin: true,
            confirmed: true,
            fault: false,
            pin_calls: 0,
            confirm_calls: 0,
            clock,
            delay: 0,
        }
    }
    fn request() -> Request {
        Request {
            operation: Operation::RevealNote,
            digest: [1; 32],
            channel: [2; 32],
        }
    }
    #[test]
    fn unprovisioned_cannot_unlock() {
        let mut gate = Gate::new(Unprovisioned);
        assert!(gate.approve(request(), || 0).is_err());
        assert_eq!(gate.consume(&request(), || 0), Err(Error::CounterExhausted));
    }
    #[test]
    fn a_correct_pin_cannot_replace_fingerprint() {
        let clock = Cell::new(1);
        let mut hw = hardware(&clock);
        hw.fingerprint = false;
        let mut gate = Gate::new(hw);
        for operation in [
            Operation::RevealNote,
            Operation::Import,
            Operation::ChangePin,
        ] {
            let r = Request {
                operation,
                ..request()
            };
            assert_eq!(gate.approve(r, || clock.get()), Err(Error::Rejected));
            assert_eq!(gate.consume(&r, || clock.get()), Err(Error::Locked));
        }
        assert_eq!(gate.verifier.pin_calls, 0);
        assert_eq!(gate.verifier.confirm_calls, 0);
    }
    #[test]
    fn additional_pin_is_derived_from_device_operation_policy() {
        let clock = Cell::new(1);
        let mut hw = hardware(&clock);
        hw.pin = false;
        let mut gate = Gate::new(hw);
        let r = Request {
            operation: Operation::Delete,
            ..request()
        };
        assert_eq!(gate.approve(r, || clock.get()), Err(Error::Rejected));
        assert_eq!(gate.verifier.confirm_calls, 0);
        gate.approve(request(), || clock.get()).unwrap();
        assert_eq!(gate.consume(&request(), || clock.get()), Ok(()));
        assert_eq!(gate.consume(&request(), || clock.get()), Err(Error::Locked));
    }
    #[test]
    fn substitution_expiry_disconnect_and_fault_revoke_access() {
        let clock = Cell::new(1);
        let mut gate = Gate::new(hardware(&clock));
        for which in 0..3 {
            gate.approve(request(), || clock.get()).unwrap();
            let mut changed = request();
            match which {
                0 => changed.channel[0] ^= 1,
                1 => changed.digest[0] ^= 1,
                _ => changed.operation = Operation::Delete,
            }
            assert_eq!(
                gate.consume(&changed, || clock.get()),
                Err(Error::WrongRequest)
            );
            assert_eq!(gate.consume(&request(), || clock.get()), Err(Error::Locked));
        }
        gate.approve(request(), || clock.get()).unwrap();
        gate.lock();
        assert_eq!(gate.consume(&request(), || clock.get()), Err(Error::Locked));
        gate.approve(request(), || clock.get()).unwrap();
        clock.set(15_001);
        assert_eq!(
            gate.consume(&request(), || clock.get()),
            Err(Error::Expired)
        );
        clock.set(15_002);
        gate.verifier.fault = true;
        assert_eq!(
            gate.approve(request(), || clock.get()),
            Err(Error::HardwareFault)
        );
        gate.verifier.fault = false;
        assert_eq!(
            gate.approve(request(), || clock.get()),
            Err(Error::CounterExhausted)
        );
    }
    #[test]
    fn blocking_authentication_does_not_extend_deadline() {
        let clock = Cell::new(1);
        let mut hw = hardware(&clock);
        hw.delay = 15_000;
        let mut gate = Gate::new(hw);
        assert_eq!(gate.approve(request(), || clock.get()), Err(Error::Expired));
        assert_eq!(gate.verifier.confirm_calls, 0);
        assert_eq!(gate.consume(&request(), || clock.get()), Err(Error::Locked));
    }
    #[test]
    fn rollback_requires_trusted_recovery() {
        let clock = Cell::new(100);
        let mut gate = Gate::new(hardware(&clock));
        gate.approve(request(), || clock.get()).unwrap();
        clock.set(99);
        assert_eq!(
            gate.consume(&request(), || clock.get()),
            Err(Error::TimeWentBackwards)
        );
        clock.set(100);
        assert_eq!(
            gate.approve(request(), || clock.get()),
            Err(Error::CounterExhausted)
        );
    }
    #[test]
    fn cancelled_confirmation_clears_an_earlier_grant() {
        let clock = Cell::new(1);
        let mut gate = Gate::new(hardware(&clock));
        gate.approve(request(), || clock.get()).unwrap();
        gate.verifier.confirmed = false;
        assert_eq!(
            gate.approve(request(), || clock.get()),
            Err(Error::Rejected)
        );
        assert_eq!(gate.consume(&request(), || clock.get()), Err(Error::Locked));
    }
}
