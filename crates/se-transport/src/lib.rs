#![no_std]
#![forbid(unsafe_code)]
//! Strict normal-length SE05x T=1-over-I2C framing. Reference: NXP UM11225
//! and official nano-package26d55b43225bc84de364057660cba676fa69f6e4.
//! CRC/sequence handling provides transport integrity, NOT authentication.
//! Only a separately authenticated SCP03 layer may release private responses.

pub const MAX_INF: usize = 254;
pub const MAX_FRAME: usize = MAX_INF + 5;
pub mod atr;
pub mod transport;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Length,
    Address,
    Crc,
    Control,
    Sequence,
    Timeout,
    Faulted,
    Io,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    HostToSe,
    SeToHost,
}
impl Direction {
    const fn address(self) -> u8 {
        match self {
            Self::HostToSe => 0x5a,
            Self::SeToHost => 0xa5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiveError {
    None,
    Crc,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Resync,
    Ifs,
    Abort,
    Wtx,
    EndSession,
    ChipReset,
    Atr,
    InterfaceReset,
}
impl Control {
    const fn code(self) -> u8 {
        match self {
            Self::Resync => 0,
            Self::Ifs => 1,
            Self::Abort => 2,
            Self::Wtx => 3,
            Self::EndSession => 5,
            Self::ChipReset => 6,
            Self::Atr => 7,
            Self::InterfaceReset => 15,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Information {
        sequence: bool,
        more: bool,
    },
    Receive {
        next_sequence: bool,
        error: ReceiveError,
    },
    Supervisory {
        response: bool,
        control: Control,
    },
}
impl Kind {
    const fn byte(self) -> u8 {
        match self {
            Self::Information { sequence, more } => ((sequence as u8) << 6) | ((more as u8) << 5),
            Self::Receive {
                next_sequence,
                error,
            } => {
                0x80 | ((next_sequence as u8) << 4)
                    | match error {
                        ReceiveError::None => 0,
                        ReceiveError::Crc => 1,
                        ReceiveError::Other => 2,
                    }
            }
            Self::Supervisory { response, control } => {
                0xc0 | ((response as u8) << 5) | control.code()
            }
        }
    }
    fn parse(byte: u8) -> Result<Self, Error> {
        if byte & 0x9f == 0 {
            return Ok(Self::Information {
                sequence: byte & 0x40 != 0,
                more: byte & 0x20 != 0,
            });
        }
        if byte & 0xec == 0x80 && byte & 3 < 3 {
            return Ok(Self::Receive {
                next_sequence: byte & 0x10 != 0,
                error: match byte & 3 {
                    0 => ReceiveError::None,
                    1 => ReceiveError::Crc,
                    _ => ReceiveError::Other,
                },
            });
        }
        if byte & 0xd0 != 0xc0 {
            return Err(Error::Control);
        }
        let control = match byte & 0x0f {
            0 => Control::Resync,
            1 => Control::Ifs,
            2 => Control::Abort,
            3 => Control::Wtx,
            5 => Control::EndSession,
            6 => Control::ChipReset,
            7 => Control::Atr,
            15 => Control::InterfaceReset,
            _ => return Err(Error::Control),
        };
        Ok(Self::Supervisory {
            response: byte & 0x20 != 0,
            control,
        })
    }
    fn validate(self, info: &[u8]) -> Result<(), Error> {
        match self {
            Self::Information { .. } if info.is_empty() => Err(Error::Length),
            Self::Receive { .. } if !info.is_empty() => Err(Error::Length),
            Self::Supervisory {
                control: Control::Ifs,
                ..
            } if info.len() != 1 || info[0] == 0 || usize::from(info[0]) > MAX_INF => {
                Err(Error::Control)
            }
            Self::Supervisory {
                control: Control::Wtx,
                ..
            } if info.len() != 1 || !(1..=59).contains(&info[0]) => Err(Error::Control),
            Self::Supervisory {
                control: Control::Resync | Control::Abort | Control::EndSession,
                ..
            } if !info.is_empty() => Err(Error::Length),
            Self::Supervisory {
                response: false,
                control: Control::Atr | Control::ChipReset | Control::InterfaceReset,
            } if !info.is_empty() => Err(Error::Length),
            _ => Ok(()),
        }
    }
}

/// CRC-16/IBM-SDLC (X25), reflected0x1021,init/xorout0xffff, wire LE.
/// This checksum is public error detection and must not be treated as a MAC.
pub fn crc(bytes: &[u8]) -> u16 {
    let mut value = 0xffff;
    for byte in bytes {
        value ^= u16::from(*byte);
        for _ in 0..8 {
            value = (value >> 1) ^ if value & 1 == 1 { 0x8408 } else { 0 };
        }
    }
    value ^ 0xffff
}

/// Bounded outgoing transport bytes; no Debug implementation for payloads.
pub struct WireFrame {
    bytes: [u8; MAX_FRAME],
    len: usize,
}
impl WireFrame {
    pub fn encode(direction: Direction, kind: Kind, info: &[u8]) -> Result<Self, Error> {
        if info.len() > MAX_INF {
            return Err(Error::Length);
        }
        kind.validate(info)?;
        let mut frame = Self {
            bytes: [0; MAX_FRAME],
            len: info.len() + 5,
        };
        frame.bytes[..3].copy_from_slice(&[direction.address(), kind.byte(), info.len() as u8]);
        frame.bytes[3..3 + info.len()].copy_from_slice(info);
        let checksum = crc(&frame.bytes[..3 + info.len()]).to_le_bytes();
        frame.bytes[3 + info.len()..frame.len].copy_from_slice(&checksum);
        Ok(frame)
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Still untrusted after a valid CRC; only secure-channel verification can
/// create an authenticated response. No key object or authorization is here.
pub struct UntrustedFrame<'a> {
    pub kind: Kind,
    pub info: &'a [u8],
}
impl<'a> UntrustedFrame<'a> {
    pub fn decode(bytes: &'a [u8], direction: Direction) -> Result<Self, Error> {
        if bytes.len() < 5 || bytes.len() > MAX_FRAME {
            return Err(Error::Length);
        }
        let len = usize::from(bytes[2]);
        if len > MAX_INF || len + 5 != bytes.len() {
            return Err(Error::Length);
        }
        if bytes[0] != direction.address() {
            return Err(Error::Address);
        }
        if crc(&bytes[..len + 3]).to_le_bytes() != bytes[len + 3..] {
            return Err(Error::Crc);
        }
        let kind = Kind::parse(bytes[1])?;
        let info = &bytes[3..len + 3];
        kind.validate(info)?;
        Ok(Self { kind, info })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_known_answer_and_normal_response() {
        assert_eq!(crc(b"123456789"), 0x906e);
        let frame = WireFrame::encode(
            Direction::SeToHost,
            Kind::Information {
                sequence: false,
                more: false,
            },
            &[0x90, 0],
        )
        .unwrap();
        let decoded = UntrustedFrame::decode(frame.bytes(), Direction::SeToHost).unwrap();
        assert_eq!(decoded.info, [0x90, 0]);
        assert_eq!(
            decoded.kind,
            Kind::Information {
                sequence: false,
                more: false
            }
        );
        assert!(matches!(
            UntrustedFrame::decode(frame.bytes(), Direction::HostToSe),
            Err(Error::Address)
        ));
    }
    #[test]
    fn every_one_bit_corruption_is_rejected() {
        let frame = WireFrame::encode(
            Direction::SeToHost,
            Kind::Information {
                sequence: true,
                more: false,
            },
            &[0x55; MAX_INF],
        )
        .unwrap();
        for index in 0..frame.len {
            for bit in 0..8 {
                let mut altered = frame.bytes;
                altered[index] ^= 1 << bit;
                assert!(
                    UntrustedFrame::decode(&altered[..frame.len], Direction::SeToHost).is_err()
                );
            }
        }
    }
    #[test]
    fn noncanonical_control_lengths_and_extended_length_are_rejected() {
        assert!(
            WireFrame::encode(
                Direction::SeToHost,
                Kind::Receive {
                    next_sequence: false,
                    error: ReceiveError::None
                },
                &[0]
            )
            .is_err()
        );
        for value in [0, 60, 255] {
            assert!(
                WireFrame::encode(
                    Direction::SeToHost,
                    Kind::Supervisory {
                        response: false,
                        control: Control::Wtx
                    },
                    &[value]
                )
                .is_err()
            );
        }
        for len in 0..=MAX_FRAME {
            let frame = [0u8; MAX_FRAME];
            let _ = UntrustedFrame::decode(&frame[..len], Direction::SeToHost);
        }
        assert!(
            WireFrame::encode(
                Direction::HostToSe,
                Kind::Information {
                    sequence: false,
                    more: false
                },
                &[0; 255]
            )
            .is_err()
        );
    }
    #[test]
    fn all_accepted_pcbs_round_trip_canonically() {
        for byte in 0..=u8::MAX {
            if let Ok(kind) = Kind::parse(byte) {
                assert_eq!(kind.byte(), byte);
            }
        }
    }
}
