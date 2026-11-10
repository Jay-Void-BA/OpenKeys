//! Public synthetic fixtures only; never provisioning keys or a factory PIN.
extern crate std;
use super::*;
use crate::private;
use std::{cell::RefCell, rc::Rc};

const TEST_PIN: [u8; PIN_DIGITS] = [9, 2, 7, 0, 4, 6];
// Independent Python hashlib/hmac RFC5869 fixture: salt=00..1f,
// IKM=a5*32, info=KDF_INFO || TEST_PIN bytes, L=32. Not a deployed key.
const EXPECTED: [u8; 32] = [
    0xd3, 0x30, 0xad, 0xad, 0x81, 0x50, 0x83, 0x90, 0xb6, 0x93, 0x58, 0x16, 0x85, 0xbc, 0xf1, 0xcb,
    0xe3, 0xe1, 0x1d, 0x6f, 0xd7, 0xc8, 0xe2, 0xca, 0x62, 0x60, 0x9c, 0x4c, 0x49, 0x53, 0xb7, 0xcf,
];
#[derive(Default)]
struct Db {
    spent: u64,
    erase: u64,
    fail_erase: bool,
    credit: u64,
    closed: usize,
    kdf_calls: usize,
    wrong_type: bool,
    bad_reply: bool,
    fail_debit: bool,
    fail_credit: bool,
    credit_writes: usize,
}
type Shared = Rc<RefCell<Db>>;
struct Link(Shared);
impl private::Sealed for Link {}
impl Exchange for Link {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        let mut d = self.0.borrow_mut();
        assert_eq!(c.class, 0x80);
        assert_eq!(c.le, Some(0));
        assert_eq!(c.data[..2], [0x41, 4]);
        let id = u32::from_be_bytes(c.data[2..6].try_into().unwrap());
        let (kind, size) = match id {
            SPENT_ID | CREDIT_ID | ERASE_ID => (0x0d, 8),
            SALT_ID | VERIFIER_ID | MARKER_ID => (0x0b, 32),
            PEPPER_ID => (0x11, 32),
            _ => panic!("unexpected object"),
        };
        let reply: std::vec::Vec<u8> = match (c.instruction, c.p1, c.p2) {
            (2, 0, 0x26) => std::vec![0x41, 1, if d.wrong_type { 0 } else { kind }, 0x42, 1, 1],
            (2, 0, 7) => std::vec![0x41, 2, 0, size],
            (2, 0, 0) => {
                if matches!(id, SPENT_ID | CREDIT_ID | ERASE_ID) {
                    assert_eq!(c.data.len(), 6);
                    let mut v = std::vec![0x41, 8];
                    v.extend_from_slice(
                        &match id {
                            SPENT_ID => d.spent,
                            CREDIT_ID => d.credit,
                            ERASE_ID => d.erase,
                            _ => unreachable!(),
                        }
                        .to_be_bytes(),
                    );
                    v
                } else {
                    assert_ne!(id, PEPPER_ID, "pepper is never read");
                    assert_eq!(&c.data[6..], &[0x43, 2, 0, 32]);
                    let mut v = std::vec![0x41, 32];
                    match id {
                        SALT_ID => v.extend(0_u8..32),
                        VERIFIER_ID => v.extend_from_slice(&EXPECTED),
                        MARKER_ID => v.extend_from_slice(&marker()),
                        _ => panic!(),
                    };
                    v
                }
            }
            (1, 8, 0) => {
                if id == SPENT_ID {
                    assert_eq!(c.data.len(), 6);
                    d.spent += 1;
                    if d.fail_debit {
                        return Err(Error::Channel);
                    }
                } else if id == ERASE_ID {
                    assert_eq!(c.data.len(), 6);
                    d.erase += 1;
                    if d.fail_erase {
                        return Err(Error::Channel);
                    }
                } else {
                    assert_eq!(id, CREDIT_ID);
                    assert_eq!(&c.data[6..8], &[0x43, 8]);
                    d.credit = u64::from_be_bytes(c.data[8..].try_into().unwrap());
                    d.credit_writes += 1;
                    if d.fail_credit {
                        return Err(Error::Channel);
                    }
                }
                std::vec![]
            }
            (3, 0, 0x2d) => {
                assert_eq!(id, PEPPER_ID);
                assert_eq!(&c.data[6..11], &[0x42, 1, 4, 0x43, 32]);
                assert_eq!(&c.data[11..43], &(0_u8..32).collect::<std::vec::Vec<_>>());
                assert_eq!(
                    &c.data[43..45],
                    &[0x44, (KDF_INFO.len() + PIN_DIGITS) as u8]
                );
                assert_eq!(&c.data[45..45 + KDF_INFO.len()], KDF_INFO);
                let digits = &c.data[45 + KDF_INFO.len()..45 + KDF_INFO.len() + PIN_DIGITS];
                assert_eq!(
                    &c.data[45 + KDF_INFO.len() + PIN_DIGITS..],
                    &[0x45, 2, 0, 32]
                );
                d.kdf_calls += 1;
                if d.bad_reply {
                    std::vec![0x41, 32, 1]
                } else {
                    let mut v = std::vec![0x41, 32];
                    v.extend_from_slice(if digits == TEST_PIN {
                        &EXPECTED
                    } else {
                        &[0; 32]
                    });
                    v
                }
            }
            _ => panic!("unexpected APDU"),
        };
        out[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }
    fn close(&mut self) {
        self.0.borrow_mut().closed += 1;
    }
}
fn request() -> Request {
    Request {
        operation: device_policy::Operation::FidoSign,
        digest: [1; 32],
        channel: [2; 32],
    }
}
fn open(db: &Shared) -> PinState<Link> {
    PinState::open(Link(db.clone())).unwrap()
}

#[test]
fn precharge_precedes_one_se_kdf_and_only_correct_pin_credits() {
    let db = Shared::default();
    let mut p = open(&db);
    let r = request();
    p.reserve_attempt(&r).unwrap();
    assert_eq!(db.borrow().spent, 1);
    assert_eq!(db.borrow().kdf_calls, 0);
    assert_eq!(p.verify(&r, &[1; PIN_DIGITS]), Err(PinError::Incorrect));
    assert_eq!(db.borrow().credit, 0);
    p.reserve_attempt(&r).unwrap();
    p.verify(&r, &TEST_PIN).unwrap();
    assert_eq!(
        (db.borrow().spent, db.borrow().credit, db.borrow().kdf_calls),
        (2, 2, 2)
    );
    assert_eq!(db.borrow().credit_writes, 1);
}
#[test]
fn eleventh_wrong_guess_latches_erase_and_reboot_cannot_unlock() {
    let db = Shared::default();
    let r = request();
    for i in 0..11 {
        let mut p = open(&db);
        p.reserve_attempt(&r).unwrap();
        assert_eq!(
            p.verify(&r, &[1; PIN_DIGITS]),
            Err(if i == 10 {
                PinError::Storage
            } else {
                PinError::Incorrect
            })
        );
        assert_eq!(db.borrow().erase, u64::from(i == 10));
    }
    assert!(matches!(
        PinState::open(Link(db.clone())),
        Err(Error::EraseRequired)
    ));
    assert_eq!(db.borrow().spent, 11);
    assert_eq!(db.borrow().credit_writes, 0);
}
#[test]
fn ten_wrong_then_correct_preserves_data_but_ambiguous_eleventh_verification_erases() {
    let db = Shared::default();
    let r = request();
    let mut p = open(&db);
    for _ in 0..10 {
        p.reserve_attempt(&r).unwrap();
        assert_eq!(p.verify(&r, &[1; PIN_DIGITS]), Err(PinError::Incorrect));
    }
    p.reserve_attempt(&r).unwrap();
    p.verify(&r, &TEST_PIN).unwrap();
    assert_eq!(db.borrow().erase, 0);
    assert_eq!(db.borrow().credit, 11);
    for _ in 0..11 {
        p.reserve_attempt(&r).unwrap();
        if db.borrow().spent - db.borrow().credit < 11 {
            assert_eq!(p.verify(&r, &[1; PIN_DIGITS]), Err(PinError::Incorrect));
        }
    }
    drop(p);
    assert!(matches!(
        PinState::open(Link(db.clone())),
        Err(Error::EraseRequired)
    ));
    assert_eq!(db.borrow().erase, 1);
}
#[test]
fn changing_hardware_pin_requires_authority_and_latches_before_any_new_pin() {
    use device_policy::Operation;
    let db = Shared::default();
    let mut p = open(&db);
    assert_eq!(
        p.latch_pin_change_erase(crate::keys::tests::auth(Operation::FidoSign), 0),
        Err(Error::Authorization)
    );
    assert_eq!(db.borrow().erase, 0);
    p.latch_pin_change_erase(crate::keys::tests::auth(Operation::ChangePin), 1)
        .unwrap();
    assert_eq!(db.borrow().erase, 1);
    assert!(matches!(
        PinState::open(Link(db.clone())),
        Err(Error::EraseRequired)
    ));
    let db = Shared::default();
    let mut p = open(&db);
    db.borrow_mut().fail_erase = true;
    assert!(
        p.latch_pin_change_erase(crate::keys::tests::auth(Operation::ChangePin), 1)
            .is_err()
    );
    assert_eq!(db.borrow().erase, 1);
    assert!(matches!(
        PinState::open(Link(db.clone())),
        Err(Error::EraseRequired)
    ));
}
#[test]
fn request_substitution_second_guess_and_rollback_close_the_service() {
    let db = Shared::default();
    let r = request();
    let mut p = open(&db);
    p.reserve_attempt(&r).unwrap();
    let mut wrong = r;
    wrong.digest[0] ^= 1;
    assert_eq!(p.verify(&wrong, &TEST_PIN), Err(PinError::Storage));
    assert_eq!(db.borrow().kdf_calls, 0);
    let mut p = open(&db);
    p.reserve_attempt(&r).unwrap();
    assert_eq!(p.verify(&r, &[1; PIN_DIGITS]), Err(PinError::Incorrect));
    assert_eq!(p.verify(&r, &TEST_PIN), Err(PinError::Storage));
    assert_eq!(db.borrow().credit_writes, 0);
    let mut p = open(&db);
    p.reserve_attempt(&r).unwrap();
    db.borrow_mut().spent = 0;
    assert_eq!(p.verify(&r, &TEST_PIN), Err(PinError::Storage));
}
#[test]
fn ambiguous_debit_and_credit_replies_never_allow_retry_or_success() {
    let db = Shared::default();
    let r = request();
    let mut p = open(&db);
    db.borrow_mut().fail_debit = true;
    assert_eq!(p.reserve_attempt(&r), Err(PinError::Storage));
    assert_eq!(db.borrow().spent, 1);
    db.borrow_mut().fail_debit = false;
    assert_eq!(p.reserve_attempt(&r), Err(PinError::Storage));
    assert_eq!(db.borrow().spent, 1);
    let mut p = open(&db);
    p.reserve_attempt(&r).unwrap();
    db.borrow_mut().fail_credit = true;
    assert_eq!(p.verify(&r, &TEST_PIN), Err(PinError::Storage));
    assert_eq!(db.borrow().credit_writes, 1);
    assert_eq!(p.verify(&r, &TEST_PIN), Err(PinError::Storage));
    assert_eq!(db.borrow().credit_writes, 1);
}
#[test]
fn bad_schema_malformed_kdf_and_invalid_digit_do_not_create_defaults() {
    let db = Shared::default();
    db.borrow_mut().wrong_type = true;
    assert!(PinState::open(Link(db.clone())).is_err());
    assert!(db.borrow().closed > 0);
    db.borrow_mut().wrong_type = false;
    let mut p = open(&db);
    let r = request();
    p.reserve_attempt(&r).unwrap();
    db.borrow_mut().bad_reply = true;
    assert_eq!(p.verify(&r, &TEST_PIN), Err(PinError::Storage));
    assert_eq!(db.borrow().credit_writes, 0);
    db.borrow_mut().bad_reply = false;
    let mut p = open(&db);
    p.reserve_attempt(&r).unwrap();
    let calls = db.borrow().kdf_calls;
    assert_eq!(p.verify(&r, &[10; PIN_DIGITS]), Err(PinError::Storage));
    assert_eq!(db.borrow().kdf_calls, calls);
}
