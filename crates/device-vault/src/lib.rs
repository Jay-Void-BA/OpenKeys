#![no_std]
#![forbid(unsafe_code)]
//! Bounded encrypted snapshot transactions for the R04 8MiB NOR. This is an
//! internal protected-domain component, not a host authorization interface.
//! All private reads/writes require the runtime's consumed fingerprint grant.
//! The ledger MUST be authenticated, durable and rollback-resistant in SE.
//! A normal Flash integer or an in-memory counter is not a valid adapter.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub const SECTOR_BYTES: usize = 4096;
pub const PAYLOAD_BYTES: usize = SECTOR_BYTES - 16;
pub const MAX_SNAPSHOT_BYTES: u32 = 1_048_576;
pub const BANK_BYTES: u32 = 4 * 1024 * 1024;
const DOMAIN: &[u8; 16] = b"OpenKeysVaultV1\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Storage,
    Ledger,
    Crypto,
    Invalid,
    Faulted,
    DigestMismatch,
    Rollback,
    Authorization,
}

/// Stored inside the authenticated SE ledger, not a plaintext NOR header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head {
    pub bank: u8,
    pub epoch: u64,
    pub length: u32,
    pub root: [u8; 32],
}

pub trait Ledger {
    /// None is allowed only for an explicitly provisioned empty vault state.
    /// Missing/corrupt objects, backup loss or unpaired SE must return Err.
    fn active(&mut self) -> Result<Option<Head>, Error>;
    /// Durably reserve a positive, never-reused per-key epoch BEFORE any AEAD.
    /// Ambiguous writes must return Err. Reset/abort/cancel never refunds it.
    fn reserve_epoch(&mut self) -> Result<u64, Error>;
    /// Atomically replace the protected head only after all NOR writes/reads
    /// and flushes have succeeded. An uncertain outcome returns Err; caller
    /// latches until a new boot reads the actual protected head.
    fn commit(&mut self, head: &Head) -> Result<(), Error>;
}

/// Complete sectors; adapter must bounds-check bank/index and verify writes.
/// Each bank is4MiB. Writes erase the selected sector and program/readback its
/// pages. No write may alter a sector in the other bank.
pub trait CiphertextStorage {
    fn read(&mut self, bank: u8, index: u32, sector: &mut [u8; SECTOR_BYTES]) -> Result<(), Error>;
    fn write(&mut self, bank: u8, index: u32, sector: &[u8; SECTOR_BYTES]) -> Result<(), Error>;
    fn flush(&mut self) -> Result<(), Error>;
}

/// AEAD under a unique device-vault key owned by the protected boundary.
/// AES-256-GCM; nonce=epochBE64||indexBE32. Epochs never repeat under this key.
/// The adapter must verify the tag before plaintext is made usable, use
/// constant-time tag checks and wipe any tentative output on failure.
pub trait Aead {
    fn seal(
        &mut self,
        nonce: &[u8; 12],
        aad: &[u8; 48],
        bytes: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<[u8; 16], Error>;
    fn open(
        &mut self,
        nonce: &[u8; 12],
        aad: &[u8; 48],
        tag: &[u8; 16],
        bytes: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<(), Error>;
}

pub trait Source {
    fn length(&self) -> u32;
    /// Fill the complete requested slice; never report partial reads as Ok.
    fn read(&mut self, offset: u32, output: &mut [u8]) -> Result<(), Error>;
}

/// Protected runtime validation of the exact request/channel, live monotonic
/// deadline, cancellation and latched hardware faults. Not a cached host flag.
/// The final ledger commit must remain in this same protected execution path.
pub trait Checkpoint {
    fn check(&mut self) -> Result<(), Error>;
}

fn chunks(length: u32) -> u32 {
    length.div_ceil(PAYLOAD_BYTES as u32)
}
fn used(head: &Head, index: u32) -> usize {
    (head.length - index * PAYLOAD_BYTES as u32).min(PAYLOAD_BYTES as u32) as usize
}
fn aad(head: &Head, index: u32) -> [u8; 48] {
    let mut out = [0; 48];
    out[..16].copy_from_slice(DOMAIN);
    out[16..24].copy_from_slice(&head.epoch.to_be_bytes());
    out[24] = head.bank;
    out[25..29].copy_from_slice(&head.length.to_be_bytes());
    out[29..33].copy_from_slice(&index.to_be_bytes());
    out[33..35].copy_from_slice(&(used(head, index) as u16).to_be_bytes());
    out
}
fn nonce(head: &Head, index: u32) -> [u8; 12] {
    let mut out = [0; 12];
    out[..8].copy_from_slice(&head.epoch.to_be_bytes());
    out[8..].copy_from_slice(&index.to_be_bytes());
    out
}
fn root_hasher(head: &Head) -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update([head.bank]);
    hash.update(head.epoch.to_be_bytes());
    hash.update(head.length.to_be_bytes());
    hash
}
fn valid(head: &Head) -> bool {
    head.bank < 2
        && head.epoch > 0
        && head.length <= MAX_SNAPSHOT_BYTES
        && u64::from(chunks(head.length)) * SECTOR_BYTES as u64 <= u64::from(BANK_BYTES)
}

pub struct Vault<S, L, C> {
    storage: S,
    ledger: L,
    crypto: C,
    head: Option<Head>,
    last_reserved: u64,
    faulted: bool,
}

impl<S: CiphertextStorage, L: Ledger, C: Aead> Vault<S, L, C> {
    /// Verify the entire ciphertext commitment before making reads available.
    /// This performs no private-data decryption and grants no user access.
    pub fn mount(storage: S, mut ledger: L, crypto: C) -> Result<Self, Error> {
        let head = ledger.active()?;
        if head.as_ref().is_some_and(|h| !valid(h)) {
            return Err(Error::Invalid);
        }
        let mut vault = Self {
            storage,
            ledger,
            crypto,
            head,
            last_reserved: head.map_or(0, |h| h.epoch),
            faulted: false,
        };
        if let Some(h) = head {
            let mut hash = root_hasher(&h);
            let mut sector = [0; SECTOR_BYTES];
            for index in 0..chunks(h.length) {
                vault.storage.read(h.bank, index, &mut sector)?;
                hash.update(sector);
            }
            if !bool::from(hash.finalize().as_slice().ct_eq(&h.root)) {
                return Err(Error::DigestMismatch);
            }
        }
        Ok(vault)
    }

    /// A complete replacement, committed only after source digest and NOR
    /// readback match. expected_plaintext is device-derived and bound to the
    /// exact approved import, not an unauthenticated host-selected checksum.
    pub fn replace(
        &mut self,
        source: &mut impl Source,
        expected_plaintext: &[u8; 32],
        authorization: &mut impl Checkpoint,
    ) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        let result = self.replace_inner(source, expected_plaintext, authorization);
        if result.is_err() {
            self.faulted = true;
        }
        result
    }

    fn replace_inner(
        &mut self,
        source: &mut impl Source,
        expected: &[u8; 32],
        authorization: &mut impl Checkpoint,
    ) -> Result<(), Error> {
        authorization.check()?;
        let length = source.length();
        if length > MAX_SNAPSHOT_BYTES {
            return Err(Error::Invalid);
        }
        let epoch = self.ledger.reserve_epoch()?;
        if epoch == 0 || epoch <= self.last_reserved {
            return Err(Error::Rollback);
        }
        self.last_reserved = epoch;
        authorization.check()?;
        let mut head = Head {
            bank: self.head.map_or(0, |h| 1 - h.bank),
            epoch,
            length,
            root: [0; 32],
        };
        let mut root = root_hasher(&head);
        let mut plaintext = Sha256::new();
        let mut sector = [0; SECTOR_BYTES];
        let mut readback = [0; SECTOR_BYTES];
        let mut bytes = Zeroizing::new([0; PAYLOAD_BYTES]);
        for index in 0..chunks(length) {
            authorization.check()?;
            bytes.fill(0);
            let count = used(&head, index);
            source.read(index * PAYLOAD_BYTES as u32, &mut bytes[..count])?;
            authorization.check()?;
            plaintext.update(&bytes[..count]);
            let tag = self
                .crypto
                .seal(&nonce(&head, index), &aad(&head, index), &mut bytes)?;
            authorization.check()?;
            sector[..PAYLOAD_BYTES].copy_from_slice(&bytes[..]);
            sector[PAYLOAD_BYTES..].copy_from_slice(&tag);
            self.storage.write(head.bank, index, &sector)?;
            authorization.check()?;
            self.storage.read(head.bank, index, &mut readback)?;
            authorization.check()?;
            if !bool::from(sector.ct_eq(&readback)) {
                return Err(Error::Storage);
            }
            root.update(sector);
        }
        if source.length() != length || !bool::from(plaintext.finalize().as_slice().ct_eq(expected))
        {
            return Err(Error::DigestMismatch);
        }
        authorization.check()?;
        self.storage.flush()?;
        authorization.check()?;
        head.root = root.finalize().into();
        self.ledger.commit(&head)?;
        self.head = Some(head);
        Ok(())
    }

    /// One bounded sector of plaintext after the caller's private-operation
    /// authorization. The output is zeroed on every error, including failures
    /// after partial decryption. Return value is the exact useful byte count.
    pub fn read_chunk(
        &mut self,
        index: u32,
        output: &mut [u8; PAYLOAD_BYTES],
        authorization: &mut impl Checkpoint,
    ) -> Result<usize, Error> {
        output.fill(0);
        if self.faulted {
            return Err(Error::Faulted);
        }
        let result = self.read_inner(index, output, authorization);
        if result.is_err() {
            output.fill(0);
            self.faulted = true;
        }
        result
    }

    fn read_inner(
        &mut self,
        index: u32,
        output: &mut [u8; PAYLOAD_BYTES],
        authorization: &mut impl Checkpoint,
    ) -> Result<usize, Error> {
        authorization.check()?;
        let head = self.head.ok_or(Error::Invalid)?;
        if index >= chunks(head.length) {
            return Err(Error::Invalid);
        }
        let mut sector = [0; SECTOR_BYTES];
        self.storage.read(head.bank, index, &mut sector)?;
        authorization.check()?;
        let mut plaintext = Zeroizing::new([0; PAYLOAD_BYTES]);
        plaintext.copy_from_slice(&sector[..PAYLOAD_BYTES]);
        let tag = sector[PAYLOAD_BYTES..]
            .try_into()
            .map_err(|_| Error::Invalid)?;
        self.crypto.open(
            &nonce(&head, index),
            &aad(&head, index),
            tag,
            &mut plaintext,
        )?;
        authorization.check()?;
        let count = used(&head, index);
        if plaintext[count..].iter().any(|x| *x != 0) {
            return Err(Error::Invalid);
        }
        output[..count].copy_from_slice(&plaintext[..count]);
        Ok(count)
    }
}
