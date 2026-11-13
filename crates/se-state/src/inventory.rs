//! Authenticated, bounded SE object inventory for locked commissioning/reset.
//! This is NOT a wipe command, blank-device detector or authorization issuer.
//! A baseline must come from an independently qualified exact chip profile,
//! not a host packet or the objects observed after a possibly partial reset.
//! Only a newly authenticated, exclusively owned platform SCP may run this.
use crate::{Error, Exchange};
use openkeys_scp03::{Command, Le};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub const MAX_BASELINE_OBJECTS: usize = 64;
const RESPONSE_BYTES: usize = 512;
// Pinned NXP SDK SE05x_MoreIndicator_t. Zero is INVALID, not end-of-list.
const NO_MORE: u8 = 1;
const MORE: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpectedObject {
    pub id: u32,
    pub kind: u8,
}

/// Trusted configuration, not evidence that these objects are NXP-provisioned.
/// Origin, public identity, ACL and exact OEF/Applet need separate qualification.
pub struct Baseline<'a> {
    objects: &'a [ExpectedObject],
    profile_digest: [u8; 32],
}
impl<'a> Baseline<'a> {
    pub const fn profile_digest(&self) -> &[u8; 32] {
        &self.profile_digest
    }
    /// IDs must be sorted, unique, nonempty and inside the applet reserved range.
    /// Never whitelist an old OpenKeys private object just to make reset pass.
    pub fn from_qualified_profile(
        objects: &'a [ExpectedObject],
        profile_digest: [u8; 32],
    ) -> Result<Self, Error> {
        if objects.is_empty()
            || objects.len() > MAX_BASELINE_OBJECTS
            || profile_digest == [0; 32]
            || objects
                .iter()
                .any(|o| o.id < 0x7fff_0000 || o.id > 0x7fff_ffff || o.kind == 0)
            || objects.windows(2).any(|p| p[0].id >= p[1].id)
        {
            return Err(Error::Schema);
        }
        Ok(Self {
            objects,
            profile_digest,
        })
    }
}

/// Root verifies the same durable reset/setup generation, exclusive SE owner,
/// locked external interfaces and live hardware state before/after every APDU.
/// This phase cannot renew either PIN lease or expose a private operation.
pub trait Checkpoint {
    fn check(&mut self) -> Result<(), Error>;
}

/// Observation only. Does not prove DeleteAll, NOR erasure, ACL, Origin or a
/// legitimate initial factory lifecycle. It cannot authorize PIN installation.
pub struct Report {
    count: usize,
    profile_digest: [u8; 32],
    digest: [u8; 32],
}
impl Report {
    pub const fn object_count(&self) -> usize {
        self.count
    }
    pub const fn profile_digest(&self) -> &[u8; 32] {
        &self.profile_digest
    }
    pub const fn inventory_digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

fn call(
    io: &mut impl Exchange,
    checkpoint: &mut impl Checkpoint,
    command: &Command<'_>,
    response: &mut [u8],
) -> Result<usize, Error> {
    response.fill(0);
    checkpoint.check()?;
    let n = io.exchange(command, response)?;
    checkpoint.check()?;
    if n > response.len() {
        return Err(Error::Schema);
    }
    Ok(n)
}

fn page(response: &[u8]) -> Result<(bool, &[u8]), Error> {
    if response.len() < 5 || response[..2] != [0x41, 1] || response[3] != 0x42 {
        return Err(Error::Schema);
    }
    let more = match response[2] {
        NO_MORE => false,
        MORE => true,
        _ => return Err(Error::Schema),
    };
    let (at, length) = match response[4] {
        n @ 0..=127 => (5, usize::from(n)),
        0x81 if response.len() >= 6 && response[5] >= 128 => (6, usize::from(response[5])),
        0x82 if response.len() >= 7 => {
            let n = usize::from(u16::from_be_bytes([response[5], response[6]]));
            if n <= 255 {
                return Err(Error::Schema);
            }
            (7, n)
        }
        _ => return Err(Error::Schema),
    };
    let ids = response.get(at..).ok_or(Error::Schema)?;
    if ids.len() != length || !length.is_multiple_of(4) || (more && length == 0) {
        return Err(Error::Schema);
    }
    Ok((more, ids))
}

/// Compare the entire authenticated list and each object's exact persistent
/// type with the qualified baseline. Missing/extra/repeated objects, ambiguous
/// I/O, hidden pagination, malformed TLVs or a Root fault close the exchange.
pub fn verify_baseline(
    io: &mut impl Exchange,
    expected: &Baseline<'_>,
    checkpoint: &mut impl Checkpoint,
) -> Result<Report, Error> {
    let result = verify_inner(io, expected, checkpoint);
    if result.is_err() {
        io.close();
    }
    result
}
fn verify_inner(
    io: &mut impl Exchange,
    expected: &Baseline<'_>,
    checkpoint: &mut impl Checkpoint,
) -> Result<Report, Error> {
    let mut response = Zeroizing::new([0; RESPONSE_BYTES]);
    let mut seen = 0_u64;
    let mut offset = 0_u16;
    let mut finished = false;
    for _ in 0..=MAX_BASELINE_OBJECTS {
        let [hi, lo] = offset.to_be_bytes();
        let command = Command {
            class: 0x80,
            instruction: 2,
            p1: 0,
            p2: 0x25,
            data: &[0x41, 2, hi, lo, 0x42, 1, 0xff],
            le: Some(Le::Extended(RESPONSE_BYTES as u16)),
        };
        let n = call(io, checkpoint, &command, &mut response[..])?;
        let (more, ids) = page(&response[..n])?;
        for bytes in ids.as_chunks::<4>().0 {
            let id = u32::from_be_bytes(*bytes);
            let index = expected
                .objects
                .binary_search_by_key(&id, |o| o.id)
                .map_err(|_| Error::Record)?;
            let mask = 1_u64 << index;
            if seen & mask != 0 {
                return Err(Error::Record);
            }
            seen |= mask;
            offset = offset.checked_add(1).ok_or(Error::Exhausted)?;
        }
        response.fill(0);
        if !more {
            finished = true;
            break;
        }
    }
    let all = u64::MAX >> (MAX_BASELINE_OBJECTS - expected.objects.len());
    if !finished || seen != all || usize::from(offset) != expected.objects.len() {
        return Err(Error::Record);
    }
    let mut hash = Sha256::new();
    hash.update(b"OpenKeys/R05/reset-object-inventory/v1");
    hash.update(expected.profile_digest);
    hash.update(offset.to_be_bytes());
    for object in expected.objects {
        let mut data = [0x41, 4, 0, 0, 0, 0];
        data[2..].copy_from_slice(&object.id.to_be_bytes());
        let n = call(
            io,
            checkpoint,
            &Command {
                class: 0x80,
                instruction: 2,
                p1: 0,
                p2: 0x26,
                data: &data,
                le: None,
            },
            &mut response[..],
        )?;
        if response[..n] != [0x41, 1, object.kind, 0x42, 1, 1] {
            return Err(Error::Schema);
        }
        hash.update(object.id.to_be_bytes());
        hash.update([object.kind, 1]);
    }
    checkpoint.check()?;
    Ok(Report {
        count: expected.objects.len(),
        profile_digest: expected.profile_digest,
        digest: hash.finalize().into(),
    })
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
