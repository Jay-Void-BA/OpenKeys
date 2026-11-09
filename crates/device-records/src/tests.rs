extern crate std;
use super::*;
use device_vault::Source;
use std::{collections::BTreeMap, vec, vec::Vec};
#[path = "../test-support/media.rs"]
mod media;
use media::*;
type Store = Records<Flash, Se, Crypto>;
fn store(s: &Shared) -> Store {
    Records::new(vault(s))
}
fn id(namespace: Namespace, key: u16) -> Id {
    Id { namespace, key }
}
fn get(s: &mut Store, id: Id) -> Option<Vec<u8>> {
    s.get(id, &mut Allow).unwrap().map(|v| v.to_vec())
}

#[test]
fn notes_recovery_and_fido_are_isolated_and_survive_streamed_edits_and_reboot() {
    let state = Shared::default();
    let mut s = store(&state);
    assert_eq!(s.ids(Namespace::Note, &mut Allow).unwrap(), vec![]);
    let note = vec![0x61; MAX_VALUE_BYTES];
    for (i, value) in [
        (id(Namespace::Note, 7), note.as_slice()),
        (id(Namespace::Fido, 7), b"credential"),
        (id(Namespace::Recovery, 7), b"recovery"),
    ] {
        s.put(i, value, &mut Allow).unwrap();
    }
    for mut s in [s, store(&state)] {
        assert_eq!(get(&mut s, id(Namespace::Note, 7)), Some(note.clone()));
        assert_eq!(
            get(&mut s, id(Namespace::Fido, 7)),
            Some(b"credential".to_vec())
        );
        assert_eq!(
            get(&mut s, id(Namespace::Recovery, 7)),
            Some(b"recovery".to_vec())
        );
    }
    let mut s = store(&state);
    s.put(id(Namespace::Note, 7), b"changed", &mut Allow)
        .unwrap();
    s.remove(id(Namespace::Fido, 7), &mut Allow).unwrap();
    let mut s = store(&state);
    assert_eq!(
        get(&mut s, id(Namespace::Note, 7)),
        Some(b"changed".to_vec())
    );
    assert!(get(&mut s, id(Namespace::Fido, 7)).is_none());
    assert!(get(&mut s, id(Namespace::Recovery, 7)).is_some());
}
#[test]
fn deterministic_mixed_operation_corpus_matches_an_independent_map_after_each_reboot() {
    let state = Shared::default();
    let mut expected = BTreeMap::new();
    let mut seed = 0x19738u32;
    for round in 0..90 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let ns = [Namespace::Fido, Namespace::Note, Namespace::Recovery][(seed as usize >> 4) % 3];
        let id = id(ns, ((seed >> 16) % 13 + 1) as u16);
        let mut s = store(&state);
        if round % 4 == 0 {
            s.remove(id, &mut Allow).unwrap();
            expected.remove(&id);
        } else {
            let value = vec![(seed % 251) as u8; (round % 31 + 1) as usize];
            s.put(id, &value, &mut Allow).unwrap();
            expected.insert(id, value);
        }
        let mut s = store(&state);
        for ns in [Namespace::Fido, Namespace::Note, Namespace::Recovery] {
            let keys: Vec<_> = expected
                .keys()
                .filter(|i| i.namespace == ns)
                .map(|i| i.key)
                .collect();
            assert_eq!(s.ids(ns, &mut Allow).unwrap(), keys);
            for key in keys {
                let id = id_fn(ns, key);
                assert_eq!(get(&mut s, id).as_ref(), expected.get(&id));
            }
        }
    }
}
fn id_fn(namespace: Namespace, key: u16) -> Id {
    id(namespace, key)
}
#[test]
fn invalid_ids_lengths_and_noop_deletion_do_not_consume_an_epoch() {
    let state = Shared::default();
    let mut s = store(&state);
    for (id, value) in [
        (id(Namespace::Fido, 4096), vec![1]),
        (id(Namespace::Note, 0), vec![1]),
        (id(Namespace::Fido, 1), vec![1; 1024]),
        (id(Namespace::Note, 1), vec![1; MAX_VALUE_BYTES + 1]),
        (id(Namespace::Recovery, 1), vec![]),
    ] {
        assert_eq!(s.put(id, &value, &mut Allow), Err(Error::Input));
    }
    s.remove(id(Namespace::Note, 1), &mut Allow).unwrap();
    assert_eq!(state.borrow().epoch, 0);
}
struct Bytes(Vec<u8>);
impl Source for Bytes {
    fn length(&self) -> u32 {
        self.0.len() as u32
    }
    fn read(&mut self, o: u32, out: &mut [u8]) -> Result<(), device_vault::Error> {
        out.copy_from_slice(&self.0[o as usize..o as usize + out.len()]);
        Ok(())
    }
}
fn raw(state: &Shared, bytes: Vec<u8>) {
    vault(state)
        .replace(
            &mut Bytes(bytes.clone()),
            &Sha256::digest(&bytes).into(),
            &mut Allow,
        )
        .unwrap();
}
fn encoded(items: &[(Id, Vec<u8>)]) -> Vec<u8> {
    let mut v = header(items.len(), 1).to_vec();
    for (i, b) in items {
        v.extend(record(*i, b.len() as u32));
        v.extend(b);
    }
    v
}
#[test]
fn authenticated_ciphertext_still_requires_strict_record_schema_order_and_bounds() {
    let valid = encoded(&[(id(Namespace::Fido, 7), b"x".to_vec())]);
    for mode in 0..11 {
        let state = Shared::default();
        let mut bad = valid.clone();
        match mode {
            0 => bad[0] ^= 1,
            1 => bad[28] = 1,
            2 => bad[20..28].fill(0),
            3 => bad[16..20].copy_from_slice(&4097u32.to_be_bytes()),
            4 => bad[32..34].copy_from_slice(&4u16.to_be_bytes()),
            5 => bad[34..36].copy_from_slice(&4096u16.to_be_bytes()),
            6 => bad[36..40].copy_from_slice(&u32::MAX.to_be_bytes()),
            7 => {
                bad.pop();
            }
            8 => bad.push(0),
            9 => {
                bad = encoded(&[
                    (id(Namespace::Note, 2), vec![1]),
                    (id(Namespace::Note, 2), vec![2]),
                ])
            }
            _ => {
                bad = encoded(&[
                    (id(Namespace::Recovery, 2), vec![1]),
                    (id(Namespace::Note, 1), vec![2]),
                ])
            }
        }
        raw(&state, bad);
        let mut s = store(&state);
        assert_eq!(
            s.ids(Namespace::Fido, &mut Allow),
            Err(Error::Format),
            "mode {mode}"
        );
        assert_eq!(s.ids(Namespace::Note, &mut Allow), Err(Error::Closed));
    }
}
#[test]
fn combined_note_recovery_limit_is_enforced_without_creating_a_partial_record() {
    let state = Shared::default();
    let items: Vec<_> = (1..=256)
        .map(|key| {
            (
                id(
                    if key < 200 {
                        Namespace::Note
                    } else {
                        Namespace::Recovery
                    },
                    key,
                ),
                vec![1],
            )
        })
        .collect();
    raw(&state, encoded(&items));
    let mut s = store(&state);
    let before = state.borrow().epoch;
    assert_eq!(
        s.put(id(Namespace::Note, 300), b"over limit", &mut Allow),
        Err(Error::Capacity)
    );
    assert_eq!(state.borrow().epoch, before);
    s.remove(id(Namespace::Recovery, 256), &mut Allow).unwrap();
    s.put(id(Namespace::Note, 300), b"replacement", &mut Allow)
        .unwrap();
    assert!(get(&mut store(&state), id(Namespace::Note, 300)).is_some());
}
#[test]
fn locked_or_late_expired_index_and_value_requests_do_not_release_plaintext() {
    let state = Shared::default();
    let mut s = store(&state);
    s.put(id(Namespace::Note, 1), b"private", &mut Allow)
        .unwrap();
    s.lock();
    let opens = state.borrow().opens;
    assert_eq!(
        s.ids(Namespace::Note, &mut Budget(0)),
        Err(Error::Authorization)
    );
    assert!(matches!(
        s.get(id(Namespace::Note, 1), &mut Budget(0)),
        Err(Error::Authorization)
    ));
    assert_eq!(state.borrow().opens, opens);
    assert_eq!(
        get(&mut s, id(Namespace::Note, 1)),
        Some(b"private".to_vec())
    );
    // Cached index still cannot bypass a new live authorization check.
    assert_eq!(
        s.ids(Namespace::Note, &mut Budget(0)),
        Err(Error::Authorization)
    );
}
#[test]
fn every_record_update_io_cut_recovers_only_old_or_complete_new_without_cross_namespace_damage() {
    fn initial(s: &Shared) {
        let mut r = store(s);
        r.put(id(Namespace::Fido, 1), b"fido", &mut Allow).unwrap();
        r.put(
            id(Namespace::Note, 7),
            &vec![0x31; PAYLOAD_BYTES + 1],
            &mut Allow,
        )
        .unwrap();
    }
    let probe = Shared::default();
    initial(&probe);
    let mut r = store(&probe);
    r.ids(Namespace::Note, &mut Allow).unwrap();
    probe.borrow_mut().operations = 0;
    r.put(id(Namespace::Note, 7), b"new", &mut Allow).unwrap();
    let operations = probe.borrow().operations;
    for cut in 0..operations {
        for ambiguous in [false, true] {
            let state = Shared::default();
            initial(&state);
            let mut r = store(&state);
            r.ids(Namespace::Note, &mut Allow).unwrap();
            {
                let mut s = state.borrow_mut();
                s.operations = 0;
                s.cut = Some(cut);
                s.commit_then_error = ambiguous;
            }
            assert!(r.put(id(Namespace::Note, 7), b"new", &mut Allow).is_err());
            state.borrow_mut().cut = None;
            let mut reboot = store(&state);
            let value = get(&mut reboot, id(Namespace::Note, 7)).unwrap();
            assert!(
                value == b"new" || value == vec![0x31; PAYLOAD_BYTES + 1],
                "cut {cut}"
            );
            assert_eq!(
                get(&mut reboot, id(Namespace::Fido, 1)),
                Some(b"fido".to_vec())
            );
        }
    }
}

#[test]
fn a_private_read_fault_also_closes_cached_metadata_and_cannot_be_cleared_by_lock() {
    let state = Shared::default();
    let mut s = store(&state);
    let note = id(Namespace::Note, 7);
    s.put(note, b"secret", &mut Allow).unwrap();
    s.ids(Namespace::Note, &mut Allow).unwrap();
    let bank = state.borrow().head.unwrap().bank;
    state.borrow_mut().sectors.get_mut(&(bank, 0)).unwrap()[100] ^= 1;
    assert!(s.get(note, &mut Allow).is_err());
    assert_eq!(s.ids(Namespace::Note, &mut Allow), Err(Error::Closed));
    s.lock();
    assert_eq!(s.ids(Namespace::Note, &mut Allow), Err(Error::Closed));
}

#[test]
fn present_empty_fido_flags_are_distinct_from_a_missing_key() {
    let state = Shared::default();
    let mut s = store(&state);
    let flag = id(Namespace::Fido, 2047);
    s.put(flag, &[], &mut Allow).unwrap();
    assert_eq!(get(&mut store(&state), flag), Some(vec![]));
    s.remove(flag, &mut Allow).unwrap();
    assert_eq!(get(&mut store(&state), flag), None);
}

#[test]
fn public_private_record_api_validates_schema_and_never_exposes_fido_as_notes() {
    let state = Shared::default();
    let mut s = store(&state);
    let note = id(Namespace::Note, 7);
    let recovery = id(Namespace::Recovery, 7);
    let n = item::note("Name", "Body").unwrap();
    let r = item::recovery("Backup", &["A", "B"]).unwrap();
    s.put_private(note, &n, &mut Allow).unwrap();
    s.put_private(recovery, &r, &mut Allow).unwrap();
    assert_eq!(
        &s.get_private(note, &mut Allow).unwrap().unwrap()[..],
        &n[..]
    );
    assert_eq!(s.put_private(note, &r, &mut Allow), Err(Error::Input));
    assert_eq!(
        s.put_private(id(Namespace::Fido, 1), &n, &mut Allow),
        Err(Error::Input)
    );
    assert!(matches!(
        s.get_private(id(Namespace::Fido, 1), &mut Allow),
        Err(Error::Input)
    ));
    assert!(matches!(
        s.get_private(note, &mut Budget(0)),
        Err(Error::Authorization)
    ));
    // Authenticated storage is not enough if its logical payload is corrupt.
    s.put(note, b"wrong record format", &mut Allow).unwrap();
    assert!(matches!(
        s.get_private(note, &mut Allow),
        Err(Error::Format)
    ));
    assert_eq!(s.ids(Namespace::Note, &mut Allow), Err(Error::Closed));
}
