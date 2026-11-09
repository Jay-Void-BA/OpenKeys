extern crate std;
use super::*;
use std::{cell::Cell, rc::Rc, vec};
#[path = "../../../device-records/test-support/media.rs"]
mod media;
use media::*;

struct Counter(Rc<Cell<u32>>);
impl SignatureCounter for Counter {
    fn current(&mut self) -> CtapResult<u32> {
        Ok(self.0.get())
    }
    fn advance(&mut self, n: u32) -> CtapResult<()> {
        if !(1..=8).contains(&n) {
            return Err(INTERNAL);
        }
        self.0.set(self.0.get().checked_add(n).ok_or(INTERNAL)?);
        Ok(())
    }
}
struct Live(Rc<Cell<bool>>);
impl Checkpoint for Live {
    fn check(&mut self) -> Result<(), device_vault::Error> {
        if self.0.get() {
            Ok(())
        } else {
            Err(device_vault::Error::Authorization)
        }
    }
}
type Adapter = RecordPersist<Flash, Se, Crypto, Live, Counter>;
fn profile() -> Profile {
    Profile {
        marker: [0x33; 32],
        aaguid: [0x57; 16],
    }
}
fn adapter(state: &Shared, live: &Rc<Cell<bool>>, counter: &Rc<Cell<u32>>) -> Adapter {
    RecordPersist::new(
        Records::new(vault(state)),
        Live(live.clone()),
        Counter(counter.clone()),
        profile(),
    )
}
fn commissioned(state: &Shared) {
    let mut records = Records::new(vault(state));
    records.put_fido(0, &profile().marker, &mut Allow).unwrap();
    records
        .put_fido(keys::AAGUID as u16, &profile().aaguid, &mut Allow)
        .unwrap();
}

#[test]
fn real_encrypted_records_back_fido_find_insert_empty_flags_remove_and_iteration() {
    let state = Shared::default();
    commissioned(&state);
    let live = Rc::new(Cell::new(true));
    let counter = Rc::new(Cell::new(0));
    let mut p = adapter(&state, &live, &counter);
    p.init().unwrap();
    p.insert(40, b"credential").unwrap();
    p.insert(41, &[]).unwrap();
    assert_eq!(p.find(40).unwrap(), Some(b"credential".to_vec()));
    assert_eq!(p.find(41).unwrap(), Some(vec![]));
    let ids = p.iter().unwrap().collect::<CtapResult<Vec<_>>>().unwrap();
    assert_eq!(ids, vec![keys::AAGUID, 40, 41]);
    p.remove(40).unwrap();
    p.lock();
    let mut reboot = adapter(&state, &live, &counter);
    reboot.init().unwrap();
    assert!(reboot.find(40).unwrap().is_none());
    assert_eq!(reboot.find(41).unwrap(), Some(vec![]));
}
#[test]
fn missing_wrong_or_torn_profile_is_never_initialized_or_recreated() {
    for mode in 0..4 {
        let state = Shared::default();
        if mode > 0 {
            commissioned(&state);
            let mut records = Records::new(vault(&state));
            match mode {
                1 => records.put_fido(0, &[0; 32], &mut Allow).unwrap(),
                2 => records
                    .put_fido(keys::AAGUID as u16, &[0; 16], &mut Allow)
                    .unwrap(),
                _ => records
                    .put_fido(keys::RESET_COMPLETION as u16, &[], &mut Allow)
                    .unwrap(),
            }
        }
        let before = state.borrow().epoch;
        let mut p = adapter(&state, &Rc::new(Cell::new(true)), &Rc::new(Cell::new(0)));
        assert!(p.init().is_err());
        assert_eq!(state.borrow().epoch, before);
        assert!(p.find(40).is_err());
    }
}
#[test]
fn signature_counter_uses_se_adapter_and_does_not_rewrite_nor_or_other_namespaces() {
    let state = Shared::default();
    commissioned(&state);
    let live = Rc::new(Cell::new(true));
    let counter = Rc::new(Cell::new(19));
    let mut p = adapter(&state, &live, &counter);
    p.init().unwrap();
    let epoch = state.borrow().epoch;
    assert_eq!(p.global_signature_counter().unwrap(), 19);
    p.incr_global_signature_counter(8).unwrap();
    assert_eq!(counter.get(), 27);
    assert_eq!(state.borrow().epoch, epoch);
    for key in [
        0,
        keys::AAGUID,
        keys::GLOBAL_SIGNATURE_COUNTER,
        keys::RESET_COMPLETION,
    ] {
        assert!(p.insert(key, b"x").is_err());
        assert!(p.remove(key).is_err());
    }
    assert!(p.reset().is_err());
    assert_eq!(state.borrow().epoch, epoch);
}
#[test]
fn loss_of_live_scope_poison_and_reentry_cannot_release_cached_private_values() {
    let state = Shared::default();
    commissioned(&state);
    let live = Rc::new(Cell::new(true));
    let counter = Rc::new(Cell::new(0));
    let mut p = adapter(&state, &live, &counter);
    p.init().unwrap();
    p.insert(40, b"private metadata").unwrap();
    live.set(false);
    assert!(p.find(40).is_err());
    live.set(true);
    p.lock();
    assert!(p.find(40).is_err());
    let p = adapter(&state, &live, &counter);
    assert!(
        p.with(|_| {
            assert!(p.find(40).is_err());
            Ok(())
        })
        .is_err()
    );
    assert!(p.find(40).is_err());
    // A denied caller does not decrypt any sector before the first checkpoint.
    let p = RecordPersist::new(
        Records::new(vault(&state)),
        Budget(0),
        Counter(counter),
        profile(),
    );
    let opens = state.borrow().opens;
    assert!(p.find(40).is_err());
    assert_eq!(state.borrow().opens, opens);
}
