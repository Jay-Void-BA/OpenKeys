//! R05 native-PIN namespace installation over authenticated platform SCP03.
//! This performs real fixed APDUs, not factory-key selection or a whole-device
//! commissioner. Root must supply qualified closed boot, exact C2 profile,
//! an independently trusted blank lifecycle and a durable installation journal.
//! There is NO Root/journal adapter or default production credential here.
use crate::{
    Error, Exchange,
    inventory::{self, Baseline},
    native_pin::{self, NativePin, PowerCut},
};
use device_policy::{
    Operation, Request,
    pin::ProtectedPin,
    pin_setup::{Error as SetupError, ProtectedCommissioning},
};
use openkeys_scp03::Command;
use zeroize::Zeroizing;

const READ: u32 = 0x0020_0000;
const WRITE: u32 = 0x0010_0000;

/// Protected Root, never a host-reported factory-blank flag. The journal must
/// survive SE DeleteAll, loss of backup power, reset and firmware update.
pub trait Root {
    /// Closed verified boot, exact device/profile identity, exclusive owner,
    /// trusted maintenance generation, hardware/erase state and interfaces shut.
    fn check(&mut self) -> Result<(), Error>;
    /// An authenticated explicit Blank state after qualified manufacturing or
    /// complete SE+NOR+Root destruction. Missing state is not Blank.
    fn qualified_blank(&mut self) -> Result<(), Error>;
    /// Persist+readback Installing BEFORE the first SE mutation. Ambiguous ACK
    /// must return Err and remain nonblank; no reset or caller retry refunds it.
    fn begin_pin_install(&mut self, profile: &[u8; 32], inventory: &[u8; 32]) -> Result<(), Error>;
    /// Commit only this verified PIN-namespace milestone, not whole-device
    /// Ready. Other roots/apps/pairing must finish before the global Ready seal.
    fn commit_pin_namespace(&mut self) -> Result<(), Error>;
}
struct Check<'a, R>(&'a mut R);
impl<R: Root> inventory::Checkpoint for Check<'_, R> {
    fn check(&mut self) -> Result<(), Error> {
        self.0.check()
    }
}

pub struct Installer<'a, X: Exchange, P: PowerCut, R: Root> {
    io: Option<X>,
    power: Option<P>,
    root: R,
    baseline: Baseline<'a>,
    closed: bool,
}
impl<'a, X: Exchange, P: PowerCut, R: Root> Installer<'a, X, P, R> {
    pub fn new(io: X, power: P, root: R, baseline: Baseline<'a>) -> Self {
        Self {
            io: Some(io),
            power: Some(power),
            root,
            baseline,
            closed: false,
        }
    }
    fn close(&mut self) {
        self.closed = true;
        if let Some(io) = &mut self.io {
            io.close();
        }
        if let Some(power) = &mut self.power {
            let _ = power.disable_se();
        }
    }
    fn pristine(&mut self) -> Result<inventory::Report, Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        self.root.check()?;
        self.root.qualified_blank()?;
        inventory::verify_baseline(
            self.io.as_mut().ok_or(Error::Closed)?,
            &self.baseline,
            &mut Check(&mut self.root),
        )
    }
    fn write(&mut self, instruction: u8, p1: u8, data: &[u8]) -> Result<(), Error> {
        self.root.check()?;
        let mut out = Zeroizing::new([0; 16]);
        let n = self.io.as_mut().ok_or(Error::Closed)?.exchange(
            &Command {
                class: 0x80,
                instruction,
                p1,
                p2: 0,
                data,
                le: None,
            },
            &mut out[..],
        )?;
        self.root.check()?;
        if n != 0 {
            return Err(Error::Schema);
        }
        Ok(())
    }
    fn counter(&mut self, id: u32, policy: &[u8]) -> Result<(), Error> {
        let mut bytes = [0; 32];
        let at = policy.len();
        if at > 20 {
            return Err(Error::Schema);
        }
        bytes[..at].copy_from_slice(policy);
        bytes[at..at + 2].copy_from_slice(&[0x41, 4]);
        bytes[at + 2..at + 6].copy_from_slice(&id.to_be_bytes());
        bytes[at + 6..at + 10].copy_from_slice(&[0x42, 2, 0, 8]);
        self.write(1, 8, &bytes[..at + 10])?;
        // An ACK is insufficient: a newly created counter must be exactly zero.
        let mut out = Zeroizing::new([0; 16]);
        self.root.check()?;
        let n = self.io.as_mut().ok_or(Error::Closed)?.exchange(
            &Command {
                class: 0x80,
                instruction: 2,
                p1: 0,
                p2: 0,
                data: &bytes[at..at + 6],
                le: None,
            },
            &mut out[..],
        )?;
        self.root.check()?;
        if n != 10 || out[..10] != [0x41, 8, 0, 0, 0, 0, 0, 0, 0, 0] {
            return Err(Error::Counter);
        }
        Ok(())
    }
    fn install(&mut self, digits: &[u8; 6]) -> Result<(), Error> {
        if digits.iter().any(|&d| d > 9) {
            return Err(Error::Schema);
        }
        let inventory = self.pristine()?;
        self.root
            .begin_pin_install(inventory.profile_digest(), inventory.inventory_digest())?;
        self.root.check()?;

        let mut data = Zeroizing::new([0; 96]);
        let policy = default_policy(native_pin::FORBID_ALL);
        data[..11].copy_from_slice(&policy);
        data[11..15].copy_from_slice(&[0x12, 2, 0, 11]);
        data[15..17].copy_from_slice(&[0x41, 4]);
        data[17..21].copy_from_slice(&native_pin::PIN_ID.to_be_bytes());
        data[21..23].copy_from_slice(&[0x42, 6]);
        for (target, digit) in data[23..29].iter_mut().zip(digits) {
            *target = b'0' + digit;
        }
        // INS_AUTH_OBJECT is mandatory; maxAttempts=11 is never omitted/zero.
        self.write(0x41, 7, &data[..29])?;
        data.fill(0);
        self.counter(native_pin::SPENT_ID, &default_policy(READ | WRITE))?;
        let mut credit = native_pin::owner_policy(READ | WRITE);
        // Platform can READ accounting but may not WRITE credit. Only the
        // successfully authenticated PIN owner can credit a precharged guess.
        credit[16..20].copy_from_slice(&(READ | native_pin::REQUIRE_SM).to_be_bytes());
        self.counter(native_pin::CREDIT_ID, &credit)?;
        self.counter(native_pin::ERASE_ID, &default_policy(READ | WRITE))?;
        let policy = default_policy(READ);
        data[..11].copy_from_slice(&policy);
        data[11..13].copy_from_slice(&[0x41, 4]);
        data[13..17].copy_from_slice(&native_pin::MARKER_ID.to_be_bytes());
        data[17..25].copy_from_slice(&[0x42, 2, 0, 0, 0x43, 2, 0, 32]);
        data[25..27].copy_from_slice(&[0x44, 32]);
        data[27..59].copy_from_slice(&native_pin::marker());
        self.write(1, 6, &data[..59])?; // immutable marker LAST
        data.fill(0);

        let mut pin = NativePin::open(
            self.io.take().ok_or(Error::Closed)?,
            self.power.take().ok_or(Error::Closed)?,
        )?;
        let request = Request {
            operation: Operation::UnlockSession,
            digest: native_pin::marker(),
            channel: *inventory.profile_digest(),
        };
        pin.reserve_attempt(&request)
            .map_err(|_| Error::Authorization)?;
        pin.verify(&request, digits)
            .map_err(|_| Error::Authorization)?;
        pin.revoke()?;
        drop(pin);
        self.root.check()?;
        self.root.commit_pin_namespace()?;
        self.root.check()?;
        self.closed = true;
        Ok(())
    }
}
fn default_policy(rights: u32) -> [u8; 11] {
    let mut p = [0x11, 9, 8, 0, 0, 0, 0, 0, 0, 0, 0];
    p[7..].copy_from_slice(&(rights | native_pin::REQUIRE_SM).to_be_bytes());
    p
}
impl<X: Exchange, P: PowerCut, R: Root> ProtectedCommissioning for Installer<'_, X, P, R> {
    fn qualified_factory_blank(&mut self) -> Result<(), SetupError> {
        if self.pristine().is_err() {
            self.close();
            return Err(SetupError::NotBlank);
        }
        Ok(())
    }
    fn install_owner_pin(&mut self, pin: &[u8; 6]) -> Result<(), SetupError> {
        if self.install(pin).is_err() {
            self.close();
            return Err(SetupError::Persistence);
        }
        Ok(())
    }
}
impl<X: Exchange, P: PowerCut, R: Root> Drop for Installer<'_, X, P, R> {
    fn drop(&mut self) {
        self.close();
    }
}
