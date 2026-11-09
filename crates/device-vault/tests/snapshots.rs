use aes_gcm::{Aes256Gcm, KeyInit, aead::AeadInPlace};
use device_vault::{
    Aead, CiphertextStorage, Error, Head, Ledger, PAYLOAD_BYTES, SECTOR_BYTES, Source, Vault,
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
