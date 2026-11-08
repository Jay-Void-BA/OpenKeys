#![forbid(unsafe_code)]

//! Bounded import preparation. Call only after device authorization. This crate
//! neither authenticates a user nor implements persistent vault encryption.
//! Plaintext import necessarily exists in host memory; zeroization is best effort.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub const MAX_FILE_BYTES: usize = 1_048_576;
pub const MAX_ENTRIES: usize = 256;
pub const MAX_NOTE_BYTES: usize = 16_384;
pub const MAX_RECOVERY_CODES: usize = 64;

pub struct SecretText(Zeroizing<String>);
impl SecretText {
    pub fn new(text: String) -> Self {
        Self(Zeroizing::new(text))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for SecretText {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Self::new)
    }
}

pub struct Note {
    pub title: SecretText,
    pub body: SecretText,
}
pub struct RecoveryCodes {
    pub title: SecretText,
    pub codes: Vec<SecretText>,
}
pub enum Entry {
    Note(Note),
    RecoveryCodes(RecoveryCodes),
}
impl Entry {
    pub fn title(&self) -> &str {
        match self {
            Self::Note(x) => x.title.expose(),
            Self::RecoveryCodes(x) => x.title.expose(),
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Note(_) => "Note",
            Self::RecoveryCodes(_) => "Recovery codes",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportError {
    TooLarge,
    TooManyEntries,
    InvalidFormat,
    UnsupportedType,
    InvalidMetadata,
}
impl core::fmt::Display for ImportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ImportError {}

fn valid_text(s: &str, limit: usize, multiline: bool) -> bool {
    !s.is_empty()
        && s.len() <= limit
        && !s.chars().any(|c| {
            (c.is_control() && !(multiline && (c == '\n' || c == '\t' || c == '\r')))
                || matches!(c, '\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
        })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonBatch {
    version: u8,
    entries: Vec<JsonEntry>,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum JsonEntry {
    Note {
        title: SecretText,
        body: SecretText,
    },
    RecoveryCodes {
        title: SecretText,
        codes: Vec<SecretText>,
    },
}

pub fn parse_import(input: &[u8]) -> Result<Vec<Entry>, ImportError> {
    if input.len() > MAX_FILE_BYTES {
        return Err(ImportError::TooLarge);
    }
    let text = std::str::from_utf8(input)
        .map_err(|_| ImportError::InvalidFormat)?
        .trim();
    if text.starts_with("otpauth:") {
        return Err(ImportError::UnsupportedType);
    }
    let mut output = Vec::new();
    if text.starts_with('{') {
        let batch: JsonBatch =
            serde_json::from_str(text).map_err(|_| ImportError::InvalidFormat)?;
        if batch.version != 2 {
            return Err(ImportError::InvalidFormat);
        }
        if batch.entries.is_empty() || batch.entries.len() > MAX_ENTRIES {
            return Err(ImportError::TooManyEntries);
        }
        for entry in batch.entries {
            output.push(match entry {
                JsonEntry::Note { title, body } => {
                    if !valid_text(title.expose(), 256, false)
                        || !valid_text(body.expose(), MAX_NOTE_BYTES, true)
                    {
                        return Err(ImportError::InvalidMetadata);
                    }
                    Entry::Note(Note { title, body })
                }
                JsonEntry::RecoveryCodes { title, codes } => {
                    if !valid_text(title.expose(), 256, false)
                        || codes.is_empty()
                        || codes.len() > MAX_RECOVERY_CODES
                        || codes.iter().any(|x| !valid_text(x.expose(), 256, false))
                    {
                        return Err(ImportError::InvalidMetadata);
                    }
                    Entry::RecoveryCodes(RecoveryCodes { title, codes })
                }
            });
        }
    } else {
        return Err(ImportError::InvalidFormat);
    }
    if output.is_empty() {
        return Err(ImportError::InvalidFormat);
    }
    Ok(output)
}

/// Must be computed again by the device from the decrypted, validated import.
/// Not a MAC, UV proof, or a substitute for an authenticated channel.
pub fn manifest_digest(entries: &[Entry]) -> [u8; 32] {
    fn field(hash: &mut Sha256, bytes: &[u8]) {
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    }
    let mut hash = Sha256::new();
    hash.update(b"OPENKEYS-IMPORT-V2\0");
    hash.update((entries.len() as u64).to_be_bytes());
    for e in entries {
        field(&mut hash, e.kind().as_bytes());
        field(&mut hash, e.title().as_bytes());
        match e {
            Entry::Note(x) => field(&mut hash, x.body.expose().as_bytes()),
            Entry::RecoveryCodes(x) => {
                field(&mut hash, &(x.codes.len() as u64).to_be_bytes());
                for c in &x.codes {
                    field(&mut hash, c.expose().as_bytes());
                }
            }
        }
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOTE: &[u8] =
        br#"{"version":2,"entries":[{"type":"note","title":"example","body":"private"}]}"#;
    #[test]
    fn notes_and_recovery_codes_are_distinct() {
        let entries = parse_import(br#"{"version":2,"entries":[{"type":"note","title":"account","body":"private note"},{"type":"recovery_codes","title":"backup","codes":["a-1","b-2"]}]}"#).unwrap();
        assert!(matches!(entries[0], Entry::Note(_)));
        assert!(matches!(entries[1], Entry::RecoveryCodes(_)));
    }
    #[test]
    fn removed_otp_legacy_versions_and_unknown_fields_are_rejected() {
        for bytes in [b"otpauth://totp/example?secret=AAAA".as_slice(),
            br#"{"version":2,"entries":[{"type":"otp","uri":"otpauth://totp/example"}]}"#,
            br#"{"version":1,"entries":[{"type":"note","title":"x","body":"x"}]}"#,
            br#"{"version":2,"entries":[{"type":"note","title":"x","body":"x","extra":"x"}]}"#,
            br#"{"version":2,"version":2,"entries":[]}"#,
            br#"{"version":2,"entries":[{"type":"note","title":"first","title":"second","body":"x"}]}"#] {
            assert!(parse_import(bytes).is_err());
        }
    }
    #[test]
    fn bounds_empty_and_confusing_metadata_are_rejected() {
        assert!(matches!(
            parse_import(&vec![b'a'; MAX_FILE_BYTES + 1]),
            Err(ImportError::TooLarge)
        ));
        for bytes in [
            b" ".as_slice(),
            b"\xff",
            br#"{"version":2,"entries":[]}"#,
            br#"{"version":2,"entries":[{"type":"note","title":"bad\u202ename","body":"x"}]}"#,
            br#"{"version":2,"entries":[{"type":"recovery_codes","title":"x","codes":[]}]}"#,
        ] {
            assert!(parse_import(bytes).is_err());
        }
        let item = r#"{"type":"note","title":"x","body":"x"}"#;
        let data = format!(
            "{{\"version\":2,\"entries\":[{}]}}",
            vec![item; MAX_ENTRIES + 1].join(",")
        );
        assert!(matches!(
            parse_import(data.as_bytes()),
            Err(ImportError::TooManyEntries)
        ));
    }
    #[test]
    fn digest_binds_content_titles_order_and_code_boundaries() {
        let a = parse_import(br#"{"version":2,"entries":[{"type":"recovery_codes","title":"x","codes":["ab","c"]}]}"#).unwrap();
        let b = parse_import(br#"{"version":2,"entries":[{"type":"recovery_codes","title":"x","codes":["a","bc"]}]}"#).unwrap();
        assert_ne!(manifest_digest(&a), manifest_digest(&b));
        let mut c = parse_import(NOTE).unwrap();
        let before = manifest_digest(&c);
        c.push(Entry::Note(Note {
            title: SecretText::new("other".into()),
            body: SecretText::new("value".into()),
        }));
        assert_ne!(before, manifest_digest(&c));
        let ordered = manifest_digest(&c);
        c.reverse();
        assert_ne!(ordered, manifest_digest(&c));
    }
    #[test]
    fn malformed_public_corpus_does_not_panic() {
        let mut state = 17u64;
        for len in 0..512 {
            let mut bytes = vec![0; len];
            for byte in &mut bytes {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *byte = (state >> 32) as u8;
            }
            let _ = parse_import(&bytes);
        }
        for position in 0..NOTE.len() {
            for byte in [0u8, b'"', b'{', b'}', b'\n', 0xff] {
                let mut bytes = NOTE.to_vec();
                bytes[position] = byte;
                let _ = parse_import(&bytes);
            }
        }
    }
}
