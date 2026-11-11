//! Public synthetic PIN/SID fixtures only. No owner profile, key or device.
extern crate std;
use super::*;
use std::{cell::RefCell, collections::VecDeque, rc::Rc, vec, vec::Vec};

const SID: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
const PIN: [u8; 6] = [9, 2, 7, 0, 4, 6];
enum Reply {
    Body(Vec<u8>, usize),
    Fail(TransportError),
}
struct Packet {
    header: [u8; 4],
    data: Vec<u8>,
    le: Option<openkeys_scp03::Le>,
}
#[derive(Default)]
struct Db {
    replies: VecDeque<Reply>,
    packets: Vec<Packet>,
    closed: bool,
    cuts: usize,
    fail_cut: bool,
}
type Shared = Rc<RefCell<Db>>;
struct Io(Shared);
impl crate::private::Sealed for Io {}
impl Exchange for Io {
    fn exchange(&mut self, command: &Command<'_>, out: &mut [u8]) -> Result<usize, TransportError> {
        let mut d = self.0.borrow_mut();
        assert!(!d.closed);
        d.packets.push(Packet {
            header: [command.class, command.instruction, command.p1, command.p2],
            data: command.data.into(),
            le: command.le,
        });
        match d.replies.pop_front().expect("Unexpected APDU or retry") {
            Reply::Body(body, n) => {
                assert!(body.len() <= out.len());
                out[..body.len()].copy_from_slice(&body);
                Ok(n)
            }
            Reply::Fail(error) => Err(error),
        }
    }
    fn close(&mut self) {
        self.0.borrow_mut().closed = true;
    }
}
struct Power(Shared);
impl PowerCut for Power {
    fn disable_se(&mut self) -> Result<(), Error> {
        let mut d = self.0.borrow_mut();
        d.cuts += 1;
        if d.fail_cut {
            Err(Error::PowerCut)
        } else {
            Ok(())
        }
    }
}
fn created() -> Reply {
    let mut response = vec![0x41, 8];
    response.extend_from_slice(&SID);
    Reply::Body(response, 10)
}
fn db(replies: Vec<Reply>) -> Shared {
    Rc::new(RefCell::new(Db {
        replies: replies.into(),
        ..Db::default()
    }))
}
fn begin(db: &Shared) -> Result<Probe<Io, Power>, Error> {
    Probe::begin(Io(db.clone()), Power(db.clone()))
}

#[test]
fn create_verify_close_match_fixed_native_wire_without_generic_forwarding() {
    let d = db(vec![
        created(),
        Reply::Body(vec![], 0),
        Reply::Body(vec![], 0),
    ]);
    let mut probe = begin(&d).unwrap();
    probe.verify_local_pin(&PIN).unwrap();
    probe.close().unwrap();
    let d = d.borrow();
    assert_eq!(d.packets.len(), 3);
    assert_eq!(d.packets[0].header, [0x80, 4, 0, 0x1b]);
    assert_eq!(d.packets[0].data, [0x41, 4, 0x4f, 0x4b, 8, 0]);
    // Independent explicit PROCESS/SESSION_ID/embedded-APDU fixture.
    assert_eq!(d.packets[1].header, [0x80, 5, 0, 0]);
    assert_eq!(
        d.packets[1].data,
        [
            0x10, 8, 1, 2, 3, 4, 5, 6, 7, 8, 0x41, 13, 0x80, 4, 0, 0x2c, 8, 0x41, 6, b'9', b'2',
            b'7', b'0', b'4', b'6'
        ]
    );
    assert_eq!(d.packets[2].header, [0x80, 5, 0, 0]);
    assert_eq!(
        d.packets[2].data,
        [0x10, 8, 1, 2, 3, 4, 5, 6, 7, 8, 0x41, 4, 0x80, 4, 0, 0x1c]
    );
    assert!(d.packets.iter().all(|p| p.le.is_none()));
    assert!(d.closed);
    assert_eq!(d.cuts, 1);
}

#[test]
fn malformed_truncated_extra_or_zero_session_id_fails_closed_without_retry() {
    let cases = [
        (vec![], 0),
        (vec![0x41, 8, 1], 3),
        (vec![0x40, 8, 1, 2, 3, 4, 5, 6, 7, 8], 10),
        (vec![0x41, 7, 1, 2, 3, 4, 5, 6, 7, 8], 10),
        (vec![0x41, 8, 0, 0, 0, 0, 0, 0, 0, 0], 10),
        (vec![0x41, 8, 1, 2, 3, 4, 5, 6, 7, 8, 0], 11),
        (vec![], usize::MAX),
    ];
    for (body, n) in cases {
        let d = db(vec![Reply::Body(body, n)]);
        assert!(matches!(begin(&d), Err(Error::Response)));
        let d = d.borrow();
        assert!(d.closed);
        assert_eq!(d.cuts, 1);
        assert_eq!(d.packets.len(), 1);
    }
}

#[test]
fn wrong_or_uncertain_verification_erases_local_state_and_cannot_be_retried() {
    for failure in [
        Reply::Fail(TransportError::Authorization),
        Reply::Fail(TransportError::Channel),
        Reply::Body(vec![0], 1),
    ] {
        let d = db(vec![created(), failure]);
        let mut p = begin(&d).unwrap();
        let error = p.verify_local_pin(&PIN).unwrap_err();
        assert!(p.sid.iter().all(|&x| x == 0));
        assert_eq!(p.verify_local_pin(&PIN), Err(Error::Closed));
        assert_eq!(p.close(), Err(error));
        assert_eq!(d.borrow().packets.len(), 2);
        assert_eq!(d.borrow().cuts, 1);
    }
}

#[test]
fn invalid_or_repeated_pin_input_is_not_an_extra_chip_verification() {
    let d = db(vec![created()]);
    let mut p = begin(&d).unwrap();
    assert_eq!(
        p.verify_local_pin(&[0, 1, 2, 3, 4, 255]),
        Err(Error::InvalidPin)
    );
    assert_eq!(d.borrow().packets.len(), 1);
    assert_eq!(d.borrow().cuts, 1);
    let d = db(vec![created(), Reply::Body(vec![], 0)]);
    let mut p = begin(&d).unwrap();
    p.verify_local_pin(&PIN).unwrap();
    assert_eq!(p.verify_local_pin(&PIN), Err(Error::RepeatedVerification));
    assert_eq!(d.borrow().packets.len(), 2);
    assert_eq!(d.borrow().cuts, 1);
}

#[test]
fn close_ack_failure_or_unreliable_power_removal_is_never_success() {
    let d = db(vec![created(), Reply::Fail(TransportError::Channel)]);
    assert_eq!(
        begin(&d).unwrap().close(),
        Err(Error::Transport(TransportError::Channel))
    );
    assert_eq!(d.borrow().packets.len(), 2);
    assert_eq!(d.borrow().cuts, 1);
    let d = db(vec![created(), Reply::Body(vec![], 0)]);
    let p = begin(&d).unwrap();
    d.borrow_mut().fail_cut = true;
    assert_eq!(p.close(), Err(Error::PowerCut));
    assert!(d.borrow().closed);
    assert!(d.borrow().cuts >= 1);
}

#[test]
fn dropping_an_unfinished_probe_cuts_power_without_a_cleanup_apdu() {
    let d = db(vec![created()]);
    let p = begin(&d).unwrap();
    drop(p);
    let d = d.borrow();
    assert!(d.closed);
    assert_eq!(d.packets.len(), 1);
    assert_eq!(d.cuts, 1);
}
