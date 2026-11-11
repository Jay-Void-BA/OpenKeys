//! Common ISO7816 framing for USB CCID and NFC ISO-DEP. SW is transport status,
//! never authentication. Only Noise-authenticated records carry application
//! results. Native disconnect/reselect MUST call Endpoint::reset and lock Root.
use crate::{Error, MAX_RECORD};
use alloc::{vec, vec::Vec};
use zeroize::Zeroizing;

/// Proprietary-use AID, not a claim of an assigned ISO7816 RID.
pub const AID: &[u8] = b"\xF0OPENKEYS\x05\x01";
pub const FRAGMENT: usize = 240;
pub const MESSAGE_MS: u64 = 5_000;
const OK: [u8; 2] = [0x90, 0];

pub trait Handler {
    /// Empty-payload Noise XX only; input32 -> output96, input64 -> empty.
    fn handshake(&mut self, message: &[u8]) -> Result<Vec<u8>, Error>;
    /// Input and output MUST be encrypted records, never decoded payloads.
    fn encrypted_record(&mut self, record: &[u8]) -> Result<Vec<u8>, Error>;
    fn revoke(&mut self);
}
pub struct Endpoint<H: Handler> {
    handler: H,
    selected: bool,
    incoming: Zeroizing<Vec<u8>>,
    outgoing: Vec<u8>, // ciphertext only
    expected: usize,
    next_sequence: u8,
    read_offset: usize,
    deadline: Option<u64>,
    last: u64,
    faulted: bool,
}
impl<H: Handler> Endpoint<H> {
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            selected: false,
            incoming: Zeroizing::new(Vec::new()),
            outgoing: Vec::new(),
            expected: 0,
            next_sequence: 0,
            read_offset: 0,
            deadline: None,
            last: 0,
            faulted: false,
        }
    }
    /// Root additionally increments the native link epoch. A SELECT, including
    /// reselect of the same AID, does not preserve Noise keys or either PIN.
    pub fn reset(&mut self) {
        self.handler.revoke();
        self.selected = false;
        self.incoming.clear();
        self.outgoing.clear();
        self.expected = 0;
        self.next_sequence = 0;
        self.read_offset = 0;
        self.deadline = None;
    }
    pub fn tick(&mut self, now: u64) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Closed);
        }
        if now < self.last {
            self.faulted = true;
            self.reset();
            return Err(Error::Time);
        }
        self.last = now;
        if self.deadline.is_some_and(|d| now >= d) {
            self.reset();
            return Err(Error::Expired);
        }
        Ok(())
    }
    fn reject(&mut self) -> Vec<u8> {
        self.reset();
        vec![0x69, 0x85]
    }
    fn payload(apdu: &[u8]) -> Result<&[u8], Error> {
        if apdu.len() < 5 {
            return Err(Error::Format);
        }
        let n = apdu[4] as usize;
        if n == 0 || apdu.len() != n + 5 {
            return Err(Error::Format);
        }
        Ok(&apdu[5..])
    }
    pub fn receive(&mut self, apdu: &[u8], mut clock: impl FnMut() -> u64) -> Vec<u8> {
        let now = clock();
        if self.tick(now).is_err() {
            return self.reject();
        }
        // SELECT is the only APDU permitted while unselected. Canonical short
        // APDU has no optional Le, preventing ambiguous trailing bytes.
        if apdu.get(..4) == Some(&[0, 0xa4, 4, 0]) {
            self.reset();
            return match Self::payload(apdu) {
                Ok(data) if data == AID => {
                    self.selected = true;
                    OK.to_vec()
                }
                _ => vec![0x6a, 0x82],
            };
        }
        if !self.selected || apdu.len() < 4 || apdu[0] != 0x80 {
            return self.reject();
        }
        let result = self.receive_inner(apdu, now, &mut clock);
        match result {
            Ok(reply) => reply,
            Err(_) => self.reject(),
        }
    }
    fn receive_inner(
        &mut self,
        apdu: &[u8],
        now: u64,
        clock: &mut impl FnMut() -> u64,
    ) -> Result<Vec<u8>, Error> {
        let data = Self::payload(apdu)?;
        match apdu[1] {
            0x70 | 0x71
                if apdu[2..4] == [0; 2] && self.expected == 0 && self.outgoing.is_empty() =>
            {
                let expected = if apdu[1] == 0x70 { 32 } else { 64 };
                if data.len() != expected {
                    return Err(Error::Format);
                }
                let mut response = self.handler.handshake(data)?;
                self.tick(clock())?;
                if response.len() != if expected == 32 { 96 } else { 0 } {
                    return Err(Error::Bounds);
                }
                response.extend_from_slice(&OK);
                Ok(response)
            }
            0x72 if self.outgoing.is_empty() => {
                // P1 0:first,1:continuation. P2 strict fragment number. First
                // payload begins with total u16. No partial-message restart.
                if apdu[2] == 0 {
                    if self.expected != 0 || apdu[3] != 0 || data.len() < 3 {
                        return Err(Error::State);
                    }
                    let total = u16::from_be_bytes([data[0], data[1]]) as usize;
                    if !(33..=MAX_RECORD).contains(&total) {
                        return Err(Error::Bounds);
                    }
                    self.expected = total;
                    self.next_sequence = 1;
                    self.deadline = Some(now.checked_add(MESSAGE_MS).ok_or(Error::Time)?);
                    self.incoming.extend_from_slice(&data[2..]);
                } else if apdu[2] == 1 {
                    if self.expected == 0 || apdu[3] != self.next_sequence {
                        return Err(Error::Replay);
                    }
                    self.next_sequence = self.next_sequence.checked_add(1).ok_or(Error::Bounds)?;
                    self.incoming.extend_from_slice(data);
                } else {
                    return Err(Error::Format);
                }
                if data.len() > FRAGMENT || self.incoming.len() > self.expected {
                    return Err(Error::Bounds);
                }
                if self.incoming.len() == self.expected {
                    // Complete reception ends the fragment deadline. The Root
                    // service enforces its own PIN/review/execution budget.
                    self.deadline = None;
                    let response = self.handler.encrypted_record(&self.incoming)?;
                    let completed = clock();
                    self.tick(completed)?;
                    self.incoming.clear();
                    self.expected = 0;
                    self.next_sequence = 0;
                    if !(33..=MAX_RECORD).contains(&response.len()) {
                        return Err(Error::Bounds);
                    }
                    self.outgoing = response;
                    self.read_offset = 0;
                    self.deadline = Some(completed.checked_add(MESSAGE_MS).ok_or(Error::Time)?);
                    let mut reply = (self.outgoing.len() as u16).to_be_bytes().to_vec();
                    reply.extend_from_slice(&OK);
                    Ok(reply)
                } else {
                    Ok(OK.to_vec())
                }
            }
            0x73 if apdu[2..4] == [0; 2] && data.len() == 2 && !self.outgoing.is_empty() => {
                let offset = u16::from_be_bytes([data[0], data[1]]) as usize;
                if offset != self.read_offset {
                    return Err(Error::Replay);
                }
                let end = self.outgoing.len().min(offset + FRAGMENT);
                let mut out = self.outgoing[offset..end].to_vec();
                self.read_offset = end;
                if end == self.outgoing.len() {
                    self.outgoing.clear();
                    self.read_offset = 0;
                    self.deadline = None;
                }
                out.extend_from_slice(&OK);
                Ok(out)
            }
            _ => Err(Error::Format),
        }
    }
}
impl<H: Handler> Drop for Endpoint<H> {
    fn drop(&mut self) {
        self.reset();
    }
}

/// Native adapter owns a PCSC/CTK/IsoDep handle. A failed or uncertain transfer
/// must close it, increment its local epoch and destroy the associated Channel.
pub trait Transport {
    fn transmit(&mut self, command: &[u8], out: &mut [u8]) -> Result<usize, Error>;
    fn close(&mut self);
}
pub struct Client<T: Transport> {
    transport: T,
    closed: bool,
}
impl<T: Transport> Client<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            closed: false,
        }
    }
    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.transport.close();
        }
    }
    fn call(&mut self, ins: u8, p1: u8, p2: u8, data: &[u8]) -> Result<Vec<u8>, Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        let result = (|| {
            if data.is_empty() || data.len() > 255 {
                return Err(Error::Bounds);
            }
            let mut apdu = vec![0x80, ins, p1, p2, data.len() as u8];
            apdu.extend_from_slice(data);
            if ins == 0xa4 {
                apdu[0] = 0;
            }
            let mut output = [0; FRAGMENT + 2];
            let n = self.transport.transmit(&apdu, &mut output)?;
            if n < 2 || n > output.len() || output[n - 2..n] != OK {
                return Err(Error::Link);
            }
            Ok(output[..n - 2].to_vec())
        })();
        if result.is_err() {
            self.close();
        }
        result
    }
    pub fn select(&mut self) -> Result<(), Error> {
        if !self.call(0xa4, 4, 0, AID)?.is_empty() {
            self.close();
            return Err(Error::Format);
        }
        Ok(())
    }
    pub fn handshake(&mut self, message: &[u8]) -> Result<Vec<u8>, Error> {
        let (ins, length) = match message.len() {
            32 => (0x70, 96),
            64 => (0x71, 0),
            _ => {
                self.close();
                return Err(Error::Bounds);
            }
        };
        let response = self.call(ins, 0, 0, message)?;
        if response.len() != length {
            self.close();
            return Err(Error::Format);
        }
        Ok(response)
    }
    pub fn exchange(&mut self, encrypted: &[u8]) -> Result<Vec<u8>, Error> {
        if !(33..=MAX_RECORD).contains(&encrypted.len()) {
            self.close();
            return Err(Error::Bounds);
        }
        let result = self.exchange_inner(encrypted);
        if result.is_err() {
            self.close();
        }
        result
    }
    fn exchange_inner(&mut self, encrypted: &[u8]) -> Result<Vec<u8>, Error> {
        let mut first = (encrypted.len() as u16).to_be_bytes().to_vec();
        let take = encrypted.len().min(FRAGMENT - 2);
        first.extend_from_slice(&encrypted[..take]);
        let mut response = self.call(0x72, 0, 0, &first)?;
        let mut consumed = take;
        let mut sequence = 1u8;
        while consumed < encrypted.len() {
            if !response.is_empty() {
                return Err(Error::Format);
            }
            let end = encrypted.len().min(consumed + FRAGMENT);
            response = self.call(0x72, 1, sequence, &encrypted[consumed..end])?;
            sequence = sequence.checked_add(1).ok_or(Error::Bounds)?;
            consumed = end;
        }
        if response.len() != 2 {
            return Err(Error::Format);
        }
        let total = u16::from_be_bytes([response[0], response[1]]) as usize;
        if !(33..=MAX_RECORD).contains(&total) {
            return Err(Error::Bounds);
        }
        let mut reply = Vec::with_capacity(total);
        while reply.len() < total {
            let part = self.call(0x73, 0, 0, &(reply.len() as u16).to_be_bytes())?;
            if part.len() != (total - reply.len()).min(FRAGMENT) {
                return Err(Error::Format);
            }
            reply.extend_from_slice(&part);
        }
        Ok(reply)
    }
}
impl<T: Transport> Drop for Client<T> {
    fn drop(&mut self) {
        self.close();
    }
}
