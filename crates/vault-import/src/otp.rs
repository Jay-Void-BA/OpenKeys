//! Strict, bounded otpauth importer. Decode after a hardware-PIN session is
//! established. Never render a URI, base32 seed or generated code in the UI.
use device_records::otp::{Algorithm, Import, Kind};
use zeroize::Zeroizing;

pub const MAX_URI: usize = 2048;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Format,
    Duplicate,
    Unsupported,
    Label,
    Seed,
    Counter,
}
pub struct Prepared {
    issuer: Zeroizing<String>,
    account: Zeroizing<String>,
    seed: Zeroizing<Vec<u8>>,
    kind: Kind,
    algorithm: Algorithm,
    counter: u64,
}
impl Prepared {
    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    pub fn account(&self) -> &str {
        &self.account
    }
    pub fn kind(&self) -> Kind {
        self.kind
    }
    pub fn canonical(&self) -> Result<Zeroizing<Vec<u8>>, Error> {
        Import::new(
            self.kind,
            self.algorithm,
            &self.issuer,
            &self.account,
            self.counter,
            &self.seed,
        )
        .map(|x| x.encode())
        .map_err(|_| Error::Format)
    }
}
fn decoded(input: &str) -> Result<Zeroizing<String>, Error> {
    let bytes = input.as_bytes();
    let mut out = Zeroizing::new(Vec::with_capacity(bytes.len()));
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let pair = bytes.get(i + 1..i + 3).ok_or(Error::Format)?;
            let hex = |b: u8| -> Result<u8, Error> {
                match b {
                    b'0'..=b'9' => Ok(b - b'0'),
                    b'A'..=b'F' => Ok(b - b'A' + 10),
                    b'a'..=b'f' => Ok(b - b'a' + 10),
                    _ => Err(Error::Format),
                }
            };
            out.push((hex(pair[0])? << 4) | hex(pair[1])?);
            i += 3;
        } else {
            // Literal plus is not a space in an otpauth URI. Reject fragments
            // and raw control/non-ASCII input instead of silently normalizing.
            if !(0x20..=0x7e).contains(&bytes[i]) || bytes[i] == b'#' {
                return Err(Error::Format);
            }
            out.push(bytes[i]);
            i += 1;
        }
    }
    let text = core::str::from_utf8(&out).map_err(|_| Error::Format)?;
    Ok(Zeroizing::new(text.to_owned()))
}
fn decimal(text: &str) -> Result<u64, Error> {
    if text.is_empty() || text.len() > 20 || (text.len() > 1 && text.starts_with('0')) {
        return Err(Error::Counter);
    }
    text.bytes().try_fold(0u64, |n, b| {
        if !b.is_ascii_digit() {
            return Err(Error::Counter);
        }
        n.checked_mul(10)
            .and_then(|n| n.checked_add(u64::from(b - b'0')))
            .ok_or(Error::Counter)
    })
}
fn base32(text: &str) -> Result<Zeroizing<Vec<u8>>, Error> {
    if text.is_empty() || text.len() > 104 {
        return Err(Error::Seed);
    }
    // Accept standard padded/unpadded RFC4648 and case-insensitive alphabet.
    // Never discard spaces, punctuation, bad pad bits or extra '='.
    let unpadded = text.trim_end_matches('=');
    let padding = text.len() - unpadded.len();
    let remainder = unpadded.len() % 8;
    if !matches!(remainder, 0 | 2 | 4 | 5 | 7)
        || (padding != 0 && (!text.len().is_multiple_of(8) || padding != (8 - remainder) % 8))
    {
        return Err(Error::Seed);
    }
    let mut out = Zeroizing::new(Vec::with_capacity(unpadded.len() * 5 / 8));
    let mut bits = 0u32;
    let mut count = 0;
    for b in unpadded.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a',
            b'2'..=b'7' => b - b'2' + 26,
            _ => return Err(Error::Seed),
        };
        bits = (bits << 5) | u32::from(v);
        count += 5;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
        bits &= (1 << count) - 1;
    }
    if bits != 0 || !(16..=64).contains(&out.len()) {
        return Err(Error::Seed);
    }
    Ok(out)
}
pub fn parse(uri: &str) -> Result<Prepared, Error> {
    if uri.len() > MAX_URI {
        return Err(Error::Bounds);
    }
    let path = uri.strip_prefix("otpauth://").ok_or(Error::Format)?;
    let (kind, rest) = path.split_once('/').ok_or(Error::Format)?;
    let kind = match kind {
        "totp" => Kind::Totp,
        "hotp" => Kind::Hotp,
        _ => return Err(Error::Unsupported),
    };
    let (label, query) = rest.split_once('?').ok_or(Error::Format)?;
    if label.is_empty() || label.contains('/') || query.contains('#') {
        return Err(Error::Format);
    }
    let label = decoded(label)?;
    let mut secret = None;
    let mut issuer = None;
    let mut algorithm = None;
    let mut counter = None;
    let mut digits = None;
    let mut period = None;
    let mut fields = 0;
    for part in query.split('&') {
        fields += 1;
        if fields > 6 {
            return Err(Error::Bounds);
        }
        let (name, value) = part.split_once('=').ok_or(Error::Format)?;
        if value.is_empty() {
            return Err(Error::Format);
        }
        // No decoded-name aliases or duplicate parameters with last-wins.
        let target = match name {
            "secret" => &mut secret,
            "issuer" => &mut issuer,
            "algorithm" => &mut algorithm,
            "counter" => &mut counter,
            "digits" => &mut digits,
            "period" => &mut period,
            _ => return Err(Error::Unsupported),
        };
        if target.is_some() {
            return Err(Error::Duplicate);
        }
        *target = Some(decoded(value)?);
    }
    if digits.as_deref().is_some_and(|x| x.as_str() != "6")
        || period.as_deref().is_some_and(|x| x.as_str() != "30")
        || (kind == Kind::Hotp && period.is_some())
    {
        return Err(Error::Unsupported);
    }
    let algorithm = match algorithm.as_ref().map(|s| s.as_str()).unwrap_or("SHA1") {
        "SHA1" => Algorithm::Sha1,
        "SHA256" => Algorithm::Sha256,
        "SHA512" => Algorithm::Sha512,
        _ => return Err(Error::Unsupported),
    };
    let (prefix, account) = match label.split_once(':') {
        Some((a, b)) => (Some(a), b),
        None => (None, label.as_str()),
    };
    let issuer_text = issuer.as_ref().map(|s| s.as_str()).or(prefix).unwrap_or("");
    if prefix.is_some_and(|p| p != issuer_text) || account.contains(':') {
        return Err(Error::Label);
    }
    let counter = match (kind, counter.as_ref()) {
        (Kind::Totp, None) => 0,
        (Kind::Hotp, Some(v)) => decimal(v)?,
        _ => return Err(Error::Counter),
    };
    let seed = base32(secret.as_ref().ok_or(Error::Seed)?)?;
    Import::new(kind, algorithm, issuer_text, account, counter, &seed)
        .map_err(|_| Error::Format)?;
    Ok(Prepared {
        issuer: Zeroizing::new(issuer_text.to_owned()),
        account: Zeroizing::new(account.to_owned()),
        seed,
        kind,
        algorithm,
        counter,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const SEED: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"; // RFC4226 synthetic20
    fn uri(query: &str) -> String {
        format!("otpauth://totp/Example:alice?secret={SEED}{query}")
    }
    #[test]
    fn rfc_seed_decodes_without_ever_producing_a_host_code() {
        let ready = parse(&uri("&issuer=Example&algorithm=SHA1&digits=6&period=30")).unwrap();
        let canonical = ready.canonical().unwrap();
        let parsed = Import::parse(&canonical).unwrap();
        assert_eq!(parsed.seed_for_provisioning(), b"12345678901234567890");
        assert_eq!(parsed.issuer(), "Example");
        assert_eq!(parsed.account(), "alice");
        let hotp = format!("otpauth://hotp/Example:alice?secret={SEED}&counter=42");
        let prepared = parse(&hotp).unwrap().canonical().unwrap();
        assert_eq!(Import::parse(&prepared).unwrap().initial_counter(), 42);
    }
    #[test]
    fn ambiguous_duplicate_unsupported_and_misleading_uris_are_rejected() {
        for suffix in [
            "&secret=AAAA",
            "&issuer=Other",
            "&issuer=Example&issuer=Example",
            "&digits=8",
            "&period=60",
            "&counter=1",
            "&algorithm=MD5",
            "&foo=bar",
            "#fragment",
        ] {
            assert!(parse(&uri(suffix)).is_err(), "{suffix}");
        }
        for bad in [
            "otpauth://totp/a?secret=A",
            "otpauth://hotp/a?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
            "otpauth://totp/a%0Ab?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
            "otpauth://totp/a?se%63ret=AAAA",
        ] {
            assert!(parse(bad).is_err());
        }
    }
    #[test]
    fn base32_padding_bits_and_counter_syntax_are_not_silently_normalized() {
        for bad in [
            "MZXW6===",
            "A!AAAAAAAAAAAAAAAAAAAAAAAA",
            "AAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "AAAAAAAAAAAAAAAAAAAAAAAAAB",
            "AAAAAAAAAAAAAAAAAAAAAAAAAA=====",
        ] {
            assert!(base32(bad).is_err(), "{bad}");
        }
        assert_eq!(
            base32("AAAAAAAAAAAAAAAAAAAAAAAAAA======").unwrap().len(),
            16
        );
        for bad in ["", "+1", "-1", " 1", "01", "18446744073709551616"] {
            assert!(decimal(bad).is_err());
        }
    }
}
