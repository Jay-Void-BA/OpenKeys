extern crate std;
use super::*;
use crate::{
    pin::{Error as PinError, Press, ProtectedPin},
    pin_only::{Digit, PinEntry},
};
use std::{cell::RefCell, rc::Rc};

struct Pin;
impl ProtectedPin<6> for Pin {
    fn reserve_attempt(&mut self, _: &Request) -> Result<(), PinError> {
        Ok(())
    }
    fn verify(&mut self, _: &Request, p: &[u8; 6]) -> Result<(), PinError> {
        assert_eq!(*p, [1; 6]);
        Ok(())
    }
}
fn proof(b: &Binding, now: u64) -> VerifiedPinRequest {
    let mut p = PinEntry::begin(b.unlock_request(), || now).unwrap();
    for _ in 0..6 {
        p.press(Digit::One, Press::Short, now).unwrap();
    }
    p.submit(&mut Pin, || now).unwrap()
}
fn binding(epoch: u64) -> Binding {
    Binding::from_verified_channel([2; 32], [3; 32], epoch, Transport::Nfc).unwrap()
}
struct Db {
    expected: Binding,
    connected: bool,
    confirm: bool,
    protocol: [bool; 6],
    revokes: usize,
    protocol_revokes: usize,
    confirms: usize,
    denied_digest: Option<[u8; 32]>,
}
type Shared = Rc<RefCell<Db>>;
struct Verify(Shared);
impl Verifier for Verify {
    fn check_link_and_hardware(&mut self, b: &Binding) -> Result<(), Error> {
        let d = self.0.borrow();
        if d.connected && d.expected == *b {
            Ok(())
        } else {
            Err(Error::HardwareFault)
        }
    }
    fn check_protocol_pin(&mut self, _: &Binding, p: Protocol) -> Result<(), Error> {
        if self.0.borrow().protocol[p as usize] {
            Ok(())
        } else {
            Err(Error::Rejected)
        }
    }
    fn confirm_on_device(&mut self, _: &Request) -> Result<(), Error> {
        let mut d = self.0.borrow_mut();
        d.confirms += 1;
        if d.confirm {
            Ok(())
        } else {
            Err(Error::Rejected)
        }
    }
    fn check_protocol_request(
        &mut self,
        b: &Binding,
        p: Protocol,
        r: &Request,
    ) -> Result<(), Error> {
        self.check_protocol_pin(b, p)?;
        let d = self.0.borrow();
        if r.channel != b.channel() || d.denied_digest == Some(r.digest) {
            Err(Error::Rejected)
        } else {
            Ok(())
        }
    }
    fn revoke_all(&mut self) {
        let mut d = self.0.borrow_mut();
        d.protocol.fill(false);
        d.revokes += 1;
    }
    fn revoke_protocol(&mut self, p: Protocol) {
        let mut d = self.0.borrow_mut();
        d.protocol[p as usize] = false;
        d.protocol_revokes += 1;
    }
}
fn model() -> (Gate<Verify>, Shared, Binding) {
    let b = binding(1);
    let db = Rc::new(RefCell::new(Db {
        expected: b,
        connected: true,
        confirm: true,
        protocol: [false; 6],
        revokes: 0,
        protocol_revokes: 0,
        confirms: 0,
        denied_digest: None,
    }));
    let mut g = Gate::new(Verify(db.clone()));
    g.unlock(proof(&b, 1), b, || 1).unwrap();
    (g, db, b)
}
fn request(b: &Binding, op: Operation) -> Request {
    Request {
        operation: op,
        digest: [9; 32],
        channel: b.channel(),
    }
}
fn complete(g: &mut Gate<Verify>, b: &Binding, op: Operation, p: Option<Protocol>, now: u64) {
    let r = request(b, op);
    g.approve(r, p, b, || now).unwrap();
    let a = g.consume_authorized(&r, b, || now).unwrap();
    let e = a.activate(&r, g, || now).unwrap();
    let done = e.finish(&r, g, || now).unwrap();
    g.completed(done, b, || now).unwrap();
}

#[test]
fn physical_connection_and_keepalive_polling_cannot_renew_thirty_seconds() {
    let (mut g, _, b) = model();
    for t in [2, 100, 1000, 10_000, 20_000, 30_000] {
        g.tick(&b, || t).unwrap();
    }
    assert_eq!(g.tick(&b, || 30_001), Err(Error::Expired));
    assert!(
        g.approve(request(&b, Operation::RevealNote), None, &b, || 30_002)
            .is_err()
    );
}
#[test]
fn hardware_and_protocol_pin_flags_are_independent_and_scoped_to_role() {
    let (mut g, db, b) = model();
    assert_eq!(
        g.note_protocol_verified(&b, Protocol::PivPin, || 2),
        Err(Error::Rejected)
    );
    assert_eq!(
        g.approve(request(&b, Operation::Piv), None, &b, || 3),
        Err(Error::Rejected)
    );
    assert_eq!(
        g.approve(
            request(&b, Operation::Piv),
            Some(Protocol::PivPin),
            &b,
            || 3
        ),
        Err(Error::Locked)
    );
    db.borrow_mut().protocol[Protocol::PivPin as usize] = true;
    g.note_protocol_verified(&b, Protocol::PivPin, || 4)
        .unwrap();
    assert_eq!(
        g.approve(
            request(&b, Operation::OpenPgp),
            Some(Protocol::OpenPgpAdmin),
            &b,
            || 5
        ),
        Err(Error::Locked)
    );
    complete(&mut g, &b, Operation::Piv, Some(Protocol::PivPin), 6);
}
#[test]
fn unrelated_activity_does_not_extend_an_unused_protocol_pin() {
    let (mut g, db, b) = model();
    db.borrow_mut().protocol[Protocol::PivPin as usize] = true;
    g.note_protocol_verified(&b, Protocol::PivPin, || 2)
        .unwrap();
    complete(&mut g, &b, Operation::RevealNote, None, 20_000);
    g.tick(&b, || 31_000).unwrap();
    assert!(!db.borrow().protocol[Protocol::PivPin as usize]);
    assert_eq!(db.borrow().protocol_revokes, 1);
    assert_eq!(
        g.approve(
            request(&b, Operation::Piv),
            Some(Protocol::PivPin),
            &b,
            || 31_001
        ),
        Err(Error::Locked)
    );
    assert_eq!(g.tick(&b, || 50_000), Err(Error::Expired));
}
#[test]
fn rejected_or_cancelled_requests_never_refresh_idle_or_replace_an_operation() {
    let (mut g, db, b) = model();
    db.borrow_mut().confirm = false;
    assert_eq!(
        g.approve(request(&b, Operation::RevealNote), None, &b, || 20_000),
        Err(Error::Rejected)
    );
    assert_eq!(g.tick(&b, || 30_001), Err(Error::Expired));
    let (mut g, _, b) = model();
    let r = request(&b, Operation::RevealNote);
    g.approve(r, None, &b, || 2).unwrap();
    assert_eq!(
        g.approve(request(&b, Operation::RevealRecoveryCode), None, &b, || 3),
        Err(Error::Rejected)
    );
    let a = g.consume_authorized(&r, &b, || 3).unwrap();
    let e = a.activate(&r, &mut g, || 3).unwrap();
    let done = e.finish(&r, &mut g, || 4).unwrap();
    g.completed(done, &b, || 4).unwrap();
}
#[test]
fn nfc_loss_reentry_epoch_and_peer_change_never_inherit_a_cache() {
    let (mut g, db, b) = model();
    db.borrow_mut().protocol[Protocol::PivPin as usize] = true;
    g.note_protocol_verified(&b, Protocol::PivPin, || 2)
        .unwrap();
    db.borrow_mut().connected = false;
    assert_eq!(g.tick(&b, || 3), Err(Error::HardwareFault));
    assert!(!db.borrow().protocol.iter().any(|&x| x));
    let next = binding(2);
    db.borrow_mut().connected = true;
    db.borrow_mut().expected = next;
    assert_eq!(g.unlock(proof(&b, 4), next, || 4), Err(Error::WrongRequest));
    g.unlock(proof(&next, 5), next, || 5).unwrap();
    assert_eq!(
        g.approve(
            request(&next, Operation::Piv),
            Some(Protocol::PivPin),
            &next,
            || 6
        ),
        Err(Error::Locked)
    );
    let other = Binding::from_verified_channel([2; 32], [4; 32], 2, Transport::Nfc).unwrap();
    assert_eq!(
        g.approve(request(&other, Operation::RevealNote), None, &other, || 7),
        Err(Error::WrongRequest)
    );
    assert_eq!(g.tick(&next, || 35_005), Err(Error::Expired));
}
#[test]
fn manual_lock_revokes_both_caches_and_inflight_authority() {
    let (mut g, db, b) = model();
    let r = request(&b, Operation::RevealNote);
    g.approve(r, None, &b, || 2).unwrap();
    let a = g.consume_authorized(&r, &b, || 2).unwrap();
    let mut e = a.activate(&r, &mut g, || 2).unwrap();
    g.lock();
    assert!(e.check(&r, &mut g, || 3).is_err());
    assert!(!db.borrow().protocol.iter().any(|&x| x));
    assert!(g.tick(&b, || 4).is_err());
}
#[test]
fn long_operation_has_fixed_deadline_and_old_completion_cannot_unlock_again() {
    let (mut g, _, b) = model();
    let r = request(&b, Operation::Import);
    g.approve(r, None, &b, || 2).unwrap();
    let a = g.consume_authorized(&r, &b, || 2).unwrap();
    let mut e = a.activate(&r, &mut g, || 2).unwrap();
    g.tick(&b, || 100_000).unwrap();
    e.check(&r, &mut g, || 100_000).unwrap();
    let done = e.finish(&r, &mut g, || 100_001).unwrap();
    g.completed(done, &b, || 100_001).unwrap();
    assert_eq!(g.tick(&b, || 130_001), Err(Error::Expired));
    let (mut g, _, b) = model();
    let r = request(&b, Operation::Import);
    g.approve(r, None, &b, || 2).unwrap();
    let a = g.consume_authorized(&r, &b, || 2).unwrap();
    let e = a.activate(&r, &mut g, || 2).unwrap();
    let old = e.finish(&r, &mut g, || 3).unwrap();
    g.lock();
    g.unlock(proof(&b, 4), b, || 4).unwrap();
    assert!(g.completed(old, &b, || 5).is_err());
    assert_eq!(g.tick(&b, || 30_004), Err(Error::Expired));
}
#[test]
fn rollback_and_counter_exhaustion_are_terminal_and_empty_bindings_are_rejected() {
    let (mut g, _, b) = model();
    g.tick(&b, || 10).unwrap();
    assert_eq!(g.tick(&b, || 9), Err(Error::TimeWentBackwards));
    assert_eq!(g.tick(&b, || 11), Err(Error::HardwareFault));
    assert!(Binding::from_verified_channel([0; 32], [1; 32], 1, Transport::Usb).is_err());
    let (mut g, _, b) = model();
    g.generation = u64::MAX - 1;
    assert_eq!(
        g.approve(request(&b, Operation::RevealNote), None, &b, || 2),
        Err(Error::CounterExhausted)
    );
    assert_eq!(g.tick(&b, || 3), Err(Error::HardwareFault));
}
#[test]
fn cached_protocol_pin_does_not_bypass_native_request_permissions() {
    let (mut g, db, b) = model();
    db.borrow_mut().protocol[Protocol::FidoPin as usize] = true;
    g.note_protocol_verified(&b, Protocol::FidoPin, || 2)
        .unwrap();
    let r = request(&b, Operation::FidoSign);
    db.borrow_mut().denied_digest = Some(r.digest);
    let before = db.borrow().confirms;
    assert_eq!(
        g.approve(r, Some(Protocol::FidoPin), &b, || 3),
        Err(Error::Rejected)
    );
    assert_eq!(db.borrow().confirms, before);
    db.borrow_mut().denied_digest = None;
    g.approve(r, Some(Protocol::FidoPin), &b, || 4).unwrap();
    db.borrow_mut().denied_digest = Some(r.digest);
    assert!(g.consume_authorized(&r, &b, || 5).is_err());
    assert_eq!(g.tick(&b, || 30_004), Err(Error::Expired));
}
#[test]
fn protocol_expiring_between_confirmation_and_execution_is_not_authorized() {
    let (mut g, db, b) = model();
    db.borrow_mut().protocol[Protocol::PivPin as usize] = true;
    g.note_protocol_verified(&b, Protocol::PivPin, || 2)
        .unwrap();
    complete(&mut g, &b, Operation::RevealNote, None, 20_000);
    let r = request(&b, Operation::Piv);
    g.approve(r, Some(Protocol::PivPin), &b, || 30_001).unwrap();
    assert!(g.consume_authorized(&r, &b, || 30_002).is_err());
    assert!(!db.borrow().protocol[Protocol::PivPin as usize]);
}
#[test]
fn failed_protocol_verification_revokes_only_its_role_unless_in_use() {
    let (mut g, db, b) = model();
    for p in [Protocol::PivPin, Protocol::FidoPin] {
        db.borrow_mut().protocol[p as usize] = true;
        g.note_protocol_verified(&b, p, || 2).unwrap();
    }
    g.protocol_failed(Protocol::PivPin);
    assert!(!db.borrow().protocol[Protocol::PivPin as usize]);
    complete(&mut g, &b, Operation::FidoSign, Some(Protocol::FidoPin), 3);
    let r = request(&b, Operation::FidoSign);
    g.approve(r, Some(Protocol::FidoPin), &b, || 4).unwrap();
    let a = g.consume_authorized(&r, &b, || 4).unwrap();
    let mut e = a.activate(&r, &mut g, || 4).unwrap();
    g.protocol_failed(Protocol::FidoPin);
    assert!(e.check(&r, &mut g, || 5).is_err());
    assert!(!db.borrow().protocol.iter().any(|&v| v));
}
#[test]
fn in_flight_native_permission_loss_revokes_execution_without_a_host_event() {
    let (mut g, db, b) = model();
    db.borrow_mut().protocol[Protocol::FidoPin as usize] = true;
    g.note_protocol_verified(&b, Protocol::FidoPin, || 2)
        .unwrap();
    let r = request(&b, Operation::FidoSign);
    g.approve(r, Some(Protocol::FidoPin), &b, || 3).unwrap();
    let a = g.consume_authorized(&r, &b, || 3).unwrap();
    let mut e = a.activate(&r, &mut g, || 3).unwrap();
    db.borrow_mut().denied_digest = Some(r.digest);
    assert!(e.check(&r, &mut g, || 4).is_err());
    assert!(!db.borrow().protocol.iter().any(|&flag| flag));
    assert!(g.tick(&b, || 5).is_err());
}
#[test]
fn approved_long_operation_keeps_only_its_own_protocol_until_fixed_deadline() {
    let (mut g, db, b) = model();
    for p in [Protocol::PivManagement, Protocol::FidoPin] {
        db.borrow_mut().protocol[p as usize] = true;
        g.note_protocol_verified(&b, p, || 2).unwrap();
    }
    let r = request(&b, Operation::ImportKey);
    g.approve(r, Some(Protocol::PivManagement), &b, || 3)
        .unwrap();
    let a = g.consume_authorized(&r, &b, || 3).unwrap();
    let mut e = a.activate(&r, &mut g, || 3).unwrap();
    g.tick(&b, || 40_000).unwrap();
    assert!(!db.borrow().protocol[Protocol::FidoPin as usize]);
    assert!(db.borrow().protocol[Protocol::PivManagement as usize]);
    e.check(&r, &mut g, || 40_000).unwrap();
    let done = e.finish(&r, &mut g, || 40_001).unwrap();
    g.completed(done, &b, || 40_001).unwrap();
    assert_eq!(g.tick(&b, || 70_001), Err(Error::Expired));
}
#[test]
fn delayed_activation_cannot_restart_a_fixed_execution_budget() {
    let (mut g, _, b) = model();
    let r = request(&b, Operation::RevealNote);
    g.approve(r, None, &b, || 2).unwrap();
    let a = g.consume_authorized(&r, &b, || 3).unwrap();
    let mut e = a.activate(&r, &mut g, || 10_000).unwrap();
    assert_eq!(e.expires_ms(), 15_003);
    assert_eq!(e.check(&r, &mut g, || 15_003), Err(Error::Expired));
    assert_eq!(g.tick(&b, || 15_003), Err(Error::Expired));
}
