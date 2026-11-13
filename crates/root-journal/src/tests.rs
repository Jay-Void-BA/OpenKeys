extern crate std;
use super::*;
use hmac::{Hmac, Mac as HmacTrait};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    vec,
    vec::Vec,
};

#[derive(Clone, Copy)]
enum Fault {
    Before,
    After,
    Torn,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Program(u8, usize),
    Erase(u8),
}
struct Memory {
    pages: [Vec<u8>; 2],
    calls: usize,
    fault: Option<(usize, Fault)>,
    events: Vec<Event>,
}
impl Default for Memory {
    fn default() -> Self {
        Self {
            pages: [vec![0xff; SECTOR_BYTES], vec![0xff; SECTOR_BYTES]],
            calls: 0,
            fault: None,
            events: Vec::new(),
        }
    }
}
type Shared = Rc<RefCell<Memory>>;
struct Flash(Shared);
impl Storage for Flash {
    fn read(&mut self, slot: u8, offset: usize, out: &mut [u8]) -> Result<(), Error> {
        let mut m = self.0.borrow_mut();
        m.calls += 1;
        if matches!(m.fault, Some((at, Fault::Before)) if at == m.calls) {
            return Err(Error::Io);
        }
        out.copy_from_slice(&m.pages[usize::from(slot)][offset..offset + out.len()]);
        if matches!(m.fault, Some((at, Fault::After | Fault::Torn)) if at == m.calls) {
            return Err(Error::Io);
        }
        Ok(())
    }
    fn program(
        &mut self,
        slot: u8,
        offset: usize,
        phrase: &[u8; PHRASE_BYTES],
    ) -> Result<(), Error> {
        let mut m = self.0.borrow_mut();
        m.calls += 1;
        if matches!(m.fault, Some((at, Fault::Before)) if at == m.calls) {
            return Err(Error::Io);
        }
        assert!(offset.is_multiple_of(16));
        assert!(erased(
            &m.pages[usize::from(slot)][offset..offset + PHRASE_BYTES]
        ));
        let length = if matches!(m.fault, Some((at, Fault::Torn)) if at == m.calls) {
            8
        } else {
            16
        };
        m.pages[usize::from(slot)][offset..offset + length].copy_from_slice(&phrase[..length]);
        m.events.push(Event::Program(slot, offset));
        if matches!(m.fault, Some((at, Fault::After | Fault::Torn)) if at == m.calls) {
            return Err(Error::Io);
        }
        Ok(())
    }
    fn erase(&mut self, slot: u8) -> Result<(), Error> {
        let mut m = self.0.borrow_mut();
        m.calls += 1;
        if matches!(m.fault, Some((at, Fault::Before)) if at == m.calls) {
            return Err(Error::Io);
        }
        let length = if matches!(m.fault, Some((at, Fault::Torn)) if at == m.calls) {
            SECTOR_BYTES / 2
        } else {
            SECTOR_BYTES
        };
        m.pages[usize::from(slot)][..length].fill(0xff);
        m.events.push(Event::Erase(slot));
        if matches!(m.fault, Some((at, Fault::After | Fault::Torn)) if at == m.calls) {
            return Err(Error::Io);
        }
        Ok(())
    }
}
struct ModelMac {
    fail: Rc<Cell<bool>>,
    key: u8,
}
impl Default for ModelMac {
    fn default() -> Self {
        Self {
            fail: Rc::new(Cell::new(false)),
            key: 0x92,
        }
    }
}
impl Mac for ModelMac {
    fn tag(&mut self, body: &[u8; BODY_BYTES]) -> Result<[u8; 16], Error> {
        if self.fail.get() {
            return Err(Error::Crypto);
        }
        // Synthetic ideal-MAC oracle only. Not a production HUK provider.
        let mut mac = <Hmac<Sha256> as HmacTrait>::new_from_slice(&[self.key; 32]).unwrap();
        mac.update(body);
        Ok(mac.finalize().into_bytes()[..16].try_into().unwrap())
    }
}
struct Root {
    checks: usize,
    deny_check: Option<usize>,
    deny_factory: bool,
    deny_transition: bool,
    nonce: u8,
}
impl Default for Root {
    fn default() -> Self {
        Self {
            checks: 0,
            deny_check: None,
            deny_factory: false,
            deny_transition: false,
            nonce: 0x50,
        }
    }
}
impl Guard for Root {
    fn check(&mut self) -> Result<(), Error> {
        self.checks += 1;
        if self.deny_check == Some(self.checks) {
            Err(Error::Authorization)
        } else {
            Ok(())
        }
    }
    fn fresh_owner_nonce(&mut self) -> Result<[u8; 32], Error> {
        self.nonce += 1;
        Ok([self.nonce; 32])
    }
    fn authorize_transition(&mut self, _: &State, _: &State) -> Result<(), Error> {
        if self.deny_transition {
            Err(Error::Authorization)
        } else {
            Ok(())
        }
    }
    fn authorize_factory_initialize(&mut self, _: &Context) -> Result<(), Error> {
        if self.deny_factory {
            Err(Error::Authorization)
        } else {
            Ok(())
        }
    }
}
fn context() -> Context {
    Context {
        device: [1; 32],
        board: [2; 32],
        profile: [3; 32],
        policy: [4; 32],
    }
}
fn seed(phase: Phase) -> (Shared, Journal<Flash, ModelMac>, Root) {
    let m = Shared::default();
    let mut root = Root::default();
    let mut j =
        Journal::factory_initialize(Flash(m.clone()), ModelMac::default(), context(), &mut root)
            .unwrap();
    if phase != Phase::FactoryBlank {
        for p in [
            Phase::PinInstalling,
            Phase::PinInstalled,
            Phase::ApplicationsInstalling,
            Phase::Ready,
        ] {
            j.advance(p, &mut root).unwrap();
            if p == phase {
                break;
            }
        }
    }
    m.borrow_mut().calls = 0;
    m.borrow_mut().events.clear();
    (m, j, root)
}
fn reopen(m: &Shared, root: &mut Root) -> Result<Journal<Flash, ModelMac>, Error> {
    Journal::open(Flash(m.clone()), ModelMac::default(), context(), root)
}

#[test]
fn empty_storage_and_denied_factory_authority_never_mean_blank() {
    let m = Shared::default();
    let mut root = Root::default();
    assert!(matches!(reopen(&m, &mut root), Err(Error::Uninitialized)));
    root.deny_factory = true;
    assert!(matches!(
        Journal::factory_initialize(Flash(m.clone()), ModelMac::default(), context(), &mut root),
        Err(Error::Authorization)
    ));
    assert!(m.borrow().events.is_empty());
}
#[test]
fn legal_lifecycle_persists_and_every_wipe_changes_the_se_bound_owner() {
    let (m, mut j, mut root) = seed(Phase::Ready);
    let before = j.state().unwrap();
    assert_eq!((before.sequence, before.generation), (5, 1));
    let erase = j.advance(Phase::ErasePending, &mut root).unwrap();
    assert_eq!((erase.sequence, erase.generation), (6, 2));
    assert_ne!(before.owner, erase.owner);
    j.advance(Phase::FactoryBlank, &mut root).unwrap();
    assert_eq!(j.state().unwrap().owner, erase.owner);
    drop(j);
    let restored = reopen(&m, &mut root).unwrap().state().unwrap();
    assert_eq!(restored.phase, Phase::FactoryBlank);
    assert_eq!(restored.generation, 2);
    assert_eq!(restored.owner, erase.owner);
}
#[test]
fn plan_is_committed_and_read_back_before_the_other_sector_is_erased() {
    let (m, mut j, mut root) = seed(Phase::Ready);
    let active = j.head.slot;
    j.advance(Phase::ErasePending, &mut root).unwrap();
    let events = &m.borrow().events;
    let erase_at = events
        .iter()
        .position(|e| *e == Event::Erase(1 - active))
        .unwrap();
    assert_eq!(
        events[erase_at - 1],
        Event::Program(active, PLAN_AT + RECORD_BYTES - 16)
    );
    assert!(
        events[..erase_at]
            .iter()
            .all(|e| matches!(e, Event::Program(s, at) if *s == active && *at >= PLAN_AT))
    );
    assert_eq!(
        events.last(),
        Some(&Event::Program(1 - active, RECORD_BYTES - 16))
    );
}
#[test]
fn corrupt_newest_head_plan_or_unused_tail_never_falls_back_to_old_ready() {
    let (m, mut j, mut root) = seed(Phase::Ready);
    let old = j.head.slot;
    j.advance(Phase::ErasePending, &mut root).unwrap();
    let latest = j.head.slot;
    let bytes = m.borrow().pages.clone();
    for (slot, at) in [
        (latest, 10),
        (latest, 239),
        (latest, 255),
        (old, PLAN_AT + 19),
        (latest, 8191),
    ] {
        m.borrow_mut().pages = bytes.clone();
        m.borrow_mut().pages[usize::from(slot)][at] ^= 1;
        assert!(matches!(reopen(&m, &mut root), Err(Error::Integrity)));
    }
}
#[test]
fn every_read_program_and_erase_cut_cannot_reuse_ready_after_any_intent_write() {
    let (base, mut j, mut root) = seed(Phase::Ready);
    let bytes = base.borrow().pages.clone();
    let active = j.head.slot;
    j.advance(Phase::ErasePending, &mut root).unwrap();
    let calls = base.borrow().calls;
    for at in 1..=calls {
        for fault in [Fault::Before, Fault::After, Fault::Torn] {
            let m = Rc::new(RefCell::new(Memory {
                pages: bytes.clone(),
                ..Memory::default()
            }));
            let mut guard = Root {
                nonce: 0x60,
                ..Root::default()
            };
            let mut owner = reopen(&m, &mut guard).unwrap();
            m.borrow_mut().calls = 0;
            m.borrow_mut().fault = Some((at, fault));
            assert!(
                owner.advance(Phase::ErasePending, &mut guard).is_err(),
                "unreached storage cut {at}"
            );
            assert_eq!(m.borrow().calls, at);
            assert_eq!(owner.state(), Err(Error::Closed));
            assert!(owner.recover(&mut guard).is_err());
            let intent_written =
                !erased(&m.borrow().pages[usize::from(active)][PLAN_AT..PLAN_AT + RECORD_BYTES]);
            m.borrow_mut().fault = None;
            if let Ok(mut reopened) = reopen(&m, &mut guard) {
                match reopened.state() {
                    Ok(state) => assert!(!intent_written || state.phase != Phase::Ready),
                    Err(Error::Pending) => {
                        assert_eq!(
                            reopened.pending_target().unwrap().unwrap().phase,
                            Phase::ErasePending
                        );
                        assert_eq!(
                            reopened.recover(&mut guard).unwrap().phase,
                            Phase::ErasePending
                        );
                    }
                    other => panic!("unexpected recovery state {other:?}"),
                }
            }
        }
    }
}
#[test]
fn invalid_phase_shortcuts_zero_or_repeated_nonce_and_counter_wrap_are_rejected() {
    let (m, mut j, mut root) = seed(Phase::Ready);
    let bytes = m.borrow().pages.clone();
    assert_eq!(j.advance(Phase::FactoryBlank, &mut root), Err(Error::State));
    assert!(m.borrow().events.is_empty());
    m.borrow_mut().pages = bytes;
    let s = State {
        sequence: 5,
        generation: 1,
        phase: Phase::Ready,
        owner: [1; 32],
    };
    for target in [
        State {
            sequence: 6,
            generation: 2,
            phase: Phase::ErasePending,
            owner: [1; 32],
        },
        State {
            sequence: 6,
            generation: 2,
            phase: Phase::ErasePending,
            owner: [0; 32],
        },
        State {
            sequence: 6,
            generation: 1,
            phase: Phase::ErasePending,
            owner: [2; 32],
        },
        State {
            sequence: 0,
            generation: 2,
            phase: Phase::ErasePending,
            owner: [2; 32],
        },
    ] {
        assert!(!transition(&s, &target));
    }
    let terminal = State {
        sequence: u64::MAX - 1,
        ..s
    };
    assert!(!transition(
        &terminal,
        &State {
            sequence: u64::MAX,
            generation: 2,
            phase: Phase::ErasePending,
            owner: [2; 32]
        }
    ));
}
#[test]
fn device_board_profile_policy_key_or_mac_failure_cannot_open_another_context() {
    let (m, _, mut root) = seed(Phase::Ready);
    for field in 0..4 {
        let mut c = context();
        match field {
            0 => c.device[0] ^= 1,
            1 => c.board[0] ^= 1,
            2 => c.profile[0] ^= 1,
            _ => c.policy[0] ^= 1,
        }
        assert!(matches!(
            Journal::open(Flash(m.clone()), ModelMac::default(), c, &mut root),
            Err(Error::Integrity)
        ));
    }
    assert!(matches!(
        Journal::open(
            Flash(m.clone()),
            ModelMac {
                key: 7,
                ..ModelMac::default()
            },
            context(),
            &mut root
        ),
        Err(Error::Integrity)
    ));
    let mac = ModelMac::default();
    mac.fail.set(true);
    assert!(matches!(
        Journal::open(Flash(m.clone()), mac, context(), &mut root),
        Err(Error::Crypto)
    ));
}
#[test]
fn denied_authority_or_live_guard_failure_never_returns_usable_old_state() {
    let (m, mut j, mut root) = seed(Phase::Ready);
    root.deny_transition = true;
    assert_eq!(
        j.advance(Phase::ErasePending, &mut root),
        Err(Error::Authorization)
    );
    assert_eq!(j.state(), Err(Error::Closed));
    assert!(m.borrow().events.is_empty());
    root.deny_transition = false;
    let mut j = reopen(&m, &mut root).unwrap();
    root.deny_check = Some(root.checks + 1);
    assert_eq!(
        j.advance(Phase::ErasePending, &mut root),
        Err(Error::Authorization)
    );
    assert_eq!(j.state(), Err(Error::Closed));
}

#[test]
fn native_installer_bridge_gets_owner_only_from_committed_installing_phase() {
    use openkeys_se_state::commissioning::Root as _;
    let (m, mut j, mut guard) = seed(Phase::FactoryBlank);
    let owner = j.state().unwrap().owner;
    {
        let mut root = crate::pin::PinRoot::new(&mut j, &mut guard);
        root.qualified_blank().unwrap();
        assert!(root.owner_nonce().is_err());
        assert!(root.begin_pin_install(&[9; 32], &[1; 32]).is_err());
        assert!(
            root.begin_pin_install(&context().profile, &[0; 32])
                .is_err()
        );
        root.begin_pin_install(&context().profile, &[1; 32])
            .unwrap();
        assert!(root.qualified_blank().is_err());
        assert_eq!(root.owner_nonce().unwrap(), owner);
        root.commit_pin_namespace().unwrap();
        assert!(root.owner_nonce().is_err());
        assert!(
            root.begin_pin_install(&context().profile, &[1; 32])
                .is_err()
        );
    }
    assert_eq!(
        reopen(&m, &mut guard).unwrap().state().unwrap().phase,
        Phase::PinInstalled
    );
}

#[test]
fn a_whole_old_snapshot_still_needs_independent_se_owner_matching() {
    let (m, mut j, mut guard) = seed(Phase::Ready);
    let snapshot = m.borrow().pages.clone();
    let old = j.state().unwrap().owner;
    let next = j.advance(Phase::ErasePending, &mut guard).unwrap().owner;
    j.advance(Phase::FactoryBlank, &mut guard).unwrap();
    assert_ne!(old, next);
    // A MAC alone does not identify a replay of both protected sectors.
    // The independent immutable SE owner must be checked by NativePin::open
    // and every private port before this metadata can authorize anything.
    m.borrow_mut().pages = snapshot;
    let replayed = reopen(&m, &mut guard).unwrap().state().unwrap();
    assert_eq!(replayed.phase, Phase::Ready);
    assert_ne!(replayed.owner, next);
}
