//! Actual OpenSK logical persistence over the R04 encrypted record engine.
//! No plaintext Flash/RAM fallback, permissive guard, implicit commissioning or
//! incremental factory reset. The live checkpoint is owned by the root scope.
use alloc::{boxed::Box, vec::Vec};
use core::cell::{Cell, RefCell};
use device_records::{Id, Namespace, Records};
use device_vault::{Aead, Checkpoint, CiphertextStorage, Ledger};
use opensk::{
    api::persist::{Persist, PersistIter, keys},
    ctap::status_code::{Ctap2StatusCode, CtapResult},
};
use zeroize::Zeroize;

const INTERNAL: Ctap2StatusCode = Ctap2StatusCode::CTAP2_ERR_VENDOR_INTERNAL_ERROR;

/// A dedicated rollback-resistant SE counter avoids rewriting all notes/NOR
/// sectors on every FIDO signature. Errors/ambiguous increments never refund.
pub trait SignatureCounter {
    fn current(&mut self) -> CtapResult<u32>;
    fn advance(&mut self, increment: u32) -> CtapResult<()>;
}
impl<X: openkeys_se_state::Exchange> SignatureCounter for openkeys_se_state::StateRef<'_, X> {
    fn current(&mut self) -> CtapResult<u32> {
        self.signature_counter().map_err(|_| INTERNAL)
    }
    fn advance(&mut self, increment: u32) -> CtapResult<()> {
        self.advance_signature_counter(increment)
            .map_err(|_| INTERNAL)
    }
}
/// Public commissioning values, not keys. Key0 is reserved by upstream OpenSK
/// and carries the exact provisioned root profile marker in this integration.
pub struct Profile {
    pub marker: [u8; 32],
    pub aaguid: [u8; 16],
}
struct Owner<S, L, C, A, T> {
    records: Records<S, L, C>,
    authorization: A,
    counter: T,
}
pub struct RecordPersist<S, L, C, A, T> {
    owner: RefCell<Owner<S, L, C, A, T>>,
    poisoned: Cell<bool>,
    profile: Profile,
}
impl<S: CiphertextStorage, L: Ledger, C: Aead, A: Checkpoint, T: SignatureCounter>
    RecordPersist<S, L, C, A, T>
{
    pub fn new(records: Records<S, L, C>, authorization: A, counter: T, profile: Profile) -> Self {
        Self {
            owner: RefCell::new(Owner {
                records,
                authorization,
                counter,
            }),
            poisoned: Cell::new(false),
            profile,
        }
    }
    fn with<R: Zeroize>(
        &self,
        f: impl FnOnce(&mut Owner<S, L, C, A, T>) -> CtapResult<R>,
    ) -> CtapResult<R> {
        if self.poisoned.get() {
            return Err(INTERNAL);
        }
        let Ok(mut owner) = self.owner.try_borrow_mut() else {
            self.poisoned.set(true);
            return Err(INTERNAL);
        };
        let mut result = (|| {
            owner.authorization.check().map_err(|_| INTERNAL)?;
            let mut result = f(&mut owner)?;
            if owner.authorization.check().is_err() {
                result.zeroize();
                return Err(INTERNAL);
            }
            Ok(result)
        })();
        if result.is_err() || self.poisoned.get() {
            if let Ok(value) = &mut result {
                value.zeroize();
            }
            self.poisoned.set(true);
            owner.records.lock();
            return Err(INTERNAL);
        }
        result
    }
    /// Root invokes this when ending any scope. A poisoned owner cannot be
    /// restored by dropping cached metadata or starting a new host connection.
    pub fn lock(&self) {
        if let Ok(mut o) = self.owner.try_borrow_mut() {
            o.records.lock();
        } else {
            self.poisoned.set(true);
        }
    }
    fn key(key: usize) -> CtapResult<u16> {
        let key = u16::try_from(key).map_err(|_| INTERNAL)?;
        if key >= 4096 { Err(INTERNAL) } else { Ok(key) }
    }
}
impl<S: CiphertextStorage, L: Ledger, C: Aead, A: Checkpoint, T: SignatureCounter> Persist
    for RecordPersist<S, L, C, A, T>
{
    fn find(&self, key: usize) -> CtapResult<Option<Vec<u8>>> {
        let key = Self::key(key)?;
        self.with(|o| {
            o.records
                .get_fido(key, &mut o.authorization)
                .map(|value| value.map(|mut b| core::mem::take(&mut *b)))
                .map_err(|_| INTERNAL)
        })
    }
    fn insert(&mut self, key: usize, value: &[u8]) -> CtapResult<()> {
        // AAGUID/profile are commissioned separately. Signature-counter writes
        // must use the dedicated SE counter, not create a second authority.
        if matches!(
            key,
            0 | keys::AAGUID | keys::GLOBAL_SIGNATURE_COUNTER | keys::RESET_COMPLETION
        ) {
            return Err(INTERNAL);
        }
        let key = Self::key(key)?;
        self.with(|o| {
            o.records
                .put_fido(key, value, &mut o.authorization)
                .map_err(|_| INTERNAL)
        })
    }
    fn remove(&mut self, key: usize) -> CtapResult<()> {
        if matches!(
            key,
            0 | keys::AAGUID | keys::GLOBAL_SIGNATURE_COUNTER | keys::RESET_COMPLETION
        ) {
            return Err(INTERNAL);
        }
        let key = Self::key(key)?;
        self.with(|o| {
            o.records
                .remove(
                    Id {
                        namespace: Namespace::Fido,
                        key,
                    },
                    &mut o.authorization,
                )
                .map_err(|_| INTERNAL)
        })
    }
    fn iter(&self) -> CtapResult<PersistIter<'_>> {
        let ids = self.with(|o| {
            o.records
                .ids(Namespace::Fido, &mut o.authorization)
                .map_err(|_| INTERNAL)
        })?;
        // Key0 is integration metadata and never an OpenSK reset/credential key.
        Ok(Box::new(
            ids.into_iter().filter(|k| *k != 0).map(|k| Ok(k as usize)),
        ))
    }
    fn init(&mut self) -> CtapResult<()> {
        // A corrupt/missing provisioned marker or torn reset MUST NOT become
        // an initialized empty authenticator. No key creation is done here.
        let valid = self.find(0)?.is_some_and(|b| b == self.profile.marker)
            && self
                .find(keys::AAGUID)?
                .is_some_and(|b| b == self.profile.aaguid)
            && self.find(keys::RESET_COMPLETION)?.is_none();
        if !valid {
            self.poisoned.set(true);
            return Err(INTERNAL);
        }
        self.with(|o| o.counter.current().map(drop))
    }
    fn global_signature_counter(&self) -> CtapResult<u32> {
        self.with(|o| o.counter.current())
    }
    fn incr_global_signature_counter(&mut self, increment: u32) -> CtapResult<()> {
        self.with(|o| o.counter.advance(increment))
    }
    fn reset(&mut self) -> CtapResult<()> {
        // Full sensor/SE/vault reset belongs to the root's durable global
        // lifecycle transaction; OpenSK's per-key loop cannot perform it.
        Err(INTERNAL)
    }
}

#[cfg(test)]
mod tests;
