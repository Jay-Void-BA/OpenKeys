//! Authenticated management-session PIN leases. Mere USB presence, NFC field,
//! status polling and authenticated keepalive DO NOT renew idle authorization.
//! Production root must supply real paired-peer/physical-link checks, live
//! protected time, protocol-PIN checks and release-only physical confirmation.
use crate::{
    Authorization, Error, Operation, Request,
    execution::{Completed, State},
    pin_only::VerifiedPinRequest,
};
use sha2::{Digest, Sha256};

pub const IDLE_MS: u64 = 30_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    Usb,
    Nfc,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    channel: [u8; 32],
    peer: [u8; 32],
    presence_epoch: u64,
    transport: Transport,
}
impl Binding {
    /// ROOT-ONLY input from a completed authenticated/pinned channel and a
    /// native USB attachment or NFC-field epoch. Never deserialize a claimed
    /// peer, field-present Boolean or epoch from a manager packet.
    pub fn from_verified_channel(
        channel: [u8; 32],
        peer: [u8; 32],
        presence_epoch: u64,
        transport: Transport,
    ) -> Result<Self, Error> {
        if channel == [0; 32] || peer == [0; 32] || presence_epoch == 0 {
            return Err(Error::Rejected);
        }
        Ok(Self {
            channel,
            peer,
            presence_epoch,
            transport,
        })
    }
    pub const fn channel(&self) -> [u8; 32] {
        self.channel
    }
    pub fn unlock_request(&self) -> Request {
        let mut h = Sha256::new();
        h.update(b"OpenKeys/PIN-session/unlock/v1");
        h.update(self.peer);
        h.update(self.presence_epoch.to_be_bytes());
        h.update([match self.transport {
            Transport::Usb => 1,
            Transport::Nfc => 2,
        }]);
        Request {
            operation: Operation::UnlockSession,
            digest: h.finalize().into(),
            channel: self.channel,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Protocol {
    FidoPin,
    OpenPgpSign,
    OpenPgpOther,
    OpenPgpAdmin,
    PivPin,
    PivManagement,
}
impl Protocol {
    pub const ALL: [Self; 6] = [
        Self::FidoPin,
        Self::OpenPgpSign,
        Self::OpenPgpOther,
        Self::OpenPgpAdmin,
        Self::PivPin,
        Self::PivManagement,
    ];
}
pub trait Verifier {
    /// Verify the SAME paired authenticated channel and native physical link.
    /// A new NFC field/selection/crypto handshake never inherits an old lease.
    /// Also reject erase-pending, tamper, untrusted boot and hardware faults.
    fn check_link_and_hardware(&mut self, binding: &Binding) -> Result<(), Error>;
    /// Inspect real protocol-library verification state for the matching
    /// application/role. Hardware PIN proof must never set this state.
    fn check_protocol_pin(&mut self, binding: &Binding, protocol: Protocol) -> Result<(), Error>;
    /// Check native per-request permissions too (FIDO RP/permission token,
    /// OpenPGP key role or PIV slot policy). A cached PIN is not blanket access.
    fn check_protocol_request(
        &mut self,
        binding: &Binding,
        protocol: Protocol,
        request: &Request,
    ) -> Result<(), Error>;
    /// Complete exact request/identity review and require a NEW local release.
    /// Must discard held keys across scenes; no host-supplied success flag.
    fn confirm_on_device(&mut self, request: &Request) -> Result<(), Error>;
    /// Clear protocol flags/tokens, active execution scopes, private display
    /// and pending input. This local operation cannot depend on a host ACK.
    fn revoke_all(&mut self);
    fn revoke_protocol(&mut self, protocol: Protocol);
}
struct Grant {
    request: Request,
    protocol: Option<Protocol>,
    issued: u64,
    expires: u64,
    generation: u64,
}
struct Active {
    request: Request,
    protocol: Option<Protocol>,
    until: u64,
    generation: u64,
}
pub struct Gate<V: Verifier> {
    verifier: V,
    binding: Option<Binding>,
    hardware_until: u64,
    protocol_until: [Option<u64>; 6],
    pending: Option<Grant>,
    active: Option<Active>,
    generation: u64,
    last: u64,
    faulted: bool,
}
impl<V: Verifier> Gate<V> {
    pub const fn new(verifier: V) -> Self {
        Self {
            verifier,
            binding: None,
            hardware_until: 0,
            protocol_until: [None; 6],
            pending: None,
            active: None,
            generation: 0,
            last: 0,
            faulted: false,
        }
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    fn bump(&mut self) -> Result<(), Error> {
        match self.generation.checked_add(1) {
            Some(n) if n < u64::MAX => {
                self.generation = n;
                Ok(())
            }
            _ => {
                self.generation = u64::MAX;
                self.faulted = true;
                Err(Error::CounterExhausted)
            }
        }
    }
    pub fn lock(&mut self) {
        self.binding = None;
        self.hardware_until = 0;
        self.protocol_until.fill(None);
        self.pending = None;
        self.active = None;
        self.verifier.revoke_all();
        let _ = self.bump();
    }
    fn time(&mut self, now: u64) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::HardwareFault);
        }
        if now < self.last {
            self.lock();
            self.faulted = true;
            return Err(Error::TimeWentBackwards);
        }
        self.last = now;
        Ok(())
    }
    fn deadline(now: u64) -> Result<u64, Error> {
        now.checked_add(IDLE_MS).ok_or(Error::CounterExhausted)
    }
    /// A fresh local six-digit PIN is bound to this exact paired connection.
    /// Cached protocol PINs are always revoked when creating a new HW lease.
    /// Root must call lock BEFORE starting local PIN entry, including retries.
    pub fn unlock(
        &mut self,
        pin: VerifiedPinRequest,
        binding: Binding,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(), Error> {
        self.lock();
        self.time(clock())?;
        let request = binding.unlock_request();
        if pin.request() != &request {
            return Err(Error::WrongRequest);
        }
        if self.last >= pin.expires_ms() {
            return Err(Error::Expired);
        }
        self.verifier.check_link_and_hardware(&binding)?;
        self.time(clock())?;
        if self.last >= pin.expires_ms() {
            return Err(Error::Expired);
        }
        self.verifier.confirm_on_device(&request)?;
        self.time(clock())?;
        if self.last >= pin.expires_ms() {
            return Err(Error::Expired);
        }
        self.verifier.check_link_and_hardware(&binding)?;
        self.time(clock())?;
        if self.last >= pin.expires_ms() {
            return Err(Error::Expired);
        }
        self.hardware_until = Self::deadline(self.last)?;
        self.binding = Some(binding);
        Ok(())
    }
    /// Observing traffic/link state never changes an idle deadline. Root calls
    /// this even with no traffic. An already approved bounded operation may
    /// finish within its fixed Execution budget; no other operation can start.
    pub fn tick(&mut self, binding: &Binding, mut clock: impl FnMut() -> u64) -> Result<(), Error> {
        self.time(clock())?;
        if self.binding.as_ref() != Some(binding) {
            return Err(Error::WrongRequest);
        }
        if self.verifier.check_link_and_hardware(binding).is_err() {
            self.lock();
            return Err(Error::HardwareFault);
        }
        self.time(clock())?;
        let until = self
            .active
            .as_ref()
            .map_or(self.hardware_until, |a| a.until);
        if self.last >= until {
            self.lock();
            return Err(Error::Expired);
        }
        for p in Protocol::ALL {
            let in_use = self.active.as_ref().is_some_and(|a| a.protocol == Some(p));
            if !in_use && self.protocol_until[p as usize].is_some_and(|t| self.last >= t) {
                self.protocol_until[p as usize] = None;
                self.verifier.revoke_protocol(p);
            }
        }
        Ok(())
    }
    /// ROOT event for failed/replaced protocol verification. Revoke the native
    /// library state as well as our lease; cancel any operation using that role.
    pub fn protocol_failed(&mut self, protocol: Protocol) {
        if self
            .pending
            .as_ref()
            .is_some_and(|g| g.protocol == Some(protocol))
            || self
                .active
                .as_ref()
                .is_some_and(|a| a.protocol == Some(protocol))
        {
            self.lock();
        } else {
            self.protocol_until[protocol as usize] = None;
            self.verifier.revoke_protocol(protocol);
        }
    }
    /// ROOT event after successful independent protocol verification. This
    /// checks the library's actual flag; the call itself never refreshes HW PIN.
    pub fn note_protocol_verified(
        &mut self,
        binding: &Binding,
        protocol: Protocol,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(), Error> {
        if self.pending.is_some() || self.active.is_some() {
            return Err(Error::Rejected);
        }
        self.tick(binding, &mut clock)?;
        self.verifier.check_protocol_pin(binding, protocol)?;
        self.time(clock())?;
        if self.last >= self.hardware_until {
            return Err(Error::Expired);
        }
        self.protocol_until[protocol as usize] = Some(Self::deadline(self.last)?);
        Ok(())
    }
    fn protocol_matches(operation: Operation, protocol: Option<Protocol>) -> bool {
        match operation {
            Operation::FidoRegister | Operation::FidoSign | Operation::DeriveHmacSecret => {
                protocol == Some(Protocol::FidoPin)
            }
            Operation::OpenPgp => matches!(
                protocol,
                Some(Protocol::OpenPgpSign | Protocol::OpenPgpOther | Protocol::OpenPgpAdmin)
            ),
            Operation::Piv => matches!(protocol, Some(Protocol::PivPin | Protocol::PivManagement)),
            _ => true,
        }
    }
    pub fn approve(
        &mut self,
        request: Request,
        protocol: Option<Protocol>,
        binding: &Binding,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(), Error> {
        if self.pending.is_some() || self.active.is_some() {
            return Err(Error::Rejected);
        }
        if matches!(
            request.operation,
            Operation::EnrollFingerprint | Operation::DeleteFingerprint | Operation::UnlockSession
        ) || !Self::protocol_matches(request.operation, protocol)
        {
            return Err(Error::Rejected);
        }
        self.tick(binding, &mut clock)?;
        if request.channel != binding.channel {
            return Err(Error::WrongRequest);
        }
        if let Some(p) = protocol {
            if self.protocol_until[p as usize].is_none_or(|until| self.last >= until) {
                return Err(Error::Locked);
            }
            self.verifier.check_protocol_pin(binding, p)?;
            self.verifier.check_protocol_request(binding, p, &request)?;
        }
        self.verifier.confirm_on_device(&request)?;
        self.tick(binding, &mut clock)?;
        if let Some(p) = protocol {
            if self.protocol_until[p as usize].is_none_or(|until| self.last >= until) {
                return Err(Error::Locked);
            }
            self.verifier.check_protocol_pin(binding, p)?;
            self.verifier.check_protocol_request(binding, p, &request)?;
        }
        if self.bump().is_err() {
            self.lock();
            return Err(Error::CounterExhausted);
        }
        // Confirmation authorizes this exact request but does not renew an
        // idle lease. Only completed successful output/commit may renew it.
        // Otherwise confirmed-but-never-executed requests could keep it alive.
        self.pending = Some(Grant {
            request,
            protocol,
            issued: self.last,
            expires: self.hardware_until,
            generation: self.generation,
        });
        Ok(())
    }
    pub fn consume_authorized(
        &mut self,
        request: &Request,
        binding: &Binding,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Authorization, Error> {
        self.tick(binding, &mut clock)?;
        if let Some(p) = self.pending.as_ref().and_then(|g| g.protocol) {
            if self.protocol_until[p as usize].is_none_or(|until| self.last >= until) {
                self.lock();
                return Err(Error::Locked);
            }
            self.verifier.check_protocol_pin(binding, p)?;
            self.verifier.check_protocol_request(binding, p, request)?;
            self.tick(binding, &mut clock)?;
            if self.protocol_until[p as usize].is_none_or(|until| self.last >= until) {
                self.lock();
                return Err(Error::Locked);
            }
        }
        let grant = self.pending.take().ok_or(Error::Locked)?;
        if grant.request != *request
            || grant.generation != self.generation
            || self.last >= grant.expires
        {
            self.lock();
            return Err(Error::WrongRequest);
        }
        let until = self
            .last
            .checked_add(request.operation.execution_budget_ms())
            .ok_or(Error::CounterExhausted)?;
        self.active = Some(Active {
            request: *request,
            protocol: grant.protocol,
            until,
            generation: grant.generation,
        });
        Ok(Authorization {
            request: *request,
            issued: grant.issued,
            expires: grant.expires,
            generation: grant.generation,
            execution_deadline: Some(until),
        })
    }
    /// The executor must call Execution::finish only after successful protected
    /// output/commit. Error/cancel paths call lock; they never renew a lease.
    pub fn completed(
        &mut self,
        completed: Completed,
        binding: &Binding,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(), Error> {
        self.tick(binding, &mut clock)?;
        let active = self.active.take().ok_or(Error::Locked)?;
        if completed.authorization.request() != &active.request
            || completed.authorization.generation() != active.generation
            || completed.finished_ms > self.last
            || completed.finished_ms >= active.until
        {
            self.lock();
            return Err(Error::WrongRequest);
        }
        self.hardware_until = Self::deadline(self.last)?;
        if let Some(p) = active.protocol {
            self.protocol_until[p as usize] = Some(self.hardware_until)
        }
        Ok(())
    }
}
impl<V: Verifier> State for Gate<V> {
    fn generation(&self) -> u64 {
        self.generation
    }
    fn check_security_state(&mut self) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::HardwareFault);
        }
        let binding = self.binding.ok_or(Error::Locked)?;
        if self.verifier.check_link_and_hardware(&binding).is_err() {
            self.lock();
            return Err(Error::HardwareFault);
        }
        let active = self.active.as_ref().ok_or(Error::Locked)?;
        if let Some(p) = active.protocol
            && (self.verifier.check_protocol_pin(&binding, p).is_err()
                || self
                    .verifier
                    .check_protocol_request(&binding, p, &active.request)
                    .is_err())
        {
            self.lock();
            return Err(Error::Rejected);
        }
        Ok(())
    }
}
impl<V: Verifier> Drop for Gate<V> {
    fn drop(&mut self) {
        self.lock();
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
