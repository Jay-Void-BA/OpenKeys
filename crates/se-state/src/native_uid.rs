//! Default-off lab probe for native SE05x UserID authentication over Platform
//! SCP03. Wire facts: NXP 7102413e0457d3dfa4fc4be686ca8ec50bd9f5e2,
//! se05x_tlv.c::se05x_Transform and se05x_APDU_impl.h session commands.
//!
//! This is NOT ProtectedPin, Exchange, a profile verifier, an authorization
//! issuer or a private-key backend. It never creates/overwrites objects. A lab
//! must provision a SYNTHETIC UserID object, six ASCII digits, native maxAttempts
//! 11 and reviewed ACLs beforehand. Do not use an owner's authentication object.
//! Firmware retry/erase accounting, actual policy readback and Root integration
//! are still missing. A successful UserID comparison does not prove those ACLs.
use crate::{Error as TransportError, Exchange};
use device_policy::pin_only::PIN_DIGITS;
use openkeys_scp03::Command;
use zeroize::Zeroizing;

pub const LAB_AUTH_OBJECT: u32 = 0x4f4b_0800;
const _: () = assert!(PIN_DIGITS == 6);

/// Root-owned physical SE disable. This must cut the chip's actual supply/
/// enable, not just forget a software session or accept a host success flag.
/// GPIO/supervisor timing and power removal still require board measurement.
pub trait PowerCut {
    fn disable_se(&mut self) -> Result<(), Error>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Closed,
    RepeatedVerification,
    InvalidPin,
    Response,
    Transport(TransportError),
    PowerCut,
}

#[must_use = "Explicitly close the lab session and check power removal"]
pub struct Probe<X: Exchange, P: PowerCut> {
    io: X,
    power: P,
    sid: Zeroizing<[u8; 8]>,
    verified: bool,
    closed: bool,
    power_off: bool,
    fault: Option<Error>,
}
impl<X: Exchange, P: PowerCut> Probe<X, P> {
    /// No object creation or factory/default PIN. The fixed lab namespace
    /// prevents selecting reserved platform-SCP administration identities.
    pub fn begin(io: X, power: P) -> Result<Self, Error> {
        let mut probe = Self {
            io,
            power,
            sid: Zeroizing::new([0; 8]),
            verified: false,
            closed: false,
            power_off: false,
            fault: None,
        };
        let mut data = [0x41, 4, 0, 0, 0, 0];
        data[2..].copy_from_slice(&LAB_AUTH_OBJECT.to_be_bytes());
        let mut response = Zeroizing::new([0; 16]);
        let result = probe.io.exchange(
            &Command {
                class: 0x80,
                instruction: 4,
                p1: 0,
                p2: 0x1b,
                data: &data,
                // SDK ordinary commands use no outer Le; body still returned.
                le: None,
            },
            &mut *response,
        );
        match result {
            Ok(10) if response[..2] == [0x41, 8] && response[2..10] != [0; 8] => {
                probe.sid.copy_from_slice(&response[2..10]);
                Ok(probe)
            }
            Ok(_) => Err(probe.fail(Error::Response)),
            Err(error) => Err(probe.fail(Error::Transport(error))),
        }
    }
    fn shutdown(&mut self) -> Result<(), Error> {
        self.closed = true;
        self.verified = false;
        self.sid.fill(0);
        self.io.close();
        if !self.power_off {
            self.power.disable_se().map_err(|_| Error::PowerCut)?;
            self.power_off = true;
        }
        Ok(())
    }
    fn fail(&mut self, error: Error) -> Error {
        let error = match self.shutdown() {
            Ok(()) => error,
            Err(fault) => fault,
        };
        self.fault = Some(error);
        error
    }
    /// Only two fixed management commands, never arbitrary APDU forwarding.
    /// The outer PROCESS TLV is itself encrypted/authenticated by sealed X.
    fn management(&mut self, p2: u8, payload: &[u8]) -> Result<(), Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        // The only payload is TLV[1] six ASCII digits (8 bytes). Lc is short,
        // and the CloseSession inner command has neither Lc nor Le.
        if !matches!((p2, payload.len()), (0x2c, 8) | (0x1c, 0)) {
            return Err(self.fail(Error::InvalidPin));
        }
        let mut wire = Zeroizing::new([0; 25]);
        wire[..2].copy_from_slice(&[0x10, 8]);
        wire[2..10].copy_from_slice(&*self.sid);
        wire[10] = 0x41;
        wire[11] = if payload.is_empty() { 4 } else { 13 };
        wire[12..16].copy_from_slice(&[0x80, 4, 0, p2]);
        let end = if payload.is_empty() {
            16
        } else {
            wire[16] = 8;
            wire[17..].copy_from_slice(payload);
            25
        };
        let mut response = Zeroizing::new([0; 16]);
        match self.io.exchange(
            &Command {
                class: 0x80,
                instruction: 5,
                p1: 0,
                p2: 0,
                data: &wire[..end],
                le: None,
            },
            &mut *response,
        ) {
            Ok(0) => Ok(()),
            Ok(_) => Err(self.fail(Error::Response)),
            Err(error) => Err(self.fail(Error::Transport(error))),
        }
    }
    /// Numeric digits (0..9), encoded as six ASCII bytes. The caller must source
    /// them from local GPIO: this slice alone cannot prove where they came from.
    /// The return value is LAB STATUS, not a Gate capability.
    /// One submission per probe, no retry of a failed/ambiguous verification.
    pub fn verify_local_pin(&mut self, digits: &[u8; PIN_DIGITS]) -> Result<(), Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        if self.verified {
            return Err(self.fail(Error::RepeatedVerification));
        }
        if digits.iter().any(|&d| d > 9) {
            return Err(self.fail(Error::InvalidPin));
        }
        let mut payload = Zeroizing::new([0; 8]);
        payload[..2].copy_from_slice(&[0x41, PIN_DIGITS as u8]);
        for (dst, digit) in payload[2..].iter_mut().zip(digits) {
            *dst = b'0' + digit;
        }
        self.management(0x2c, &*payload)?;
        self.verified = true;
        Ok(())
    }
    /// Even an acknowledged CloseSession is followed by physical disable.
    /// Failure remains failure; no ACK/status can assert that power was cut.
    pub fn close(mut self) -> Result<(), Error> {
        let result = if self.closed {
            self.fault.map_or(Ok(()), Err)
        } else {
            self.management(0x1c, &[])
        };
        let powered_down = self.shutdown();
        powered_down.and(result)
    }
}
impl<X: Exchange, P: PowerCut> Drop for Probe<X, P> {
    fn drop(&mut self) {
        // No blocking I2C or ambiguous APDU retry from Drop. Physical disable
        // is idempotent and may be tried again after a GPIO-reported failure.
        let _ = self.shutdown();
    }
}

#[cfg(test)]
#[path = "native_uid_tests.rs"]
mod tests;
