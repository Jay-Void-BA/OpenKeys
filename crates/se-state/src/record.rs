use crate::{Error, objects::HEAD_BYTES};
use device_vault::{Head, MAX_SNAPSHOT_BYTES};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

const DOMAIN: &[u8; 16] = b"OpenKeysHeadV1\0\0";
pub(crate) fn valid(head: &Head) -> bool {
    head.bank < 2 && head.epoch > 0 && head.length <= MAX_SNAPSHOT_BYTES
}

pub(crate) fn encode(sequence: u64, head: Option<&Head>) -> Result<[u8; HEAD_BYTES], Error> {
    let mut out = [0; HEAD_BYTES];
    out[..16].copy_from_slice(DOMAIN);
    out[16..24].copy_from_slice(&sequence.to_be_bytes());
    match head {
        None if sequence == 0 => {}
        Some(head) if sequence > 0 && sequence <= head.epoch && valid(head) => {
            out[24] = 1;
            out[25] = head.bank;
            out[26..34].copy_from_slice(&head.epoch.to_be_bytes());
            out[34..38].copy_from_slice(&head.length.to_be_bytes());
            out[38..70].copy_from_slice(&head.root);
        }
        _ => return Err(Error::Record),
    }
    let digest = Sha256::digest(&out[..72]);
    out[72..].copy_from_slice(&digest);
    Ok(out)
}
pub(crate) fn decode(bytes: &[u8; HEAD_BYTES], sequence: u64) -> Result<Option<Head>, Error> {
    let digest = Sha256::digest(&bytes[..72]);
    if bytes[..16] != DOMAIN[..]
        || !bool::from(digest[..].ct_eq(&bytes[72..]))
        || bytes[16..24] != sequence.to_be_bytes()
        || bytes[70..72] != [0, 0]
    {
        return Err(Error::Record);
    }
    if sequence == 0 {
        if bytes[24..72].iter().any(|b| *b != 0) {
            return Err(Error::Record);
        }
        return Ok(None);
    }
    let head = Head {
        bank: bytes[25],
        epoch: u64::from_be_bytes(bytes[26..34].try_into().map_err(|_| Error::Record)?),
        length: u32::from_be_bytes(bytes[34..38].try_into().map_err(|_| Error::Record)?),
        root: bytes[38..70].try_into().map_err(|_| Error::Record)?,
    };
    if bytes[24] != 1 || sequence > head.epoch || !valid(&head) {
        return Err(Error::Record);
    }
    Ok(Some(head))
}
