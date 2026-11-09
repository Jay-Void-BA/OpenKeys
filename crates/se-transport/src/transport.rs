//! Bounded T=1 normal-profile exchange. All responses remain unauthenticated
//! until an independent SCP03 layer verifies them. No APDU retry is attempted
//! after an ambiguous write, bad sequence, malformed frame or timeout.
use crate::{
    Control, Direction, Error, Kind, MAX_FRAME, MAX_INF, ReceiveError, UntrustedFrame, WireFrame,
    atr::Atr,
};

pub const MAX_APDU: usize = 1024;
const OPERATION_US: u64 = 5_000_000;
const MAX_RECEIVED_FRAMES: u8 = 32;
const MAX_WTX: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoError {
    AddressNotReady,
    Fault,
}
/// An owned link to the fixed SE I2C address. Mid-frame/unknown NACKs are
/// Fault, not AddressNotReady. A hardware watchdog must bound stuck HAL I/O.
pub trait Io {
    fn write(&mut self, bytes: &[u8]) -> Result<(), IoError>;
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError>;
}
/// Live device clock, never a host timestamp. wait_us may service the trusted
/// scheduler/watchdog, but must not extend the operation's fixed deadline.
pub trait Clock {
    fn now_us(&mut self) -> u64;
    fn wait_us(&mut self, us: u32);
}
struct Budget {
    until: u64,
    frames: u8,
    wtx: u8,
}

pub struct UntrustedApdu<'a>(&'a [u8]);
impl UntrustedApdu<'_> {
    pub fn wire_bytes(&self) -> &[u8] {
        self.0
    }
}

pub struct Transport<I, C> {
    io: I,
    clock: C,
    last_time: u64,
    profile: Option<Atr>,
    tx_sequence: bool,
    rx_sequence: bool,
    faulted: bool,
}
impl<I: Io, C: Clock> Transport<I, C> {
    pub fn new(io: I, mut clock: C) -> Self {
        let last_time = clock.now_us();
        Self {
            io,
            clock,
            last_time,
            profile: None,
            tx_sequence: false,
            rx_sequence: false,
            faulted: false,
        }
    }
    fn now(&mut self) -> Result<u64, Error> {
        let current = self.clock.now_us();
        if current < self.last_time {
            return Err(Error::Timeout);
        }
        self.last_time = current;
        Ok(current)
    }
    fn begin(&mut self) -> Result<Budget, Error> {
        Ok(Budget {
            until: self
                .now()?
                .checked_add(OPERATION_US)
                .ok_or(Error::Timeout)?,
            frames: 0,
            wtx: 0,
        })
    }
    fn check(&mut self, b: &Budget) -> Result<(), Error> {
        if self.now()? >= b.until {
            Err(Error::Timeout)
        } else {
            Ok(())
        }
    }
    fn wait(&mut self, us: u32, b: &Budget) -> Result<(), Error> {
        self.check(b)?;
        self.clock.wait_us(us);
        self.check(b)
    }
    fn guard(&self) -> u32 {
        self.profile.map_or(100, |a| a.guard_us)
    }
    fn poll(&self) -> u32 {
        self.profile.map_or(1000, |a| a.poll_us)
    }
    fn latch(&mut self) {
        self.faulted = true;
        self.profile = None;
    }

    fn send(&mut self, frame: &WireFrame, b: &Budget) -> Result<(), Error> {
        for _ in 0..5000 {
            self.wait(self.guard(), b)?;
            match self.io.write(frame.bytes()) {
                Ok(()) => return self.check(b),
                Err(IoError::AddressNotReady) => self.wait(self.poll(), b)?,
                Err(IoError::Fault) => return Err(Error::Io),
            }
        }
        Err(Error::Timeout)
    }
    fn read_part(&mut self, bytes: &mut [u8], b: &Budget) -> Result<(), Error> {
        self.wait(self.guard(), b)?;
        // Once the header was consumed, an uncertain partial read must not be
        // retried as a fresh frame. This avoids silent byte/sequence shifts.
        self.io.read(bytes).map_err(|_| Error::Io)?;
        self.check(b)
    }
    fn receive_frame(&mut self, b: &mut Budget) -> Result<WireFrame, Error> {
        if b.frames == MAX_RECEIVED_FRAMES {
            return Err(Error::Timeout);
        }
        let mut frame = WireFrame {
            bytes: [0; MAX_FRAME],
            len: 0,
        };
        let start = self.now()?;
        let block_limit = start
            .checked_add(u64::from(
                self.profile.map_or(1_000_000, |a| a.block_wait_us),
            ))
            .ok_or(Error::Timeout)?
            .min(b.until);
        let mut ready = false;
        for _ in 0..5000 {
            self.wait(self.guard(), b)?;
            if self.now()? >= block_limit {
                return Err(Error::Timeout);
            }
            match self.io.read(&mut frame.bytes[..3]) {
                Ok(()) => {
                    ready = true;
                    break;
                }
                Err(IoError::AddressNotReady) => self.wait(self.poll(), b)?,
                Err(IoError::Fault) => return Err(Error::Io),
            }
        }
        if !ready {
            return Err(Error::Timeout);
        }
        self.check(b)?;
        let len = usize::from(frame.bytes[2]);
        if frame.bytes[0] != 0xa5 {
            return Err(Error::Address);
        }
        if len > MAX_INF {
            return Err(Error::Length);
        }
        frame.len = len + 5;
        if len != 0 {
            self.read_part(&mut frame.bytes[3..3 + len], b)?;
        }
        self.read_part(&mut frame.bytes[3 + len..frame.len], b)?;
        UntrustedFrame::decode(frame.bytes(), Direction::SeToHost)?;
        b.frames += 1;
        Ok(frame)
    }
    fn receive(&mut self, b: &mut Budget) -> Result<WireFrame, Error> {
        loop {
            let frame = self.receive_frame(b)?;
            let decoded = UntrustedFrame::decode(frame.bytes(), Direction::SeToHost)?;
            if decoded.kind
                == (Kind::Supervisory {
                    response: false,
                    control: Control::Wtx,
                })
            {
                if b.wtx == MAX_WTX {
                    return Err(Error::Timeout);
                }
                b.wtx += 1;
                let reply = WireFrame::encode(
                    Direction::HostToSe,
                    Kind::Supervisory {
                        response: true,
                        control: Control::Wtx,
                    },
                    decoded.info,
                )?;
                self.send(&reply, b)?;
                // WTX may request more work, but never restarts either the
                // total deadline or frame/poll/WTX budgets.
                continue;
            }
            return Ok(frame);
        }
    }

    /// Reset the public link and strictly parse the qualified normal-profile
    /// ATR. This neither authenticates the SE nor changes keys/lifecycle.
    /// After any fault construct a new instance only in trusted recovery; do
    /// not automatically resubmit the interrupted private APDU.
    pub fn synchronize(&mut self, expected_version: u8) -> Result<Atr, Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        if self.profile.is_some() {
            return Err(Error::Control);
        }
        let result = self.synchronize_inner(expected_version);
        if result.is_err() {
            self.latch();
        }
        result
    }
    fn synchronize_inner(&mut self, version: u8) -> Result<Atr, Error> {
        let mut budget = self.begin()?;
        for control in [Control::Resync, Control::InterfaceReset] {
            let request = WireFrame::encode(
                Direction::HostToSe,
                Kind::Supervisory {
                    response: false,
                    control,
                },
                &[],
            )?;
            self.send(&request, &budget)?;
            let response = self.receive(&mut budget)?;
            let decoded = UntrustedFrame::decode(response.bytes(), Direction::SeToHost)?;
            if decoded.kind
                != (Kind::Supervisory {
                    response: true,
                    control,
                })
            {
                return Err(Error::Control);
            }
            if control == Control::InterfaceReset {
                let profile = Atr::parse(decoded.info, version)?;
                self.profile = Some(profile);
                self.tx_sequence = false;
                self.rx_sequence = false;
                return Ok(profile);
            }
        }
        Err(Error::Control)
    }

    /// Input/output are raw APDU bytes, not authenticated plaintext. A private
    /// caller MUST wrap/verify with mandatory C-MAC/C-ENC/R-MAC/R-ENC first.
    pub fn exchange_untrusted<'a>(
        &mut self,
        command: &[u8],
        output: &'a mut [u8],
    ) -> Result<UntrustedApdu<'a>, Error> {
        output.fill(0);
        if self.faulted {
            return Err(Error::Faulted);
        }
        if self.profile.is_none() {
            return Err(Error::Control);
        }
        if command.is_empty()
            || command.len() > MAX_APDU
            || output.len() < 2
            || output.len() > MAX_APDU
        {
            return Err(Error::Length);
        }
        match self.exchange_inner(command, output) {
            Ok(len) => Ok(UntrustedApdu(&output[..len])),
            Err(error) => {
                output.fill(0);
                self.latch();
                Err(error)
            }
        }
    }
    fn exchange_inner(&mut self, command: &[u8], output: &mut [u8]) -> Result<usize, Error> {
        let profile = self.profile.ok_or(Error::Control)?;
        let mut budget = self.begin()?;
        let count = command.len().div_ceil(profile.ifsc);
        for (index, chunk) in command.chunks(profile.ifsc).enumerate() {
            let more = index + 1 < count;
            let frame = WireFrame::encode(
                Direction::HostToSe,
                Kind::Information {
                    sequence: self.tx_sequence,
                    more,
                },
                chunk,
            )?;
            self.send(&frame, &budget)?;
            let next = !self.tx_sequence;
            if more {
                let ack = self.receive(&mut budget)?;
                let decoded = UntrustedFrame::decode(ack.bytes(), Direction::SeToHost)?;
                if decoded.kind
                    != (Kind::Receive {
                        next_sequence: next,
                        error: ReceiveError::None,
                    })
                {
                    return Err(Error::Sequence);
                }
            }
            self.tx_sequence = next;
        }
        let mut written = 0;
        loop {
            let frame = self.receive(&mut budget)?;
            let decoded = UntrustedFrame::decode(frame.bytes(), Direction::SeToHost)?;
            let Kind::Information { sequence, more } = decoded.kind else {
                return Err(Error::Control);
            };
            if sequence != self.rx_sequence {
                return Err(Error::Sequence);
            }
            let end = written + decoded.info.len();
            if end > output.len() {
                return Err(Error::Length);
            }
            output[written..end].copy_from_slice(decoded.info);
            written = end;
            self.rx_sequence = !self.rx_sequence;
            if !more {
                self.check(&budget)?;
                return if written >= 2 {
                    Ok(written)
                } else {
                    Err(Error::Length)
                };
            }
            let ack = WireFrame::encode(
                Direction::HostToSe,
                Kind::Receive {
                    next_sequence: self.rx_sequence,
                    error: ReceiveError::None,
                },
                &[],
            )?;
            self.send(&ack, &budget)?;
        }
    }
}
