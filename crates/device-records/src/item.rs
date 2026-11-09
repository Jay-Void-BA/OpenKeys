//! Canonical private note/recovery record format, shared by host and device.
//! Parsing borrows the bounded zeroizing backing buffer; it does not allocate
//! strings or trust host-provided lengths/counts before checking them.
use crate::{MAX_VALUE_BYTES, Namespace};
use alloc::vec::Vec;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Format,
    Bounds,
    Text,
    Kind,
}
pub enum Content<'a> {
    Note(&'a str),
    Recovery(Codes<'a>),
}
pub struct View<'a> {
    pub title: &'a str,
    pub content: Content<'a>,
}
pub struct Codes<'a> {
    bytes: &'a [u8],
    count: u16,
}
impl<'a> Codes<'a> {
    pub fn len(&self) -> usize {
        self.count as usize
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub fn iter(&self) -> CodeIter<'a> {
        CodeIter {
            bytes: self.bytes,
            remaining: self.count,
        }
    }
}
pub struct CodeIter<'a> {
    bytes: &'a [u8],
    remaining: u16,
}
impl<'a> Iterator for CodeIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        // Only a fully validated View constructs Codes. Still use checked
        // slicing/UTF8 so a coding fault cannot become an unchecked reference.
        let length = self.bytes.get(..2)?;
        let n = u16::from_be_bytes([length[0], length[1]]) as usize;
        let code = core::str::from_utf8(self.bytes.get(2..2 + n)?).ok()?;
        self.bytes = self.bytes.get(2 + n..)?;
        self.remaining -= 1;
        Some(code)
    }
}
fn valid(text: &str, limit: usize, multiline: bool) -> bool {
    if text.is_empty() || text.len() > limit {
        return false;
    }
    !text.chars().any(|c| {
        (c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')))
            || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    })
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let value = self.0.get(..n).ok_or(Error::Format)?;
        self.0 = &self.0[n..];
        Ok(value)
    }
    fn short(&mut self) -> Result<usize, Error> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]) as usize)
    }
    fn text(&mut self, n: usize, limit: usize, multiline: bool) -> Result<&'a str, Error> {
        if n > limit {
            return Err(Error::Bounds);
        }
        let s = core::str::from_utf8(self.take(n)?).map_err(|_| Error::Text)?;
        if !valid(s, limit, multiline) {
            return Err(Error::Text);
        }
        Ok(s)
    }
}
impl<'a> View<'a> {
    pub fn parse(namespace: Namespace, bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_VALUE_BYTES {
            return Err(Error::Bounds);
        }
        let mut r = Reader(bytes);
        let h = r.take(2)?;
        if h[0] != 1 {
            return Err(Error::Format);
        }
        let kind = match namespace {
            Namespace::Note => 1,
            Namespace::Recovery => 2,
            Namespace::Fido => return Err(Error::Kind),
        };
        if h[1] != kind {
            return Err(Error::Kind);
        }
        let n = r.short()?;
        let title = r.text(n, 256, false)?;
        let content = if kind == 1 {
            let b = r.take(4)?;
            let n = u32::from_be_bytes(b.try_into().map_err(|_| Error::Format)?) as usize;
            Content::Note(r.text(n, 16384, true)?)
        } else {
            let count = r.short()?;
            if !(1..=64).contains(&count) {
                return Err(Error::Bounds);
            }
            let codes = r.0;
            for _ in 0..count {
                let n = r.short()?;
                r.text(n, 256, false)?;
            }
            Content::Recovery(Codes {
                bytes: codes,
                count: count as u16,
            })
        };
        if !r.0.is_empty() {
            return Err(Error::Format);
        }
        Ok(Self { title, content })
    }
}
pub fn note(title: &str, body: &str) -> Result<Zeroizing<Vec<u8>>, Error> {
    if !valid(title, 256, false) || !valid(body, 16384, true) {
        return Err(Error::Text);
    }
    let mut b = Zeroizing::new(Vec::with_capacity(8 + title.len() + body.len()));
    b.extend([1, 1]);
    b.extend((title.len() as u16).to_be_bytes());
    b.extend(title.as_bytes());
    b.extend((body.len() as u32).to_be_bytes());
    b.extend(body.as_bytes());
    Ok(b)
}
pub fn recovery(title: &str, codes: &[&str]) -> Result<Zeroizing<Vec<u8>>, Error> {
    if !valid(title, 256, false)
        || !(1..=64).contains(&codes.len())
        || codes.iter().any(|c| !valid(c, 256, false))
    {
        return Err(Error::Text);
    }
    let length = 6 + title.len() + codes.iter().map(|c| 2 + c.len()).sum::<usize>();
    if length > MAX_VALUE_BYTES {
        return Err(Error::Bounds);
    }
    let mut b = Zeroizing::new(Vec::with_capacity(length));
    b.extend([1, 2]);
    b.extend((title.len() as u16).to_be_bytes());
    b.extend(title.as_bytes());
    b.extend((codes.len() as u16).to_be_bytes());
    for c in codes {
        b.extend((c.len() as u16).to_be_bytes());
        b.extend(c.as_bytes());
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec;
    #[test]
    fn exact_note_and_recovery_encodings_round_trip_without_owned_plaintext_copies() {
        let b = note("Title", "Body\nline2").unwrap();
        assert_eq!(&b[..], b"\x01\x01\x00\x05Title\x00\x00\x00\x0aBody\nline2");
        let v = View::parse(Namespace::Note, &b).unwrap();
        assert_eq!(v.title, "Title");
        assert!(matches!(v.content, Content::Note("Body\nline2")));
        let b = recovery("Backup", &["A-123", "B-456"]).unwrap();
        let v = View::parse(Namespace::Recovery, &b).unwrap();
        assert_eq!(v.title, "Backup");
        let Content::Recovery(c) = v.content else {
            panic!()
        };
        assert_eq!(c.len(), 2);
        assert_eq!(c.iter().collect::<Vec<_>>(), vec!["A-123", "B-456"]);
        assert!(View::parse(Namespace::Note, &b).is_err());
        assert!(View::parse(Namespace::Fido, &b).is_err());
    }
    #[test]
    fn every_truncation_bad_length_trailer_and_confusing_text_is_rejected() {
        for b in [
            note("Title", "Body").unwrap(),
            recovery("Backup", &["A", "B"]).unwrap(),
        ] {
            let ns = if b[1] == 1 {
                Namespace::Note
            } else {
                Namespace::Recovery
            };
            for n in 0..b.len() {
                assert!(View::parse(ns, &b[..n]).is_err());
            }
            let mut b = b.to_vec();
            b.push(0);
            assert!(View::parse(ns, &b).is_err());
            b.pop();
            b[2..4].copy_from_slice(&u16::MAX.to_be_bytes());
            assert!(View::parse(ns, &b).is_err());
        }
        for c in [
            '\0', '\u{061c}', '\u{200e}', '\u{200f}', '\u{202e}', '\u{2066}',
        ] {
            let bad = alloc::format!("name{c}");
            assert!(note(&bad, "ok").is_err());
            assert!(note("ok", &bad).is_err());
            assert!(recovery("ok", &[&bad]).is_err());
        }
        assert!(note("", "body").is_err());
        assert!(recovery("name", &[]).is_err());
        assert!(recovery("name", &["x"; 65]).is_err());
    }
    #[test]
    fn maximum_import_record_sizes_fit_the_same_device_bound() {
        let title = "t".repeat(256);
        let body = "b".repeat(16384);
        let b = note(&title, &body).unwrap();
        assert!(b.len() <= MAX_VALUE_BYTES);
        View::parse(Namespace::Note, &b).unwrap();
        let code = "c".repeat(256);
        let codes = [code.as_str(); 64];
        let b = recovery(&title, &codes).unwrap();
        assert!(b.len() <= MAX_VALUE_BYTES);
        View::parse(Namespace::Recovery, &b).unwrap();
    }
    #[test]
    fn bounded_malformed_input_corpus_never_panics() {
        let mut seed = 7u32;
        for n in 0..1024 {
            let mut b = vec![0; n];
            for byte in &mut b {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *byte = (seed >> 24) as u8;
            }
            let _ = View::parse(Namespace::Note, &b);
            let _ = View::parse(Namespace::Recovery, &b);
        }
    }
}
