//! Connects the real native-PIN installer to authenticated lifecycle phases.
//! The caller still supplies protected MCU flash, hardware MAC and boot/scene
//! guard. It is not a production hardware implementation of those interfaces.
use crate::{Error, Guard, Journal, Mac, Phase, State, Storage};
use openkeys_se_state::{Error as SeError, commissioning::Root};
use subtle::ConstantTimeEq;

/// Exclusive setup-scene borrow. It cannot be used as a normal private-operation
/// State, PIN lease, or host-supplied blank assertion.
pub struct PinRoot<'a, S: Storage, M: Mac, G: Guard> {
    journal: &'a mut Journal<S, M>,
    guard: &'a mut G,
}
impl<'a, S: Storage, M: Mac, G: Guard> PinRoot<'a, S, M, G> {
    pub fn new(journal: &'a mut Journal<S, M>, guard: &'a mut G) -> Self {
        Self { journal, guard }
    }
    fn state(&mut self) -> Result<State, SeError> {
        self.guard.check().map_err(map)?;
        let state = self.journal.state().map_err(map)?;
        if !matches!(
            state.phase,
            Phase::FactoryBlank | Phase::PinInstalling | Phase::PinInstalled
        ) {
            return Err(SeError::Authorization);
        }
        Ok(state)
    }
}
impl<S: Storage, M: Mac, G: Guard> Root for PinRoot<'_, S, M, G> {
    fn check(&mut self) -> Result<(), SeError> {
        self.state().map(|_| ())
    }
    fn qualified_blank(&mut self) -> Result<(), SeError> {
        if self.state()?.phase == Phase::FactoryBlank {
            Ok(())
        } else {
            Err(SeError::Record)
        }
    }
    fn begin_pin_install(
        &mut self,
        profile: &[u8; 32],
        inventory: &[u8; 32],
    ) -> Result<(), SeError> {
        self.qualified_blank()?;
        if *inventory == [0; 32] || !bool::from(profile.ct_eq(&self.journal.context.profile)) {
            return Err(SeError::Schema);
        }
        self.journal
            .advance(Phase::PinInstalling, self.guard)
            .map_err(map)?;
        self.check()
    }
    fn owner_nonce(&mut self) -> Result<[u8; 32], SeError> {
        let state = self.state()?;
        if state.phase != Phase::PinInstalling {
            return Err(SeError::Authorization);
        }
        Ok(state.owner)
    }
    fn commit_pin_namespace(&mut self) -> Result<(), SeError> {
        if self.state()?.phase != Phase::PinInstalling {
            return Err(SeError::Record);
        }
        self.journal
            .advance(Phase::PinInstalled, self.guard)
            .map_err(map)?;
        self.check()
    }
}
fn map(error: Error) -> SeError {
    match error {
        Error::Authorization => SeError::Authorization,
        Error::Closed => SeError::Closed,
        Error::Exhausted => SeError::Exhausted,
        Error::Io | Error::Crypto => SeError::Channel,
        _ => SeError::Record,
    }
}
