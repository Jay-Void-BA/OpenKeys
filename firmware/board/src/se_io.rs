//! Fixed R04 SE050 I2C0x48. Only an address NACK is safely retryable;
//! data/unknown NACKs and bus faults can mean an indeterminate command.
use embedded_hal::i2c::{Error as _, ErrorKind, I2c, NoAcknowledgeSource};
use openkeys_se_transport::{
    MAX_FRAME,
    transport::{Io, IoError},
};

pub struct SeIo<I> {
    bus: I,
}
impl<I: I2c> SeIo<I> {
    pub fn new(bus: I) -> Self {
        Self { bus }
    }
}
fn classify(error: ErrorKind) -> IoError {
    match error {
        ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address) => IoError::AddressNotReady,
        _ => IoError::Fault,
    }
}
impl<I: I2c> Io for SeIo<I> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), IoError> {
        if bytes.is_empty() || bytes.len() > MAX_FRAME {
            return Err(IoError::Fault);
        }
        self.bus.write(0x48, bytes).map_err(|e| classify(e.kind()))
    }
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), IoError> {
        if bytes.is_empty() || bytes.len() > MAX_FRAME {
            return Err(IoError::Fault);
        }
        self.bus.read(0x48, bytes).map_err(|e| classify(e.kind()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_or_unknown_nacks_cannot_be_retried_as_address_busy() {
        assert_eq!(
            classify(ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)),
            IoError::AddressNotReady
        );
        for e in [
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Unknown),
            ErrorKind::Bus,
            ErrorKind::ArbitrationLoss,
            ErrorKind::Overrun,
            ErrorKind::Other,
        ] {
            assert_eq!(classify(e), IoError::Fault);
        }
    }
}
