use crate::{Error, limits};
use alloc::{string::String, vec::Vec};
use device_policy::{Operation, Request};
use opensk::ctap::Channel;
use sha2::{Digest, Sha256};
use sk_cbor::Value;
use zeroize::Zeroizing;

pub enum Review {
    Website(String),
    Action(&'static str),
    ApplicationHash([u8; 32]),
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    Ctap2,
    U2f,
}
pub struct Prepared {
    pub(crate) bytes: Zeroizing<Vec<u8>>,
    pub(crate) channel: Channel,
    pub(crate) request: Option<Request>,
    pub(crate) binding: [u8; 32],
    pub(crate) continuation: Option<([u8; 32], u32)>,
    review: Review,
    pub(crate) dialect: Dialect,
}
impl Prepared {
    pub fn request(&self) -> Option<&Request> {
        self.request.as_ref()
    }
    pub fn review(&self) -> &Review {
        &self.review
    }
}
fn field(map: &[(Value, Value)], key: u64) -> Option<&Value> {
    map.iter()
        .find(|(k, _)| *k == Value::unsigned(key))
        .map(|(_, v)| v)
}
fn subcommand(map: &[(Value, Value)], key: u64) -> Result<u64, Error> {
    field(map, key)
        .cloned()
        .and_then(Value::extract_unsigned)
        .ok_or(Error::Input)
}
fn rp_id(value: &Value) -> Result<String, Error> {
    let id = value.clone().extract_text_string().ok_or(Error::Input)?;
    if id.is_empty()
        || id.len() > 253
        || !id.is_ascii()
        || id.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err(Error::Input);
    }
    Ok(id)
}
fn require_uv(map: &mut Vec<(Value, Value)>, key: u64, up: bool) -> Result<(), Error> {
    let position = map.iter().position(|(k, _)| *k == Value::unsigned(key));
    let mut options = if let Some(index) = position {
        map.remove(index).1.extract_map().ok_or(Error::Input)?
    } else {
        Vec::new()
    };
    let uv_key = Value::text_string("uv".into());
    let up_key = Value::text_string("up".into());
    options.retain(|(k, _)| *k != uv_key && (!up || *k != up_key));
    options.push((uv_key, Value::bool_value(true)));
    if up {
        options.push((up_key, Value::bool_value(true)));
    }
    map.push((Value::unsigned(key), Value::map(options)));
    Ok(())
}
fn operation_code(op: Operation) -> u8 {
    match op {
        Operation::FidoRegister => 1,
        Operation::FidoSign => 2,
        Operation::ListPrivateMetadata => 3,
        Operation::Delete => 4,
        Operation::Import => 5,
        Operation::FactoryReset => 6,
        Operation::EnrollFingerprint => 7,
        Operation::DeleteFingerprint => 8,
        Operation::DeriveHmacSecret => 9,
        Operation::ProtocolPin => 10,
        _ => 255,
    }
}
/// Called only with the root's current device-session binding and transport
/// channel, not values deserialized from the host. Biometric/config commands
/// are rejected. Host protocol-PIN commands remain distinct from local PIN.
pub fn prepare_ctap2(raw: &[u8], binding: [u8; 32], channel: Channel) -> Result<Prepared, Error> {
    if raw.is_empty() || raw.len() > limits::MAX_BYTES {
        return Err(Error::Limit);
    }
    let cmd = raw[0];
    if cmd == 4 {
        if raw.len() != 1 {
            return Err(Error::Input);
        }
        return Ok(Prepared {
            bytes: Zeroizing::new(raw.to_vec()),
            channel,
            request: None,
            binding,
            continuation: None,
            dialect: Dialect::Ctap2,
            review: Review::Action("Device information"),
        });
    }
    if !matches!(cmd, 1 | 2 | 6 | 7 | 8 | 10 | 11 | 0x41) {
        return Err(Error::Unsupported);
    }
    let mut map = if matches!(cmd, 7 | 8 | 11) {
        if raw.len() != 1 {
            return Err(Error::Input);
        }
        Vec::new()
    } else {
        limits::check(&raw[1..])?;
        opensk::ctap::cbor_read(&raw[1..])
            .map_err(|_| Error::Input)?
            .extract_map()
            .ok_or(Error::Input)?
    };
    let (operation, review) = match cmd {
        1 => {
            let rp = field(&map, 2)
                .cloned()
                .and_then(Value::extract_map)
                .ok_or(Error::Input)?;
            let id = rp
                .iter()
                .find(|(k, _)| *k == Value::text_string("id".into()))
                .ok_or(Error::Input)?;
            let id = rp_id(&id.1)?;
            require_uv(&mut map, 7, false)?;
            (Some(Operation::FidoRegister), Review::Website(id))
        }
        2 => {
            let id = rp_id(field(&map, 1).ok_or(Error::Input)?)?;
            let hmac = field(&map, 4)
                .cloned()
                .and_then(Value::extract_map)
                .is_some_and(|extensions| {
                    extensions
                        .iter()
                        .any(|(k, _)| *k == Value::text_string("hmac-secret".into()))
                });
            require_uv(&mut map, 5, true)?;
            (
                Some(if hmac {
                    Operation::DeriveHmacSecret
                } else {
                    Operation::FidoSign
                }),
                Review::Website(id),
            )
        }
        6 => {
            match subcommand(&map, 2)? {
                2 => (None, Review::Action("Key agreement")),
                1 => (
                    Some(Operation::ProtocolPin),
                    Review::Action("FIDO PIN retries"),
                ),
                3 => (Some(Operation::ProtocolPin), Review::Action("Set FIDO PIN")),
                4 => (
                    Some(Operation::ProtocolPin),
                    Review::Action("Change FIDO PIN"),
                ),
                5 => (
                    Some(Operation::ProtocolPin),
                    Review::Action("Verify FIDO PIN"),
                ),
                6 if matches!(subcommand(&map, 9)?, 1..=7) => (
                    Some(Operation::ProtocolPin),
                    Review::Action("Verify device user"),
                ),
                9 if matches!(subcommand(&map, 9)?, 1..=7) => (
                    Some(Operation::ProtocolPin),
                    Review::Action("Verify FIDO PIN"),
                ),
                // Local UV tokens are rejected by the PIN provider whenever
                // an independent host PIN is set. No unsupported permissions.
                _ => return Err(Error::Unsupported),
            }
        }
        7 => (
            Some(Operation::FactoryReset),
            Review::Action("Erase FIDO data"),
        ),
        8 => return Err(Error::Unsupported), // Explicit bound continuation added by runtime.
        10 | 0x41 => {
            let op = match subcommand(&map, 1)? {
                1..=5 => Operation::ListPrivateMetadata,
                6 => Operation::Delete,
                7 => Operation::Import,
                _ => return Err(Error::Unsupported),
            };
            (Some(op), Review::Action("Manage FIDO keys"))
        }
        11 => (Some(Operation::FidoSign), Review::Action("Select this key")),
        _ => return Err(Error::Unsupported),
    };
    let mut bytes = Zeroizing::new(Vec::new());
    bytes.push(cmd);
    if !matches!(cmd, 7 | 11) {
        opensk::ctap::cbor_write(Value::map(map), &mut bytes).map_err(|_| Error::Input)?;
    }
    if bytes.len() > limits::MAX_BYTES {
        return Err(Error::Limit);
    }
    let request = operation.map(|op| {
        let mut hash = Sha256::new();
        hash.update(b"OpenKeys/CTAP2/effective-request/v1\0");
        hash.update([operation_code(op)]);
        hash.update((bytes.len() as u32).to_be_bytes());
        hash.update(&bytes);
        Request {
            operation: op,
            digest: hash.finalize().into(),
            channel: binding,
        }
    });
    Ok(Prepared {
        bytes,
        channel,
        request,
        binding,
        continuation: None,
        dialect: Dialect::Ctap2,
        review,
    })
}

pub(crate) fn next_request(
    binding: [u8; 32],
    channel: Channel,
    rp: String,
    seed: [u8; 32],
    sequence: u32,
    operation: Operation,
) -> Prepared {
    let mut hash = Sha256::new();
    hash.update(b"OpenKeys/CTAP2/next-assertion/v1\0");
    hash.update(seed);
    hash.update(sequence.to_be_bytes());
    Prepared {
        bytes: Zeroizing::new(alloc::vec![8]),
        channel,
        binding,
        request: Some(Request {
            operation,
            digest: hash.finalize().into(),
            channel: binding,
        }),
        review: Review::Website(rp),
        continuation: Some((seed, sequence)),
        dialect: Dialect::Ctap2,
    }
}

pub fn prepare_u2f(raw: &[u8], binding: [u8; 32], channel: Channel) -> Result<Prepared, Error> {
    if raw.len() < 4 || raw.len() > 7609 || raw[0] != 0 || raw[3] != 0 {
        return Err(Error::Input);
    }
    let (start, len): (usize, usize) = if raw.len() == 4 {
        (4, 0)
    } else if raw.len() == 5 {
        (5, 0)
    } else if raw[4] != 0 {
        (5, usize::from(raw[4]))
    } else if raw.len() >= 7 {
        (7, usize::from(u16::from_be_bytes([raw[5], raw[6]])))
    } else {
        return Err(Error::Input);
    };
    let end = start.checked_add(len).ok_or(Error::Input)?;
    let allowed = if start == 5 && len != 0 { 1 } else { 2 };
    if end > raw.len() || (raw.len() != end && raw.len() != end + allowed) {
        return Err(Error::Input);
    }
    let mut bytes = Zeroizing::new(raw.to_vec());
    let (op, review) = match raw[1] {
        3 if len == 0 && raw[2] == 0 => (None, Review::Action("U2F version")),
        1 if len == 64 && raw[2] == 0 => (
            Some(Operation::FidoRegister),
            Review::ApplicationHash(
                raw[start + 32..start + 64]
                    .try_into()
                    .map_err(|_| Error::Input)?,
            ),
        ),
        2 if len >= 65
            && matches!(raw[2], 3 | 7 | 8)
            && len == 65 + usize::from(raw[start + 64]) =>
        {
            if raw[2] == 8 {
                bytes[2] = 3;
            }
            let op = if raw[2] == 7 {
                Operation::ListPrivateMetadata
            } else {
                Operation::FidoSign
            };
            (
                Some(op),
                Review::ApplicationHash(
                    raw[start + 32..start + 64]
                        .try_into()
                        .map_err(|_| Error::Input)?,
                ),
            )
        }
        _ => return Err(Error::Unsupported),
    };
    let request = op.map(|operation| {
        let mut hash = Sha256::new();
        hash.update(b"OpenKeys/U2F/effective-request/v1\0");
        hash.update([operation_code(operation)]);
        hash.update((bytes.len() as u32).to_be_bytes());
        hash.update(&bytes);
        Request {
            operation,
            digest: hash.finalize().into(),
            channel: binding,
        }
    });
    Ok(Prepared {
        bytes,
        channel,
        request,
        binding,
        review,
        continuation: None,
        dialect: Dialect::U2f,
    })
}
