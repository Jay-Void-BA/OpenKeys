use crate::{
    Error, Exchange,
    objects::{Counter, File, HEAD_BYTES, Objects},
    record,
};
use core::cell::{Cell, RefCell};
use device_policy::{
    Request,
    biometric::{AuthenticatedMatch, Error as BioError, ProtectedAttempts},
};
use device_vault::{Error as VaultError, Head, Ledger};

/// Fixed engineering policy: eight consecutive persistent failures block
/// biometric attempts. Power cycle/PIN never resets this. A successful match
/// credits only its own precharged attempt; changing this policy/recovery
/// requires separately reviewed commissioning and availability assessment.
pub const BIO_MAX_FAILURES: u64 = 8;

struct PendingMatch {
    request: Request,
    tag: u16,
    spent: u64,
}

pub struct State<X: Exchange> {
    objects: Objects<X>,
    head: Option<Head>,
    sequence: u64,
    initialized: bool,
    reserved: Option<u64>,
    pending: Option<PendingMatch>,
    faulted: bool,
}
impl<X: Exchange> State<X> {
    /// Existing commissioned namespace only. No missing-object initialization
    /// or fallback to default/factory keys, transient objects or normal Flash.
    pub fn open(exchange: X) -> Result<Self, Error> {
        let mut this = Self {
            objects: Objects::open(exchange)?,
            head: None,
            sequence: 0,
            initialized: false,
            reserved: None,
            pending: None,
            faulted: false,
        };
        this.run(|state| {
            state.active_inner()?;
            let spent = state.objects.read_counter(Counter::BioSpent)?;
            let credit = state.objects.read_counter(Counter::BioCredit)?;
            let tag = state.objects.read_counter(Counter::BioTag)?;
            if credit > spent
                || spent - credit > BIO_MAX_FAILURES
                || tag > u64::from(u16::MAX)
                || tag > spent
                || (credit > 0 && tag == 0)
            {
                return Err(Error::Counter);
            }
            Ok(())
        })?;
        Ok(this)
    }
    fn run<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if self.faulted {
            return Err(Error::Closed);
        }
        let result = operation(self);
        if result.is_err() {
            self.faulted = true;
            self.pending = None;
            self.reserved = None;
            self.objects.close();
        }
        result
    }
    fn active_inner(&mut self) -> Result<Option<Head>, Error> {
        let sequence = self.objects.read_counter(Counter::VaultCommit)?;
        if self.initialized && sequence != self.sequence {
            return Err(Error::Counter);
        }
        let mut bytes = [0; HEAD_BYTES];
        self.objects
            .read_file(File::for_sequence(sequence), &mut bytes)?;
        let head = record::decode(&bytes, sequence)?;
        let nonce = self.objects.read_counter(Counter::VaultNonce)?;
        if head.is_some_and(|head| head.epoch > nonce)
            || self.objects.read_counter(Counter::VaultCommit)? != sequence
        {
            return Err(Error::Counter);
        }
        if self.initialized && self.head != head {
            return Err(Error::Record);
        }
        self.head = head;
        self.sequence = sequence;
        self.initialized = true;
        Ok(head)
    }
    fn reserve_inner(&mut self) -> Result<u64, Error> {
        self.reserved = None;
        self.active_inner()?;
        if self.sequence == u64::MAX {
            return Err(Error::Exhausted);
        }
        let previous = self.objects.read_counter(Counter::VaultNonce)?;
        let next = self.objects.increment(Counter::VaultNonce, previous)?;
        self.reserved = Some(next);
        Ok(next)
    }
    fn commit_inner(&mut self, head: &Head) -> Result<(), Error> {
        let reserved = self.reserved.take().ok_or(Error::Counter)?;
        self.active_inner()?;
        if !record::valid(head)
            || head.epoch != reserved
            || head.epoch <= self.head.map_or(0, |head| head.epoch)
            || head.bank != self.head.map_or(0, |head| 1 - head.bank)
            || self.objects.read_counter(Counter::VaultNonce)? != reserved
        {
            return Err(Error::Record);
        }
        let next = self.sequence.checked_add(1).ok_or(Error::Exhausted)?;
        let bytes = record::encode(next, Some(head))?;
        self.objects.write_head(File::for_sequence(next), &bytes)?;
        // This increment is the linearization point. If its acknowledgement
        // is lost, close this owner; the next boot reads the real anchor.
        self.objects
            .increment(Counter::VaultCommit, self.sequence)?;
        self.head = Some(*head);
        self.sequence = next;
        self.active_inner()?;
        Ok(())
    }
    fn reserve_match_inner(&mut self, request: &Request) -> Result<u32, Error> {
        self.pending = None;
        let spent = self.objects.read_counter(Counter::BioSpent)?;
        let credit = self.objects.read_counter(Counter::BioCredit)?;
        let tag = self.objects.read_counter(Counter::BioTag)?;
        if credit > spent {
            return Err(Error::Counter);
        }
        if spent - credit >= BIO_MAX_FAILURES || tag >= u64::from(u16::MAX) {
            return Err(Error::Exhausted);
        }
        // Persist debit BEFORE tag reservation. Neither a reset, cancellation
        // nor an uncertain reply can refund the debit or the lifetime tag.
        let spent = self.objects.increment(Counter::BioSpent, spent)?;
        let tag = self.objects.increment(Counter::BioTag, tag)?;
        let tag = u16::try_from(tag).map_err(|_| Error::Exhausted)?;
        self.pending = Some(PendingMatch {
            request: *request,
            tag,
            spent,
        });
        Ok(u32::from(tag))
    }
    fn match_inner(&mut self, proof: &AuthenticatedMatch) -> Result<(), Error> {
        let pending = self.pending.take().ok_or(Error::Counter)?;
        if proof.request() != &pending.request
            || proof.tag() != pending.tag
            || self.objects.read_counter(Counter::BioTag)? != u64::from(pending.tag)
            || self.objects.read_counter(Counter::BioSpent)? != pending.spent
        {
            return Err(Error::Counter);
        }
        let credit = self.objects.read_counter(Counter::BioCredit)?;
        self.objects.credit_match(credit, pending.spent)
    }
}
fn vault(error: Error) -> VaultError {
    match error {
        Error::Authorization => VaultError::Authorization,
        Error::Closed => VaultError::Faulted,
        Error::Counter => VaultError::Rollback,
        _ => VaultError::Ledger,
    }
}
fn biometric(error: Error) -> BioError {
    match error {
        Error::Exhausted => BioError::Exhausted,
        Error::Closed => BioError::RecoveryRequired,
        Error::Counter => BioError::InvalidReservation,
        _ => BioError::Storage,
    }
}
impl<X: Exchange> Ledger for State<X> {
    fn active(&mut self) -> Result<Option<Head>, VaultError> {
        self.run(|s| s.active_inner()).map_err(vault)
    }
    fn reserve_epoch(&mut self) -> Result<u64, VaultError> {
        self.run(|s| s.reserve_inner()).map_err(vault)
    }
    fn commit(&mut self, head: &Head) -> Result<(), VaultError> {
        self.run(|s| s.commit_inner(head)).map_err(vault)
    }
}
impl<X: Exchange> ProtectedAttempts for State<X> {
    fn reserve_identify_attempt(&mut self, request: &Request) -> Result<u32, BioError> {
        self.run(|s| s.reserve_match_inner(request))
            .map_err(biometric)
    }
    fn record_match(&mut self, proof: &AuthenticatedMatch) -> Result<(), BioError> {
        self.run(|s| s.match_inner(proof)).map_err(biometric)
    }
}

/// Shared protected service, not duplicate SE owners. Short exclusive borrows
/// serialize the vault/biometric adapters; reentry fails instead of panicking.
/// Actual transport/clock/crypto/guard ownership still belongs to the runtime.
pub struct Service<X: Exchange> {
    state: RefCell<State<X>>,
    poisoned: Cell<bool>,
}
impl<X: Exchange> Service<X> {
    pub fn new(state: State<X>) -> Self {
        Self {
            state: RefCell::new(state),
            poisoned: Cell::new(false),
        }
    }
    pub fn handle(&self) -> StateRef<'_, X> {
        StateRef { service: self }
    }
}
pub struct StateRef<'a, X: Exchange> {
    service: &'a Service<X>,
}
impl<X: Exchange> Copy for StateRef<'_, X> {}
impl<X: Exchange> Clone for StateRef<'_, X> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, X: Exchange> StateRef<'a, X> {
    fn with<T>(&self, f: impl FnOnce(&mut State<X>) -> Result<T, Error>) -> Result<T, Error> {
        if self.service.poisoned.get() {
            return Err(Error::Closed);
        }
        let Ok(mut state) = self.service.state.try_borrow_mut() else {
            self.service.poisoned.set(true);
            return Err(Error::Borrow);
        };
        let result = state.run(f);
        // A reentrant callback may have poisoned the service while the outer
        // operation held its borrow. It must not release an otherwise-Ok proof.
        if self.service.poisoned.get() {
            state.faulted = true;
            state.objects.close();
            return Err(Error::Closed);
        }
        result
    }
}

impl<X: Exchange> Ledger for StateRef<'_, X> {
    fn active(&mut self) -> Result<Option<Head>, VaultError> {
        self.with(|s| s.active_inner()).map_err(vault)
    }
    fn reserve_epoch(&mut self) -> Result<u64, VaultError> {
        self.with(|s| s.reserve_inner()).map_err(vault)
    }
    fn commit(&mut self, head: &Head) -> Result<(), VaultError> {
        self.with(|s| s.commit_inner(head)).map_err(vault)
    }
}
impl<X: Exchange> ProtectedAttempts for StateRef<'_, X> {
    fn reserve_identify_attempt(&mut self, request: &Request) -> Result<u32, BioError> {
        self.with(|s| s.reserve_match_inner(request))
            .map_err(biometric)
    }
    fn record_match(&mut self, proof: &AuthenticatedMatch) -> Result<(), BioError> {
        self.with(|s| s.match_inner(proof)).map_err(biometric)
    }
}

#[cfg(test)]
mod borrow_tests {
    use super::*;
    #[test]
    fn reentry_poison_is_shared_and_cannot_be_cleared_by_dropping_the_borrow() {
        let (exchange, _) = crate::tests::model();
        let service = Service::new(State::open(exchange).unwrap());
        let mut first = service.handle();
        let mut second = service.handle();
        let held = service.state.borrow_mut();
        assert_eq!(first.reserve_epoch(), Err(VaultError::Ledger));
        drop(held);
        assert_eq!(second.active(), Err(VaultError::Faulted));
        assert_eq!(
            first.reserve_identify_attempt(&crate::tests::request()),
            Err(BioError::RecoveryRequired)
        );
    }
}
