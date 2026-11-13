//! Fixed, destructive SE request under a committed Root ErasePending epoch.
//! This is not whole-device wipe completion. DeleteAll may return an
//! unauthenticated response, so ordinary SCP authentication is NEVER relaxed.
//! Cold power-cut + newly authenticated inventory remain separate mandatory
//! observations; NOR, Root capsules, protocol/pairing state and recovery still
//! require a complete Root coordinator.
use crate::{
    Error, Exchange,
    inventory::{self, Baseline, Report},
    native_pin::{PowerCut, process_envelope},
};
use openkeys_scp03::{Command, MAX_DATA};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub const RESET_ID: u32 = 0x7fff_0205;

/// Public identity, not an authorization token. Must equal the committed
/// ErasePending state and independently authenticated paired SE identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Epoch {
    pub generation: u64,
    pub owner: [u8; 32],
    pub profile: [u8; 32],
    pub chip_identity: [u8; 32],
}
impl Epoch {
    fn valid(&self) -> bool {
        self.generation >= 2
            && self.owner != [0; 32]
            && self.profile != [0; 32]
            && self.chip_identity != [0; 32]
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Authentication,
    DeleteRequest,
    AfterDeleteRequest,
    PoweredOff,
    FreshInventory,
}

/// Trusted protected Root ONLY. No defaults, host phase flags or "wipe done"
/// input. Check exact committed epoch, verified boot/profile/device, revoked
/// grants, blank LCD, exclusive maintenance ownership and closed interfaces.
/// FreshInventory additionally proves a physical cold cut followed by a NEW
/// authenticated SCP03 channel to the same chip, not the old session/reply.
pub trait Root {
    fn check_pending(&mut self, epoch: &Epoch, stage: Stage) -> Result<(), Error>;
    /// A separately commissioned random16-byte reset-only UserID credential
    /// for RESET_ID, from protected storage. NOT the6-digit device PIN, protocol
    /// PIN, UID, factory/default constant or host packet. No adapter is supplied
    /// here. The exact C2 profile/ACL/credential type must be qualified first.
    fn reset_credential(&mut self, epoch: &Epoch) -> Result<Zeroizing<[u8; 16]>, Error>;
}

/// Transport description ONLY, never erasure evidence. An authenticated ACK
/// also cannot certify deletion. The important boundary is fresh inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    AuthenticatedEmptyResponse,
    Uncertain,
}
pub struct Pending {
    epoch: Epoch,
    delivery: Delivery,
}
impl Pending {
    pub const fn epoch(&self) -> &Epoch {
        &self.epoch
    }
    pub const fn delivery(&self) -> Delivery {
        self.delivery
    }

    /// Resume ONLY from independently authenticated ErasePending metadata,
    /// never missing/corrupt journals or a manager request. This does not
    /// resend DeleteAll and makes no claim that a previous request arrived.
    pub fn resume(epoch: Epoch, root: &mut impl Root) -> Result<Self, Error> {
        if !epoch.valid() {
            return Err(Error::Authorization);
        }
        root.check_pending(&epoch, Stage::PoweredOff)?;
        Ok(Self {
            epoch,
            delivery: Delivery::Uncertain,
        })
    }

    /// Observe baseline using a newly authenticated exclusive SCP03 owner.
    /// Result is still NOT whole-device Blank, Origin/ACL proof or a grant.
    pub fn observe<X: Exchange, P: PowerCut>(
        self,
        mut io: X,
        mut power: P,
        root: &mut impl Root,
        baseline: &Baseline<'_>,
    ) -> Result<Observation, Error> {
        struct Check<'a, R> {
            root: &'a mut R,
            epoch: Epoch,
        }
        impl<R: Root> inventory::Checkpoint for Check<'_, R> {
            fn check(&mut self) -> Result<(), Error> {
                self.root.check_pending(&self.epoch, Stage::FreshInventory)
            }
        }
        let result = (|| {
            if baseline.profile_digest() != &self.epoch.profile {
                return Err(Error::Authorization);
            }
            root.check_pending(&self.epoch, Stage::FreshInventory)?;
            let report = inventory::verify_baseline(
                &mut io,
                baseline,
                &mut Check {
                    root,
                    epoch: self.epoch,
                },
            )?;
            if report.profile_digest() != &self.epoch.profile {
                return Err(Error::Authorization);
            }
            root.check_pending(&self.epoch, Stage::FreshInventory)?;
            Ok(report)
        })();
        io.close();
        let cut = power.disable_se();
        let report = result?;
        cut?;
        root.check_pending(&self.epoch, Stage::PoweredOff)?;
        Ok(Observation {
            epoch: self.epoch,
            report,
        })
    }
}
pub struct Observation {
    epoch: Epoch,
    report: Report,
}
impl Observation {
    pub const fn epoch(&self) -> &Epoch {
        &self.epoch
    }
    pub const fn inventory(&self) -> &Report {
        &self.report
    }
}

pub struct Attempt<X: Exchange, P: PowerCut> {
    io: X,
    power: P,
    sid: Zeroizing<[u8; 8]>,
    epoch: Epoch,
    power_off: bool,
}
impl<X: Exchange, P: PowerCut> Attempt<X, P> {
    pub fn new(io: X, power: P, epoch: Epoch) -> Self {
        Self {
            io,
            power,
            sid: Zeroizing::new([0; 8]),
            epoch,
            power_off: false,
        }
    }
    fn cut(&mut self) -> Result<(), Error> {
        self.sid.fill(0);
        self.io.close();
        if !self.power_off {
            self.power.disable_se()?;
            self.power_off = true;
        }
        Ok(())
    }
    fn call(
        &mut self,
        root: &mut impl Root,
        instruction: u8,
        p2: u8,
        data: &[u8],
        out: &mut [u8],
    ) -> Result<usize, Error> {
        out.fill(0);
        root.check_pending(&self.epoch, Stage::Authentication)?;
        let n = self.io.exchange(
            &Command {
                class: 0x80,
                instruction,
                p1: 0,
                p2,
                data,
                le: None,
            },
            out,
        )?;
        root.check_pending(&self.epoch, Stage::Authentication)?;
        if n > out.len() {
            out.fill(0);
            return Err(Error::Schema);
        }
        Ok(n)
    }
    /// Exactly ONE destructive request. Any response/error after submission
    /// remains Pending, never a successful wipe. A missing/failed reset UserID
    /// does not fall back to a public key or another auth object. All paths
    /// close SCP and request physical SE disable; no APDU retry is performed.
    pub fn issue(mut self, root: &mut impl Root) -> Result<Pending, Error> {
        let result = (|| {
            if !self.epoch.valid() {
                return Err(Error::Authorization);
            }
            let mut out = Zeroizing::new([0; 32]);
            let id = [0x41, 4, 0x7f, 0xff, 2, 5];
            let n = self.call(root, 2, 0x26, &id, &mut out[..])?;
            if out[..n] != [0x41, 1, 0x0c, 0x42, 1, 1] {
                return Err(Error::Schema);
            }
            // ReadSize is invalid for UserID; never use it as qualification.
            let n = self.call(root, 4, 0x1b, &id, &mut out[..])?;
            if n != 10 || out[..2] != [0x41, 8] || bool::from(out[2..10].ct_eq(&[0; 8])) {
                return Err(Error::Schema);
            }
            self.sid.copy_from_slice(&out[2..10]);
            let secret = root.reset_credential(&self.epoch)?;
            let invalid = secret[..].ct_eq(&[0; 16]) | secret[..].ct_eq(&[0xff; 16]);
            if bool::from(invalid) {
                return Err(Error::Authorization);
            }
            let mut data = Zeroizing::new([0x41; 18]);
            data[1] = 16;
            data[2..].copy_from_slice(secret.as_ref());
            drop(secret);
            let mut wire = Zeroizing::new([0; MAX_DATA]);
            let end = process_envelope(
                &self.sid,
                &Command {
                    class: 0x80,
                    instruction: 4,
                    p1: 0,
                    p2: 0x2c,
                    data: data.as_ref(),
                    le: None,
                },
                &mut wire,
            )?;
            if self.call(root, 5, 0, &wire[..end], &mut out[..])? != 0 {
                return Err(Error::Schema);
            }
            data.fill(0);
            wire.fill(0);
            root.check_pending(&self.epoch, Stage::DeleteRequest)?;
            let end = process_envelope(
                &self.sid,
                &Command {
                    class: 0x80,
                    instruction: 4,
                    p1: 0,
                    p2: 0x2a,
                    data: &[],
                    le: None,
                },
                &mut wire,
            )?;
            out.fill(0);
            // Do NOT relax Exchange/SCP03 to accept bare9000 or retry on its
            // authentication failure. Either outcome requires cold re-observe.
            let delivery = match self.io.exchange(
                &Command {
                    class: 0x80,
                    instruction: 5,
                    p1: 0,
                    p2: 0,
                    data: &wire[..end],
                    le: None,
                },
                &mut out[..],
            ) {
                Ok(0) => Delivery::AuthenticatedEmptyResponse,
                _ => Delivery::Uncertain,
            };
            root.check_pending(&self.epoch, Stage::AfterDeleteRequest)?;
            Ok(delivery)
        })();
        let cut = self.cut();
        let delivery = result?;
        cut?;
        root.check_pending(&self.epoch, Stage::PoweredOff)?;
        Ok(Pending {
            epoch: self.epoch,
            delivery,
        })
    }
}
impl<X: Exchange, P: PowerCut> Drop for Attempt<X, P> {
    fn drop(&mut self) {
        let _ = self.cut();
    }
}

#[cfg(test)]
#[path = "reset_tests.rs"]
mod tests;
