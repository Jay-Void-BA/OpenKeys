//! Borrow the authenticated Ready lifecycle and real backup continuity for
//! every private APDU. Protocol/link/lease/slot checks remain independently
//! mandatory; this adapter does not create a PIN lease or an Execution.
use crate::{ContinuityGuard, Error, Journal, Mac, Phase, State, Storage};
use device_policy::{Error as PolicyError, Request, execution::State as ExecutionState};
use openkeys_se_state::native_pin::PrivateState;
use subtle::ConstantTimeEq;

/// An exclusive guarded view. The immutable journal borrow prevents ordinary
/// lifecycle transitions while private work is in progress. Drop it and revoke
/// the native SE/transport sessions before entering maintenance or erase.
/// The underlying request checker has no default or host-Boolean substitute.
/// A boot-only journal guard cannot substitute for backup continuity:
///
/// ```compile_fail
/// use openkeys_root_journal::{Guard, Journal, Mac, Storage, private_state::PrivateRoot};
/// use openkeys_se_state::native_pin::PrivateState;
/// fn cannot_skip_continuity<S: Storage, M: Mac, G: Guard, T: PrivateState>(
///     journal: &Journal<S, M>, guard: &mut G, requests: &mut T,
/// ) {
///     let _ = PrivateRoot::new(journal, guard, requests);
/// }
/// ```
pub struct PrivateRoot<'a, S: Storage, M: Mac, G: ContinuityGuard, T: PrivateState> {
    journal: &'a Journal<S, M>,
    guard: &'a mut G,
    inner: &'a mut T,
    selected: State,
    closed: bool,
}
impl<'a, S: Storage, M: Mac, G: ContinuityGuard, T: PrivateState> PrivateRoot<'a, S, M, G, T> {
    pub fn new(
        journal: &'a Journal<S, M>,
        guard: &'a mut G,
        inner: &'a mut T,
    ) -> Result<Self, Error> {
        let selected = journal.state()?;
        let mut this = Self {
            journal,
            guard,
            inner,
            selected,
            closed: false,
        };
        this.check_security_state()
            .map_err(|_| Error::Authorization)?;
        Ok(this)
    }

    fn selected_ready(&mut self) -> Result<(), Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        self.guard.check()?;
        let current = self.journal.state()?;
        if current != self.selected || current.phase != Phase::Ready {
            return Err(Error::Authorization);
        }
        self.guard.verify_backup_owner(&current.owner)?;
        self.guard.check()
    }

    fn finish(&mut self, result: Result<(), PolicyError>) -> Result<(), PolicyError> {
        if result.is_err() {
            self.closed = true;
        }
        result
    }
}
impl<S: Storage, M: Mac, G: ContinuityGuard, T: PrivateState> ExecutionState
    for PrivateRoot<'_, S, M, G, T>
{
    fn generation(&self) -> u64 {
        if self.closed {
            u64::MAX
        } else {
            self.inner.generation()
        }
    }

    fn check_security_state(&mut self) -> Result<(), PolicyError> {
        let result = (|| {
            self.selected_ready()
                .map_err(|_| PolicyError::HardwareFault)?;
            self.inner.check_security_state()?;
            self.selected_ready()
                .map_err(|_| PolicyError::HardwareFault)
        })();
        self.finish(result)
    }
}
impl<S: Storage, M: Mac, G: ContinuityGuard, T: PrivateState> PrivateState
    for PrivateRoot<'_, S, M, G, T>
{
    fn check_private_request(
        &mut self,
        request: &Request,
        owner: &[u8; 32],
    ) -> Result<(), PolicyError> {
        let result = (|| {
            self.selected_ready()
                .map_err(|_| PolicyError::HardwareFault)?;
            if !bool::from(self.selected.owner.ct_eq(owner)) {
                return Err(PolicyError::Rejected);
            }
            self.inner.check_private_request(request, owner)?;
            self.selected_ready()
                .map_err(|_| PolicyError::HardwareFault)
        })();
        self.finish(result)
    }
}
