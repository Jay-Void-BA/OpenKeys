use crate::{CHANNEL_MS, Error, Role};
use alloc::{vec, vec::Vec};
use device_policy::session::{Binding, Transport};
use noise_protocol::CipherState;
use noise_rust_crypto::ChaCha20Poly1305;
use zeroize::Zeroizing;

pub const MAX_PLAINTEXT: usize = 17_408;
const HEADER: usize = 16;
pub const MAX_RECORD: usize = HEADER + MAX_PLAINTEXT + 16;
const MAX_RECORDS: u64 = 65_536;
const MAX_BYTES: u64 = 16_777_216;

/// Supplied only by native USB/RF lifecycle code. Increment epoch on attach,
/// reset, suspend, select/reselect and field loss. Zero/host epochs are invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeLink {
    transport: Transport,
    epoch: u64,
}
impl NativeLink {
    pub fn new(transport: Transport, epoch: u64) -> Result<Self, Error> {
        let link = Self { transport, epoch };
        link.validate()?;
        Ok(link)
    }
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.epoch == 0 {
            Err(Error::Link)
        } else {
            Ok(())
        }
    }
    pub(crate) const fn transport_byte(self) -> u8 {
        match self.transport {
            Transport::Usb => 1,
            Transport::Nfc => 2,
        }
    }
    pub(crate) fn epoch_bytes(self) -> [u8; 8] {
        self.epoch.to_be_bytes()
    }
}
pub struct Context {
    transcript: [u8; 32],
    peer: [u8; 32],
    link: NativeLink,
}
impl Context {
    pub(crate) fn new(
        transcript: [u8; 32],
        peer: [u8; 32],
        link: NativeLink,
    ) -> Result<Self, Error> {
        if transcript == [0; 32] || peer == [0; 32] {
            return Err(Error::Peer);
        }
        link.validate()?;
        Ok(Self {
            transcript,
            peer,
            link,
        })
    }
    pub const fn transcript(&self) -> [u8; 32] {
        self.transcript
    }
    pub const fn peer(&self) -> [u8; 32] {
        self.peer
    }
    pub const fn link(&self) -> NativeLink {
        self.link
    }
}
pub struct Channel {
    send: Option<CipherState<ChaCha20Poly1305>>,
    receive: Option<CipherState<ChaCha20Poly1305>>,
    pub(crate) role: Role,
    context: Context,
    last: u64,
    expires: u64,
    sent: u64,
    received: u64,
}
impl Channel {
    pub(crate) fn new(
        send: CipherState<ChaCha20Poly1305>,
        receive: CipherState<ChaCha20Poly1305>,
        role: Role,
        context: Context,
        now: u64,
    ) -> Result<Self, Error> {
        Ok(Self {
            send: Some(send),
            receive: Some(receive),
            role,
            context,
            last: now,
            expires: now.checked_add(CHANNEL_MS).ok_or(Error::Time)?,
            sent: 0,
            received: 0,
        })
    }
    pub fn context(&self) -> &Context {
        &self.context
    }
    /// Only accepted peers have this API. Pairing candidates cannot obtain a
    /// cached-PIN binding; their only key-side path is fresh local approval.
    pub fn binding(&self) -> Result<Binding, Error> {
        Binding::from_verified_channel(
            self.context.transcript,
            self.context.peer,
            self.context.link.epoch,
            self.context.link.transport,
        )
        .map_err(|_| Error::Link)
    }
    /// Immediate nonblocking key destruction. Root also revokes PIN leases,
    /// clears private UI/input and cuts any native SE authentication session.
    pub fn close(&mut self) {
        self.send = None;
        self.receive = None;
    }
    fn fail<T>(&mut self, error: Error) -> Result<T, Error> {
        self.close();
        Err(error)
    }
    pub(crate) fn check(&mut self, link: NativeLink, now: u64) -> Result<(), Error> {
        if self.send.is_none() || self.receive.is_none() {
            return Err(Error::Closed);
        }
        if link != self.context.link {
            return self.fail(Error::Link);
        }
        if now < self.last {
            return self.fail(Error::Time);
        }
        if now >= self.expires {
            return self.fail(Error::Expired);
        }
        self.last = now;
        Ok(())
    }
    fn header(sender: u8, nonce: u64, length: usize) -> [u8; HEADER] {
        let mut header = [0; HEADER];
        header[..4].copy_from_slice(&[b'O', b'K', 1, sender]);
        header[4..12].copy_from_slice(&nonce.to_be_bytes());
        header[12..14].copy_from_slice(&(length as u16).to_be_bytes());
        header
    }
    fn ad(&self, header: &[u8]) -> [u8; 48] {
        let mut ad = [0; 48];
        ad[..16].copy_from_slice(header);
        ad[16..].copy_from_slice(&self.context.transcript);
        ad
    }
    pub fn seal(&mut self, link: NativeLink, now: u64, plaintext: &[u8]) -> Result<Vec<u8>, Error> {
        self.check(link, now)?;
        if plaintext.is_empty() || plaintext.len() > MAX_PLAINTEXT {
            return self.fail(Error::Bounds);
        }
        let nonce = self.send.as_ref().ok_or(Error::Closed)?.get_next_n();
        if nonce >= MAX_RECORDS || self.sent + plaintext.len() as u64 > MAX_BYTES {
            return self.fail(Error::Expired);
        }
        let header = Self::header(self.role.sender(), nonce, plaintext.len());
        let ad = self.ad(&header);
        let mut out = vec![0; HEADER + plaintext.len() + 16];
        out[..HEADER].copy_from_slice(&header);
        self.send
            .as_mut()
            .ok_or(Error::Closed)?
            .encrypt_ad(&ad, plaintext, &mut out[HEADER..]);
        self.sent += plaintext.len() as u64;
        Ok(out)
    }
    /// Caller MUST recheck native epoch after blocking I/O and pass only one
    /// complete bounded record. No unauthenticated plaintext is returned.
    pub fn open(
        &mut self,
        link: NativeLink,
        now: u64,
        record: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, Error> {
        self.check(link, now)?;
        if record.len() < HEADER + 17 || record.len() > MAX_RECORD {
            return self.fail(Error::Bounds);
        }
        let n = u16::from_be_bytes([record[12], record[13]]) as usize;
        if n == 0 || n > MAX_PLAINTEXT || record.len() != HEADER + n + 16 {
            return self.fail(Error::Bounds);
        }
        let nonce = self.receive.as_ref().ok_or(Error::Closed)?.get_next_n();
        if nonce >= MAX_RECORDS || self.received + n as u64 > MAX_BYTES {
            return self.fail(Error::Expired);
        }
        let expected = Self::header(3 - self.role.sender(), nonce, n);
        if record[..HEADER] != expected {
            return self.fail(Error::Replay);
        }
        let ad = self.ad(&expected);
        let mut out = Zeroizing::new(vec![0; n]);
        if self
            .receive
            .as_mut()
            .ok_or(Error::Closed)?
            .decrypt_ad(&ad, &record[HEADER..], &mut out)
            .is_err()
        {
            return self.fail(Error::Crypto);
        }
        self.received += n as u64;
        Ok(out)
    }
}
