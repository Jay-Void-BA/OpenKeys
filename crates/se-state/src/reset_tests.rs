extern crate std;
use super::*;
use std::{cell::RefCell, rc::Rc, vec, vec::Vec};

const PROFILE: [u8; 32] = [0x71; 32];
const EPOCH: Epoch = Epoch {
    generation: 2,
    owner: [0x72; 32],
    profile: PROFILE,
    chip_identity: [0x73; 32],
};
const OBJECTS: [inventory::ExpectedObject; 1] = [inventory::ExpectedObject {
    id: 0x7fff_1001,
    kind: 0x0b,
}];
const SECRET: [u8; 16] = [0x39; 16]; // synthetic, cfg(test) ONLY.
#[derive(Default)]
struct Model {
    calls: usize,
    commands: Vec<(u8, u8, Vec<u8>)>,
    closes: usize,
    cuts: usize,
    off: bool,
    deny_power: bool,
    fail_at: Option<(usize, bool)>,
    bad_type: bool,
    bad_sid: bool,
    verify_error: bool,
    delete_reply: u8,
    actually_delete: bool,
    private_object: bool,
    channel_epoch: u64,
    inventory_queries: usize,
}
type Shared = Rc<RefCell<Model>>;
struct Io {
    db: Shared,
    epoch: u64,
    closed: bool,
}
impl crate::private::Sealed for Io {}
impl Exchange for Io {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        assert!(!self.closed, "old closed session reused");
        let mut m = self.db.borrow_mut();
        assert!(!m.off && self.epoch == m.channel_epoch);
        m.calls += 1;
        let call = m.calls;
        m.commands.push((c.instruction, c.p2, c.data.to_vec()));
        assert_eq!((c.class, c.p1), (0x80, 0));
        if m.fail_at == Some((call, false)) {
            return Err(Error::Channel);
        }
        let reply = match (c.instruction, c.p2, c.data) {
            (2, 0x26, [0x41, 4, 0x7f, 0xff, 2, 5]) => {
                assert_eq!(c.le, None);
                vec![0x41, 1, if m.bad_type { 0x11 } else { 0x0c }, 0x42, 1, 1]
            }
            (4, 0x1b, [0x41, 4, 0x7f, 0xff, 2, 5]) => {
                assert_eq!(c.le, None);
                if m.bad_sid {
                    vec![0x41, 8, 0, 0, 0, 0, 0, 0, 0, 0]
                } else {
                    vec![0x41, 8, 1, 2, 3, 4, 5, 6, 7, 8]
                }
            }
            (5, 0, data) => {
                assert_eq!(c.le, None);
                let mut verify = vec![
                    0x10, 8, 1, 2, 3, 4, 5, 6, 7, 8, 0x41, 0x17, 0x80, 4, 0, 0x2c, 0x12, 0x41, 0x10,
                ];
                verify.extend_from_slice(&SECRET);
                let delete = [0x10, 8, 1, 2, 3, 4, 5, 6, 7, 8, 0x41, 4, 0x80, 4, 0, 0x2a];
                if data.len() == verify.len() && data[..19] == verify[..19] {
                    if m.verify_error || data[19..] != SECRET {
                        return Err(Error::Channel);
                    }
                    vec![]
                } else {
                    assert_eq!(data, delete);
                    if m.actually_delete {
                        m.private_object = false;
                    }
                    match m.delete_reply {
                        1 => return Err(Error::Channel), // strict SCP rejects bare9000/no R-MAC.
                        2 => vec![0x90, 0], // unexpected body is also not completion evidence.
                        3 => return Ok(out.len() + 1),
                        _ => vec![],
                    }
                }
            }
            (2, 0x25, [0x41, 2, 0, 0, 0x42, 1, 0xff]) => {
                assert_eq!(c.le, Some(openkeys_scp03::Le::Extended(512)));
                m.inventory_queries += 1;
                let mut value = vec![
                    0x41,
                    1,
                    1,
                    0x42,
                    if m.private_object { 8 } else { 4 },
                    0x7f,
                    0xff,
                    0x10,
                    1,
                ];
                if m.private_object {
                    value.extend_from_slice(&[0x4f, 0x4b, 0x08, 0x02]);
                }
                value
            }
            (2, 0x26, [0x41, 4, 0x7f, 0xff, 0x10, 1]) => vec![0x41, 1, 0x0b, 0x42, 1, 1],
            _ => panic!("unexpected reset command"),
        };
        out[..reply.len()].copy_from_slice(&reply);
        if m.fail_at == Some((call, true)) {
            return Err(Error::Channel);
        }
        Ok(reply.len())
    }
    fn close(&mut self) {
        self.closed = true;
        self.db.borrow_mut().closes += 1;
    }
}
struct Power(Shared);
impl PowerCut for Power {
    fn disable_se(&mut self) -> Result<(), Error> {
        let mut m = self.0.borrow_mut();
        m.cuts += 1;
        if m.deny_power {
            return Err(Error::Channel);
        }
        m.off = true;
        Ok(())
    }
}
struct TestRoot {
    db: Shared,
    epoch: Epoch,
    calls: usize,
    fail_at: Option<usize>,
    credential: [u8; 16],
    credential_reads: usize,
}
impl Root for TestRoot {
    fn check_pending(&mut self, epoch: &Epoch, stage: Stage) -> Result<(), Error> {
        self.calls += 1;
        if self.fail_at == Some(self.calls) || *epoch != self.epoch {
            return Err(Error::Authorization);
        }
        let m = self.db.borrow();
        match stage {
            Stage::PoweredOff if !m.off || m.cuts == 0 => return Err(Error::Authorization),
            Stage::FreshInventory if m.off || m.cuts == 0 || m.channel_epoch < 2 => {
                return Err(Error::Authorization);
            }
            Stage::Authentication | Stage::DeleteRequest | Stage::AfterDeleteRequest if m.off => {
                return Err(Error::Authorization);
            }
            _ => {}
        }
        Ok(())
    }
    fn reset_credential(&mut self, epoch: &Epoch) -> Result<Zeroizing<[u8; 16]>, Error> {
        assert_eq!(*epoch, self.epoch);
        self.credential_reads += 1;
        Ok(Zeroizing::new(self.credential))
    }
}
fn fixture() -> (Shared, TestRoot) {
    let db = Rc::new(RefCell::new(Model {
        actually_delete: true,
        private_object: true,
        channel_epoch: 1,
        ..Model::default()
    }));
    let root = TestRoot {
        db: db.clone(),
        epoch: EPOCH,
        calls: 0,
        fail_at: None,
        credential: SECRET,
        credential_reads: 0,
    };
    (db, root)
}
fn attempt(db: &Shared, root: &mut TestRoot) -> Result<Pending, Error> {
    Attempt::new(
        Io {
            db: db.clone(),
            epoch: 1,
            closed: false,
        },
        Power(db.clone()),
        EPOCH,
    )
    .issue(root)
}
fn observe(pending: Pending, db: &Shared, root: &mut TestRoot) -> Result<Observation, Error> {
    {
        let mut m = db.borrow_mut();
        m.off = false;
        m.channel_epoch = 2;
    }
    let baseline = Baseline::from_qualified_profile(&OBJECTS, PROFILE).unwrap();
    pending.observe(
        Io {
            db: db.clone(),
            epoch: 2,
            closed: false,
        },
        Power(db.clone()),
        root,
        &baseline,
    )
}
#[test]
fn literal_reset_only_userid_then_one_delete_request_and_cold_fresh_inventory() {
    let (db, mut root) = fixture();
    let pending = attempt(&db, &mut root).unwrap();
    assert_eq!(pending.delivery(), Delivery::AuthenticatedEmptyResponse);
    assert_eq!(*pending.epoch(), EPOCH);
    assert_eq!(root.credential_reads, 1);
    assert_eq!(db.borrow().calls, 4);
    assert_eq!(db.borrow().cuts, 1);
    assert!(db.borrow().off);
    let observation = observe(pending, &db, &mut root).unwrap();
    assert_eq!(*observation.epoch(), EPOCH);
    assert_eq!(observation.inventory().object_count(), 1);
    assert_eq!(db.borrow().inventory_queries, 1);
    assert_eq!(db.borrow().cuts, 2);
    assert!(db.borrow().off);
}
#[test]
fn bare_untrusted_or_lost_delete_ack_is_only_pending_and_never_retried() {
    for reply in 1..=3 {
        let (db, mut root) = fixture();
        db.borrow_mut().delete_reply = reply;
        let pending = attempt(&db, &mut root).unwrap();
        assert_eq!(pending.delivery(), Delivery::Uncertain);
        assert_eq!(db.borrow().calls, 4);
        assert!(db.borrow().off);
        observe(pending, &db, &mut root).unwrap();
        assert_eq!(db.borrow().calls, 6);
    }
    for after in [false, true] {
        let (db, mut root) = fixture();
        db.borrow_mut().fail_at = Some((4, after));
        let pending = attempt(&db, &mut root).unwrap();
        assert_eq!(pending.delivery(), Delivery::Uncertain);
        db.borrow_mut().fail_at = None;
        let result = observe(pending, &db, &mut root);
        assert_eq!(result.is_ok(), after);
        assert!(db.borrow().off);
    }
}
#[test]
fn no_delete_when_identity_session_credential_or_authenticated_verify_fails() {
    for fault in 0..6 {
        let (db, mut root) = fixture();
        match fault {
            0 => db.borrow_mut().bad_type = true,
            1 => db.borrow_mut().bad_sid = true,
            2 => db.borrow_mut().verify_error = true,
            3 => root.credential = [0; 16],
            4 => root.credential = [0xff; 16],
            _ => root.credential = [0x38; 16],
        }
        assert!(attempt(&db, &mut root).is_err());
        let m = db.borrow();
        assert!(m.private_object && m.off && m.cuts >= 1);
        assert!(m.calls <= 3);
        assert!(
            m.commands
                .iter()
                .all(|(_, _, data)| !data.ends_with(&[0x80, 4, 0, 0x2a]))
        );
    }
    for after in [false, true] {
        for at in 1..=3 {
            let (db, mut root) = fixture();
            db.borrow_mut().fail_at = Some((at, after));
            assert!(attempt(&db, &mut root).is_err());
            assert_eq!(db.borrow().calls, at);
            assert!(db.borrow().private_object && db.borrow().off);
        }
    }
}
#[test]
fn every_root_checkpoint_failure_and_unconfirmed_power_cut_keeps_reset_locked() {
    let (db, mut baseline_root) = fixture();
    attempt(&db, &mut baseline_root).unwrap();
    for at in 1..=baseline_root.calls {
        let (db, mut root) = fixture();
        root.fail_at = Some(at);
        assert!(attempt(&db, &mut root).is_err());
        assert!(db.borrow().off);
        assert!(db.borrow().calls <= 4);
    }
    let (db, mut root) = fixture();
    db.borrow_mut().deny_power = true;
    assert!(attempt(&db, &mut root).is_err());
    assert!(db.borrow().cuts >= 1);
    assert_eq!(db.borrow().calls, 4);
}
#[test]
fn old_objects_wrong_profile_stale_channel_or_changed_epoch_are_not_blank_evidence() {
    for fault in 0..4 {
        let (db, mut root) = fixture();
        let pending = attempt(&db, &mut root).unwrap();
        let result = match fault {
            0 => {
                db.borrow_mut().private_object = true;
                observe(pending, &db, &mut root)
            }
            1 => {
                root.epoch.generation += 1;
                observe(pending, &db, &mut root)
            }
            2 => {
                db.borrow_mut().off = false;
                let baseline = Baseline::from_qualified_profile(&OBJECTS, PROFILE).unwrap();
                pending.observe(
                    Io {
                        db: db.clone(),
                        epoch: 1,
                        closed: false,
                    },
                    Power(db.clone()),
                    &mut root,
                    &baseline,
                )
            }
            _ => {
                let baseline = Baseline::from_qualified_profile(&OBJECTS, [0x99; 32]).unwrap();
                pending.observe(
                    Io {
                        db: db.clone(),
                        epoch: 2,
                        closed: false,
                    },
                    Power(db.clone()),
                    &mut root,
                    &baseline,
                )
            }
        };
        assert!(result.is_err());
        assert!(db.borrow().off);
    }
}
#[test]
fn pending_resume_requires_committed_exact_epoch_and_actual_cold_cut() {
    let (db, mut root) = fixture();
    assert!(Pending::resume(EPOCH, &mut root).is_err());
    db.borrow_mut().off = true;
    db.borrow_mut().cuts = 1;
    let pending = Pending::resume(EPOCH, &mut root).unwrap();
    assert_eq!(pending.delivery(), Delivery::Uncertain);
    // No DeleteAll was issued; lingering private objects still reject observe.
    assert!(observe(pending, &db, &mut root).is_err());
    let mut epoch = EPOCH;
    epoch.generation = 1;
    assert!(Pending::resume(epoch, &mut root).is_err());
}

#[test]
fn every_fresh_inventory_io_and_root_fault_yields_no_observation_and_cuts_power() {
    let (db, mut baseline_root) = fixture();
    let pending = attempt(&db, &mut baseline_root).unwrap();
    db.borrow_mut().calls = 0;
    baseline_root.calls = 0;
    observe(pending, &db, &mut baseline_root).unwrap();
    let io_calls = db.borrow().calls;
    let root_calls = baseline_root.calls;
    assert_eq!(io_calls, 2);
    for after in [false, true] {
        for at in 1..=io_calls {
            let (db, mut root) = fixture();
            let pending = attempt(&db, &mut root).unwrap();
            db.borrow_mut().calls = 0;
            db.borrow_mut().fail_at = Some((at, after));
            assert!(observe(pending, &db, &mut root).is_err());
            assert!(db.borrow().off);
            assert_eq!(db.borrow().calls, at);
        }
    }
    for at in 1..=root_calls {
        let (db, mut root) = fixture();
        let pending = attempt(&db, &mut root).unwrap();
        root.calls = 0;
        root.fail_at = Some(at);
        assert!(observe(pending, &db, &mut root).is_err());
        assert!(db.borrow().off);
    }
    let (db, mut root) = fixture();
    let pending = attempt(&db, &mut root).unwrap();
    db.borrow_mut().deny_power = true;
    assert!(observe(pending, &db, &mut root).is_err());
    assert!(db.borrow().closes >= 2);
}
