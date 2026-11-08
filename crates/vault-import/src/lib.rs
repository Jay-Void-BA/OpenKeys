#![forbid(unsafe_code)]

//! Bounded import preparation. Call only after device authorization. This crate
//! neither authenticates a user nor implements persistent vault encryption.
//! Plaintext import necessarily exists in host memory; zeroization is best effort.

use data_encoding::{BASE32, BASE32_NOPAD};
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Algorithm {
    Sha1,
    Sha256,
    Sha512,
}
impl Algorithm {
    pub fn name(self) -> &'static str {
        match self {
            Self::Sha1 => "SHA1",
            Self::Sha256 => "SHA256",
            Self::Sha512 => "SHA512",
        }
    }
    pub fn digest_len(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha512 => 64,
        }
    }
}

pub struct Otp {
    pub title: SecretText,
    pub issuer: SecretText,
    pub account: SecretText,
    pub secret: Zeroizing<Vec<u8>>,
    pub algorithm: Algorithm,
    pub digits: u8,
    pub period: u32,
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
    Otp(Otp),
    Note(Note),
    RecoveryCodes(RecoveryCodes),
}
impl Entry {
    pub fn title(&self) -> &str {
        match self {
            Self::Otp(x) => x.title.expose(),
            Self::Note(x) => x.title.expose(),
            Self::RecoveryCodes(x) => x.title.expose(),
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Otp(_) => "TOTP",
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
    InvalidUri,
    UnsupportedType,
    DuplicateParameter,
    InvalidSecret,
    InvalidParameters,
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

fn decode_secret(s: &str) -> Result<Zeroizing<Vec<u8>>, ImportError> {
    if s.len() > 512 {
        return Err(ImportError::InvalidSecret);
    }
    // Whitespace is permitted in manually grouped Base32 strings.
    let clean: Zeroizing<String> = Zeroizing::new(
        s.chars()
            .filter(|c| !c.is_ascii_whitespace())
            .map(|c| c.to_ascii_uppercase())
            .collect(),
    );
    let decoded = if clean.contains('=') {
        BASE32.decode(clean.as_bytes())
    } else {
        BASE32_NOPAD.decode(clean.as_bytes())
    }
    .map_err(|_| ImportError::InvalidSecret)?;
    let decoded = Zeroizing::new(decoded);
    if !(10..=128).contains(&decoded.len()) {
        return Err(ImportError::InvalidSecret);
    }
    Ok(decoded)
}

fn percent_decode(s: &str, query: bool) -> Result<SecretText, ImportError> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(s.len()));
    let mut it = s.bytes();
    while let Some(c) = it.next() {
        if c == b'%' {
            fn hex(c: u8) -> Option<u8> {
                (c as char).to_digit(16).map(|x| x as u8)
            }
            let a = it.next().and_then(hex).ok_or(ImportError::InvalidUri)?;
            let b = it.next().and_then(hex).ok_or(ImportError::InvalidUri)?;
            bytes.push(a * 16 + b);
        } else {
            bytes.push(if query && c == b'+' { b' ' } else { c });
        }
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| ImportError::InvalidUri)?;
    Ok(SecretText::new(text.to_owned()))
}

pub fn parse_uri(uri: &str) -> Result<Entry, ImportError> {
    if uri.len() > 4096 {
        return Err(ImportError::TooLarge);
    }
    let rest = uri
        .strip_prefix("otpauth://totp/")
        .ok_or(ImportError::UnsupportedType)?;
    if rest.contains('#') {
        return Err(ImportError::InvalidUri);
    }
    let (label, query) = rest.split_once('?').ok_or(ImportError::InvalidUri)?;
    let label = percent_decode(label, false)?;
    if !valid_text(label.expose(), 256, false) {
        return Err(ImportError::InvalidMetadata);
    }
    let mut params: std::collections::BTreeMap<String, SecretText> =
        std::collections::BTreeMap::new();
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').ok_or(ImportError::InvalidUri)?;
        if !matches!(key, "secret" | "issuer" | "algorithm" | "digits" | "period") {
            return Err(ImportError::InvalidParameters);
        }
        if params
            .insert(key.to_owned(), percent_decode(value, true)?)
            .is_some()
        {
            return Err(ImportError::DuplicateParameter);
        }
    }
    let secret = decode_secret(
        params
            .get("secret")
            .ok_or(ImportError::InvalidSecret)?
            .expose(),
    )?;
    let (label_issuer, account) = label
        .expose()
        .split_once(':')
        .unwrap_or(("", label.expose()));
    let issuer = params
        .get("issuer")
        .map(|x| x.expose())
        .unwrap_or(label_issuer);
    if !label_issuer.is_empty() && issuer != label_issuer {
        return Err(ImportError::InvalidMetadata);
    }
    let algorithm = match params
        .get("algorithm")
        .map(|x| x.expose())
        .unwrap_or("SHA1")
    {
        "SHA1" => Algorithm::Sha1,
        "SHA256" => Algorithm::Sha256,
        "SHA512" => Algorithm::Sha512,
        _ => return Err(ImportError::InvalidParameters),
    };
    let digits: u8 = params
        .get("digits")
        .map(|x| x.expose())
        .unwrap_or("6")
        .parse()
        .map_err(|_| ImportError::InvalidParameters)?;
    let period: u32 = params
        .get("period")
        .map(|x| x.expose())
        .unwrap_or("30")
        .parse()
        .map_err(|_| ImportError::InvalidParameters)?;
    validate_otp(Entry::Otp(Otp {
        title: SecretText::new(label.expose().to_owned()),
        issuer: SecretText::new(issuer.to_owned()),
        account: SecretText::new(account.to_owned()),
        secret,
        algorithm,
        digits,
        period,
    }))
}

fn validate_otp(entry: Entry) -> Result<Entry, ImportError> {
    if let Entry::Otp(otp) = &entry {
        if !valid_text(otp.title.expose(), 256, false)
            || !valid_text(otp.account.expose(), 256, false)
            || (!otp.issuer.expose().is_empty() && !valid_text(otp.issuer.expose(), 256, false))
        {
            return Err(ImportError::InvalidMetadata);
        }
        if !matches!(otp.digits, 6 | 8) || !(15..=120).contains(&otp.period) {
            return Err(ImportError::InvalidParameters);
        }
    }
    Ok(entry)
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
    Otp {
        uri: SecretText,
    },
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
    let mut output = Vec::new();
    if text.starts_with('{') {
        let batch: JsonBatch =
            serde_json::from_str(text).map_err(|_| ImportError::InvalidFormat)?;
        if batch.version != 1 {
            return Err(ImportError::InvalidFormat);
        }
        if batch.entries.is_empty() || batch.entries.len() > MAX_ENTRIES {
            return Err(ImportError::TooManyEntries);
        }
        for entry in batch.entries {
            output.push(match entry {
                JsonEntry::Otp { uri } => parse_uri(uri.expose())?,
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
        for line in text.lines().map(str::trim).filter(|x| !x.is_empty()) {
            if output.len() == MAX_ENTRIES {
                return Err(ImportError::TooManyEntries);
            }
            output.push(parse_uri(line)?);
        }
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
    hash.update(b"AUTH-IMPORT-V1\0");
    hash.update((entries.len() as u64).to_be_bytes());
    for e in entries {
        field(&mut hash, e.kind().as_bytes());
        field(&mut hash, e.title().as_bytes());
        match e {
            Entry::Otp(x) => {
                field(&mut hash, x.issuer.expose().as_bytes());
                field(&mut hash, x.account.expose().as_bytes());
                field(&mut hash, &x.secret);
                field(&mut hash, x.algorithm.name().as_bytes());
                field(&mut hash, &[x.digits]);
                field(&mut hash, &x.period.to_be_bytes());
            }
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

/// RFC 4226 dynamic truncation of a MAC provided by the secure element.
/// The seed and HMAC calculation belong in the SE backend, not this function.
pub fn decimal_code(
    mac: &[u8],
    algorithm: Algorithm,
    digits: u8,
) -> Result<SecretText, ImportError> {
    if mac.len() != algorithm.digest_len() || !matches!(digits, 6 | 8) {
        return Err(ImportError::InvalidParameters);
    }
    let offset = (mac[mac.len() - 1] & 0xf) as usize;
    let value = u32::from_be_bytes(
        mac[offset..offset + 4]
            .try_into()
            .map_err(|_| ImportError::InvalidParameters)?,
    ) & 0x7fff_ffff;
    Ok(SecretText::new(format!(
        "{:0width$}",
        value % 10u32.pow(digits as u32),
        width = digits as usize
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    const URI: &str =
        "otpauth://totp/Example:alice?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Example";
    #[test]
    fn parses_uri_and_metadata() {
        let Entry::Otp(x) = parse_uri(URI).unwrap() else {
            panic!()
        };
        assert_eq!(x.secret.as_slice(), b"12345678901234567890");
        assert_eq!(x.issuer.expose(), "Example");
        assert_eq!(x.account.expose(), "alice");
        assert_eq!(x.period, 30);
        assert_eq!(x.digits, 6);
    }
    #[test]
    fn rejects_ambiguous_malformed_and_oversized_input() {
        assert!(matches!(
            parse_uri(&format!("{URI}&secret=GEZDGNBVGY3TQOJQ")),
            Err(ImportError::DuplicateParameter)
        ));
        assert!(parse_uri("otpauth://totp/%XX?secret=AAAA").is_err());
        assert!(parse_uri(&format!("{URI}&digits=10")).is_err());
        assert!(parse_uri(&format!("{URI}&period=0")).is_err());
        assert!(parse_uri(&URI.replace("issuer=Example", "issuer=Attacker")).is_err());
        assert!(parse_uri(&URI.replace("alice", "%E2%80%AEalice")).is_err());
        assert!(parse_import(&vec![b'a'; MAX_FILE_BYTES + 1]).is_err());
        assert!(parse_import(b"{\"version\":1,\"entries\":[],\"unexpected\":true}").is_err());
        assert!(parse_import(&[0xff]).is_err());
    }
    #[test]
    fn recovery_codes_and_notes_are_separate_types() {
        let input=br#"{"version":1,"entries":[{"type":"note","title":"account","body":"private note"},{"type":"recovery_codes","title":"backup","codes":["a-1","b-2"]}]}"#;
        let entries = parse_import(input).unwrap();
        assert!(matches!(entries[0], Entry::Note(_)));
        assert!(matches!(entries[1], Entry::RecoveryCodes(_)));
    }
    #[test]
    fn manifest_binds_payload_order_and_code_boundaries() {
        let a=parse_import(br#"{"version":1,"entries":[{"type":"recovery_codes","title":"x","codes":["ab","c"]}]}"#).unwrap();
        let b=parse_import(br#"{"version":1,"entries":[{"type":"recovery_codes","title":"x","codes":["a","bc"]}]}"#).unwrap();
        assert_ne!(manifest_digest(&a), manifest_digest(&b));
        let c = parse_import(URI.as_bytes()).unwrap();
        let d = parse_import(URI.replace("alice", "bob").as_bytes()).unwrap();
        assert_ne!(manifest_digest(&c), manifest_digest(&d));
    }
    #[test]
    fn rfc6238_sha1_vectors() {
        use hmac::{Hmac, Mac};
        for (time, expected) in [
            (59u64, "94287082"),
            (1_111_111_109, "07081804"),
            (1_111_111_111, "14050471"),
            (1_234_567_890, "89005924"),
            (2_000_000_000, "69279037"),
            (20_000_000_000, "65353130"),
        ] {
            let mut mac = Hmac::<sha1::Sha1>::new_from_slice(b"12345678901234567890").unwrap();
            mac.update(&(time / 30).to_be_bytes());
            let result = Zeroizing::new(mac.finalize().into_bytes().to_vec());
            assert_eq!(
                decimal_code(&result, Algorithm::Sha1, 8).unwrap().expose(),
                expected
            );
        }
    }
    #[test]
    fn rfc6238_sha256_and_sha512_vectors() {
        use hmac::{Hmac, Mac};
        for (time, e256, e512) in [
            (59u64, "46119246", "90693936"),
            (1_111_111_109, "68084774", "25091201"),
            (1_111_111_111, "67062674", "99943326"),
            (1_234_567_890, "91819424", "93441116"),
            (2_000_000_000, "90698825", "38618901"),
            (20_000_000_000, "77737706", "47863826"),
        ] {
            let mut h256 =
                Hmac::<sha2::Sha256>::new_from_slice(b"12345678901234567890123456789012").unwrap();
            h256.update(&(time / 30).to_be_bytes());
            let mut h512 = Hmac::<sha2::Sha512>::new_from_slice(
                b"1234567890123456789012345678901234567890123456789012345678901234",
            )
            .unwrap();
            h512.update(&(time / 30).to_be_bytes());
            assert_eq!(
                decimal_code(&h256.finalize().into_bytes(), Algorithm::Sha256, 8)
                    .unwrap()
                    .expose(),
                e256
            );
            assert_eq!(
                decimal_code(&h512.finalize().into_bytes(), Algorithm::Sha512, 8)
                    .unwrap()
                    .expose(),
                e512
            );
        }
    }
    #[test]
    fn import_limit_and_empty_data() {
        let data = vec![URI; MAX_ENTRIES + 1].join("\n");
        assert!(matches!(
            parse_import(data.as_bytes()),
            Err(ImportError::TooManyEntries)
        ));
        assert!(matches!(
            parse_import(b"   "),
            Err(ImportError::InvalidFormat)
        ));
        assert!(decimal_code(&[0u8; 1], Algorithm::Sha1, 6).is_err());
    }
    #[test]
    fn malformed_public_corpus_does_not_panic() {
        // Deterministic test data, not a source of cryptographic randomness.
        let mut state = 17u64;
        for len in 0..512 {
            let mut bytes = vec![0; len];
            for byte in &mut bytes {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *byte = (state >> 32) as u8;
            }
            let _ = parse_import(&bytes);
        }
        for position in 0..URI.len() {
            for byte in [0u8, b'%', b'&', b'=', b'\n', 0xff] {
                let mut bytes = URI.as_bytes().to_vec();
                bytes[position] = byte;
                let _ = parse_import(&bytes);
            }
        }
    }
}
