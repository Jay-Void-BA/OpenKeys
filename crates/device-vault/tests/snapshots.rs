use aes_gcm::{Aes256Gcm, KeyInit, aead::AeadInPlace};
use device_vault::{
    Aead, CiphertextStorage, Error, Head, Ledger, PAYLOAD_BYTES, SECTOR_BYTES, SnapshotReader,
    SnapshotSource, Source, Vault,
};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

#[derive(Default)]
struct State {
    opens: usize,
    sectors: BTreeMap<(u8, u32), [u8; SECTOR_BYTES]>,
    active: Option<Head>,
    counter: u64,
    operations: usize,
    cut: Option<usize>,
    commit_then_error: bool,
    nonces: BTreeSet<[u8; 12]>,
}
type Shared = Rc<RefCell<State>>;
impl State {
    fn step(&mut self) -> bool {
        let fail = self.cut == Some(self.operations);
        self.operations += 1;
        fail
    }
}
struct Memory(Shared);
impl CiphertextStorage for Memory {
    fn read(&mut self, b: u8, i: u32, out: &mut [u8; SECTOR_BYTES]) -> Result<(), Error> {
        let mut s = self.0.borrow_mut();
        if s.step() {
            return Err(Error::Storage);
        }
        out.copy_from_slice(s.sectors.get(&(b, i)).ok_or(Error::Storage)?);
        Ok(())
    }
    fn write(&mut self, b: u8, i: u32, data: &[u8; SECTOR_BYTES]) -> Result<(), Error> {
        let mut s = self.0.borrow_mut();
        if s.step() {
            let mut partial = [0xff; SECTOR_BYTES];
            partial[..123].copy_from_slice(&data[..123]);
            s.sectors.insert((b, i), partial);
            return Err(Error::Storage);
        }
        s.sectors.insert((b, i), *data);
        Ok(())
    }
    fn flush(&mut self) -> Result<(), Error> {
        if self.0.borrow_mut().step() {
            Err(Error::Storage)
        } else {
            Ok(())
        }
    }
}
struct ProtectedModel(Shared);
impl Ledger for ProtectedModel {
    fn active(&mut self) -> Result<Option<Head>, Error> {
        Ok(self.0.borrow().active)
    }
    fn reserve_epoch(&mut self) -> Result<u64, Error> {
        let mut s = self.0.borrow_mut();
        s.counter = s.counter.checked_add(1).ok_or(Error::Ledger)?;
        if s.step() {
            return Err(Error::Ledger);
        }
        Ok(s.counter)
    }
    fn commit(&mut self, head: &Head) -> Result<(), Error> {
        let mut s = self.0.borrow_mut();
        if s.step() {
            if s.commit_then_error {
                s.active = Some(*head)
            }
            return Err(Error::Ledger);
        }
        s.active = Some(*head);
        Ok(())
    }
}
struct Approved;
impl device_vault::Checkpoint for Approved {
    fn check(&mut self) -> Result<(), Error> {
        Ok(())
    }
}
struct Budget(usize);
impl device_vault::Checkpoint for Budget {
    fn check(&mut self) -> Result<(), Error> {
        if self.0 == 0 {
            Err(Error::Authorization)
        } else {
            self.0 -= 1;
            Ok(())
        }
    }
}
struct Crypto(Shared);
impl Aead for Crypto {
    fn seal(
        &mut self,
        nonce: &[u8; 12],
        aad: &[u8; 48],
        data: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<[u8; 16], Error> {
        assert!(
            self.0.borrow_mut().nonces.insert(*nonce),
            "Nonce reused under same test key"
        );
        let aes = Aes256Gcm::new_from_slice(&[0x42; 32]).unwrap();
        aes.encrypt_in_place_detached(nonce.into(), aad, data)
            .map(Into::into)
            .map_err(|_| Error::Crypto)
    }
    fn open(
        &mut self,
        nonce: &[u8; 12],
        aad: &[u8; 48],
        tag: &[u8; 16],
        data: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<(), Error> {
        self.0.borrow_mut().opens += 1;
        let aes = Aes256Gcm::new_from_slice(&[0x42; 32]).unwrap();
        aes.decrypt_in_place_detached(nonce.into(), aad, data, tag.into())
            .map_err(|_| Error::Crypto)
    }
}
struct Bytes(Vec<u8>);
impl Source for Bytes {
    fn length(&self) -> u32 {
        self.0.len().try_into().unwrap()
    }
    fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), Error> {
        let start = offset as usize;
        out.copy_from_slice(self.0.get(start..start + out.len()).ok_or(Error::Invalid)?);
        Ok(())
    }
}
type TestVault = Vault<Memory, ProtectedModel, Crypto>;
fn mount(s: &Shared) -> Result<TestVault, Error> {
    Vault::mount(
        Memory(s.clone()),
        ProtectedModel(s.clone()),
        Crypto(s.clone()),
    )
}
fn replace(v: &mut TestVault, bytes: &[u8]) -> Result<(), Error> {
    v.replace(
        &mut Bytes(bytes.to_vec()),
        &Sha256::digest(bytes).into(),
        &mut Approved,
    )
}
fn all(v: &mut TestVault, length: usize) -> Vec<u8> {
    let mut result = Vec::new();
    let mut chunk = [0; PAYLOAD_BYTES];
    for i in 0..length.div_ceil(PAYLOAD_BYTES) {
        let n = v.read_chunk(i as u32, &mut chunk, &mut Approved).unwrap();
        result.extend_from_slice(&chunk[..n]);
    }
    result
}

#[test]
fn every_update_cut_recovers_only_old_or_complete_new_and_burns_nonce() {
    for cut in 0..7 {
        for commit_then_error in [false, true] {
            let state = Shared::default();
            let old = vec![0x31; PAYLOAD_BYTES + 7];
            let new = vec![0x72; PAYLOAD_BYTES + 7];
            replace(&mut mount(&state).unwrap(), &old).unwrap();
            let mut v = mount(&state).unwrap();
            {
                let mut s = state.borrow_mut();
                s.operations = 0;
                s.cut = Some(cut);
                s.commit_then_error = commit_then_error;
            }
            assert!(replace(&mut v, &new).is_err());
            let mut out = [0xa5; PAYLOAD_BYTES];
            assert_eq!(
                v.read_chunk(0, &mut out, &mut Approved),
                Err(Error::Faulted)
            );
            assert!(out.iter().all(|b| *b == 0));
            drop(v);
            state.borrow_mut().cut = None;
            let mut recovered = mount(&state).unwrap();
            let actual = all(&mut recovered, old.len());
            assert!(actual == old || actual == new);
            replace(&mut recovered, &new).unwrap();
            assert_eq!(all(&mut recovered, new.len()), new);
            assert_eq!(state.borrow().counter, 3);
        }
    }
}

#[test]
fn old_ciphertext_cannot_be_replayed_against_new_protected_head() {
    let state = Shared::default();
    let old = vec![1; PAYLOAD_BYTES + 1];
    let new = vec![2; old.len()];
    replace(&mut mount(&state).unwrap(), &old).unwrap();
    let old_image = state.borrow().sectors.clone();
    replace(&mut mount(&state).unwrap(), &new).unwrap();
    state.borrow_mut().sectors = old_image;
    assert!(matches!(
        mount(&state),
        Err(Error::Storage | Error::DigestMismatch)
    ));
}

#[test]
fn post_mount_mutation_or_chunk_swap_releases_no_plaintext() {
    for swap in [false, true] {
        let state = Shared::default();
        let data = vec![0x81; PAYLOAD_BYTES + 1];
        replace(&mut mount(&state).unwrap(), &data).unwrap();
        let mut v = mount(&state).unwrap();
        {
            let mut s = state.borrow_mut();
            let bank = s.active.unwrap().bank;
            if swap {
                let a = s.sectors[&(bank, 0)];
                let b = s.sectors[&(bank, 1)];
                s.sectors.insert((bank, 0), b);
                s.sectors.insert((bank, 1), a);
            } else {
                s.sectors.get_mut(&(bank, 0)).unwrap()[111] ^= 1;
            }
        }
        let mut out = [0xa5; PAYLOAD_BYTES];
        assert_eq!(v.read_chunk(0, &mut out, &mut Approved), Err(Error::Crypto));
        assert!(out.iter().all(|x| *x == 0));
    }
}

#[test]
fn changed_source_digest_never_changes_committed_head() {
    let state = Shared::default();
    let old = b"approved note";
    replace(&mut mount(&state).unwrap(), old).unwrap();
    let previous = state.borrow().active;
    let mut v = mount(&state).unwrap();
    assert_eq!(
        v.replace(
            &mut Bytes(b"different note".to_vec()),
            &Sha256::digest(old).into(),
            &mut Approved,
        ),
        Err(Error::DigestMismatch)
    );
    assert_eq!(state.borrow().active, previous);
    assert_eq!(all(&mut mount(&state).unwrap(), old.len()), old);
}

#[test]
fn authorization_expires_during_decrypt_without_releasing_plaintext() {
    let state = Shared::default();
    replace(&mut mount(&state).unwrap(), b"private recovery code").unwrap();
    let mut v = mount(&state).unwrap();
    let mut output = [0xa5; PAYLOAD_BYTES];
    assert_eq!(
        v.read_chunk(0, &mut output, &mut Budget(2)),
        Err(Error::Authorization)
    );
    assert_eq!(state.borrow().opens, 1);
    assert!(output.iter().all(|b| *b == 0));
}

#[test]
fn cancellation_at_every_write_checkpoint_preserves_the_old_commit() {
    for budget in 0..9 {
        let state = Shared::default();
        replace(&mut mount(&state).unwrap(), b"old record").unwrap();
        let old = state.borrow().active;
        let mut v = mount(&state).unwrap();
        assert_eq!(
            v.replace(
                &mut Bytes(b"new record".to_vec()),
                &Sha256::digest(b"new record").into(),
                &mut Budget(budget)
            ),
            Err(Error::Authorization)
        );
        assert_eq!(state.borrow().active, old);
    }
}

struct Splice {
    old_len: u32,
    start: u32,
    removed: u32,
    insert: Vec<u8>,
    calls: usize,
}
impl SnapshotSource for Splice {
    fn length(&self) -> u32 {
        self.old_len - self.removed + self.insert.len() as u32
    }
    fn read(
        &mut self,
        previous: &mut dyn SnapshotReader,
        offset: u32,
        out: &mut [u8],
    ) -> Result<(), Error> {
        assert!(out.len() <= PAYLOAD_BYTES);
        assert_eq!(previous.length(), self.old_len);
        self.calls += 1;
        let mut pos = offset;
        let mut written = 0;
        let insert_end = self.start + self.insert.len() as u32;
        while written < out.len() {
            if pos < self.start {
                let count = (self.start - pos).min((out.len() - written) as u32) as usize;
                previous.read_at(pos, &mut out[written..written + count])?;
                written += count;
                pos += count as u32;
            } else if pos < insert_end {
                let count = (insert_end - pos).min((out.len() - written) as u32) as usize;
                let from = (pos - self.start) as usize;
                out[written..written + count].copy_from_slice(&self.insert[from..from + count]);
                written += count;
                pos += count as u32;
            } else {
                previous.read_at(
                    pos - insert_end + self.start + self.removed,
                    &mut out[written..],
                )?;
                break;
            }
        }
        Ok(())
    }
}
fn splice(old: &[u8], start: usize, removed: usize, insert: &[u8]) -> (Splice, Vec<u8>) {
    let mut result = old[..start].to_vec();
    result.extend(insert);
    result.extend(&old[start + removed..]);
    (
        Splice {
            old_len: old.len() as u32,
            start: start as u32,
            removed: removed as u32,
            insert: insert.into(),
            calls: 0,
        },
        result,
    )
}

#[test]
fn streaming_edits_copy_old_sectors_with_one_owned_backend_at_all_boundaries() {
    let old: Vec<_> = (0..PAYLOAD_BYTES * 3 + 7)
        .map(|i| (i % 251) as u8)
        .collect();
    for (start, removed, insert) in [
        (0, 0, vec![3; 12]),
        (PAYLOAD_BYTES - 1, 2, vec![9; PAYLOAD_BYTES + 3]),
        (PAYLOAD_BYTES, 17, vec![]),
        (old.len(), 0, vec![7; 10]),
        (0, old.len(), vec![]),
    ] {
        let state = Shared::default();
        let mut vault = mount(&state).unwrap();
        replace(&mut vault, &old).unwrap();
        let (mut edit, expected) = splice(&old, start, removed, &insert);
        vault
            .replace_from(&mut edit, &Sha256::digest(&expected).into(), &mut Approved)
            .unwrap();
        assert_eq!(edit.calls, expected.len().div_ceil(PAYLOAD_BYTES));
        assert_eq!(vault.length(&mut Approved).unwrap(), expected.len() as u32);
        let mut got = vec![0; expected.len()];
        vault.read_at(0, &mut got, &mut Approved).unwrap();
        assert_eq!(got, expected);
        let mut recovered = mount(&state).unwrap();
        assert_eq!(all(&mut recovered, expected.len()), expected);
    }
}

#[test]
fn full_one_mib_streamed_update_never_requests_a_large_plaintext_scratch_buffer() {
    let old = vec![0x35; device_vault::MAX_SNAPSHOT_BYTES as usize];
    let state = Shared::default();
    let mut vault = mount(&state).unwrap();
    replace(&mut vault, &old).unwrap();
    let (mut edit, expected) = splice(&old, PAYLOAD_BYTES - 3, 100, &[0x77; 100]);
    vault
        .replace_from(&mut edit, &Sha256::digest(&expected).into(), &mut Approved)
        .unwrap();
    assert_eq!(edit.calls, old.len().div_ceil(PAYLOAD_BYTES));
    let mut output = vec![0; 300];
    vault
        .read_at((PAYLOAD_BYTES - 100) as u32, &mut output, &mut Approved)
        .unwrap();
    assert_eq!(output, expected[PAYLOAD_BYTES - 100..PAYLOAD_BYTES + 200]);
}

#[test]
fn every_streamed_edit_io_cut_keeps_only_the_old_or_complete_new_commit() {
    let old = vec![0x12; PAYLOAD_BYTES + 17];
    // First measure all read/write/flush/ledger calls, including reads of old
    // plaintext while constructing the inactive bank. Then cut each one.
    let probe = Shared::default();
    let mut vault = mount(&probe).unwrap();
    replace(&mut vault, &old).unwrap();
    let (mut edit, expected) = splice(&old, PAYLOAD_BYTES - 2, 7, b"changed");
    probe.borrow_mut().operations = 0;
    vault
        .replace_from(&mut edit, &Sha256::digest(&expected).into(), &mut Approved)
        .unwrap();
    let operations = probe.borrow().operations;
    for cut in 0..operations {
        for ambiguous_commit in [false, true] {
            let state = Shared::default();
            let mut vault = mount(&state).unwrap();
            replace(&mut vault, &old).unwrap();
            {
                let mut s = state.borrow_mut();
                s.operations = 0;
                s.cut = Some(cut);
                s.commit_then_error = ambiguous_commit;
            }
            let (mut edit, _) = splice(&old, PAYLOAD_BYTES - 2, 7, b"changed");
            assert!(
                vault
                    .replace_from(&mut edit, &Sha256::digest(&expected).into(), &mut Approved)
                    .is_err()
            );
            let mut out = [0xa5; 16];
            assert_eq!(
                vault.read_at(0, &mut out, &mut Approved),
                Err(Error::Faulted)
            );
            assert_eq!(out, [0; 16]);
            state.borrow_mut().cut = None;
            let mut reboot = mount(&state).unwrap();
            let got = all(&mut reboot, old.len());
            assert!(got == old || got == expected, "cut {cut}");
            // An epoch was burned even when the first reserve reply was lost.
            assert_eq!(state.borrow().counter, 2);
        }
    }
}

#[test]
fn range_bounds_and_empty_state_are_checked_without_releasing_partial_output() {
    for (offset, count) in [(0, 1), (1, 0), (u32::MAX, 8)] {
        let state = Shared::default();
        let mut vault = mount(&state).unwrap();
        let mut out = vec![0xa5; count];
        assert_eq!(
            vault.read_at(offset, &mut out, &mut Approved),
            Err(Error::Invalid)
        );
        assert!(out.iter().all(|b| *b == 0));
    }
    let state = Shared::default();
    let mut vault = mount(&state).unwrap();
    assert_eq!(vault.length(&mut Approved).unwrap(), 0);
    vault.read_at(0, &mut [], &mut Approved).unwrap();
}

#[test]
fn later_range_tag_failure_and_cancellation_wipe_all_earlier_copied_plaintext() {
    let state = Shared::default();
    let mut vault = mount(&state).unwrap();
    let old = vec![0x76; PAYLOAD_BYTES + 10];
    replace(&mut vault, &old).unwrap();
    let bank = state.borrow().active.unwrap().bank;
    state.borrow_mut().sectors.get_mut(&(bank, 1)).unwrap()[10] ^= 1;
    let mut out = vec![0xa5; old.len()];
    assert_eq!(
        vault.read_at(0, &mut out, &mut Approved),
        Err(Error::Crypto)
    );
    assert!(out.iter().all(|b| *b == 0));
    for checks in 0..8 {
        let state = Shared::default();
        let mut vault = mount(&state).unwrap();
        replace(&mut vault, &old).unwrap();
        let mut out = vec![0xa5; old.len()];
        assert_eq!(
            vault.read_at(0, &mut out, &mut Budget(checks)),
            Err(Error::Authorization)
        );
        assert!(out.iter().all(|b| *b == 0));
    }
}
