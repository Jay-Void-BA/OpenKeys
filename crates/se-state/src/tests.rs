extern crate std;
use super::*;
use crate::{
    objects::{Counter, File, HEAD_BYTES, marker},
    record,
};
use aes_gcm::{Aes256Gcm, KeyInit, aead::AeadInPlace};
use device_policy::{
    Operation, Request,
    biometric::{Error as BioError, ProtectedAttempts, ProtectedGcm, Session},
};
use device_vault::{Error as VaultError, Head, Ledger};
use openkeys_scp03::Command;
use std::{cell::RefCell, rc::Rc, vec::Vec};

#[derive(Clone, Copy)]
enum Cut {
    Before,
    After,
    Half,
}
#[derive(Clone)]
struct Trace {
    ins: u8,
    p1: u8,
    p2: u8,
    data: Vec<u8>,
}
pub(crate) struct Silicon {
    counters: [Option<u64>; 7],
    files: [Option<Vec<u8>>; 3],
    calls: Vec<Trace>,
    cut: Option<(usize, Cut)>,
    transient: bool,
    bad_counter_width: bool,
    bad_marker: bool,
    append_bytes: bool,
    wrong_type: bool,
}
impl Default for Silicon {
    fn default() -> Self {
        let empty = record::encode(0, None).unwrap().to_vec();
        Self {
            counters: [Some(0); 7],
            files: [Some(empty.clone()), Some(empty), Some(marker().to_vec())],
            calls: Vec::new(),
            cut: None,
            transient: false,
            bad_counter_width: false,
            bad_marker: false,
            append_bytes: false,
            wrong_type: false,
        }
    }
}
pub(crate) struct Model {
    shared: Rc<RefCell<Silicon>>,
    closed: bool,
}
pub(crate) fn model() -> (Model, Rc<RefCell<Silicon>>) {
    let shared = Rc::new(RefCell::new(Silicon::default()));
    (peer(&shared), shared)
}
fn peer(shared: &Rc<RefCell<Silicon>>) -> Model {
    Model {
        shared: shared.clone(),
        closed: false,
    }
}
impl private::Sealed for Model {}
impl Silicon {
    fn process(&mut self, c: &Command<'_>, partial: bool) -> Result<Vec<u8>, Error> {
        assert_eq!(c.class, 0x80);
        assert_eq!(c.le, Some(0));
        assert!(c.data.len() >= 6);
        assert_eq!(&c.data[..2], &[0x41, 4]);
        let id = u32::from_be_bytes(c.data[2..6].try_into().unwrap());
        let counter = Counter::ALL.into_iter().find(|v| v.id() == id);
        let file = File::ALL.into_iter().find(|v| v.id() == id);
        assert!(
            counter.is_some() || file.is_some(),
            "unreviewed raw object ID"
        );
        let (kind, size) = if let Some(counter) = counter {
            if self.counters[counter.index()].is_none() {
                return Err(Error::Channel);
            }
            (0x0d, if self.bad_counter_width { 4 } else { 8 })
        } else {
            let file = file.unwrap();
            let Some(bytes) = &self.files[file as usize] else {
                return Err(Error::Channel);
            };
            (0x0b, bytes.len())
        };
        if c.instruction == 2 && c.p2 == 0x26 {
            assert_eq!(c.p1, 0);
            assert_eq!(c.data.len(), 6);
            return Ok(std::vec![
                0x41,
                1,
                if self.wrong_type { 0x0c } else { kind },
                0x42,
                1,
                if self.transient { 2 } else { 1 }
            ]);
        }
        if c.instruction == 2 && c.p2 == 7 {
            assert_eq!(c.p1, 0);
            assert_eq!(c.data.len(), 6);
            return Ok(std::vec![0x41, 2, (size >> 8) as u8, size as u8]);
        }
        if c.instruction == 2 && c.p2 == 0 {
            assert_eq!(c.p1, 0);
            if let Some(counter) = counter {
                assert_eq!(c.data.len(), 6);
                let mut out = std::vec![0x41, 8];
                out.extend_from_slice(&self.counters[counter.index()].unwrap().to_be_bytes());
                return Ok(out);
            }
            let file = file.unwrap();
            assert_eq!(
                &c.data[6..],
                &[0x42, 2, 0, 0, 0x43, 2, 0, file.length() as u8]
            );
            let bytes = self.files[file as usize].as_ref().unwrap();
            let mut out = std::vec![0x41, bytes.len() as u8];
            out.extend_from_slice(bytes);
            if self.bad_marker && file == File::Marker {
                out[2] ^= 1;
            }
            return Ok(out);
        }
        assert_eq!(c.instruction, 1);
        assert_eq!(c.p2, 0);
        if c.p1 == 8 {
            let counter = counter.unwrap();
            let current = self.counters[counter.index()].unwrap();
            let next = if c.data.len() == 6 {
                current.checked_add(1).ok_or(Error::Exhausted)?
            } else {
                assert_eq!(counter.id(), Counter::BioCredit.id());
                assert_eq!(&c.data[6..8], &[0x43, 8]);
                u64::from_be_bytes(c.data[8..].try_into().unwrap())
            };
            assert!(next >= current, "counter decrement");
            self.counters[counter.index()] = Some(next);
        } else {
            assert_eq!(c.p1, 6);
            let file = file.unwrap();
            assert!(file != File::Marker);
            assert_eq!(&c.data[6..8], &[0x44, HEAD_BYTES as u8]);
            assert_eq!(c.data.len(), HEAD_BYTES + 8);
            let bytes = self.files[file as usize].as_mut().unwrap();
            if partial {
                bytes[..HEAD_BYTES / 2].copy_from_slice(&c.data[8..8 + HEAD_BYTES / 2]);
            } else {
                bytes.copy_from_slice(&c.data[8..]);
            }
        }
        Ok(Vec::new())
    }
}
impl Exchange for Model {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        assert!(!self.closed, "command retried on a closed owner");
        let mut s = self.shared.borrow_mut();
        let at = s.calls.len();
        s.calls.push(Trace {
            ins: c.instruction,
            p1: c.p1,
            p2: c.p2,
            data: c.data.to_vec(),
        });
        let cut = s
            .cut
            .filter(|(target, _)| *target == at)
            .map(|(_, kind)| kind);
        if matches!(cut, Some(Cut::Before)) {
            return Err(Error::Channel);
        }
        let mut result = s.process(c, matches!(cut, Some(Cut::Half)))?;
        if cut.is_some() {
            return Err(Error::Channel);
        }
        if s.append_bytes {
            result.push(0);
        }
        if result.len() > out.len() {
            return Err(Error::Schema);
        }
        out[..result.len()].copy_from_slice(&result);
        Ok(result.len())
    }
    fn close(&mut self) {
        self.closed = true;
    }
}
pub(crate) fn request() -> Request {
    Request {
        operation: Operation::RevealRecoveryCode,
        digest: [0x71; 32],
        channel: [0x92; 32],
    }
}
fn head(epoch: u64, bank: u8, value: u8) -> Head {
    Head {
        epoch,
        bank,
        length: 4081,
        root: [value; 32],
    }
}
fn commit(state: &mut State<Model>, bank: u8, value: u8) -> Head {
    let epoch = state.reserve_epoch().unwrap();
    let head = head(epoch, bank, value);
    state.commit(&head).unwrap();
    head
}
fn value(shared: &Rc<RefCell<Silicon>>, c: Counter) -> u64 {
    shared.borrow().counters[c.index()].unwrap()
}

#[test]
fn exact_persistent_schema_marker_and_empty_state_are_required() {
    for bad in 0..7 {
        let (io, shared) = model();
        {
            let mut s = shared.borrow_mut();
            match bad {
                0 => s.transient = true,
                1 => s.bad_counter_width = true,
                2 => s.bad_marker = true,
                3 => s.wrong_type = true,
                4 => s.files[File::Marker as usize] = None,
                5 => s.counters[0] = None,
                _ => s.append_bytes = true,
            }
        }
        assert!(State::open(io).is_err());
    }
    let (io, shared) = model();
    let mut state = State::open(io).unwrap();
    assert_eq!(state.active().unwrap(), None);
    assert!(shared.borrow().calls.iter().all(|call| call.ins == 2));
    assert!(
        shared.borrow().calls.iter().any(|call| call.ins == 2
            && call.p2 == 0x26
            && call.data == [0x41, 4, 0x4f, 0x4b, 0, 4])
    );
}

#[test]
fn codec_detects_every_single_bit_change_and_never_accepts_a_stale_sequence() {
    let h = head(3, 1, 0x22);
    let encoded = record::encode(2, Some(&h)).unwrap();
    assert_eq!(record::decode(&encoded, 2).unwrap(), Some(h));
    for bit in 0..HEAD_BYTES * 8 {
        let mut changed = encoded;
        changed[bit / 8] ^= 1 << (bit % 8);
        assert!(record::decode(&changed, 2).is_err());
    }
    assert!(record::decode(&encoded, 1).is_err());
    assert!(record::encode(0, Some(&h)).is_err());
    assert!(record::encode(4, Some(&h)).is_err());
    assert!(record::encode(1, None).is_err());
}

#[test]
fn independent_nonce_and_commit_counters_burn_cancelled_reservations() {
    let (io, shared) = model();
    let mut state = State::open(io).unwrap();
    assert_eq!(state.reserve_epoch().unwrap(), 1);
    drop(state);
    let mut state = State::open(peer(&shared)).unwrap();
    assert_eq!(state.active().unwrap(), None);
    let a = commit(&mut state, 0, 0x31);
    assert_eq!(a.epoch, 2);
    assert_eq!(value(&shared, Counter::VaultCommit), 1);
    let b = commit(&mut state, 1, 0x72);
    assert_eq!(b.epoch, 3);
    drop(state);
    assert_eq!(
        State::open(peer(&shared)).unwrap().active().unwrap(),
        Some(b)
    );
    assert_eq!(value(&shared, Counter::VaultCommit), 2);
}

#[test]
fn every_apdu_cut_recovers_only_old_or_new_without_refunding_the_nonce() {
    let (io, shared) = model();
    let mut baseline = State::open(io).unwrap();
    commit(&mut baseline, 0, 0x31);
    shared.borrow_mut().calls.clear();
    commit(&mut baseline, 1, 0x72);
    let count = shared.borrow().calls.len();
    assert!(count > 10);
    for at in 0..count {
        for cut in [Cut::Before, Cut::After, Cut::Half] {
            let (io, shared) = model();
            let mut state = State::open(io).unwrap();
            let old = commit(&mut state, 0, 0x31);
            {
                let mut s = shared.borrow_mut();
                s.calls.clear();
                s.cut = Some((at, cut));
            }
            let result = (|| {
                let epoch = state.reserve_epoch()?;
                state.commit(&head(epoch, 1, 0x72))
            })();
            assert!(result.is_err(), "cut was not reached");
            assert_eq!(state.active(), Err(VaultError::Faulted));
            drop(state);
            shared.borrow_mut().cut = None;
            let mut state = State::open(peer(&shared)).unwrap();
            let recovered = state.active().unwrap().unwrap();
            assert!(recovered == old || recovered == head(2, 1, 0x72));
            let burned = value(&shared, Counter::VaultNonce);
            let next = state.reserve_epoch().unwrap();
            assert!(next > burned);
        }
    }
}

#[test]
fn committed_slot_damage_or_replay_never_falls_back_to_an_older_head() {
    for replay in [false, true] {
        let (io, shared) = model();
        let mut state = State::open(io).unwrap();
        commit(&mut state, 0, 0x31);
        let old = shared.borrow().files[File::Head1 as usize].clone();
        commit(&mut state, 1, 0x72);
        drop(state);
        {
            let mut s = shared.borrow_mut();
            if replay {
                s.files[File::Head0 as usize] = old;
            } else {
                s.files[File::Head0 as usize].as_mut().unwrap()[80] ^= 1;
            }
        }
        assert!(matches!(State::open(peer(&shared)), Err(Error::Record)));
    }
}

#[test]
fn invalid_commit_bank_epoch_and_overflow_fail_without_private_writes() {
    for bad in 0..3 {
        let (io, shared) = model();
        let mut state = State::open(io).unwrap();
        let epoch = state.reserve_epoch().unwrap();
        shared.borrow_mut().calls.clear();
        let mut h = head(epoch, 0, 0x31);
        match bad {
            0 => h.bank = 1,
            1 => h.epoch += 1,
            _ => h.length = device_vault::MAX_SNAPSHOT_BYTES + 1,
        };
        assert!(state.commit(&h).is_err());
        assert!(shared.borrow().calls.iter().all(|call| call.ins == 2));
    }
    let (io, shared) = model();
    shared.borrow_mut().counters[Counter::VaultNonce.index()] = Some(u64::MAX);
    let mut state = State::open(io).unwrap();
    shared.borrow_mut().calls.clear();
    assert!(state.reserve_epoch().is_err());
    assert!(shared.borrow().calls.iter().all(|call| call.ins == 2));
}

struct FingerprintGcm;
impl ProtectedGcm for FingerprintGcm {
    fn decrypt_identify_payload(
        &mut self,
        header: &[u8; 8],
        iv: &[u8; 12],
        tag: &[u8; 16],
        payload: &mut [u8; 12],
    ) -> Result<(), BioError> {
        Aes256Gcm::new_from_slice(&[0x42; 32])
            .unwrap()
            .decrypt_in_place_detached(iv.into(), header, payload, tag.into())
            .map_err(|_| BioError::Unauthenticated)
    }
}
fn matched(tag: u16) -> [u8; 48] {
    use core::sync::atomic::{AtomicU64, Ordering};
    static NONCE: AtomicU64 = AtomicU64::new(1);
    let mut frame = [0; 48];
    for (index, word) in [4u16, 0x13, 0x41, 40].into_iter().enumerate() {
        frame[index * 2..index * 2 + 2].copy_from_slice(&word.to_le_bytes());
    }
    frame[8..16].copy_from_slice(&NONCE.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    let mut payload = [0; 12];
    for (index, word) in [0x55u16, 0x13, 0x61ec, 0x3034, 7, tag]
        .into_iter()
        .enumerate()
    {
        payload[index * 2..index * 2 + 2].copy_from_slice(&word.to_le_bytes());
    }
    let mac = Aes256Gcm::new_from_slice(&[0x42; 32])
        .unwrap()
        .encrypt_in_place_detached((&frame[8..20]).into(), &frame[..8], &mut payload)
        .unwrap();
    frame[20..36].copy_from_slice(&mac);
    frame[36..].copy_from_slice(&payload);
    frame
}

#[test]
fn debit_and_tag_survive_reset_and_only_authenticated_match_restores_credit() {
    let (io, shared) = model();
    let service = Service::new(State::open(io).unwrap());
    let mut biometric = Session::new(service.handle(), FingerprintGcm);
    let first = biometric.begin(request(), 7, 0).unwrap();
    let intercepted = matched(first);
    drop(service);
    let service = Service::new(State::open(peer(&shared)).unwrap());
    let mut biometric = Session::new(service.handle(), FingerprintGcm);
    let next = biometric.begin(request(), 7, 10).unwrap();
    assert!(next > first);
    assert!(matches!(
        biometric.receive(&intercepted, &request(), || 11),
        Err(BioError::WrongTag)
    ));
    assert_eq!(value(&shared, Counter::BioCredit), 0);
    let tag = biometric.begin(request(), 7, 20).unwrap();
    assert!(
        biometric
            .receive(&matched(tag), &request(), || 21)
            .unwrap()
            .is_some()
    );
    assert_eq!(value(&shared, Counter::BioCredit), 3);
    assert_eq!(value(&shared, Counter::BioSpent), 3);
    assert_eq!(value(&shared, Counter::BioTag), 3);
}

#[test]
fn persistent_failure_budget_and_tag_exhaustion_do_not_reset_on_power_cycle() {
    let (io, shared) = model();
    let mut state = State::open(io).unwrap();
    for _ in 0..BIO_MAX_FAILURES {
        state.reserve_identify_attempt(&request()).unwrap();
    }
    drop(state);
    let mut state = State::open(peer(&shared)).unwrap();
    assert_eq!(
        state.reserve_identify_attempt(&request()),
        Err(BioError::Exhausted)
    );
    assert_eq!(value(&shared, Counter::BioSpent), BIO_MAX_FAILURES);
    let (io, shared) = model();
    shared.borrow_mut().counters[Counter::BioTag.index()] = Some(u64::from(u16::MAX));
    shared.borrow_mut().counters[Counter::BioSpent.index()] = Some(u64::from(u16::MAX));
    shared.borrow_mut().counters[Counter::BioCredit.index()] = Some(u64::from(u16::MAX));
    let mut state = State::open(io).unwrap();
    shared.borrow_mut().calls.clear();
    assert_eq!(
        state.reserve_identify_attempt(&request()),
        Err(BioError::Exhausted)
    );
    assert!(shared.borrow().calls.iter().all(|call| call.ins == 2));
}

#[test]
fn match_credit_acknowledgement_uncertainty_never_releases_a_proof() {
    // Locate the actual protected credit-write APDU in a successful flow.
    let (io, shared) = model();
    let service = Service::new(State::open(io).unwrap());
    let mut biometric = Session::new(service.handle(), FingerprintGcm);
    let tag = biometric.begin(request(), 7, 0).unwrap();
    shared.borrow_mut().calls.clear();
    biometric
        .receive(&matched(tag), &request(), || 1)
        .unwrap()
        .unwrap();
    let credit_write = shared
        .borrow()
        .calls
        .iter()
        .position(|call| {
            call.ins == 1 && call.p1 == 8 && call.data[..6] == [0x41, 4, 0x4f, 0x4b, 0, 2]
        })
        .unwrap();
    for cut in [Cut::Before, Cut::After] {
        let (io, shared) = model();
        let service = Service::new(State::open(io).unwrap());
        let mut biometric = Session::new(service.handle(), FingerprintGcm);
        let tag = biometric.begin(request(), 7, 0).unwrap();
        {
            let mut s = shared.borrow_mut();
            s.calls.clear();
            s.cut = Some((credit_write, cut));
        }
        assert!(biometric.receive(&matched(tag), &request(), || 1).is_err());
        drop(service);
        shared.borrow_mut().cut = None;
        let service = Service::new(State::open(peer(&shared)).unwrap());
        let mut biometric = Session::new(service.handle(), FingerprintGcm);
        assert!(matches!(
            biometric.receive(&matched(tag), &request(), || 2),
            Err(BioError::Locked)
        ));
        assert!(biometric.begin(request(), 7, 3).unwrap() > tag);
    }
}

#[test]
fn every_biometric_precharge_cut_is_durable_and_never_returns_a_tag() {
    let (io, shared) = model();
    let mut state = State::open(io).unwrap();
    shared.borrow_mut().calls.clear();
    state.reserve_identify_attempt(&request()).unwrap();
    let count = shared.borrow().calls.len();
    for at in 0..count {
        for cut in [Cut::Before, Cut::After] {
            let (io, shared) = model();
            let mut state = State::open(io).unwrap();
            {
                let mut s = shared.borrow_mut();
                s.calls.clear();
                s.cut = Some((at, cut));
            }
            assert!(state.reserve_identify_attempt(&request()).is_err());
            assert_eq!(
                state.reserve_identify_attempt(&request()),
                Err(BioError::RecoveryRequired)
            );
            let spent = value(&shared, Counter::BioSpent);
            let tag = value(&shared, Counter::BioTag);
            drop(state);
            shared.borrow_mut().cut = None;
            let mut state = State::open(peer(&shared)).unwrap();
            assert_eq!(
                u64::from(state.reserve_identify_attempt(&request()).unwrap()),
                tag + 1
            );
            assert_eq!(value(&shared, Counter::BioSpent), spent + 1);
        }
    }
}

#[test]
fn observed_counter_rollback_and_impossible_credit_are_not_blank_state() {
    let (io, shared) = model();
    let mut state = State::open(io).unwrap();
    state.reserve_epoch().unwrap();
    shared.borrow_mut().counters[Counter::VaultNonce.index()] = Some(0);
    assert_eq!(state.active(), Err(VaultError::Rollback));
    assert_eq!(state.active(), Err(VaultError::Faulted));
    let (io, shared) = model();
    shared.borrow_mut().counters[Counter::BioCredit.index()] = Some(1);
    assert!(matches!(State::open(io), Err(Error::Counter)));
    assert!(shared.borrow().calls.iter().all(|call| call.ins == 2));
}

#[test]
fn real_vault_core_uses_the_se_commit_anchor_and_rejects_old_nor_images() {
    use device_vault::{
        Aead, Checkpoint, CiphertextStorage, PAYLOAD_BYTES, SECTOR_BYTES, Source, Vault,
    };
    use sha2::{Digest, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    type Sectors = Rc<RefCell<BTreeMap<(u8, u32), [u8; SECTOR_BYTES]>>>;
    struct Flash(Sectors);
    impl CiphertextStorage for Flash {
        fn read(
            &mut self,
            bank: u8,
            index: u32,
            out: &mut [u8; SECTOR_BYTES],
        ) -> Result<(), VaultError> {
            out.copy_from_slice(
                self.0
                    .borrow()
                    .get(&(bank, index))
                    .ok_or(VaultError::Storage)?,
            );
            Ok(())
        }
        fn write(
            &mut self,
            bank: u8,
            index: u32,
            bytes: &[u8; SECTOR_BYTES],
        ) -> Result<(), VaultError> {
            self.0.borrow_mut().insert((bank, index), *bytes);
            Ok(())
        }
        fn flush(&mut self) -> Result<(), VaultError> {
            Ok(())
        }
    }
    struct Crypto(Rc<RefCell<BTreeSet<[u8; 12]>>>);
    impl Aead for Crypto {
        fn seal(
            &mut self,
            nonce: &[u8; 12],
            aad: &[u8; 48],
            data: &mut [u8; PAYLOAD_BYTES],
        ) -> Result<[u8; 16], VaultError> {
            assert!(self.0.borrow_mut().insert(*nonce), "nonce reused");
            Aes256Gcm::new_from_slice(&[0x5a; 32])
                .unwrap()
                .encrypt_in_place_detached(nonce.into(), aad, data)
                .map(Into::into)
                .map_err(|_| VaultError::Crypto)
        }
        fn open(
            &mut self,
            nonce: &[u8; 12],
            aad: &[u8; 48],
            tag: &[u8; 16],
            data: &mut [u8; PAYLOAD_BYTES],
        ) -> Result<(), VaultError> {
            Aes256Gcm::new_from_slice(&[0x5a; 32])
                .unwrap()
                .decrypt_in_place_detached(nonce.into(), aad, data, tag.into())
                .map_err(|_| VaultError::Crypto)
        }
    }
    struct PublicSource(Vec<u8>);
    impl Source for PublicSource {
        fn length(&self) -> u32 {
            self.0.len() as u32
        }
        fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), VaultError> {
            let at = offset as usize;
            out.copy_from_slice(self.0.get(at..at + out.len()).ok_or(VaultError::Invalid)?);
            Ok(())
        }
    }
    struct TestApproved;
    impl Checkpoint for TestApproved {
        fn check(&mut self) -> Result<(), VaultError> {
            Ok(())
        }
    }
    let (io, shared) = model();
    let service = Service::new(State::open(io).unwrap());
    let sectors = Sectors::default();
    let used = Rc::new(RefCell::new(BTreeSet::new()));
    let mut vault = Vault::mount(
        Flash(sectors.clone()),
        service.handle(),
        Crypto(used.clone()),
    )
    .unwrap();
    let old = std::vec![0x31;PAYLOAD_BYTES+7];
    vault
        .replace(
            &mut PublicSource(old.clone()),
            &Sha256::digest(&old).into(),
            &mut TestApproved,
        )
        .unwrap();
    let old_image = sectors.borrow().clone();
    drop(vault);
    // Complete reboot: the SE anchor, NOR and nonce history remain; no RAM
    // object supplies the active Head to the actual device-vault core.
    drop(service);
    let service = Service::new(State::open(peer(&shared)).unwrap());
    let mut vault = Vault::mount(
        Flash(sectors.clone()),
        service.handle(),
        Crypto(used.clone()),
    )
    .unwrap();
    let new = std::vec![0x72;PAYLOAD_BYTES+7];
    vault
        .replace(
            &mut PublicSource(new.clone()),
            &Sha256::digest(&new).into(),
            &mut TestApproved,
        )
        .unwrap();
    let mut out = [0; PAYLOAD_BYTES];
    assert_eq!(
        vault.read_chunk(0, &mut out, &mut TestApproved).unwrap(),
        PAYLOAD_BYTES
    );
    assert_eq!(out, &new[..PAYLOAD_BYTES]);
    drop(vault);
    drop(service);
    *sectors.borrow_mut() = old_image;
    let service = Service::new(State::open(peer(&shared)).unwrap());
    assert!(matches!(
        Vault::mount(Flash(sectors), service.handle(), Crypto(used)),
        Err(VaultError::Storage | VaultError::DigestMismatch)
    ));
}

#[test]
fn namespace_marker_binds_the_exact_policy_and_fixed_object_layout() {
    // Independent Python/hashlib export, not generated by the Rust encoder.
    assert_eq!(
        marker(),
        [
            0x3c, 0x4a, 0xb9, 0x1d, 0xae, 0x4b, 0x19, 0xc8, 0x04, 0xe5, 0xcd, 0xfc, 0x6c, 0x82,
            0xa0, 0x83, 0xd4, 0x42, 0x02, 0x33, 0x27, 0x82, 0x2d, 0x9b, 0x6d, 0x70, 0xfa, 0x8f,
            0x09, 0x15, 0x4f, 0xe5
        ]
    );
    assert_eq!(Counter::VaultNonce.id(), 0x4f4b0004);
    assert_eq!(Counter::VaultCommit.id(), 0x4f4b0005);
    assert_eq!(Counter::FidoSignature.id(), 0x4f4b0006);
    assert_eq!(Counter::SigningSlot.id(), 0x4f4b0007);
    assert_eq!(File::Head0.id(), 0x4f4b0100);
    assert_eq!(File::Head1.id(), 0x4f4b0101);
}

#[test]
fn signature_counter_is_persistent_bounded_and_independent_of_vault_nonce_and_biometric_budget() {
    let (io, shared) = model();
    let mut s = State::open(io).unwrap();
    assert_eq!(s.signature_counter().unwrap(), 0);
    s.advance_signature_counter(8).unwrap();
    drop(s);
    let mut reboot = State::open(peer(&shared)).unwrap();
    assert_eq!(reboot.signature_counter().unwrap(), 8);
    assert_eq!(value(&shared, Counter::VaultNonce), 0);
    assert_eq!(value(&shared, Counter::BioSpent), 0);
    assert_eq!(reboot.advance_signature_counter(0), Err(Error::Counter));
    assert_eq!(reboot.signature_counter(), Err(Error::Closed));
    shared.borrow_mut().counters[Counter::FidoSignature.index()] = Some(u64::from(u32::MAX));
    let mut reboot = State::open(peer(&shared)).unwrap();
    assert_eq!(reboot.advance_signature_counter(1), Err(Error::Exhausted));
    assert_eq!(value(&shared, Counter::FidoSignature), u64::from(u32::MAX));
    shared.borrow_mut().counters[Counter::FidoSignature.index()] = Some(u64::from(u32::MAX) + 1);
    assert!(State::open(peer(&shared)).is_err());
}

#[test]
fn every_signature_increment_cut_burns_only_executed_steps_and_never_refunds_on_reboot() {
    let (io, shared) = model();
    let mut s = State::open(io).unwrap();
    shared.borrow_mut().calls.clear();
    s.advance_signature_counter(3).unwrap();
    let calls = shared.borrow().calls.len();
    for cut in 0..calls {
        for timing in [Cut::Before, Cut::After, Cut::Half] {
            let (io, shared) = model();
            let mut s = State::open(io).unwrap();
            shared.borrow_mut().calls.clear();
            shared.borrow_mut().cut = Some((cut, timing));
            assert!(s.advance_signature_counter(3).is_err());
            let persisted = value(&shared, Counter::FidoSignature);
            assert!(persisted <= 3);
            shared.borrow_mut().cut = None;
            let mut reboot = State::open(peer(&shared)).unwrap();
            assert_eq!(reboot.signature_counter().unwrap(), persisted as u32);
            reboot.advance_signature_counter(1).unwrap();
            assert_eq!(value(&shared, Counter::FidoSignature), persisted + 1);
        }
    }
}

#[test]
fn signing_slots_are_durably_reserved_never_reused_and_exhaust_without_overwrite() {
    let (io, shared) = model();
    let mut s = State::open(io).unwrap();
    let auth = crate::keys::tests::auth(Operation::FidoRegister);
    assert_eq!(s.reserve_signing_slot(&auth).unwrap(), 0);
    drop(s);
    let mut reboot = State::open(peer(&shared)).unwrap();
    assert_eq!(reboot.reserve_signing_slot(&auth).unwrap(), 1);
    let not_registration = crate::keys::tests::auth(Operation::FidoSign);
    assert_eq!(
        reboot.reserve_signing_slot(&not_registration),
        Err(Error::Authorization)
    );
    for slot in 2..crate::keys::KEY_SLOTS {
        assert_eq!(reboot.reserve_signing_slot(&auth).unwrap(), slot);
    }
    assert_eq!(reboot.reserve_signing_slot(&auth), Err(Error::Exhausted));
    assert_eq!(
        value(&shared, Counter::SigningSlot),
        u64::from(crate::keys::KEY_SLOTS)
    );
    assert_eq!(value(&shared, Counter::VaultNonce), 0);
}

#[test]
fn authenticated_fingerprint_for_a_substituted_request_does_not_restore_credit() {
    // Simulate a reservation for a different digest. A genuine test GCM match
    // is insufficient if its context differs from the durable reservation.
    struct Substituted<'a>(StateRef<'a, Model>);
    impl ProtectedAttempts for Substituted<'_> {
        fn reserve_identify_attempt(&mut self, request: &Request) -> Result<u32, BioError> {
            let mut changed = *request;
            changed.digest[0] ^= 1;
            self.0.reserve_identify_attempt(&changed)
        }
        fn record_match(
            &mut self,
            proof: &device_policy::biometric::AuthenticatedMatch,
        ) -> Result<(), BioError> {
            self.0.record_match(proof)
        }
    }
    let (io, shared) = model();
    let service = Service::new(State::open(io).unwrap());
    let mut biometric = Session::new(Substituted(service.handle()), FingerprintGcm);
    let tag = biometric.begin(request(), 7, 0).unwrap();
    assert!(biometric.receive(&matched(tag), &request(), || 1).is_err());
    assert_eq!(value(&shared, Counter::BioCredit), 0);
    assert_eq!(service.handle().active(), Err(VaultError::Faulted));
}
