#![no_std]
#![forbid(unsafe_code)]
//! Strict GlobalPlatform SCP03 S8, fixed security-level0x33, per-command
//! request/response state. References: Amendment D and NXP nano-package
//! 26d55b43225bc84de364057660cba676fa69f6e4. Primitives are injected; no
//! software-key/default-key/RNG fallback is provided for production.
//! A channel is not fingerprint authorization. Private clients must enforce
//! device policy and accept only authenticated responses from this API.
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub mod client;

pub const SECURITY_LEVEL: u8 = 0x33;
pub const MAX_DATA: usize = 239;
pub const MAX_WIRE: usize = 261;
/// Rekey well before the counter or a long-lived S8 authentication budget is
/// exhausted. Lifetime/system-wide 64-bit-tag risk still needs review.
pub const MAX_COMMANDS: u32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Input,
    Unsupported,
    Rejected,
    Integrity,
    Crypto,
    State,
    Exhausted,
    Link,
    Authorization,
}

/// AES128/AES256, CMAC (SP800-38B), CBC on complete blocks. The actual H573
/// backend must use its masked SAES and checked hardware RNG in the protected
/// domain. Backend failures are terminal; no key or nonce is logged.
pub trait Crypto<const N: usize> {
    fn random_challenge(&mut self, out: &mut [u8; 8]) -> Result<(), Error>;
    fn cmac(&mut self, key: &[u8; N], message: &[u8], out: &mut [u8; 16]) -> Result<(), Error>;
    fn encrypt_block(
        &mut self,
        key: &[u8; N],
        input: &[u8; 16],
        out: &mut [u8; 16],
    ) -> Result<(), Error>;
    fn encrypt_cbc(&mut self, key: &[u8; N], iv: &[u8; 16], bytes: &mut [u8]) -> Result<(), Error>;
    fn decrypt_cbc(&mut self, key: &[u8; N], iv: &[u8; 16], bytes: &mut [u8]) -> Result<(), Error>;
}

/// Explicit per-device keys from authenticated provisioning. Never public
/// factory/test constants, public UID derivation or a host-provided keyset.
pub struct StaticKeys<const N: usize> {
    enc: Zeroizing<[u8; N]>,
    mac: Zeroizing<[u8; N]>,
}
impl<const N: usize> StaticKeys<N> {
    pub fn new(enc: Zeroizing<[u8; N]>, mac: Zeroizing<[u8; N]>) -> Result<Self, Error> {
        if N != 16 && N != 32 {
            return Err(Error::Unsupported);
        }
        Ok(Self { enc, mac })
    }
}
struct SessionKeys<const N: usize> {
    enc: Zeroizing<[u8; N]>,
    mac: Zeroizing<[u8; N]>,
    rmac: Zeroizing<[u8; N]>,
}

fn derivation<const N: usize>(
    crypto: &mut impl Crypto<N>,
    key: &[u8; N],
    constant: u8,
    bits: u16,
    counter: u8,
    context: &[u8],
) -> Result<Zeroizing<[u8; 16]>, Error> {
    if context.len() > 32 {
        return Err(Error::Input);
    }
    let mut input = Zeroizing::new([0; 48]);
    input[11] = constant;
    input[13..15].copy_from_slice(&bits.to_be_bytes());
    input[15] = counter;
    input[16..16 + context.len()].copy_from_slice(context);
    let mut out = Zeroizing::new([0; 16]);
    crypto.cmac(key, &input[..16 + context.len()], &mut out)?;
    Ok(out)
}
fn key<const N: usize>(
    crypto: &mut impl Crypto<N>,
    master: &[u8; N],
    constant: u8,
    context: &[u8],
) -> Result<Zeroizing<[u8; N]>, Error> {
    let mut output = Zeroizing::new([0; N]);
    if N != 16 && N != 32 {
        return Err(Error::Unsupported);
    }
    for counter in 0..N / 16 {
        let block = derivation(
            crypto,
            master,
            constant,
            (N * 8) as u16,
            (counter + 1) as u8,
            context,
        )?;
        output[counter * 16..(counter + 1) * 16].copy_from_slice(&block[..]);
    }
    Ok(output)
}

/// A fresh challenge is drawn once. This state is consumed, not clonable, so
/// an ambiguous link must start over with fresh entropy and never retry EA.
pub struct Handshake<'a, const N: usize> {
    keys: &'a StaticKeys<N>,
    version: u8,
    aid: &'a [u8],
    challenge: [u8; 8],
}
impl<'a, const N: usize> Handshake<'a, N> {
    pub fn start(
        crypto: &mut impl Crypto<N>,
        keys: &'a StaticKeys<N>,
        version: u8,
        full_aid: &'a [u8],
    ) -> Result<Self, Error> {
        if version == 0 || !(5..=16).contains(&full_aid.len()) {
            return Err(Error::Input);
        }
        let mut challenge = [0; 8];
        crypto.random_challenge(&mut challenge)?;
        Ok(Self {
            keys,
            version,
            aid: full_aid,
            challenge,
        })
    }
    pub fn initialize_update(&self) -> [u8; 14] {
        let mut out = [0; 14];
        out[..5].copy_from_slice(&[0x80, 0x50, self.version, 0, 8]);
        out[5..13].copy_from_slice(&self.challenge);
        out
    }
    pub fn accept(self, crypto: &mut impl Crypto<N>, response: &[u8]) -> Result<Pending<N>, Error> {
        if !matches!(response.len(), 31 | 34) || response[response.len() - 2..] != [0x90, 0] {
            return Err(Error::Rejected);
        }
        let body = &response[..response.len() - 2];
        if body[10] != self.version || body[11] != 3 || !matches!(body[12], 0x60 | 0x70) {
            return Err(Error::Unsupported);
        }
        let pseudo = body[12] == 0x70;
        if body.len() != if pseudo { 32 } else { 29 } {
            return Err(Error::Input);
        }
        let card = &body[13..21];
        if pseudo {
            let mut context = [0; 19];
            context[..3].copy_from_slice(&body[29..32]);
            context[3..3 + self.aid.len()].copy_from_slice(self.aid);
            let expected = derivation(
                crypto,
                &self.keys.enc,
                2,
                64,
                1,
                &context[..3 + self.aid.len()],
            )?;
            if !bool::from(expected[..8].ct_eq(card)) {
                return Err(Error::Integrity);
            }
        }
        let mut context = [0; 16];
        context[..8].copy_from_slice(&self.challenge);
        context[8..].copy_from_slice(card);
        let keys = SessionKeys {
            enc: key(crypto, &self.keys.enc, 4, &context)?,
            mac: key(crypto, &self.keys.mac, 6, &context)?,
            rmac: key(crypto, &self.keys.mac, 7, &context)?,
        };
        let expected = derivation(crypto, &keys.mac, 0, 64, 1, &context)?;
        if !bool::from(expected[..8].ct_eq(&body[21..29])) {
            return Err(Error::Integrity);
        }
        let host = derivation(crypto, &keys.mac, 1, 64, 1, &context)?;
        let mut ea = [0; 21];
        ea[..5].copy_from_slice(&[0x84, 0x82, SECURITY_LEVEL, 0, 16]);
        ea[5..13].copy_from_slice(&host[..8]);
        let mut mac_input = Zeroizing::new([0; 29]);
        mac_input[16..].copy_from_slice(&ea[..13]);
        let mut chain = Zeroizing::new([0; 16]);
        crypto.cmac(&keys.mac, &mac_input[..], &mut chain)?;
        ea[13..].copy_from_slice(&chain[..8]);
        Ok(Pending { keys, chain, ea })
    }
}

pub struct Pending<const N: usize> {
    keys: SessionKeys<N>,
    chain: Zeroizing<[u8; 16]>,
    ea: [u8; 21],
}
impl<const N: usize> Pending<N> {
    pub fn external_authenticate(&self) -> &[u8; 21] {
        &self.ea
    }
    /// EA has a status-only response. This never proves user authorization or
    /// authenticates application data: the first private reply still needs a
    /// valid R-MAC before it can create SecretResponse.
    pub fn finish(self, response: &[u8]) -> Result<Channel<N>, Error> {
        if response != [0x90, 0] {
            return Err(Error::Rejected);
        }
        Ok(Channel {
            keys: self.keys,
            chain: self.chain,
            counter: 0,
            pending: false,
            faulted: false,
        })
    }
}

pub struct Command<'a> {
    pub class: u8,
    pub instruction: u8,
    pub p1: u8,
    pub p2: u8,
    pub data: &'a [u8],
    pub le: Option<u8>,
}
pub struct WireCommand {
    bytes: Zeroizing<[u8; MAX_WIRE]>,
    len: usize,
}
impl WireCommand {
    pub fn wire_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}
pub struct SecretResponse {
    bytes: Zeroizing<[u8; 240]>,
    len: usize,
}
impl SecretResponse {
    pub fn expose(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub struct Channel<const N: usize> {
    keys: SessionKeys<N>,
    chain: Zeroizing<[u8; 16]>,
    counter: u32,
    pending: bool,
    faulted: bool,
}
impl<const N: usize> Channel<N> {
    /// Revoke on cancellation, an uncertain I/O result or protected-runtime
    /// fault. No subsequent call can reuse the retained session state.
    pub fn close(&mut self) {
        self.fault();
    }
    fn fault(&mut self) {
        self.faulted = true;
        self.pending = false;
        self.keys.enc.fill(0);
        self.keys.mac.fill(0);
        self.keys.rmac.fill(0);
        self.chain.fill(0);
    }
    pub fn wrap(
        &mut self,
        crypto: &mut impl Crypto<N>,
        command: &Command<'_>,
    ) -> Result<WireCommand, Error> {
        if self.faulted {
            return Err(Error::State);
        }
        let result = self.wrap_inner(crypto, command);
        if result.is_err() {
            self.fault();
        }
        result
    }
    fn iv(
        &self,
        crypto: &mut impl Crypto<N>,
        response: bool,
    ) -> Result<Zeroizing<[u8; 16]>, Error> {
        let mut input = [0; 16];
        input[0] = if response { 0x80 } else { 0 };
        input[12..].copy_from_slice(&self.counter.to_be_bytes());
        let mut output = Zeroizing::new([0; 16]);
        crypto.encrypt_block(&self.keys.enc, &input, &mut output)?;
        Ok(output)
    }
    fn wrap_inner(
        &mut self,
        crypto: &mut impl Crypto<N>,
        c: &Command<'_>,
    ) -> Result<WireCommand, Error> {
        if self.pending {
            return Err(Error::State);
        }
        if c.data.len() > MAX_DATA
            || !matches!(c.class, 0 | 0x80)
            || matches!(c.instruction, 0x50 | 0x82)
        {
            return Err(Error::Input);
        }
        if self.counter >= MAX_COMMANDS {
            return Err(Error::Exhausted);
        }
        self.counter = self.counter.checked_add(1).ok_or(Error::Exhausted)?;
        let mut out = WireCommand {
            bytes: Zeroizing::new([0; MAX_WIRE]),
            len: 0,
        };
        let body_len = if c.data.is_empty() {
            0
        } else {
            (c.data.len() + 1).next_multiple_of(16)
        };
        out.bytes[..5].copy_from_slice(&[
            c.class | 4,
            c.instruction,
            c.p1,
            c.p2,
            (body_len + 8) as u8,
        ]);
        if body_len != 0 {
            out.bytes[5..5 + c.data.len()].copy_from_slice(c.data);
            out.bytes[5 + c.data.len()] = 0x80;
            let iv = self.iv(crypto, false)?;
            crypto.encrypt_cbc(&self.keys.enc, &iv, &mut out.bytes[5..5 + body_len])?;
        }
        let mut input = Zeroizing::new([0; 288]);
        input[..16].copy_from_slice(&self.chain[..]);
        input[16..21 + body_len].copy_from_slice(&out.bytes[..5 + body_len]);
        crypto.cmac(&self.keys.mac, &input[..21 + body_len], &mut self.chain)?;
        out.bytes[5 + body_len..13 + body_len].copy_from_slice(&self.chain[..8]);
        out.len = 13 + body_len;
        if let Some(le) = c.le {
            out.bytes[out.len] = le;
            out.len += 1;
        }
        self.pending = true;
        Ok(out)
    }
    pub fn unwrap(
        &mut self,
        crypto: &mut impl Crypto<N>,
        response: &[u8],
    ) -> Result<SecretResponse, Error> {
        if self.faulted {
            return Err(Error::State);
        }
        let result = self.unwrap_inner(crypto, response);
        if result.is_err() {
            self.fault();
        }
        result
    }
    fn unwrap_inner(
        &mut self,
        crypto: &mut impl Crypto<N>,
        response: &[u8],
    ) -> Result<SecretResponse, Error> {
        if !self.pending {
            return Err(Error::State);
        }
        // Bare errors are unauthenticated in SCP03. They can fail a session,
        // but MUST NOT become a trusted missing-object/blank-device result.
        if response.len() < 10
            || response.len() > 250
            || response[response.len() - 2..] != [0x90, 0]
        {
            return Err(Error::Rejected);
        }
        let end = response.len() - 10;
        if !end.is_multiple_of(16) {
            return Err(Error::Input);
        }
        let mut input = Zeroizing::new([0; 258]);
        input[..16].copy_from_slice(&self.chain[..]);
        input[16..16 + end].copy_from_slice(&response[..end]);
        input[16 + end..18 + end].copy_from_slice(&[0x90, 0]);
        let mut mac = Zeroizing::new([0; 16]);
        crypto.cmac(&self.keys.rmac, &input[..18 + end], &mut mac)?;
        if !bool::from(mac[..8].ct_eq(&response[end..end + 8])) {
            return Err(Error::Integrity);
        }
        let mut out = SecretResponse {
            bytes: Zeroizing::new([0; 240]),
            len: 0,
        };
        if end != 0 {
            out.bytes[..end].copy_from_slice(&response[..end]);
            let iv = self.iv(crypto, true)?;
            crypto.decrypt_cbc(&self.keys.enc, &iv, &mut out.bytes[..end])?;
            let mut at = end;
            while at > 0 && out.bytes[at - 1] == 0 {
                at -= 1;
            }
            if at == 0 || out.bytes[at - 1] != 0x80 {
                return Err(Error::Integrity);
            }
            out.len = at - 1;
            out.bytes[out.len..].fill(0);
        }
        self.pending = false;
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
