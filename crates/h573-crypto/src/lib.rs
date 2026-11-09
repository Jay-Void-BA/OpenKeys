#![no_std]
#![forbid(unsafe_code)]
//! Bounded STM32H573 SAES register sequencing, RM0481 Rev5 chapter38.
//! The target port owns the unique peripheral; no other HAL cipher runs in
//! parallel. Poll errors are checked BEFORE clearing CCF; only CCF is cleared
//! on normal progress. All modes clear key/context state on success or error.
//! Register-model tests are not on-chip KAT or physical side-channel evidence.
use zeroize::Zeroizing;

pub mod entropy;
mod shared;
pub use shared::Service;

const EN: u32 = 1;
const KEYPROT: u32 = 1 << 19;
const IPRST: u32 = 1 << 31;
const BUSY: u32 = 1 << 3;
const KEYVALID: u32 = 1 << 7;
const CCF: u32 = 1;
const ERRORS: u32 = 0b1110;
const STATUS_ERRORS: u32 = (1 << 1) | (1 << 2);
const MAX_POLLS: usize = 500_000;
pub const MAX_MESSAGE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Input,
    Hardware,
    Timeout,
    Faulted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Register {
    Cr,
    Sr,
    Isr,
    Icr,
    Ier,
    Din,
    Dout,
    Key(u8),
    Iv(u8),
}

/// Internal protected peripheral port, not a host register-access API. Owns
/// the RCC/SAES token. A stuck key erasure must reset the whole device, whose
/// hardware NRST path disables SE and clears the LCD independently.
pub trait Registers {
    fn read(&mut self, register: Register) -> u32;
    fn write(&mut self, register: Register, value: u32);
    fn emergency_reset(&mut self) -> !;
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Encrypt,
    Decrypt,
}

pub struct MaskedAes<P: Registers> {
    port: P,
    faulted: bool,
}
impl<P: Registers> MaskedAes<P> {
    pub fn new(port: P) -> Result<Self, Error> {
        let mut this = Self {
            port,
            faulted: false,
        };
        let result = this.status();
        let erase_fault = this.erase();
        result?;
        if erase_fault {
            return Err(Error::Hardware);
        }
        this.status()?;
        Ok(this)
    }
    fn status(&mut self) -> Result<(), Error> {
        if self.port.read(Register::Isr) & ERRORS != 0
            || self.port.read(Register::Sr) & STATUS_ERRORS != 0
        {
            return Err(Error::Hardware);
        }
        Ok(())
    }
    fn wait(&mut self, register: Register, mask: u32, set: bool) -> Result<(), Error> {
        for _ in 0..MAX_POLLS {
            self.status()?;
            if (self.port.read(register) & mask != 0) == set {
                return Ok(());
            }
        }
        Err(Error::Timeout)
    }
    fn idle(&mut self) -> Result<(), Error> {
        self.wait(Register::Sr, BUSY, false)
    }
    fn cr(&mut self, value: u32) -> Result<(), Error> {
        self.idle()?;
        self.port.write(Register::Cr, value);
        self.status()
    }
    fn erase(&mut self) -> bool {
        // Erasure must also run on error. Do not wait for an error bit to clear
        // and never clear it as if normal processing had succeeded.
        let mut fault = false;
        for _ in 0..MAX_POLLS {
            fault |= self.status().is_err();
            if self.port.read(Register::Sr) & BUSY == 0 {
                self.port.write(Register::Ier, 0);
                let cr = self.port.read(Register::Cr) & !EN;
                self.port.write(Register::Cr, cr);
                // Preserve faults arriving after the last output check. The
                // IPRST below must not erase them into an apparent success.
                fault |= self.status().is_err();
                self.port.write(Register::Cr, cr | IPRST);
                self.port.write(Register::Cr, 0);
                // IPRST starts a new mask fetch. Do not return/drop into low
                // power before it completes (RM0481 section38.4.3).
                for _ in 0..MAX_POLLS {
                    fault |= self.status().is_err();
                    let status = self.port.read(Register::Sr);
                    if status & BUSY == 0 {
                        if status & KEYVALID != 0 || self.port.read(Register::Cr) & EN != 0 {
                            self.port.emergency_reset();
                        }
                        return fault;
                    }
                }
                self.port.emergency_reset();
            }
        }
        self.port.emergency_reset();
    }
    fn finish<T>(&mut self, result: Result<T, Error>) -> Result<T, Error> {
        let erase_fault = self.erase();
        let result = result.and_then(|value| {
            if erase_fault {
                Err(Error::Hardware)
            } else {
                Ok(value)
            }
        });
        let result = result.and_then(|value| self.status().map(|()| value));
        if result.is_err() {
            self.faulted = true;
        }
        result
    }
    fn start<const N: usize>(
        &mut self,
        key: &[u8; N],
        mode: u32,
        direction: Direction,
        iv: Option<&[u8; 16]>,
    ) -> Result<u32, Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        if N != 16 && N != 32 {
            return Err(Error::Input);
        }
        self.status()?;
        if self.erase() {
            return Err(Error::Hardware);
        }
        let control = KEYPROT
            | if N == 32 { 1 << 18 } else { 0 }
            | (mode << 5)
            | if direction == Direction::Decrypt {
                2 << 3
            } else {
                0
            };
        self.cr(control)?;
        // RM0481 tables373/377: high word goes to the highest register.
        // IVR0 is the low counter word, NOT the first nonce word.
        if let Some(iv) = iv {
            for (index, word) in iv.as_chunks::<4>().0.iter().enumerate() {
                self.port
                    .write(Register::Iv(3 - index as u8), u32::from_be_bytes(*word));
            }
        }
        for (index, word) in key.as_chunks::<4>().0.iter().enumerate() {
            self.port.write(
                Register::Key((N / 4 - 1 - index) as u8),
                u32::from_be_bytes(*word),
            );
        }
        self.wait(Register::Sr, KEYVALID, true)?;
        if direction == Direction::Decrypt && mode <= 1 {
            self.cr((control & !(3 << 3)) | (1 << 3) | EN)?;
            self.wait(Register::Isr, CCF, true)?;
            self.port.write(Register::Icr, CCF);
            self.cr(control)?;
        }
        self.cr(control | EN)?;
        if mode == 3 {
            self.wait(Register::Isr, CCF, true)?;
            self.port.write(Register::Icr, CCF);
        } else {
            self.idle()?;
        }
        Ok(control)
    }
    fn input(&mut self, block: &[u8; 16]) -> Result<(), Error> {
        self.status()?;
        for word in block.as_chunks::<4>().0 {
            self.port.write(Register::Din, u32::from_be_bytes(*word));
        }
        self.wait(Register::Isr, CCF, true)
    }
    fn output(&mut self, out: &mut [u8; 16]) -> Result<(), Error> {
        self.status()?;
        for word in out.as_chunks_mut::<4>().0 {
            word.copy_from_slice(&self.port.read(Register::Dout).to_be_bytes());
        }
        self.status()?;
        // Never silently clear KEIF, RNGEIF or RWEIF.
        self.port.write(Register::Icr, CCF);
        Ok(())
    }
    fn block(&mut self, input: &[u8; 16], out: &mut [u8; 16]) -> Result<(), Error> {
        self.input(input)?;
        self.output(out)
    }
    pub fn ecb<const N: usize>(
        &mut self,
        key: &[u8; N],
        direction: Direction,
        input: &[u8; 16],
        out: &mut [u8; 16],
    ) -> Result<(), Error> {
        out.fill(0);
        if self.faulted {
            return Err(Error::Faulted);
        }
        let result = (|| {
            self.start(key, 0, direction, None)?;
            self.block(input, out)
        })();
        let result = self.finish(result);
        if result.is_err() {
            out.fill(0)
        }
        result
    }
    pub fn cbc<const N: usize>(
        &mut self,
        key: &[u8; N],
        direction: Direction,
        iv: &[u8; 16],
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        if self.faulted {
            bytes.fill(0);
            return Err(Error::Faulted);
        }
        let result = (|| {
            if bytes.is_empty() || bytes.len() > MAX_MESSAGE || !bytes.len().is_multiple_of(16) {
                return Err(Error::Input);
            }
            self.start(key, 1, direction, Some(iv))?;
            for block in bytes.as_chunks_mut::<16>().0 {
                let input = Zeroizing::new(*block);
                self.block(&input, block)?;
            }
            Ok(())
        })();
        let result = self.finish(result);
        if result.is_err() {
            bytes.fill(0)
        }
        result
    }
    /// SP800-38B CMAC over bounded messages. AES is hardware masked; the CPU
    /// subkey/XOR and memory paths still require physical leakage evaluation.
    pub fn cmac<const N: usize>(
        &mut self,
        key: &[u8; N],
        message: &[u8],
        out: &mut [u8; 16],
    ) -> Result<(), Error> {
        out.fill(0);
        if self.faulted {
            return Err(Error::Faulted);
        }
        let result = (|| {
            if message.len() > MAX_MESSAGE {
                return Err(Error::Input);
            }
            let mut l = Zeroizing::new([0; 16]);
            self.ecb(key, Direction::Encrypt, &[0; 16], &mut l)?;
            let k1 = double(&l);
            let k2 = double(&k1);
            let complete = !message.is_empty() && message.len().is_multiple_of(16);
            let blocks = message.len().div_ceil(16).max(1);
            self.start(key, 1, Direction::Encrypt, Some(&[0; 16]))?;
            for index in 0..blocks {
                let start = index * 16;
                let length = (message.len() - start.min(message.len())).min(16);
                let mut input = Zeroizing::new([0; 16]);
                input[..length].copy_from_slice(&message[start..start + length]);
                if index == blocks - 1 {
                    if !complete {
                        input[length] = 0x80;
                    }
                    let subkey = if complete { &k1 } else { &k2 };
                    for i in 0..16 {
                        input[i] ^= subkey[i];
                    }
                }
                self.block(&input, out)?;
            }
            Ok(())
        })();
        let result = self.finish(result);
        if result.is_err() {
            out.fill(0)
        }
        result
    }
    /// Computes a GCM tag and tentative payload in-place. A decrypt caller
    /// MUST keep bytes private/zeroizing, verify the tag in constant time and
    /// recheck the live authorization BEFORE releasing plaintext.
    pub fn gcm<const N: usize>(
        &mut self,
        key: &[u8; N],
        direction: Direction,
        nonce: &[u8; 12],
        aad: &[u8],
        bytes: &mut [u8],
    ) -> Result<[u8; 16], Error> {
        if self.faulted {
            bytes.fill(0);
            return Err(Error::Faulted);
        }
        let result = (|| {
            if aad.len() > MAX_MESSAGE || bytes.len() > MAX_MESSAGE {
                return Err(Error::Input);
            }
            let mut iv = [0; 16];
            iv[..12].copy_from_slice(nonce);
            iv[15] = 2;
            let control = self.start(key, 3, direction, Some(&iv))?;
            if !aad.is_empty() {
                self.cr(control | (1 << 13) | EN)?;
                for data in aad.chunks(16) {
                    let mut block = [0; 16];
                    block[..data.len()].copy_from_slice(data);
                    self.input(&block)?;
                    self.port.write(Register::Icr, CCF);
                }
            }
            if !bytes.is_empty() {
                self.cr(control | (2 << 13) | EN)?;
                for data in bytes.chunks_mut(16) {
                    let mut input = Zeroizing::new([0; 16]);
                    input[..data.len()].copy_from_slice(data);
                    if data.len() < 16 && direction == Direction::Encrypt {
                        self.cr(control | (2 << 13) | EN | (((16 - data.len()) as u32) << 20))?;
                    }
                    let mut output = Zeroizing::new([0; 16]);
                    self.block(&input, &mut output)?;
                    data.copy_from_slice(&output[..data.len()]);
                }
            }
            // GCM finalization must keep EN asserted across the phase change.
            self.cr(control | (3 << 13) | EN)?;
            let mut lengths = [0; 16];
            lengths[..8].copy_from_slice(&(aad.len() as u64 * 8).to_be_bytes());
            lengths[8..].copy_from_slice(&(bytes.len() as u64 * 8).to_be_bytes());
            let mut tag = [0; 16];
            self.block(&lengths, &mut tag)?;
            Ok(tag)
        })();
        let result = self.finish(result);
        if result.is_err() {
            bytes.fill(0)
        }
        result
    }
}
fn double(block: &[u8; 16]) -> Zeroizing<[u8; 16]> {
    let mut out = Zeroizing::new([0; 16]);
    for i in 0..15 {
        out[i] = (block[i] << 1) | (block[i + 1] >> 7);
    }
    out[15] = (block[15] << 1) ^ (0x87 & 0u8.wrapping_sub(block[0] >> 7));
    out
}
impl<P: Registers> Drop for MaskedAes<P> {
    fn drop(&mut self) {
        if self.erase() {
            self.port.emergency_reset();
        }
    }
}

#[cfg(test)]
mod tests;
