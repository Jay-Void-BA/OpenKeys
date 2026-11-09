//! AN12413 Rev2.12 tables45/99/105-116, checked against pinned NXP nano.
use crate::{Error, private};
use openkeys_scp03::{
    Command, Crypto,
    client::{Client, Guard, Link},
};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub const HEAD_BYTES: usize = 104;
const RESPONSE_BYTES: usize = 128;
const NAMESPACE: u32 = 0x4f4b_0000; // Private OpenKeys object namespace, not a vendor UID.

/// Sealed production boundary: only ScpSession can supply authenticated APDU
/// bodies. Test memory models implement the private seal only under cfg(test).
pub trait Exchange: private::Sealed {
    /// Body only, status already authenticated/stripped by strict SCP03.
    fn exchange(&mut self, command: &Command<'_>, out: &mut [u8]) -> Result<usize, Error>;
    fn close(&mut self);
}
pub struct ScpSession<'a, L, C, G, const N: usize> {
    client: &'a mut Client<L, N>,
    crypto: &'a mut C,
    guard: &'a mut G,
}
impl<'a, L: Link, C: Crypto<N>, G: Guard, const N: usize> ScpSession<'a, L, C, G, N> {
    pub fn new(client: &'a mut Client<L, N>, crypto: &'a mut C, guard: &'a mut G) -> Self {
        Self {
            client,
            crypto,
            guard,
        }
    }
}
impl<L, C, G, const N: usize> private::Sealed for ScpSession<'_, L, C, G, N> {}
impl<L: Link, C: Crypto<N>, G: Guard, const N: usize> Exchange for ScpSession<'_, L, C, G, N> {
    fn exchange(&mut self, command: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        out.fill(0);
        let result = (|| {
            let response = self
                .client
                .exchange(self.crypto, command, self.guard)
                .map_err(|error| {
                    if error == openkeys_scp03::Error::Authorization {
                        Error::Authorization
                    } else {
                        Error::Channel
                    }
                })?;
            let body = response.expose();
            if body.len() > out.len() {
                return Err(Error::Schema);
            }
            out[..body.len()].copy_from_slice(body);
            Ok(body.len())
        })();
        if result.is_err() {
            out.fill(0);
            self.client.close();
        }
        result
    }
    fn close(&mut self) {
        self.client.close();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum Counter {
    BioSpent = 0,
    BioCredit = 1,
    BioTag = 2,
    VaultNonce = 3,
    VaultCommit = 4,
    FidoSignature = 5,
    SigningSlot = 6,
}
impl Counter {
    pub(crate) const ALL: [Self; 7] = [
        Self::BioSpent,
        Self::BioCredit,
        Self::BioTag,
        Self::VaultNonce,
        Self::VaultCommit,
        Self::FidoSignature,
        Self::SigningSlot,
    ];
    pub(crate) const fn index(self) -> usize {
        self as usize
    }
    pub(crate) const fn id(self) -> u32 {
        NAMESPACE + 1 + self as u32
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum File {
    Head0 = 0,
    Head1 = 1,
    Marker = 2,
}
impl File {
    pub(crate) const ALL: [Self; 3] = [Self::Head0, Self::Head1, Self::Marker];
    pub(crate) const fn id(self) -> u32 {
        NAMESPACE + 0x100 + self as u32
    }
    pub(crate) const fn length(self) -> usize {
        if matches!(self, Self::Marker) {
            32
        } else {
            HEAD_BYTES
        }
    }
    pub(crate) const fn for_sequence(sequence: u64) -> Self {
        if sequence & 1 == 0 {
            Self::Head0
        } else {
            Self::Head1
        }
    }
}
pub(crate) fn marker() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"OpenKeys/R04/SE-state/v2");
    hash.update(crate::BIO_MAX_FAILURES.to_be_bytes());
    hash.update(u16::MAX.to_be_bytes());
    hash.update(device_vault::MAX_SNAPSHOT_BYTES.to_be_bytes());
    hash.update((HEAD_BYTES as u32).to_be_bytes());
    hash.update(crate::keys::KEY_SLOTS.to_be_bytes());
    for c in Counter::ALL {
        hash.update(c.id().to_be_bytes());
    }
    for file in File::ALL {
        hash.update(file.id().to_be_bytes());
    }
    hash.finalize().into()
}

pub(crate) struct Objects<X: Exchange> {
    io: X,
    last: [Option<u64>; 7],
    faulted: bool,
}
impl<X: Exchange> Objects<X> {
    pub(crate) fn open(io: X) -> Result<Self, Error> {
        let mut this = Self {
            io,
            last: [None; 7],
            faulted: false,
        };
        let result = (|| {
            for c in Counter::ALL {
                this.attributes(c.id(), 0x0d, 8)?;
            }
            for file in File::ALL {
                this.attributes(file.id(), 0x0b, file.length())?;
            }
            let mut found = [0; 32];
            this.read_file(File::Marker, &mut found)?;
            if !bool::from(found.ct_eq(&marker())) {
                return Err(Error::Schema);
            }
            Ok(())
        })();
        if let Err(error) = result {
            this.close();
            return Err(error);
        }
        Ok(this)
    }
    pub(crate) fn close(&mut self) {
        self.faulted = true;
        self.io.close();
    }
    fn call(
        &mut self,
        ins: u8,
        p1: u8,
        p2: u8,
        data: &[u8],
        out: &mut [u8],
    ) -> Result<usize, Error> {
        out.fill(0);
        if self.faulted {
            return Err(Error::Closed);
        }
        let result = self.io.exchange(
            &Command {
                class: 0x80,
                instruction: ins,
                p1,
                p2,
                data,
                le: Some(0),
            },
            out,
        );
        match result {
            Ok(n) if n <= out.len() => Ok(n),
            Ok(_) => {
                self.close();
                out.fill(0);
                Err(Error::Schema)
            }
            Err(error) => {
                self.close();
                out.fill(0);
                Err(error)
            }
        }
    }
    fn id(id: u32) -> [u8; 6] {
        let mut out = [0; 6];
        out[..2].copy_from_slice(&[0x41, 4]);
        out[2..].copy_from_slice(&id.to_be_bytes());
        out
    }
    fn attributes(&mut self, id: u32, kind: u8, size: usize) -> Result<(), Error> {
        let command = Self::id(id);
        let mut out = [0; 16];
        let n = self.call(2, 0, 0x26, &command, &mut out)?;
        // PERSISTENT=01 (not00). Unknown fields, duplicates and trailing SW
        // bytes are rejected even when the outer response MAC was valid.
        if out[..n] != [0x41, 1, kind, 0x42, 1, 1] {
            self.close();
            return Err(Error::Schema);
        }
        let n = self.call(2, 0, 7, &command, &mut out)?;
        if out[..n] != [0x41, 2, (size >> 8) as u8, size as u8] {
            self.close();
            return Err(Error::Schema);
        }
        Ok(())
    }
    pub(crate) fn read_counter(&mut self, c: Counter) -> Result<u64, Error> {
        let mut out = Zeroizing::new([0; 16]);
        let n = self.call(2, 0, 0, &Self::id(c.id()), &mut out[..])?;
        if n != 10 || out[..2] != [0x41, 8] {
            self.close();
            return Err(Error::Schema);
        }
        let value = u64::from_be_bytes(out[2..10].try_into().map_err(|_| Error::Schema)?);
        if self.last[c.index()].is_some_and(|last| value < last) {
            self.close();
            return Err(Error::Counter);
        }
        self.last[c.index()] = Some(value);
        Ok(value)
    }
    pub(crate) fn increment(&mut self, c: Counter, expected: u64) -> Result<u64, Error> {
        // Credit counters have no general increment API. Only a verified
        // matching event can advance BioCredit via the private state service.
        if c == Counter::BioCredit {
            self.close();
            return Err(Error::Counter);
        }
        if expected == u64::MAX {
            self.close();
            return Err(Error::Exhausted);
        }
        if self.read_counter(c)? != expected {
            self.close();
            return Err(Error::Counter);
        }
        let mut out = [0; 4];
        let n = self.call(1, 8, 0, &Self::id(c.id()), &mut out)?;
        if n != 0 {
            self.close();
            return Err(Error::Schema);
        }
        let value = self.read_counter(c)?;
        if value != expected + 1 {
            self.close();
            return Err(Error::Counter);
        }
        Ok(value)
    }
    pub(crate) fn credit_match(&mut self, expected_credit: u64, spent: u64) -> Result<(), Error> {
        if spent <= expected_credit
            || self.read_counter(Counter::BioCredit)? != expected_credit
            || self.read_counter(Counter::BioSpent)? != spent
        {
            self.close();
            return Err(Error::Counter);
        }
        let mut data = Zeroizing::new([0; 16]);
        data[..6].copy_from_slice(&Self::id(Counter::BioCredit.id()));
        data[6..8].copy_from_slice(&[0x43, 8]);
        data[8..].copy_from_slice(&spent.to_be_bytes());
        let mut out = [0; 4];
        let n = self.call(1, 8, 0, &data[..], &mut out)?;
        if n != 0 {
            self.close();
            return Err(Error::Schema);
        }
        if self.read_counter(Counter::BioCredit)? != spent {
            self.close();
            return Err(Error::Counter);
        }
        Ok(())
    }
    pub(crate) fn read_file(&mut self, file: File, out: &mut [u8]) -> Result<(), Error> {
        out.fill(0);
        if out.len() != file.length() {
            self.close();
            return Err(Error::Schema);
        }
        let mut data = [0; 10];
        data[..6].copy_from_slice(&Self::id(file.id()));
        data[6..].copy_from_slice(&[0x43, 2, 0, file.length() as u8]);
        let mut response = Zeroizing::new([0; RESPONSE_BYTES]);
        let n = self.call(2, 0, 0, &data, &mut response[..])?;
        if n != out.len() + 2 || response[..2] != [0x41, out.len() as u8] {
            self.close();
            return Err(Error::Schema);
        }
        out.copy_from_slice(&response[2..n]);
        Ok(())
    }
    pub(crate) fn write_head(&mut self, file: File, bytes: &[u8; HEAD_BYTES]) -> Result<(), Error> {
        if file == File::Marker {
            self.close();
            return Err(Error::Schema);
        }
        // This is an update to a known, fixed persistent object. Check its
        // existence/type/size first; no default-policy creation on a missing
        // namespace is attempted. SCP mandate/ACLs are commissioning duties.
        self.attributes(file.id(), 0x0b, HEAD_BYTES)?;
        let mut data = Zeroizing::new([0; HEAD_BYTES + 8]);
        data[..6].copy_from_slice(&Self::id(file.id()));
        data[6..8].copy_from_slice(&[0x44, HEAD_BYTES as u8]);
        data[8..].copy_from_slice(bytes);
        let mut out = [0; 4];
        let n = self.call(1, 6, 0, &data[..], &mut out)?;
        if n != 0 {
            self.close();
            return Err(Error::Schema);
        }
        let mut readback = Zeroizing::new([0; HEAD_BYTES]);
        self.read_file(file, &mut readback[..])?;
        if !bool::from(readback[..].ct_eq(bytes)) {
            self.close();
            return Err(Error::Record);
        }
        Ok(())
    }
}
