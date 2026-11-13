//! Independent synthetic applet model. No device/owner secrets or factory keys.
extern crate std;
use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    vec,
    vec::Vec,
};
const PIN: [u8; 6] = [9, 2, 7, 0, 4, 6];
const SID: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
const OWNER: [u8; 32] = [0x93; 32];

#[path = "commissioning_tests.rs"]
mod commissioning;

#[test]
fn process_envelope_uses_canonical_ber_and_inner_lc_at_independent_boundaries() {
    // Literal lengths from NXP se05x_Transform, including its >=255 extended
    // inner-Lc rule. These expected fields are not produced by the encoder.
    let cases: &[(usize, &[u8], usize)] = &[
        (0, &[0x41, 0x04, 0x80, 3, 0x0c, 9], 16),
        (122, &[0x41, 0x7f, 0x80, 3, 0x0c, 9, 0x7a], 139),
        (123, &[0x41, 0x81, 0x80, 0x80, 3, 0x0c, 9, 0x7b], 141),
        (250, &[0x41, 0x81, 0xff, 0x80, 3, 0x0c, 9, 0xfa], 268),
        (251, &[0x41, 0x82, 1, 0, 0x80, 3, 0x0c, 9, 0xfb], 270),
        (254, &[0x41, 0x82, 1, 3, 0x80, 3, 0x0c, 9, 0xfe], 273),
        (255, &[0x41, 0x82, 1, 6, 0x80, 3, 0x0c, 9, 0, 0, 0xff], 276),
        (512, &[0x41, 0x82, 2, 7, 0x80, 3, 0x0c, 9, 0, 2, 0], 533),
        (
            1994,
            &[0x41, 0x82, 7, 0xd1, 0x80, 3, 0x0c, 9, 0, 7, 0xca],
            2015,
        ),
    ];
    for &(length, fields, expected_end) in cases {
        let input = vec![0x5a; length];
        let command = Command {
            class: 0x80,
            instruction: 3,
            p1: 0x0c,
            p2: 9,
            data: &input,
            le: None,
        };
        let mut wire = [0xa5; MAX_DATA];
        let end = process_envelope(&SID, &command, &mut wire).unwrap();
        assert_eq!(end, expected_end);
        assert_eq!(&wire[..10], &[0x10, 8, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&wire[10..10 + fields.len()], fields);
        assert_eq!(&wire[10 + fields.len()..end], input);
        assert!(wire[end..].iter().all(|&byte| byte == 0));
    }
    let command = Command {
        class: 0x80,
        instruction: 3,
        p1: 0x0c,
        p2: 9,
        data: &[0x5a],
        le: Some(openkeys_scp03::Le::Extended(512)),
    };
    let mut wire = [0xa5; MAX_DATA];
    let end = process_envelope(&SID, &command, &mut wire).unwrap();
    assert_eq!(&wire[10..end], &[0x41, 8, 0x80, 3, 0x0c, 9, 0, 0, 1, 0x5a]);
}

#[test]
fn process_envelope_rejects_invalid_or_overflowing_input_before_copy() {
    for (class, sid, length) in [
        (0, SID, 0),
        (0x80, [0; 8], 0),
        (0x80, SID, 1995),
        (0x80, SID, MAX_DATA + 1),
    ] {
        let input = vec![0x5a; length];
        let command = Command {
            class,
            instruction: 3,
            p1: 0x0c,
            p2: 9,
            data: &input,
            le: None,
        };
        let mut wire = [0xa5; MAX_DATA];
        assert!(process_envelope(&sid, &command, &mut wire).is_err());
        assert!(wire.iter().all(|&byte| byte == 0));
    }
}
#[derive(Default)]
struct Db {
    spent: u64,
    credit: u64,
    erase: u64,
    native_failures: u64,
    native_verified: bool,
    native_verifies: usize,
    private: usize,
    calls: usize,
    cuts: usize,
    closes: usize,
    wrong_marker: bool,
    wrong_owner: bool,
    malformed_sid: bool,
    fail_power: bool,
    fail_at: Option<(usize, bool)>,
    packets: Vec<Vec<u8>>,
    private_time: Option<Rc<Cell<u64>>>,
    private_generation: Option<Rc<Cell<u64>>>,
    private_fault: Option<Rc<Cell<bool>>>,
    private_role: Option<Rc<Cell<bool>>>,
    private_owner: Option<Rc<Cell<[u8; 32]>>>,
}
type Shared = Rc<RefCell<Db>>;
struct Io {
    db: Shared,
    closed: bool,
}
fn private_done(db: &mut Db) {
    db.private += 1;
    if let Some(time) = &db.private_time {
        time.set(20_000);
    }
    if let Some(generation) = &db.private_generation {
        generation.set(1);
    }
    if let Some(fault) = &db.private_fault {
        fault.set(true);
    }
    if let Some(role) = &db.private_role {
        role.set(true);
    }
    if let Some(owner) = &db.private_owner {
        owner.set([0x74; 32]);
    }
}
impl private::Sealed for Io {}
fn id(data: &[u8]) -> u32 {
    assert_eq!(data[..2], [0x41, 4]);
    u32::from_be_bytes(data[2..6].try_into().unwrap())
}
impl Io {
    fn run(&self, c: &Command<'_>, db: &mut Db) -> Result<Vec<u8>, Error> {
        assert_eq!(c.class, 0x80);
        assert_eq!(c.le, None);
        if c.instruction == 5 {
            db.packets.push(c.data.to_vec());
            assert_eq!(c.data[..10], [0x10, 8, 1, 2, 3, 4, 5, 6, 7, 8]);
            assert_eq!(c.data[10], 0x41);
            let (n, at) = match c.data[11] {
                0x81 => {
                    assert!(c.data[12] >= 128);
                    (usize::from(c.data[12]), 13)
                }
                0x82 => {
                    let n = usize::from(u16::from_be_bytes([c.data[12], c.data[13]]));
                    assert!(n >= 256);
                    (n, 14)
                }
                length => {
                    assert!(length < 128);
                    (usize::from(length), 12)
                }
            };
            let inner = &c.data[at..];
            assert_eq!(inner.len(), n);
            assert_eq!(inner[0], 0x80);
            let data = if inner.len() == 4 {
                &[][..]
            } else if inner[4] == 0 {
                let n = usize::from(u16::from_be_bytes([inner[5], inner[6]]));
                assert_eq!(n, inner.len() - 7);
                &inner[7..]
            } else {
                assert_eq!(inner[4] as usize, inner.len() - 5);
                &inner[5..]
            };
            match (inner[1], inner[2], inner[3]) {
                (4, 0, 0x2c) => {
                    db.native_verifies += 1;
                    assert_eq!(data[..2], [0x41, 6]);
                    let expected = PIN.map(|x| b'0' + x);
                    if db.native_failures >= MAX_FAILURES || data[2..] != expected {
                        db.native_failures += 1;
                        return Err(Error::Authorization);
                    }
                    db.native_verified = true;
                    db.native_failures = 0;
                    Ok(vec![])
                }
                (4, 0, 0x1c) => {
                    assert!(db.native_verified);
                    db.native_verified = false;
                    Ok(vec![])
                }
                (1, 8, 0) => {
                    assert!(db.native_verified);
                    assert_eq!(id(data), CREDIT_ID);
                    assert_eq!(data[6..8], [0x43, 8]);
                    let value = u64::from_be_bytes(data[8..].try_into().unwrap());
                    assert!(value >= db.credit);
                    db.credit = value;
                    Ok(vec![])
                }
                (2, 0, 7) => {
                    assert!(db.native_verified);
                    private_done(db);
                    Ok(vec![0x41, 2, 0, 32])
                }
                (2, 0, 0) => {
                    assert!(db.native_verified);
                    private_done(db);
                    let mut reply = vec![0x41, 0x82, 2, 0];
                    reply.extend((0..512).map(|i| i as u8));
                    Ok(reply)
                }
                _ => panic!("unexpected native command"),
            }
        } else {
            let object = id(c.data);
            match (c.instruction, c.p1, c.p2) {
                (2, 0, 0x26) => Ok(vec![
                    0x41,
                    1,
                    match object {
                        PIN_ID => 0x0c,
                        MARKER_ID | OWNER_ID => 0x0b,
                        SPENT_ID | CREDIT_ID | ERASE_ID => 0x0d,
                        _ => panic!(),
                    },
                    0x42,
                    1,
                    1,
                ]),
                (2, 0, 7) => {
                    assert_ne!(object, PIN_ID, "UserID has no readable size");
                    Ok(vec![
                        0x41,
                        2,
                        0,
                        if object == MARKER_ID || object == OWNER_ID {
                            32
                        } else {
                            8
                        },
                    ])
                }
                (2, 0, 0) => {
                    if object == MARKER_ID || object == OWNER_ID {
                        assert_eq!(c.data[6..], [0x42, 2, 0, 0, 0x43, 2, 0, 32]);
                        let mut out = vec![0x41, 32];
                        out.extend(if object == MARKER_ID { marker() } else { OWNER });
                        if (object == MARKER_ID && db.wrong_marker)
                            || (object == OWNER_ID && db.wrong_owner)
                        {
                            out[2] ^= 1;
                        }
                        Ok(out)
                    } else {
                        assert_eq!(c.data.len(), 6);
                        let value = match object {
                            SPENT_ID => db.spent,
                            CREDIT_ID => db.credit,
                            ERASE_ID => db.erase,
                            _ => panic!(),
                        };
                        let mut out = vec![0x41, 8];
                        out.extend(value.to_be_bytes());
                        Ok(out)
                    }
                }
                (1, 8, 0) => {
                    assert_eq!(c.data.len(), 6, "platform SCP cannot assign retry credit");
                    match object {
                        SPENT_ID => db.spent += 1,
                        ERASE_ID => db.erase += 1,
                        _ => panic!("credit needs native auth"),
                    };
                    Ok(vec![])
                }
                (4, 0, 0x1b) => {
                    assert_eq!(object, PIN_ID);
                    db.native_verified = false;
                    let mut out = vec![0x41, 8];
                    out.extend(if db.malformed_sid { [0; 8] } else { SID });
                    Ok(out)
                }
                _ => panic!("unexpected platform command"),
            }
        }
    }
}
impl Exchange for Io {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        assert!(!self.closed);
        let mut db = self.db.borrow_mut();
        db.calls += 1;
        let call = db.calls;
        if db.fail_at == Some((call, false)) {
            return Err(Error::Channel);
        }
        let response = self.run(c, &mut db)?;
        if db.fail_at == Some((call, true)) {
            return Err(Error::Channel);
        }
        assert!(response.len() <= out.len());
        out[..response.len()].copy_from_slice(&response);
        Ok(response.len())
    }
    fn close(&mut self) {
        self.closed = true;
        self.db.borrow_mut().closes += 1;
    }
}
struct Power(Shared);
impl PowerCut for Power {
    fn disable_se(&mut self) -> Result<(), Error> {
        let mut db = self.0.borrow_mut();
        db.cuts += 1;
        if db.fail_power {
            return Err(Error::Channel);
        }
        db.native_verified = false;
        Ok(())
    }
}
fn open(db: &Shared) -> Result<NativePin<Io, Power>, Error> {
    NativePin::open(
        Io {
            db: db.clone(),
            closed: false,
        },
        Power(db.clone()),
        &OWNER,
    )
}
fn request() -> Request {
    Request {
        operation: device_policy::Operation::UnlockSession,
        digest: [1; 32],
        channel: [2; 32],
    }
}
fn private_command() -> Command<'static> {
    Command {
        class: 0x80,
        instruction: 2,
        p1: 0,
        p2: 7,
        data: &[0x41, 4, 0x4f, 0x4b, 0x10, 0],
        le: Some(openkeys_scp03::Le::Short(0)),
    }
}

#[derive(Default)]
struct LiveRoot {
    generation: Rc<Cell<u64>>,
    fault: Rc<Cell<bool>>,
    denied_role: Rc<Cell<bool>>,
    expected_owner: Option<Rc<Cell<[u8; 32]>>>,
}
impl PrivateState for LiveRoot {
    fn check_private_request(
        &mut self,
        request: &Request,
        owner: &[u8; 32],
    ) -> Result<(), device_policy::Error> {
        if self.denied_role.get()
            || request.channel != [2; 32]
            || request.digest != [1; 32]
            || *owner != self.expected_owner.as_ref().map_or(OWNER, |o| o.get())
        {
            Err(device_policy::Error::Rejected)
        } else {
            Ok(())
        }
    }
}
impl State for LiveRoot {
    fn generation(&self) -> u64 {
        self.generation.get()
    }
    fn check_security_state(&mut self) -> Result<(), device_policy::Error> {
        if self.fault.get() {
            Err(device_policy::Error::HardwareFault)
        } else {
            Ok(())
        }
    }
}
fn execution(operation: device_policy::Operation) -> Execution {
    use device_policy::{
        pin::Press,
        pin_only::{Digit, Gate, PinEntry, SecurePinVerifier, VerifiedPinRequest},
    };
    struct Local;
    impl ProtectedPin<6> for Local {
        fn reserve_attempt(&mut self, _: &Request) -> Result<(), PinError> {
            Ok(())
        }
        fn verify(&mut self, _: &Request, digits: &[u8; 6]) -> Result<(), PinError> {
            if *digits == PIN {
                Ok(())
            } else {
                Err(PinError::Incorrect)
            }
        }
    }
    struct Proof;
    impl SecurePinVerifier for Proof {
        fn check_security_state(&mut self) -> Result<(), device_policy::Error> {
            Ok(())
        }
        fn verify_device_pin(
            &mut self,
            r: &Request,
        ) -> Result<VerifiedPinRequest, device_policy::Error> {
            let mut entry = PinEntry::begin(*r, || 1).unwrap();
            for digit in PIN {
                let key = Digit::from_gpio_index(if digit == 0 { 9 } else { u32::from(digit - 1) })
                    .unwrap();
                entry.press(key, Press::Short, 1).unwrap();
            }
            Ok(entry.submit(&mut Local, || 1).unwrap())
        }
        fn confirm_on_device(
            &mut self,
            _: &VerifiedPinRequest,
        ) -> Result<(), device_policy::Error> {
            Ok(())
        }
    }
    let request = Request {
        operation,
        digest: [1; 32],
        channel: [2; 32],
    };
    let mut gate = Gate::new(Proof);
    gate.approve(request, || 1).unwrap();
    let auth = gate.consume_authorized(&request, || 1).unwrap();
    auth.activate(&request, &mut LiveRoot::default(), || 1)
        .unwrap()
}
// Test-only shorthand: every legacy wire test now obtains a real opaque
// execution from the policy gate and uses the production checked port. There
// is deliberately no production Exchange implementation on NativePin itself.
trait TestExchange {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error>;
}
impl TestExchange for NativePin<Io, Power> {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        out.fill(0);
        let scope = execution(device_policy::Operation::FidoSign);
        self.private_port(&scope, &mut LiveRoot::default(), || 1)?
            .exchange(c, out)
    }
}

#[test]
fn extended_private_read_still_requires_native_pin_and_caps_its_reply() {
    let db = Shared::default();
    let mut pin = open(&db).unwrap();
    let mut command = private_command();
    command.p2 = 0;
    command.le = Some(openkeys_scp03::Le::Extended(1024));
    let mut out = [0xa5; 1024];
    let calls = db.borrow().calls;
    assert_eq!(pin.exchange(&command, &mut out), Err(Error::Authorization));
    assert_eq!(db.borrow().calls, calls);
    assert!(out.iter().all(|&byte| byte == 0));
    pin.reserve_attempt(&request()).unwrap();
    pin.verify(&request(), &PIN).unwrap();
    assert_eq!(pin.exchange(&command, &mut out), Ok(516));
    assert_eq!(&out[..4], &[0x41, 0x82, 2, 0]);
    for (index, &byte) in out[4..516].iter().enumerate() {
        assert_eq!(byte, index as u8);
    }
    command.le = Some(openkeys_scp03::Le::Extended(515));
    assert_eq!(pin.exchange(&command, &mut out), Err(Error::Schema));
    assert!(out.iter().all(|&byte| byte == 0));
    assert!(!db.borrow().native_verified);
    assert!(db.borrow().cuts > 0);
}

#[test]
fn verified_session_cannot_send_oversized_native_envelope() {
    let db = Shared::default();
    let mut pin = open(&db).unwrap();
    pin.reserve_attempt(&request()).unwrap();
    pin.verify(&request(), &PIN).unwrap();
    let mut command = private_command();
    command.data = &[0x5a; 1995];
    let mut out = [0xa5; 16];
    let calls = db.borrow().calls;
    assert_eq!(pin.exchange(&command, &mut out), Err(Error::Schema));
    assert_eq!(db.borrow().calls, calls);
    assert!(out.iter().all(|&byte| byte == 0));
    assert!(!db.borrow().native_verified);
    assert!(db.borrow().cuts > 0);
}

#[test]
fn private_operations_require_both_native_comparison_and_durable_credit() {
    let db = Shared::default();
    let mut pin = open(&db).unwrap();
    let r = request();
    let calls = db.borrow().calls;
    let mut out = [0xaa; 16];
    assert_eq!(
        pin.exchange(&private_command(), &mut out),
        Err(Error::Authorization)
    );
    assert_eq!(out, [0; 16]);
    assert_eq!(db.borrow().calls, calls);
    pin.reserve_attempt(&r).unwrap();
    pin.verify(&r, &PIN).unwrap();
    assert_eq!((db.borrow().spent, db.borrow().credit), (1, 1));
    assert_eq!(pin.exchange(&private_command(), &mut out).unwrap(), 4);
    assert_eq!(db.borrow().private, 1);
    // A borrowed provider's normal release preserves the native session, but
    // an explicit fault/revocation cuts it. Root enforces the 30-second lease.
    let scope = execution(device_policy::Operation::FidoSign);
    let mut root = LiveRoot::default();
    {
        let mut port = pin.private_port(&scope, &mut root, || 1).unwrap();
        Exchange::release(&mut &mut port);
    }
    assert!(db.borrow().native_verified);
    pin.revoke().unwrap();
    assert!(!db.borrow().native_verified);
    assert_eq!(
        pin.exchange(&private_command(), &mut out),
        Err(Error::Authorization)
    );
    let packets = &db.borrow().packets;
    assert_eq!(
        packets[0],
        [
            0x10, 8, 1, 2, 3, 4, 5, 6, 7, 8, 0x41, 13, 0x80, 4, 0, 0x2c, 8, 0x41, 6, b'9', b'2',
            b'7', b'0', b'4', b'6'
        ]
    );
}

#[test]
fn eleventh_correct_succeeds_but_eleventh_wrong_latches_erase_on_recovery() {
    for correct in [false, true] {
        let db = Shared::default();
        for _ in 0..10 {
            let mut pin = open(&db).unwrap();
            pin.reserve_attempt(&request()).unwrap();
            assert_eq!(pin.verify(&request(), &[0; 6]), Err(PinError::Storage));
        }
        let mut pin = open(&db).unwrap();
        pin.reserve_attempt(&request()).unwrap();
        if correct {
            pin.verify(&request(), &PIN).unwrap();
            assert_eq!(db.borrow().credit, 11);
            drop(pin);
            assert!(open(&db).is_ok());
        } else {
            assert!(pin.verify(&request(), &[0; 6]).is_err());
            drop(pin);
            assert!(matches!(open(&db), Err(Error::EraseRequired)));
            assert_eq!(db.borrow().erase, 1);
            assert_eq!(db.borrow().credit, 0);
        }
    }
}

#[test]
fn cancellation_precharges_nothing_and_ambiguous_final_guess_cannot_be_refunded() {
    let db = Shared::default();
    {
        let _pin = open(&db).unwrap();
    }
    assert_eq!(db.borrow().spent, 0);
    db.borrow_mut().spent = 10;
    {
        let mut pin = open(&db).unwrap();
        pin.reserve_attempt(&request()).unwrap();
    }
    assert!(matches!(open(&db), Err(Error::EraseRequired)));
    assert_eq!(db.borrow().erase, 1);
}

#[test]
fn every_exchange_cut_prevents_private_output_and_never_replays_a_guess() {
    let baseline = Shared::default();
    {
        let mut pin = open(&baseline).unwrap();
        pin.reserve_attempt(&request()).unwrap();
        pin.verify(&request(), &PIN).unwrap();
    }
    let calls = baseline.borrow().calls;
    for at in 1..=calls {
        for after in [false, true] {
            let db = Shared::default();
            db.borrow_mut().fail_at = Some((at, after));
            if let Ok(mut pin) = open(&db) {
                let result = pin
                    .reserve_attempt(&request())
                    .and_then(|_| pin.verify(&request(), &PIN));
                assert!(result.is_err());
                let before = db.borrow().calls;
                assert!(pin.exchange(&private_command(), &mut [0; 16]).is_err());
                assert_eq!(db.borrow().calls, before);
            }
            assert_eq!(db.borrow().private, 0);
            assert!(!db.borrow().native_verified);
            assert!(db.borrow().native_verifies <= 1);
            assert!(db.borrow().credit <= db.borrow().spent);
        }
    }
}

#[test]
fn wrong_request_marker_sid_and_nested_session_commands_fail_closed() {
    let db = Shared::default();
    db.borrow_mut().wrong_marker = true;
    assert!(matches!(open(&db), Err(Error::Schema)));
    let db = Shared::default();
    let mut pin = open(&db).unwrap();
    pin.reserve_attempt(&request()).unwrap();
    let mut other = request();
    other.digest[0] ^= 1;
    assert!(pin.verify(&other, &PIN).is_err());
    assert_eq!(db.borrow().native_verifies, 0);
    let db = Shared::default();
    db.borrow_mut().malformed_sid = true;
    let mut pin = open(&db).unwrap();
    pin.reserve_attempt(&request()).unwrap();
    assert!(pin.verify(&request(), &PIN).is_err());
    assert_eq!(db.borrow().credit, 0);
    let db = Shared::default();
    let mut pin = open(&db).unwrap();
    pin.reserve_attempt(&request()).unwrap();
    pin.verify(&request(), &PIN).unwrap();
    let mut command = private_command();
    command.instruction = 5;
    assert_eq!(
        pin.exchange(&command, &mut [0; 16]),
        Err(Error::Authorization)
    );
    assert!(!db.borrow().native_verified);
}

#[test]
fn immutable_owner_policy_denies_platform_other_users_and_reports_power_failure() {
    // Independent Python marker: 9c864b82dd3331505fabdedefbb975d394109a3e19514259c167a4e30d165334
    assert_eq!(
        marker(),
        [
            0x9c, 0x86, 0x4b, 0x82, 0xdd, 0x33, 0x31, 0x50, 0x5f, 0xab, 0xde, 0xde, 0xfb, 0xb9,
            0x75, 0xd3, 0x94, 0x10, 0x9a, 0x3e, 0x19, 0x51, 0x42, 0x59, 0xc1, 0x67, 0xa4, 0xe3,
            0x0d, 0x16, 0x53, 0x34,
        ]
    );
    assert_eq!(
        owner_policy(0x1024_0000),
        [
            0x11, 18, 8, 0x4f, 0x4b, 8, 1, 0x10, 0x26, 0, 0, 8, 0, 0, 0, 0, 0x20, 0, 0, 0
        ]
    );
    assert_ne!(marker(), crate::pin_only::marker());
    let db = Shared::default();
    let mut pin = open(&db).unwrap();
    db.borrow_mut().fail_power = true;
    assert_eq!(pin.revoke(), Err(Error::Channel));
    assert!(pin.reserve_attempt(&request()).is_err());
    db.borrow_mut().fail_power = false;
    pin.revoke().unwrap();
    assert!(db.borrow().cuts >= 2);
}

#[test]
fn old_or_missing_root_owner_binding_is_rejected_before_any_pin_guess() {
    for expected in [[0; 32], [0x74; 32]] {
        let db = Shared::default();
        let result = NativePin::open(
            Io {
                db: db.clone(),
                closed: false,
            },
            Power(db.clone()),
            &expected,
        );
        assert!(matches!(result, Err(Error::Authorization)));
        assert_eq!(
            (
                db.borrow().spent,
                db.borrow().credit,
                db.borrow().native_verifies
            ),
            (0, 0, 0)
        );
        assert!(db.borrow().cuts > 0);
    }
    let db = Shared::default();
    db.borrow_mut().wrong_owner = true;
    assert!(matches!(open(&db), Err(Error::Authorization)));
    assert_eq!(db.borrow().native_verifies, 0);
}

#[test]
fn changing_pin_latches_before_new_pin_and_revokes_even_on_lost_ack() {
    struct Root;
    impl device_policy::execution::State for Root {
        fn generation(&self) -> u64 {
            0
        }
        fn check_security_state(&mut self) -> Result<(), device_policy::Error> {
            Ok(())
        }
    }
    for lost_ack in [false, true] {
        let db = Shared::default();
        let mut pin = open(&db).unwrap();
        pin.reserve_attempt(&request()).unwrap();
        pin.verify(&request(), &PIN).unwrap();
        // begin_pin_change reads erase then writes it. Lose only that write's
        // ACK AFTER persistence: the next open must still refuse old access.
        if lost_ack {
            let at = db.borrow().calls + 2;
            db.borrow_mut().fail_at = Some((at, true));
        }
        let auth = crate::keys::tests::auth(device_policy::Operation::ChangePin);
        let result = pin.begin_pin_change(auth, &mut Root, || 1);
        assert_eq!(result.is_err(), lost_ack);
        assert_eq!(db.borrow().erase, 1);
        assert!(!db.borrow().native_verified);
        drop(pin);
        db.borrow_mut().fail_at = None;
        assert!(matches!(open(&db), Err(Error::EraseRequired)));
    }
}

#[test]
fn expired_generation_changed_or_faulted_scope_cannot_open_private_port() {
    for mode in 0..5 {
        let db = Shared::default();
        let mut pin = open(&db).unwrap();
        pin.reserve_attempt(&request()).unwrap();
        pin.verify(&request(), &PIN).unwrap();
        let scope = execution(device_policy::Operation::FidoSign);
        let mut root = LiveRoot::default();
        let now = Cell::new(1);
        match mode {
            0 => now.set(scope.expires_ms()),
            1 => root.generation.set(1),
            2 => root.fault.set(true),
            3 => root.denied_role.set(true),
            _ => root.expected_owner = Some(Rc::new(Cell::new([0x74; 32]))),
        }
        let calls = db.borrow().calls;
        assert!(pin.private_port(&scope, &mut root, || now.get()).is_err());
        assert_eq!(db.borrow().calls, calls);
        assert!(!db.borrow().native_verified);
        assert!(db.borrow().cuts > 0);
        let expected = *scope.request();
        assert!(scope.finish(&expected, &mut root, || now.get()).is_err());
    }
}

#[test]
fn a_fault_or_revocation_during_native_io_wipes_output_and_the_execution() {
    for mode in 0..5 {
        let db = Shared::default();
        let mut pin = open(&db).unwrap();
        pin.reserve_attempt(&request()).unwrap();
        pin.verify(&request(), &PIN).unwrap();
        let scope = execution(device_policy::Operation::FidoSign);
        let mut root = LiveRoot::default();
        let now = Rc::new(Cell::new(1));
        match mode {
            0 => db.borrow_mut().private_time = Some(now.clone()),
            1 => db.borrow_mut().private_generation = Some(root.generation.clone()),
            2 => db.borrow_mut().private_fault = Some(root.fault.clone()),
            3 => db.borrow_mut().private_role = Some(root.denied_role.clone()),
            _ => {
                let owner = Rc::new(Cell::new(OWNER));
                root.expected_owner = Some(owner.clone());
                db.borrow_mut().private_owner = Some(owner);
            }
        }
        let mut out = [0xa5; 16];
        {
            let mut port = pin.private_port(&scope, &mut root, || now.get()).unwrap();
            assert_eq!(
                port.exchange(&private_command(), &mut out),
                Err(Error::Authorization)
            );
            let calls = db.borrow().calls;
            assert!(port.exchange(&private_command(), &mut out).is_err());
            assert_eq!(db.borrow().calls, calls);
        }
        assert_eq!(out, [0; 16]);
        assert_eq!(db.borrow().private, 1);
        assert!(!db.borrow().native_verified);
        assert!(
            scope
                .check(scope.request(), &mut root, || now.get())
                .is_err()
        );
    }
}

#[test]
fn normal_release_preserves_pin_but_does_not_extend_execution_or_hide_cancellation() {
    let db = Shared::default();
    let mut pin = open(&db).unwrap();
    pin.reserve_attempt(&request()).unwrap();
    pin.verify(&request(), &PIN).unwrap();
    let scope = execution(device_policy::Operation::FidoSign);
    let borrowed_auth = scope.authorization();
    let mut root = LiveRoot::default();
    let now = Cell::new(10);
    {
        let mut port = pin.private_port(&scope, &mut root, || now.get()).unwrap();
        assert_eq!(borrowed_auth.request(), scope.request());
        assert_eq!(port.exchange(&private_command(), &mut [0; 16]), Ok(4));
        now.set(100);
        assert_eq!(port.exchange(&private_command(), &mut [0; 16]), Ok(4));
        Exchange::release(&mut port);
    }
    assert!(db.borrow().native_verified);
    assert_eq!(scope.checked_at_ms(), 100);
    scope.cancel();
    let calls = db.borrow().calls;
    assert!(pin.private_port(&scope, &mut root, || 100).is_err());
    assert_eq!(db.borrow().calls, calls);
    assert!(!db.borrow().native_verified);
}

#[test]
fn reset_update_and_unlock_grants_are_not_general_private_ports() {
    use device_policy::Operation as O;
    for operation in [
        O::FactoryReset,
        O::ChangePin,
        O::UnlockSession,
        O::FirmwareUpdate,
    ] {
        let db = Shared::default();
        let mut pin = open(&db).unwrap();
        pin.reserve_attempt(&request()).unwrap();
        pin.verify(&request(), &PIN).unwrap();
        let scope = execution(operation);
        let calls = db.borrow().calls;
        assert!(
            pin.private_port(&scope, &mut LiveRoot::default(), || 1)
                .is_err()
        );
        assert_eq!(db.borrow().calls, calls);
        assert!(!db.borrow().native_verified);
    }
}
