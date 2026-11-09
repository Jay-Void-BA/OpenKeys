//! FPC2534 paired-runtime SPI I/O. Frames are ciphertext, never match proofs.
//! No pairing, plaintext fallback, template export or host-facing raw SPI API.
use device_policy::Request;
use device_policy::biometric::EncryptedIdentify;
use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
    spi::{Operation, SpiDevice},
};

pub const RECEIVE_BYTES: usize = 48;
const CS_WAKE_NS: u32 = 600_000; // FPC SPC27317/6 requires >500 us.
const RESET_HOLD_NS: u32 = 10_000_000; // Engineering hold, not a measured minimum.
const MAX_RECEIVED: u8 = 32;
const MAX_IO_US: u64 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Disabled,
    Faulted,
    Io,
    Authorization,
    Time,
    Frame,
    Budget,
}

/// Trusted root checks the CURRENT request/channel/deadline, cancellation,
/// tamper and power faults here. It must not require an already verified match:
/// sensor I/O is how that match is obtained, not a private operation by itself.
/// now_us must read live device monotonic time, including after blocking I/O.
pub trait Guard {
    fn check(&mut self) -> Result<(), Error>;
    fn check_request(&mut self, request: &Request) -> Result<(), Error>;
    fn now_us(&mut self) -> u64;
}

pub struct UntrustedFrame([u8; RECEIVE_BYTES]);
impl UntrustedFrame {
    /// Pass only to the protected GCM/semantic verifier. Header validation and
    /// IRQ alone never authenticate the sensor, a person or a request.
    pub fn wire_bytes(&self) -> &[u8; RECEIVE_BYTES] {
        &self.0
    }
}

#[derive(PartialEq, Eq)]
enum State {
    HeldReset,
    Active,
    Faulted,
}

pub struct Transport<D: SpiDevice, I: InputPin, R: OutputPin> {
    device: D,
    irq: I,
    reset: R,
    state: State,
    last_time: u64,
    received: u8,
}
impl<D: SpiDevice, I: InputPin, R: OutputPin> Transport<D, I, R> {
    /// Own the bus, IRQ and reset pins in the protected domain. Construction
    /// asserts reset; release is a separate guarded, timed operation.
    pub fn new(device: D, irq: I, mut reset: R) -> Result<Self, Error> {
        reset.set_low().map_err(|_| Error::Io)?;
        Ok(Self {
            device,
            irq,
            reset,
            state: State::HeldReset,
            last_time: 0,
            received: 0,
        })
    }
    fn time(&mut self, guard: &mut impl Guard) -> Result<u64, Error> {
        guard.check().map_err(|_| Error::Authorization)?;
        let now = guard.now_us();
        if now < self.last_time {
            return Err(Error::Time);
        }
        self.last_time = now;
        Ok(now)
    }
    fn after(&mut self, guard: &mut impl Guard, started: u64) -> Result<(), Error> {
        if self.time(guard)?.checked_sub(started).ok_or(Error::Time)? >= MAX_IO_US {
            return Err(Error::Time);
        }
        Ok(())
    }
    fn finish<T>(&mut self, result: Result<T, Error>) -> Result<T, Error> {
        if result.is_err() {
            self.stop();
        }
        result
    }
    fn active(&self) -> Result<(), Error> {
        match self.state {
            State::HeldReset => Err(Error::Disabled),
            State::Active => Ok(()),
            State::Faulted => Err(Error::Faulted),
        }
    }
    pub fn release(
        &mut self,
        delay: &mut impl DelayNs,
        guard: &mut impl Guard,
    ) -> Result<(), Error> {
        let result = (|| {
            if self.state != State::HeldReset {
                return Err(Error::Faulted);
            }
            let started = self.time(guard)?;
            delay.delay_ns(RESET_HOLD_NS);
            self.after(guard, started)?;
            self.reset.set_high().map_err(|_| Error::Io)?;
            self.after(guard, started)?;
            self.state = State::Active;
            Ok(())
        })();
        self.finish(result)
    }
    /// Only already-encrypted short runtime requests from the protected root.
    /// This checks framing, NOT encrypted command identity or AEAD validity.
    /// Root supplies a fresh non-reused IV and precharges Identify before send.
    /// A write is attempted once; ambiguous SPI errors are never retransmitted.
    pub fn write(&mut self, frame: &[u8], guard: &mut impl Guard) -> Result<(), Error> {
        let result = (|| {
            self.active()?;
            let started = self.time(guard)?;
            if !(40..=RECEIVE_BYTES).contains(&frame.len())
                || word(frame, 0) != 4
                || word(frame, 2) != 0x11
                || word(frame, 4) != 0x11
                || usize::from(word(frame, 6)) != frame.len() - 8
            {
                return Err(Error::Frame);
            }
            self.device
                .transaction(&mut [Operation::DelayNs(CS_WAKE_NS), Operation::Write(frame)])
                .map_err(|_| Error::Io)?;
            self.after(guard, started)?;
            self.received = 0;
            Ok(())
        })();
        self.finish(result)
    }
    /// Preferred Identify path: consume the policy-minted packet and reject
    /// expiry across a slow write. Recheck the CURRENT root request/channel
    /// explicitly before and after I/O; this is not a cross-domain RPC.
    pub fn send_identify(
        &mut self,
        command: EncryptedIdentify,
        guard: &mut impl Guard,
    ) -> Result<(), Error> {
        let result = (|| {
            self.active()?;
            guard
                .check_request(command.request())
                .map_err(|_| Error::Authorization)?;
            if self.time(guard)? / 1000 >= command.expires_ms() {
                return Err(Error::Time);
            }
            self.write(command.wire_bytes(), guard)?;
            guard
                .check_request(command.request())
                .map_err(|_| Error::Authorization)?;
            if self.time(guard)? / 1000 >= command.expires_ms() {
                return Err(Error::Time);
            }
            Ok(())
        })();
        self.finish(result)
    }
    /// One IRQ check and at most one 48-byte read per call. An idle IRQ does
    /// not wait, feed a watchdog, extend a deadline or reset the frame budget.
    /// Paired Status/Identify are the only supported incoming frame shapes.
    pub fn poll(&mut self, guard: &mut impl Guard) -> Result<Option<UntrustedFrame>, Error> {
        let result = (|| {
            self.active()?;
            let started = self.time(guard)?;
            let ready = self.irq.is_high().map_err(|_| Error::Io)?;
            self.after(guard, started)?;
            if !ready {
                return Ok(None);
            }
            if self.received >= MAX_RECEIVED {
                return Err(Error::Budget);
            }
            self.received += 1;
            let mut frame = [0; RECEIVE_BYTES];
            self.device
                .transaction(&mut [Operation::DelayNs(CS_WAKE_NS), Operation::Read(&mut frame)])
                .map_err(|_| Error::Io)?;
            self.after(guard, started)?;
            if word(&frame, 0) != 4
                || !matches!(word(&frame, 2), 0x12 | 0x13)
                || word(&frame, 4) != 0x41
                || word(&frame, 6) != 40
            {
                return Err(Error::Frame);
            }
            Ok(Some(UntrustedFrame(frame)))
        })();
        self.finish(result)
    }
    /// Terminal for this driver owner. Reset failure cannot turn it back on;
    /// the root must independently revoke grants/blank LCD/handle the fault.
    pub fn stop(&mut self) {
        self.state = State::Faulted;
        let _ = self.reset.set_low();
    }
}
impl<D: SpiDevice, I: InputPin, R: OutputPin> Drop for Transport<D, I, R> {
    fn drop(&mut self) {
        self.stop();
    }
}
fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
