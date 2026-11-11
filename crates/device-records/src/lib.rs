#![no_std]
#![forbid(unsafe_code)]
//! Bounded logical records on the R04 authenticated encrypted snapshot.
//! This is a protected-root component, not a host authorization interface.
//! Every operation, including index/count access, needs the live root checkpoint.
extern crate alloc;
pub mod item;
pub mod otp;
use alloc::vec::Vec;
use device_vault::{
    Aead, Checkpoint, CiphertextStorage, Ledger, PAYLOAD_BYTES, SnapshotReader, SnapshotSource,
    Vault,
};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

const MAGIC: &[u8; 16] = b"OpenKeysStoreV1\0";
const HEADER: u32 = 32;
const RECORD: u32 = 8;
pub const MAX_RECORDS: usize = 4096;
pub const MAX_PRIVATE_ITEMS: usize = 256;
pub const MAX_VALUE_BYTES: usize = 16_896;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Authorization,
    Storage,
    Format,
    Input,
    Capacity,
    Closed,
}
impl From<device_vault::Error> for Error {
    fn from(e: device_vault::Error) -> Self {
        if e == device_vault::Error::Authorization {
            Self::Authorization
        } else {
            Self::Storage
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u16)]
pub enum Namespace {
    Fido = 1,
    Note = 2,
    Recovery = 3,
}
impl Namespace {
    fn decode(n: u16) -> Result<Self, Error> {
        match n {
            1 => Ok(Self::Fido),
            2 => Ok(Self::Note),
            3 => Ok(Self::Recovery),
            _ => Err(Error::Format),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id {
    pub namespace: Namespace,
    pub key: u16,
}
impl Id {
    fn valid(self) -> bool {
        match self.namespace {
            Namespace::Fido => self.key < 4096,
            _ => self.key != 0,
        }
    }
    fn accepts(self, len: usize) -> bool {
        match self.namespace {
            Namespace::Fido => len <= 1023, // OpenSK stores present empty flags.
            _ => len > 0 && len <= MAX_VALUE_BYTES,
        }
    }
}
#[derive(Clone, Copy)]
struct Entry {
    id: Id,
    start: u32,
    length: u32,
}
struct Index {
    entries: Vec<Entry>,
    revision: u64,
    length: u32,
}
impl Drop for Index {
    fn drop(&mut self) {
        for entry in &mut self.entries {
            entry.id.key.zeroize();
            entry.start.zeroize();
            entry.length.zeroize();
        }
        self.revision.zeroize();
        self.length.zeroize();
    }
}
pub struct Records<S, L, C> {
    vault: Vault<S, L, C>,
    index: Option<Index>,
    closed: bool,
}

fn check(a: &mut impl Checkpoint) -> Result<(), Error> {
    a.check().map_err(Into::into)
}
fn header(count: usize, revision: u64) -> [u8; 32] {
    let mut out = [0; 32];
    out[..16].copy_from_slice(MAGIC);
    out[16..20].copy_from_slice(&(count as u32).to_be_bytes());
    out[20..28].copy_from_slice(&revision.to_be_bytes());
    out
}
fn record(id: Id, len: u32) -> [u8; 8] {
    let mut out = [0; 8];
    out[..2].copy_from_slice(&(id.namespace as u16).to_be_bytes());
    out[2..4].copy_from_slice(&id.key.to_be_bytes());
    out[4..].copy_from_slice(&len.to_be_bytes());
    out
}
impl<S: CiphertextStorage, L: Ledger, C: Aead> Records<S, L, C> {
    pub fn new(vault: Vault<S, L, C>) -> Self {
        Self {
            vault,
            index: None,
            closed: false,
        }
    }
    /// Drop cached private index metadata on device lock/end of scope. This is
    /// not permission to reopen: the next access still needs a live checkpoint.
    pub fn lock(&mut self) {
        self.index = None;
    }
    fn load(&mut self, a: &mut impl Checkpoint) -> Result<(), Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        check(a)?;
        if self.index.is_some() {
            return Ok(());
        }
        let result = (|| {
            let length = self.vault.length(a)?;
            if length == 0 {
                return Ok(Index {
                    entries: Vec::new(),
                    revision: 0,
                    length: 0,
                });
            }
            if length < HEADER {
                return Err(Error::Format);
            }
            // One cached sector prevents rereading a complete4KiB sector for
            // each small record header. No record values are cached in Index.
            let mut reader = Cursor {
                vault: &mut self.vault,
                authorization: a,
                bytes: Zeroizing::new([0; PAYLOAD_BYTES]),
                chunk: None,
                useful: 0,
            };
            let mut h = [0; 32];
            reader.read(0, &mut h)?;
            if &h[..16] != MAGIC || h[28..] != [0; 4] {
                return Err(Error::Format);
            }
            let count =
                u32::from_be_bytes(h[16..20].try_into().map_err(|_| Error::Format)?) as usize;
            let revision = u64::from_be_bytes(h[20..28].try_into().map_err(|_| Error::Format)?);
            if count > MAX_RECORDS || revision == 0 {
                return Err(Error::Format);
            }
            let mut entries = Vec::with_capacity(count);
            let mut pos = HEADER;
            let mut private = 0;
            for _ in 0..count {
                let end = pos.checked_add(RECORD).ok_or(Error::Format)?;
                if end > length {
                    return Err(Error::Format);
                }
                let mut h = [0; 8];
                reader.read(pos, &mut h)?;
                let id = Id {
                    namespace: Namespace::decode(u16::from_be_bytes([h[0], h[1]]))?,
                    key: u16::from_be_bytes([h[2], h[3]]),
                };
                let size = u32::from_be_bytes(h[4..].try_into().map_err(|_| Error::Format)?);
                if !id.valid()
                    || !id.accepts(size as usize)
                    || entries.last().is_some_and(|e: &Entry| e.id >= id)
                {
                    return Err(Error::Format);
                }
                if id.namespace != Namespace::Fido {
                    private += 1;
                    if private > MAX_PRIVATE_ITEMS {
                        return Err(Error::Format);
                    }
                }
                entries.push(Entry {
                    id,
                    start: pos,
                    length: size,
                });
                pos = end.checked_add(size).ok_or(Error::Format)?;
                if pos > length {
                    return Err(Error::Format);
                }
            }
            if pos != length {
                return Err(Error::Format);
            }
            check(reader.authorization)?;
            Ok(Index {
                entries,
                revision,
                length,
            })
        })();
        match result {
            Ok(index) => {
                self.index = Some(index);
                Ok(())
            }
            Err(e) => {
                self.closed = true;
                self.index = None;
                Err(e)
            }
        }
    }
    pub fn ids(
        &mut self,
        namespace: Namespace,
        a: &mut impl Checkpoint,
    ) -> Result<Vec<u16>, Error> {
        self.load(a)?;
        let mut ids: Vec<u16> = self
            .index
            .as_ref()
            .ok_or(Error::Closed)?
            .entries
            .iter()
            .filter(|e| e.id.namespace == namespace)
            .map(|e| e.id.key)
            .collect();
        if let Err(e) = check(a) {
            ids.zeroize();
            return Err(e);
        }
        Ok(ids)
    }
    fn get(
        &mut self,
        id: Id,
        a: &mut impl Checkpoint,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        self.load(a)?;
        if !id.valid() {
            return Err(Error::Input);
        }
        let index = self.index.as_ref().ok_or(Error::Closed)?;
        let Some(entry) = index
            .entries
            .binary_search_by_key(&id, |e| e.id)
            .ok()
            .map(|i| index.entries[i])
        else {
            check(a)?;
            return Ok(None);
        };
        let mut value = Zeroizing::new(alloc::vec![0;entry.length as usize]);
        if let Err(e) = self.vault.read_at(entry.start + RECORD, &mut value, a) {
            self.closed = true;
            self.index = None;
            return Err(e.into());
        }
        check(a)?;
        Ok(Some(value))
    }
    /// Exact operation/payload authorization and the namespace access decision
    /// are checked by the protected dispatcher. This internal method does not
    /// accept a host Boolean. Notes/recovery values need their schema validator.
    fn put(&mut self, id: Id, value: &[u8], a: &mut impl Checkpoint) -> Result<(), Error> {
        self.edit(id, Some(value), a)
    }
    pub fn get_fido(
        &mut self,
        key: u16,
        a: &mut impl Checkpoint,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        self.get(
            Id {
                namespace: Namespace::Fido,
                key,
            },
            a,
        )
    }
    pub fn put_fido(
        &mut self,
        key: u16,
        value: &[u8],
        a: &mut impl Checkpoint,
    ) -> Result<(), Error> {
        self.put(
            Id {
                namespace: Namespace::Fido,
                key,
            },
            value,
            a,
        )
    }
    pub fn get_private(
        &mut self,
        id: Id,
        a: &mut impl Checkpoint,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        check(a)?;
        if id.namespace == Namespace::Fido {
            return Err(Error::Input);
        }
        let value = self.get(id, a)?;
        if value
            .as_ref()
            .is_some_and(|v| item::View::parse(id.namespace, v).is_err())
        {
            self.closed = true;
            self.index = None;
            return Err(Error::Format);
        }
        check(a)?;
        Ok(value)
    }
    pub fn put_private(
        &mut self,
        id: Id,
        value: &[u8],
        a: &mut impl Checkpoint,
    ) -> Result<(), Error> {
        check(a)?;
        item::View::parse(id.namespace, value).map_err(|_| Error::Input)?;
        self.put(id, value, a)
    }
    pub fn remove(&mut self, id: Id, a: &mut impl Checkpoint) -> Result<(), Error> {
        self.edit(id, None, a)
    }
    fn edit(&mut self, id: Id, value: Option<&[u8]>, a: &mut impl Checkpoint) -> Result<(), Error> {
        self.load(a)?;
        if !id.valid() || value.is_some_and(|v| !id.accepts(v.len())) {
            return Err(Error::Input);
        }
        let index = self.index.as_ref().ok_or(Error::Closed)?;
        let position = index.entries.binary_search_by_key(&id, |e| e.id);
        if value.is_none() && position.is_err() {
            check(a)?;
            return Ok(());
        }
        let (start, removed, new_count) = match position {
            Ok(i) => {
                let e = index.entries[i];
                (
                    e.start,
                    RECORD + e.length,
                    index.entries.len() - usize::from(value.is_none()),
                )
            }
            Err(i) => {
                if index.entries.len() == MAX_RECORDS {
                    return Err(Error::Capacity);
                }
                if id.namespace != Namespace::Fido
                    && index
                        .entries
                        .iter()
                        .filter(|e| e.id.namespace != Namespace::Fido)
                        .count()
                        == MAX_PRIVATE_ITEMS
                {
                    return Err(Error::Capacity);
                }
                (
                    index
                        .entries
                        .get(i)
                        .map_or(index.length.max(HEADER), |e| e.start),
                    0,
                    index.entries.len() + 1,
                )
            }
        };
        let revision = index.revision.checked_add(1).ok_or(Error::Capacity)?;
        let mut plan = Plan::new(
            index.length,
            start,
            removed,
            id,
            value,
            header(new_count, revision),
        )?;
        let expected = plan.digest(&mut Old {
            vault: &mut self.vault,
            authorization: a,
            length: index.length,
        });
        let expected = match expected {
            Ok(digest) => digest,
            Err(error) => {
                self.closed = true;
                self.index = None;
                return Err(error);
            }
        };
        let result = self.vault.replace_from(&mut plan, &expected, a);
        self.index = None;
        if let Err(e) = result {
            self.closed = true;
            return Err(e.into());
        }
        // The durable commit is already linearized. A later cancel may suppress
        // the acknowledgment, but must not pretend to undo that committed edit.
        check(a)?;
        Ok(())
    }
}
struct Cursor<'a, S, L, C, A> {
    vault: &'a mut Vault<S, L, C>,
    authorization: &'a mut A,
    bytes: Zeroizing<[u8; PAYLOAD_BYTES]>,
    chunk: Option<u32>,
    useful: usize,
}
impl<S: CiphertextStorage, L: Ledger, C: Aead, A: Checkpoint> Cursor<'_, S, L, C, A> {
    fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), Error> {
        check(self.authorization)?;
        let mut position = offset;
        let mut done = 0;
        while done < out.len() {
            let chunk = position / PAYLOAD_BYTES as u32;
            if self.chunk != Some(chunk) {
                self.bytes.fill(0);
                self.useful = self
                    .vault
                    .read_chunk(chunk, &mut self.bytes, self.authorization)?;
                self.chunk = Some(chunk);
            }
            let inside = (position % PAYLOAD_BYTES as u32) as usize;
            if inside >= self.useful {
                return Err(Error::Format);
            }
            let count = (self.useful - inside).min(out.len() - done);
            out[done..done + count].copy_from_slice(&self.bytes[inside..inside + count]);
            done += count;
            position += count as u32;
        }
        check(self.authorization)
    }
}
struct Old<'a, S, L, C, A> {
    vault: &'a mut Vault<S, L, C>,
    authorization: &'a mut A,
    length: u32,
}
impl<S: CiphertextStorage, L: Ledger, C: Aead, A: Checkpoint> SnapshotReader
    for Old<'_, S, L, C, A>
{
    fn length(&self) -> u32 {
        self.length
    }
    fn read_at(&mut self, offset: u32, out: &mut [u8]) -> Result<(), device_vault::Error> {
        self.vault.read_at(offset, out, self.authorization)
    }
}
struct Plan<'a> {
    old_len: u32,
    start: u32,
    removed: u32,
    new_len: u32,
    header: [u8; 32],
    record: [u8; 8],
    value: Option<&'a [u8]>,
}
impl<'a> Plan<'a> {
    fn new(
        old_len: u32,
        start: u32,
        removed: u32,
        id: Id,
        value: Option<&'a [u8]>,
        header: [u8; 32],
    ) -> Result<Self, Error> {
        let added = value.map_or(0, |v| RECORD + v.len() as u32);
        let new_len = old_len
            .max(HEADER)
            .checked_sub(removed)
            .and_then(|n| n.checked_add(added))
            .ok_or(Error::Capacity)?;
        if new_len > device_vault::MAX_SNAPSHOT_BYTES {
            return Err(Error::Capacity);
        }
        Ok(Self {
            old_len,
            start,
            removed,
            new_len,
            header,
            record: record(id, value.map_or(0, |v| v.len() as u32)),
            value,
        })
    }
    fn digest(&mut self, previous: &mut dyn SnapshotReader) -> Result<[u8; 32], Error> {
        let mut hash = Sha256::new();
        let mut bytes = Zeroizing::new([0; PAYLOAD_BYTES]);
        let mut offset = 0;
        while offset < self.new_len {
            let n = (self.new_len - offset).min(PAYLOAD_BYTES as u32) as usize;
            self.read(previous, offset, &mut bytes[..n])?;
            hash.update(&bytes[..n]);
            bytes.fill(0);
            offset += n as u32;
        }
        Ok(hash.finalize().into())
    }
}
impl SnapshotSource for Plan<'_> {
    fn length(&self) -> u32 {
        self.new_len
    }
    fn read(
        &mut self,
        previous: &mut dyn SnapshotReader,
        offset: u32,
        out: &mut [u8],
    ) -> Result<(), device_vault::Error> {
        if previous.length() != self.old_len
            || offset
                .checked_add(out.len() as u32)
                .is_none_or(|n| n > self.new_len)
        {
            return Err(device_vault::Error::Invalid);
        }
        let inserted = self.value.map_or(0, |v| RECORD + v.len() as u32);
        let mut pos = offset;
        let mut done = 0;
        while done < out.len() {
            let remaining = (out.len() - done) as u32;
            if pos < HEADER {
                let n = (HEADER - pos).min(remaining) as usize;
                out[done..done + n].copy_from_slice(&self.header[pos as usize..pos as usize + n]);
                pos += n as u32;
                done += n;
            } else if pos < self.start {
                let n = (self.start - pos).min(remaining) as usize;
                previous.read_at(pos, &mut out[done..done + n])?;
                pos += n as u32;
                done += n;
            } else if pos < self.start + inserted {
                let relative = pos - self.start;
                let part = if relative < RECORD {
                    &self.record[relative as usize..]
                } else {
                    &self.value.ok_or(device_vault::Error::Invalid)?[(relative - RECORD) as usize..]
                };
                let n = part.len().min(remaining as usize);
                out[done..done + n].copy_from_slice(&part[..n]);
                pos += n as u32;
                done += n;
            } else {
                previous.read_at(pos - inserted + self.removed, &mut out[done..])?;
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
