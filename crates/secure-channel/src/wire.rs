//! Canonical management messages. No hardware PIN, OTP code/seed-export,
//! arbitrary SE command or host-supplied authorization response exists.
use crate::Error;

pub const MAX_BODY: usize = 16_896;
pub const REQUEST_HEADER: usize = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    Status = 1,
    Lock = 2,
    List = 3,
    ImportNote = 4,
    ImportRecovery = 5,
    Reveal = 6,
    Delete = 7,
    ImportOtp = 8,
    ShowOtpOnKey = 9,
    DeleteOtp = 10,
    SetTime = 11,
    ChangeHardwarePin = 12,
}
impl TryFrom<u8> for Command {
    type Error = Error;
    fn try_from(v: u8) -> Result<Self, Error> {
        Ok(match v {
            1 => Self::Status,
            2 => Self::Lock,
            3 => Self::List,
            4 => Self::ImportNote,
            5 => Self::ImportRecovery,
            6 => Self::Reveal,
            7 => Self::Delete,
            8 => Self::ImportOtp,
            9 => Self::ShowOtpOnKey,
            10 => Self::DeleteOtp,
            11 => Self::SetTime,
            12 => Self::ChangeHardwarePin,
            _ => return Err(Error::Format),
        })
    }
}
pub struct Request<'a> {
    pub command: Command,
    pub id: u32,
    pub revision: u64,
    pub body: &'a [u8],
}
impl<'a> Request<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() < REQUEST_HEADER || bytes.len() > REQUEST_HEADER + MAX_BODY {
            return Err(Error::Bounds);
        }
        if bytes[0] != 1 || bytes[2..4] != [0; 2] {
            return Err(Error::Format);
        }
        let command = Command::try_from(bytes[1])?;
        let id = u32::from_be_bytes(bytes[4..8].try_into().map_err(|_| Error::Format)?);
        let revision = u64::from_be_bytes(bytes[8..16].try_into().map_err(|_| Error::Format)?);
        let body = &bytes[16..];
        match command {
            Command::Status | Command::Lock | Command::ChangeHardwarePin
                if id == 0 && revision == 0 && body.is_empty() => {}
            Command::List
                if id == 0
                    && body.len() == 3
                    && matches!(body[0], 1..=4)
                    && (1..=256).contains(&u16::from_be_bytes([body[1], body[2]])) => {}
            Command::ImportNote | Command::ImportRecovery | Command::ImportOtp
                if id != 0 && !body.is_empty() => {}
            Command::Reveal | Command::Delete | Command::ShowOtpOnKey | Command::DeleteOtp
                if id != 0 && body.is_empty() => {}
            Command::SetTime if id == 0 && body.len() == 8 => {}
            _ => return Err(Error::Format),
        }
        Ok(Self {
            command,
            id,
            revision,
            body,
        })
    }
}
