//! Test-only encrypted media/SE model, never linked into production firmware.
extern crate std;
use aes_gcm::{Aes256Gcm, KeyInit, aead::AeadInPlace};
use device_vault::{
    Aead, Checkpoint, CiphertextStorage, Head, Ledger, PAYLOAD_BYTES, SECTOR_BYTES, Vault,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
pub(crate) type Shared = Rc<RefCell<Memory>>;
#[derive(Default)]
pub(crate) struct Memory {
    pub(crate) sectors: BTreeMap<(u8, u32), [u8; SECTOR_BYTES]>,
    pub(crate) head: Option<Head>,
    pub(crate) epoch: u64,
    pub(crate) operations: usize,
    pub(crate) cut: Option<usize>,
    pub(crate) commit_then_error: bool,
    pub(crate) opens: usize,
    pub(crate) nonces: BTreeSet<[u8; 12]>,
}
impl Memory {
    fn step(&mut self) -> bool {
        let fail = self.cut == Some(self.operations);
        self.operations += 1;
        fail
    }
}
pub(crate) struct Flash(pub(crate) Shared);
impl CiphertextStorage for Flash {
    fn read(
        &mut self,
        b: u8,
        i: u32,
        out: &mut [u8; SECTOR_BYTES],
    ) -> Result<(), device_vault::Error> {
        let mut s = self.0.borrow_mut();
        if s.step() {
            return Err(device_vault::Error::Storage);
        }
        out.copy_from_slice(s.sectors.get(&(b, i)).ok_or(device_vault::Error::Storage)?);
        Ok(())
    }
    fn write(
        &mut self,
        b: u8,
        i: u32,
        data: &[u8; SECTOR_BYTES],
    ) -> Result<(), device_vault::Error> {
        let mut s = self.0.borrow_mut();
        if s.step() {
            let mut partial = [0xff; SECTOR_BYTES];
            partial[..100].copy_from_slice(&data[..100]);
            s.sectors.insert((b, i), partial);
            return Err(device_vault::Error::Storage);
        }
        s.sectors.insert((b, i), *data);
        Ok(())
    }
    fn flush(&mut self) -> Result<(), device_vault::Error> {
        if self.0.borrow_mut().step() {
            Err(device_vault::Error::Storage)
        } else {
            Ok(())
        }
    }
}
pub(crate) struct Se(pub(crate) Shared);
impl Ledger for Se {
    fn active(&mut self) -> Result<Option<Head>, device_vault::Error> {
        Ok(self.0.borrow().head)
    }
    fn reserve_epoch(&mut self) -> Result<u64, device_vault::Error> {
        let mut s = self.0.borrow_mut();
        s.epoch += 1;
        if s.step() {
            Err(device_vault::Error::Ledger)
        } else {
            Ok(s.epoch)
        }
    }
    fn commit(&mut self, h: &Head) -> Result<(), device_vault::Error> {
        let mut s = self.0.borrow_mut();
        if s.step() {
            if s.commit_then_error {
                s.head = Some(*h);
            }
            Err(device_vault::Error::Ledger)
        } else {
            s.head = Some(*h);
            Ok(())
        }
    }
}
pub(crate) struct Crypto(pub(crate) Shared);
impl Aead for Crypto {
    fn seal(
        &mut self,
        n: &[u8; 12],
        a: &[u8; 48],
        b: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<[u8; 16], device_vault::Error> {
        assert!(self.0.borrow_mut().nonces.insert(*n));
        Aes256Gcm::new_from_slice(&[0x43; 32])
            .unwrap()
            .encrypt_in_place_detached(n.into(), a, b)
            .map(Into::into)
            .map_err(|_| device_vault::Error::Crypto)
    }
    fn open(
        &mut self,
        n: &[u8; 12],
        a: &[u8; 48],
        t: &[u8; 16],
        b: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<(), device_vault::Error> {
        self.0.borrow_mut().opens += 1;
        Aes256Gcm::new_from_slice(&[0x43; 32])
            .unwrap()
            .decrypt_in_place_detached(n.into(), a, b, t.into())
            .map_err(|_| device_vault::Error::Crypto)
    }
}
pub(crate) struct Allow;
impl Checkpoint for Allow {
    fn check(&mut self) -> Result<(), device_vault::Error> {
        Ok(())
    }
}
pub(crate) struct Budget(pub(crate) usize);
impl Checkpoint for Budget {
    fn check(&mut self) -> Result<(), device_vault::Error> {
        if self.0 == 0 {
            Err(device_vault::Error::Authorization)
        } else {
            self.0 -= 1;
            Ok(())
        }
    }
}
pub(crate) fn vault(s: &Shared) -> Vault<Flash, Se, Crypto> {
    Vault::mount(Flash(s.clone()), Se(s.clone()), Crypto(s.clone())).unwrap()
}
