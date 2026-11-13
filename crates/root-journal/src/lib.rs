#![no_std]
#![forbid(unsafe_code)]
//! Protected-MCU lifecycle journal. Two secure8KiB sectors, authenticated
//! write-ahead plans and commit-last records. Any corrupt/non-erased slot
//! rejects the whole journal; never fall back to an older Ready state.
//! Production secure-flash/HUK/closed-boot adapters are still required.
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;
pub mod pin;

pub const SECTOR_BYTES: usize = 8192;
pub const PHRASE_BYTES: usize = 16;
pub const RECORD_BYTES: usize = 256;
pub const BODY_BYTES: usize = 224;
const PLAN_AT: usize = RECORD_BYTES;
const HEAD_MAGIC: &[u8; 16] = b"OKRootHeadV1\0\0\0\0";
const PLAN_MAGIC: &[u8; 16] = b"OKRootPlanV1\0\0\0\0";
const COMMIT: [u8; 16] = *b"OKCommittedV1\x5a\xa5\x69";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Input,
    Io,
    Crypto,
    State,
    Integrity,
    Uninitialized,
    Pending,
    Authorization,
    Exhausted,
    Closed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    FactoryBlank = 1,
    PinInstalling,
    PinInstalled,
    ApplicationsInstalling,
    Ready,
    ErasePending,
}
impl Phase {
    fn decode(n: u8) -> Result<Self, Error> {
        match n {
            1 => Ok(Self::FactoryBlank),
            2 => Ok(Self::PinInstalling),
            3 => Ok(Self::PinInstalled),
            4 => Ok(Self::ApplicationsInstalling),
            5 => Ok(Self::Ready),
            6 => Ok(Self::ErasePending),
            _ => Err(Error::Integrity),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    pub device: [u8; 32],
    pub board: [u8; 32],
    pub profile: [u8; 32],
    pub policy: [u8; 32],
}
impl Context {
    fn valid(&self) -> bool {
        [self.device, self.board, self.profile, self.policy]
            .iter()
            .all(|h| *h != [0; 32])
    }
    fn digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"OpenKeys/R05/root-context/v1");
        for value in [self.device, self.board, self.profile, self.policy] {
            h.update(value);
        }
        h.finalize().into()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub sequence: u64,
    pub generation: u64,
    pub phase: Phase,
    pub owner: [u8; 32],
}

/// Exclusively owned internal MCU secure flash, protected from nonsecure code,
/// DMA/debug/unsigned updates and ordinary rollback. Never use external NOR or
/// a host file. Program one erased16-byte phrase and wait/readback durably.
/// Uncertain writes/erases are errors. No blind retry or pending-write cache.
pub trait Storage {
    fn read(&mut self, slot: u8, offset: usize, out: &mut [u8]) -> Result<(), Error>;
    fn program(
        &mut self,
        slot: u8,
        offset: usize,
        phrase: &[u8; PHRASE_BYTES],
    ) -> Result<(), Error>;
    fn erase(&mut self, slot: u8) -> Result<(), Error>;
}
/// Non-exportable real per-device hardware MAC. Reject unqualified lifecycle,
/// debug, pending tamper/dummy DHUK, load faults and key-context changes.
/// No public/UID/test-key or software production fallback.
pub trait Mac {
    fn tag(&mut self, body: &[u8; BODY_BYTES]) -> Result<[u8; 16], Error>;
}
/// Trusted Root, never host phase/blank/"erasure complete" flags. A pending
/// transition can outlast user leases but cannot authorize private operations.
pub trait Guard {
    fn check(&mut self) -> Result<(), Error>;
    /// Fresh healthy hardware RNG, never a counter, UID, host value or default.
    /// Root binds this nonce to an immutable SE object in the same user epoch.
    fn fresh_owner_nonce(&mut self) -> Result<[u8; 32], Error>;
    fn authorize_transition(&mut self, current: &State, target: &State) -> Result<(), Error>;
    /// Independent factory or full verified SE+NOR+Root erasure evidence.
    /// Missing/corrupt journal by itself is explicitly NOT that evidence.
    fn authorize_factory_initialize(&mut self, context: &Context) -> Result<(), Error>;
}
#[derive(Clone)]
struct Record {
    slot: u8,
    state: State,
    previous: [u8; 32],
    wire: [u8; RECORD_BYTES],
}
struct Slot {
    head: Record,
    plan: Option<Record>,
}
pub struct Journal<S, M> {
    storage: S,
    mac: M,
    context: Context,
    head: Record,
    pending: Option<Record>,
    closed: bool,
}
fn transition(current: &State, target: &State) -> bool {
    if current.sequence.checked_add(1) != Some(target.sequence)
        || target.sequence == u64::MAX
        || target.generation == 0
        || target.owner == [0; 32]
    {
        return false;
    }
    if target.phase == Phase::ErasePending {
        return current.phase != Phase::ErasePending
            && current.generation.checked_add(1) == Some(target.generation)
            && target.owner != current.owner;
    }
    current.generation == target.generation
        && current.owner == target.owner
        && matches!(
            (current.phase, target.phase),
            (Phase::FactoryBlank, Phase::PinInstalling)
                | (Phase::PinInstalling, Phase::PinInstalled)
                | (Phase::PinInstalled, Phase::ApplicationsInstalling)
                | (Phase::ApplicationsInstalling, Phase::Ready)
                | (Phase::ErasePending, Phase::FactoryBlank)
        )
}
fn digest(record: &Record) -> [u8; 32] {
    Sha256::digest(record.wire).into()
}
fn follows(current: &Record, next: &Record) -> bool {
    next.slot == 1 - current.slot
        && transition(&current.state, &next.state)
        && next.previous == digest(current)
}
fn same_target(plan: &Record, head: &Record) -> bool {
    plan.slot == head.slot && plan.state == head.state && plan.previous == head.previous
}
fn erased(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| b == 0xff)
}
fn encode(
    context: &Context,
    slot: u8,
    state: State,
    previous: [u8; 32],
    plan: bool,
    mac: &mut impl Mac,
) -> Result<Record, Error> {
    let mut wire = [0; RECORD_BYTES];
    wire[..16].copy_from_slice(if plan { PLAN_MAGIC } else { HEAD_MAGIC });
    wire[16..18].copy_from_slice(&1_u16.to_be_bytes());
    wire[18] = slot;
    wire[19] = state.phase as u8;
    wire[24..32].copy_from_slice(&state.sequence.to_be_bytes());
    wire[32..40].copy_from_slice(&state.generation.to_be_bytes());
    wire[40..72].copy_from_slice(&context.digest());
    wire[72..104].copy_from_slice(&state.owner);
    wire[104..136].copy_from_slice(&previous);
    let tag = mac.tag(wire[..BODY_BYTES].try_into().map_err(|_| Error::Input)?)?;
    wire[BODY_BYTES..BODY_BYTES + 16].copy_from_slice(&tag);
    wire[BODY_BYTES + 16..].copy_from_slice(&COMMIT);
    Ok(Record {
        slot,
        state,
        previous,
        wire,
    })
}
fn decode(
    context: &Context,
    slot: u8,
    wire: &[u8; RECORD_BYTES],
    plan: bool,
    mac: &mut impl Mac,
) -> Result<Record, Error> {
    let tag = mac.tag(
        wire[..BODY_BYTES]
            .try_into()
            .map_err(|_| Error::Integrity)?,
    )?;
    if !bool::from(tag.ct_eq(&wire[BODY_BYTES..BODY_BYTES + 16]))
        || wire[..16] != *(if plan { PLAN_MAGIC } else { HEAD_MAGIC })
        || wire[16..18] != 1_u16.to_be_bytes()
        || wire[18] != slot
        || wire[20..24] != [0; 4]
        || wire[136..BODY_BYTES] != [0; 88]
        || wire[BODY_BYTES + 16..] != COMMIT
        || wire[40..72] != context.digest()
    {
        return Err(Error::Integrity);
    }
    let state = State {
        sequence: u64::from_be_bytes(wire[24..32].try_into().map_err(|_| Error::Integrity)?),
        generation: u64::from_be_bytes(wire[32..40].try_into().map_err(|_| Error::Integrity)?),
        phase: Phase::decode(wire[19])?,
        owner: wire[72..104].try_into().map_err(|_| Error::Integrity)?,
    };
    if state.sequence == 0
        || state.sequence == u64::MAX
        || state.generation == 0
        || state.owner == [0; 32]
    {
        return Err(Error::Integrity);
    }
    Ok(Record {
        slot,
        state,
        previous: wire[104..136].try_into().map_err(|_| Error::Integrity)?,
        wire: *wire,
    })
}
fn read(
    storage: &mut impl Storage,
    guard: &mut impl Guard,
    slot: u8,
    offset: usize,
    out: &mut [u8],
) -> Result<(), Error> {
    out.fill(0);
    guard.check()?;
    storage.read(slot, offset, out)?;
    guard.check()
}
fn load(
    storage: &mut impl Storage,
    mac: &mut impl Mac,
    guard: &mut impl Guard,
    context: &Context,
    slot: u8,
) -> Result<Option<Slot>, Error> {
    let mut head = Zeroizing::new([0; RECORD_BYTES]);
    let mut plan = Zeroizing::new([0; RECORD_BYTES]);
    read(storage, guard, slot, 0, &mut head[..])?;
    read(storage, guard, slot, PLAN_AT, &mut plan[..])?;
    let mut bytes = [0; RECORD_BYTES];
    for at in (2 * RECORD_BYTES..SECTOR_BYTES).step_by(RECORD_BYTES) {
        read(storage, guard, slot, at, &mut bytes)?;
        if !erased(&bytes) {
            return Err(Error::Integrity);
        }
    }
    if erased(&head[..]) && erased(&plan[..]) {
        return Ok(None);
    }
    let head = decode(context, slot, &head, false, mac)?;
    let plan = if erased(&plan[..]) {
        None
    } else {
        Some(decode(context, 1 - slot, &plan, true, mac)?)
    };
    if plan.as_ref().is_some_and(|p| !follows(&head, p)) {
        return Err(Error::Integrity);
    }
    Ok(Some(Slot { head, plan }))
}
impl<S: Storage, M: Mac> Journal<S, M> {
    /// A torn/corrupt slot rejects BOTH slots. Recovery from that condition
    /// requires a separate independently authorized full destructive service.
    /// Complete authenticated plans can resume only the exact journal change.
    pub fn open(
        mut storage: S,
        mut mac: M,
        context: Context,
        guard: &mut impl Guard,
    ) -> Result<Self, Error> {
        if !context.valid() {
            return Err(Error::Input);
        }
        let a = load(&mut storage, &mut mac, guard, &context, 0)?;
        let b = load(&mut storage, &mut mac, guard, &context, 1)?;
        let (head, pending) = match (a, b) {
            (None, None) => return Err(Error::Uninitialized),
            (Some(a), Some(b)) => {
                let (old, new) = if a.head.state.sequence < b.head.state.sequence {
                    (a, b)
                } else {
                    (b, a)
                };
                if !follows(&old.head, &new.head)
                    || !old.plan.is_some_and(|p| same_target(&p, &new.head))
                {
                    return Err(Error::Integrity);
                }
                (new.head, new.plan)
            }
            (Some(slot), None) | (None, Some(slot)) => {
                let first = slot.head.slot == 0
                    && slot.head.state.sequence == 1
                    && slot.head.state.generation == 1
                    && slot.head.state.phase == Phase::FactoryBlank
                    && slot.head.previous == [0; 32];
                if slot.plan.is_none() && !first {
                    return Err(Error::Integrity);
                }
                (slot.head, slot.plan)
            }
        };
        guard.check()?;
        Ok(Self {
            storage,
            mac,
            context,
            head,
            pending,
            closed: false,
        })
    }
    /// No automatic missing-state initialization. This entry needs Root's
    /// independent factory/full-wipe authorization and BOTH sectors fully empty.
    pub fn factory_initialize(
        mut storage: S,
        mut mac: M,
        context: Context,
        guard: &mut impl Guard,
    ) -> Result<Self, Error> {
        if !context.valid() {
            return Err(Error::Input);
        }
        guard.check()?;
        guard.authorize_factory_initialize(&context)?;
        for slot in [0, 1] {
            if load(&mut storage, &mut mac, guard, &context, slot)?.is_some() {
                return Err(Error::State);
            }
        }
        let owner = guard.fresh_owner_nonce()?;
        if owner == [0; 32] {
            return Err(Error::Crypto);
        }
        let head = encode(
            &context,
            0,
            State {
                sequence: 1,
                generation: 1,
                phase: Phase::FactoryBlank,
                owner,
            },
            [0; 32],
            false,
            &mut mac,
        )?;
        program_record(&mut storage, guard, 0, 0, &head)?;
        Self::open(storage, mac, context, guard)
    }
    pub fn state(&self) -> Result<State, Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        if self.pending.is_some() {
            return Err(Error::Pending);
        }
        Ok(self.head.state)
    }
    pub fn pending_target(&self) -> Result<Option<State>, Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        Ok(self.pending.as_ref().map(|r| r.state))
    }
    pub fn advance(&mut self, phase: Phase, guard: &mut impl Guard) -> Result<State, Error> {
        let result = self.advance_inner(phase, guard);
        if result.is_err() {
            self.closed = true;
        }
        result
    }
    fn advance_inner(&mut self, phase: Phase, guard: &mut impl Guard) -> Result<State, Error> {
        guard.check()?;
        let current = self.state()?;
        let target = State {
            sequence: current.sequence.checked_add(1).ok_or(Error::Exhausted)?,
            generation: if phase == Phase::ErasePending {
                current.generation.checked_add(1).ok_or(Error::Exhausted)?
            } else {
                current.generation
            },
            phase,
            owner: if phase == Phase::ErasePending {
                guard.fresh_owner_nonce()?
            } else {
                current.owner
            },
        };
        if !transition(&current, &target) {
            return Err(Error::State);
        }
        guard.check()?;
        guard.authorize_transition(&current, &target)?;
        guard.check()?;
        let plan = encode(
            &self.context,
            1 - self.head.slot,
            target,
            digest(&self.head),
            true,
            &mut self.mac,
        )?;
        program_record(&mut self.storage, guard, self.head.slot, PLAN_AT, &plan)?;
        self.pending = Some(plan);
        self.recover_inner(guard)
    }
    pub fn recover(&mut self, guard: &mut impl Guard) -> Result<State, Error> {
        let result = self.recover_inner(guard);
        if result.is_err() {
            self.closed = true;
        }
        result
    }
    fn recover_inner(&mut self, guard: &mut impl Guard) -> Result<State, Error> {
        if self.closed {
            return Err(Error::Closed);
        }
        let plan = self.pending.as_ref().ok_or(Error::State)?;
        if !follows(&self.head, plan) {
            return Err(Error::Integrity);
        }
        guard.check()?;
        self.storage.erase(plan.slot)?;
        guard.check()?;
        let mut bytes = [0; RECORD_BYTES];
        for at in (0..SECTOR_BYTES).step_by(RECORD_BYTES) {
            read(&mut self.storage, guard, plan.slot, at, &mut bytes)?;
            if !erased(&bytes) {
                return Err(Error::Integrity);
            }
        }
        let new = encode(
            &self.context,
            plan.slot,
            plan.state,
            plan.previous,
            false,
            &mut self.mac,
        )?;
        program_record(&mut self.storage, guard, new.slot, 0, &new)?;
        let reloaded = load(
            &mut self.storage,
            &mut self.mac,
            guard,
            &self.context,
            new.slot,
        )?;
        if !matches!(&reloaded, Some(Slot { head, plan: None }) if same_target(head, &new)) {
            return Err(Error::Integrity);
        }
        guard.check()?;
        self.head = new;
        self.pending = None;
        Ok(self.head.state)
    }
}
fn program_record(
    storage: &mut impl Storage,
    guard: &mut impl Guard,
    physical_slot: u8,
    at: usize,
    record: &Record,
) -> Result<(), Error> {
    for (index, phrase) in record.wire.as_chunks::<PHRASE_BYTES>().0.iter().enumerate() {
        guard.check()?;
        storage.program(physical_slot, at + index * PHRASE_BYTES, phrase)?;
        guard.check()?;
        let mut readback = [0; PHRASE_BYTES];
        read(
            storage,
            guard,
            physical_slot,
            at + index * PHRASE_BYTES,
            &mut readback,
        )?;
        if readback != *phrase {
            return Err(Error::Integrity);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
