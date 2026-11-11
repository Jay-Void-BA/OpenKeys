//! R05 native UserID PIN backend with durable precharge and SE-owned credit.
//! Unlike the lab probe, implements ProtectedPin and the sealed private I/O
//! boundary. It opens ONLY an explicitly commissioned v3 namespace; it cannot
//! create/reset a PIN, restore factory defaults or clear an erase request.
//!
//! Chip ACLs must make PIN_ID immutable (including deletion), bind all private
//! objects and CREDIT writes to PIN_ID, and mandate full platform SCP03. Marker
//! installation must be last and read-only. These are commissioning facts,
//! not conclusions drawn from ReadType. Physical E2 qualification is required.
use crate::{Error, Exchange, PinBoundExchange, private};
use device_policy::{
    Request,
    pin::{Error as PinError, ProtectedPin},
};
use openkeys_scp03::{Command, MAX_DATA};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub use crate::pin_only::{CREDIT_ID, ERASE_ID, MARKER_ID, MAX_FAILURES, SPENT_ID};
pub const PIN_ID: u32 = 0x4f4b_0801;
pub const REQUIRE_SM: u32 = 0x0002_0000;
pub const FORBID_ALL: u32 = 0x2000_0000;

/// Two explicit policies: specific authenticated PIN owner, deny every other
/// user. This is independent of a software unlocked Boolean or SE_EN signal.
pub fn owner_policy(rights: u32) -> [u8; 20] {
    let mut out = [0; 20];
    out[..3].copy_from_slice(&[0x11, 18, 8]);
    out[3..7].copy_from_slice(&PIN_ID.to_be_bytes());
    out[7..11].copy_from_slice(&(rights | REQUIRE_SM).to_be_bytes());
    out[11] = 8;
    out[16..20].copy_from_slice(&FORBID_ALL.to_be_bytes());
    out
}
pub fn marker() -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"OpenKeys/R05/native-PIN-state/v3");
    h.update([6]);
    h.update(MAX_FAILURES.to_be_bytes());
    for id in [PIN_ID, SPENT_ID, CREDIT_ID, ERASE_ID, MARKER_ID] {
        h.update(id.to_be_bytes());
    }
    h.update(owner_policy(0x1024_0000)); // signing + public read + delete
    h.update(b"immutable-userid/credit-owner-only/platform-SCP03-33/marker-last");
    h.finalize().into()
}
pub trait PowerCut {
    /// Root-only physical SE disable, synchronous GPIO/supervisor action.
    /// Never an OS/host ACK; confirm actual VDD/EN decay on the board.
    fn disable_se(&mut self) -> Result<(), Error>;
}
struct Pending {
    request: Request,
    spent: u64,
    credit: u64,
}
pub struct NativePin<X: Exchange, P: PowerCut> {
    io: X,
    power: P,
    sid: Zeroizing<[u8; 8]>,
    verified: bool,
    pending: Option<Pending>,
    spent: u64,
    credit: u64,
    closed: bool,
    power_off: bool,
}
impl<X: Exchange, P: PowerCut> NativePin<X, P> {
    pub fn open(io: X, power: P) -> Result<Self, Error> {
        let mut this = Self {
            io,
            power,
            sid: Zeroizing::new([0; 8]),
            verified: false,
            pending: None,
            spent: 0,
            credit: 0,
            closed: false,
            power_off: false,
        };
        let result = (|| {
            this.schema(PIN_ID, 0x0c, None)?; // ReadSize is invalid for a UserID.
            for id in [SPENT_ID, CREDIT_ID, ERASE_ID] {
                this.schema(id, 0x0d, Some(8))?;
            }
            this.schema(MARKER_ID, 0x0b, Some(32))?;
            let mut data = [0; 14];
            data[..6].copy_from_slice(&id_tlv(MARKER_ID));
            data[6..].copy_from_slice(&[0x42, 2, 0, 0, 0x43, 2, 0, 32]);
            let mut out = Zeroizing::new([0; 40]);
            let n = this.raw(2, 0, 0, &data, &mut out[..])?;
            if n != 34 || out[..2] != [0x41, 32] || !bool::from(out[2..34].ct_eq(&marker())) {
                return Err(Error::Schema);
            }
            if this.counter(ERASE_ID)? != 0 {
                return Err(Error::EraseRequired);
            }
            let (spent, credit) = this.counters()?;
            if spent - credit == MAX_FAILURES {
                this.latch_erase()?;
                return Err(Error::EraseRequired);
            }
            Ok(())
        })();
        if let Err(error) = result {
            this.close();
            return Err(error);
        }
        Ok(this)
    }
    pub fn close(&mut self) {
        self.closed = true;
        self.verified = false;
        self.pending = None;
        self.sid.fill(0);
        self.io.close();
        if !self.power_off && self.power.disable_se().is_ok() {
            self.power_off = true;
        }
    }
    /// Explicit revocation reports a failed power-cut request. Drop remains
    /// nonblocking and retries only the idempotent physical disable, no APDU.
    pub fn revoke(&mut self) -> Result<(), Error> {
        self.close();
        if self.power_off {
            Ok(())
        } else {
            Err(Error::Channel)
        }
    }
    /// Consume exact ChangePin authority and persist the irreversible erase
    /// intent BEFORE accepting a replacement PIN. This is not deletion or a
    /// factory-blank result. Root must lock every session and run/recover the
    /// separate ALL-DATA wipe; there is deliberately no install-new-PIN API.
    pub fn begin_pin_change(
        &mut self,
        authorization: device_policy::Authorization,
        state: &mut impl device_policy::execution::State,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(), Error> {
        let result = (|| {
            let request = *authorization.request();
            if !self.verified || request.operation != device_policy::Operation::ChangePin {
                return Err(Error::Authorization);
            }
            let mut scope = authorization
                .activate(&request, state, &mut clock)
                .map_err(|_| Error::Authorization)?;
            scope
                .check(&request, state, &mut clock)
                .map_err(|_| Error::Authorization)?;
            self.latch_erase()?;
            scope
                .check(&request, state, clock)
                .map_err(|_| Error::Authorization)
        })();
        self.close();
        result
    }
    fn raw(
        &mut self,
        ins: u8,
        p1: u8,
        p2: u8,
        data: &[u8],
        out: &mut [u8],
    ) -> Result<usize, Error> {
        out.fill(0);
        if self.closed {
            return Err(Error::Closed);
        }
        let result = self.io.exchange(
            &Command {
                class: 0x80,
                instruction: ins,
                p1,
                p2,
                data,
                le: None,
            },
            out,
        );
        match result {
            Ok(n) if n <= out.len() => Ok(n),
            result => {
                out.fill(0);
                self.close();
                Err(result.err().unwrap_or(Error::Schema))
            }
        }
    }
    fn schema(&mut self, id: u32, kind: u8, size: Option<u16>) -> Result<(), Error> {
        let mut out = [0; 16];
        let data = id_tlv(id);
        let n = self.raw(2, 0, 0x26, &data, &mut out)?;
        if out[..n] != [0x41, 1, kind, 0x42, 1, 1] {
            return Err(Error::Schema);
        }
        if let Some(size) = size {
            let n = self.raw(2, 0, 7, &data, &mut out)?;
            if out[..n] != [0x41, 2, (size >> 8) as u8, size as u8] {
                return Err(Error::Schema);
            }
        }
        Ok(())
    }
    fn counter(&mut self, id: u32) -> Result<u64, Error> {
        let mut out = [0; 16];
        let n = self.raw(2, 0, 0, &id_tlv(id), &mut out)?;
        if n != 10 || out[..2] != [0x41, 8] {
            return Err(Error::Schema);
        }
        Ok(u64::from_be_bytes(
            out[2..10].try_into().map_err(|_| Error::Schema)?,
        ))
    }
    fn counters(&mut self) -> Result<(u64, u64), Error> {
        let spent = self.counter(SPENT_ID)?;
        let credit = self.counter(CREDIT_ID)?;
        if spent < self.spent
            || credit < self.credit
            || credit > spent
            || spent - credit > MAX_FAILURES
        {
            return Err(Error::Counter);
        }
        self.spent = spent;
        self.credit = credit;
        Ok((spent, credit))
    }
    fn latch_erase(&mut self) -> Result<(), Error> {
        if self.counter(ERASE_ID)? != 0 {
            return Err(Error::EraseRequired);
        }
        let mut out = [0; 4];
        if self.raw(1, 8, 0, &id_tlv(ERASE_ID), &mut out)? != 0 || self.counter(ERASE_ID)? != 1 {
            return Err(Error::Counter);
        }
        Ok(())
    }
    fn process(&mut self, command: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        out.fill(0);
        if self.closed || *self.sid == [0; 8] {
            return Err(Error::Closed);
        }
        let mut wire = Zeroizing::new([0; MAX_DATA]);
        let end = match process_envelope(&self.sid, command, &mut wire) {
            Ok(end) => end,
            Err(error) => {
                self.close();
                return Err(error);
            }
        };
        let length = self.raw(5, 0, 0, &wire[..end], out)?;
        if command.le.is_some_and(|le| length > le.maximum()) {
            out.fill(0);
            self.close();
            return Err(Error::Schema);
        }
        Ok(length)
    }
    fn reserve(&mut self, request: &Request) -> Result<(), Error> {
        self.pending = None;
        if self.verified {
            let mut out = [0; 4];
            if self.process(
                &Command {
                    class: 0x80,
                    instruction: 4,
                    p1: 0,
                    p2: 0x1c,
                    data: &[],
                    le: None,
                },
                &mut out,
            )? != 0
            {
                return Err(Error::Schema);
            }
            self.verified = false;
            self.sid.fill(0);
        }
        if self.counter(ERASE_ID)? != 0 {
            return Err(Error::EraseRequired);
        }
        let (spent, credit) = self.counters()?;
        if spent - credit >= MAX_FAILURES {
            self.latch_erase()?;
            return Err(Error::EraseRequired);
        }
        let next = spent.checked_add(1).ok_or(Error::Exhausted)?;
        let mut out = [0; 4];
        if self.raw(1, 8, 0, &id_tlv(SPENT_ID), &mut out)? != 0
            || self.counters()? != (next, credit)
        {
            return Err(Error::Counter);
        }
        self.pending = Some(Pending {
            request: *request,
            spent: next,
            credit,
        });
        Ok(())
    }
    fn verify_native(&mut self, request: &Request, digits: &[u8; 6]) -> Result<(), Error> {
        let pending = self.pending.take().ok_or(Error::Counter)?;
        if pending.request != *request
            || digits.iter().any(|&d| d > 9)
            || self.counters()? != (pending.spent, pending.credit)
        {
            return Err(Error::Authorization);
        }
        let mut out = Zeroizing::new([0; 16]);
        let n = self.raw(4, 0, 0x1b, &id_tlv(PIN_ID), &mut out[..])?;
        if n != 10 || out[..2] != [0x41, 8] || out[2..10] == [0; 8] {
            return Err(Error::Schema);
        }
        self.sid.copy_from_slice(&out[2..10]);
        let mut data = Zeroizing::new([0x41, 6, 0, 0, 0, 0, 0, 0]);
        for (target, digit) in data[2..].iter_mut().zip(digits) {
            *target = b'0' + digit;
        }
        if self.process(
            &Command {
                class: 0x80,
                instruction: 4,
                p1: 0,
                p2: 0x2c,
                data: &*data,
                le: None,
            },
            &mut out[..],
        )? != 0
        {
            return Err(Error::Schema);
        }
        // Credit is writable ONLY inside the successfully authenticated native
        // PIN session; platform SCP alone cannot reset the retry budget.
        self.verified = true;
        let mut data = [0; 16];
        data[..6].copy_from_slice(&id_tlv(CREDIT_ID));
        data[6..8].copy_from_slice(&[0x43, 8]);
        data[8..].copy_from_slice(&pending.spent.to_be_bytes());
        if self.process(
            &Command {
                class: 0x80,
                instruction: 1,
                p1: 8,
                p2: 0,
                data: &data,
                le: None,
            },
            &mut out[..],
        )? != 0
            || self.counters()? != (pending.spent, pending.spent)
        {
            return Err(Error::Counter);
        }
        Ok(())
    }
}

/// NXP se05x_Transform/ProcessSessionCmd encoding, inside authenticated SCP03.
/// The embedded APDU has Lc but no Le; an extended response hint selects the
/// SDK's extended-Lc form. Canonical BER 0x81/0x82 lengths include the complete
/// inner header. No arbitrary host APDU entry point is exposed.
fn process_envelope(
    sid: &[u8; 8],
    command: &Command<'_>,
    wire: &mut [u8; MAX_DATA],
) -> Result<usize, Error> {
    wire.fill(0);
    if command.class != 0x80 || *sid == [0; 8] || command.data.len() > MAX_DATA {
        return Err(Error::Authorization);
    }
    let lc_bytes = if command.data.is_empty() {
        0
    } else if command.data.len() < 255
        && !matches!(command.le, Some(openkeys_scp03::Le::Extended(_)))
    {
        1
    } else {
        3
    };
    let inner_len = 4 + lc_bytes + command.data.len();
    let ber_bytes = if inner_len < 128 {
        1
    } else if inner_len <= 255 {
        2
    } else {
        3
    };
    let at = 11 + ber_bytes;
    let end = at + inner_len;
    if end > wire.len() {
        return Err(Error::Schema);
    }
    wire[..2].copy_from_slice(&[0x10, 8]);
    wire[2..10].copy_from_slice(sid);
    wire[10] = 0x41;
    match ber_bytes {
        1 => wire[11] = inner_len as u8,
        2 => wire[11..13].copy_from_slice(&[0x81, inner_len as u8]),
        _ => {
            wire[11] = 0x82;
            wire[12..14].copy_from_slice(&(inner_len as u16).to_be_bytes());
        }
    }
    wire[at..at + 4].copy_from_slice(&[0x80, command.instruction, command.p1, command.p2]);
    let data_at = at + 4 + lc_bytes;
    match lc_bytes {
        1 => wire[at + 4] = command.data.len() as u8,
        3 => wire[at + 5..at + 7].copy_from_slice(&(command.data.len() as u16).to_be_bytes()),
        _ => {}
    }
    wire[data_at..end].copy_from_slice(command.data);
    Ok(end)
}
fn id_tlv(id: u32) -> [u8; 6] {
    let mut out = [0x41, 4, 0, 0, 0, 0];
    out[2..].copy_from_slice(&id.to_be_bytes());
    out
}
impl<X: Exchange, P: PowerCut> ProtectedPin<6> for NativePin<X, P> {
    fn reserve_attempt(&mut self, request: &Request) -> Result<(), PinError> {
        self.reserve(request).map_err(|_| {
            self.close();
            PinError::Storage
        })
    }
    fn verify(&mut self, request: &Request, digits: &[u8; 6]) -> Result<(), PinError> {
        self.verify_native(request, digits).map_err(|_| {
            self.close();
            PinError::Storage
        })
    }
}
impl<X: Exchange, P: PowerCut> private::Sealed for NativePin<X, P> {}
impl<X: Exchange, P: PowerCut> private::PinBound for NativePin<X, P> {}
impl<X: Exchange, P: PowerCut> PinBoundExchange for NativePin<X, P> {}
impl<X: Exchange, P: PowerCut> Exchange for NativePin<X, P> {
    fn exchange(&mut self, command: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        out.fill(0);
        if !self.verified {
            return Err(Error::Authorization);
        }
        // No host forwarding; these inputs belong only to typed key backends.
        if command.instruction == 5
            || (command.instruction == 4 && (command.p1 != 0 || !matches!(command.p2, 0x27 | 0x28)))
        {
            self.close();
            return Err(Error::Authorization);
        }
        self.process(command, out)
    }
    fn close(&mut self) {
        NativePin::close(self);
    }
}
impl<X: Exchange, P: PowerCut> Drop for NativePin<X, P> {
    fn drop(&mut self) {
        self.close();
    }
}

// Borrowed operation providers retain the native session on NORMAL release,
// allowing Root's independently checked 30-second leases. Errors close/cut it.
impl<T: PinBoundExchange> private::Sealed for &mut T {}
impl<T: PinBoundExchange> private::PinBound for &mut T {}
impl<T: PinBoundExchange> PinBoundExchange for &mut T {}
impl<T: PinBoundExchange> Exchange for &mut T {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        (**self).exchange(c, out)
    }
    fn close(&mut self) {
        (**self).close();
    }
    fn release(&mut self) {}
}

#[cfg(test)]
#[path = "native_pin_tests.rs"]
mod tests;
