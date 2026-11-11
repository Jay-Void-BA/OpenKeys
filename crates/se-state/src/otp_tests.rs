//! Synthetic RFC secret/MAC fixture and a model of the sealed SE exchange.
extern crate std;
use super::*;
use crate::Exchange;
use device_policy::{
    Error as PolicyError,
    execution::State,
    pin::{Error as PinError, Press, ProtectedPin},
    pin_only::{Digit, Gate, PinEntry, SecurePinVerifier, VerifiedPinRequest},
};
use std::{cell::RefCell, rc::Rc};

struct Local;
impl ProtectedPin<6> for Local {
    fn reserve_attempt(&mut self, _: &Request) -> Result<(), PinError> {
        Ok(())
    }
    fn verify(&mut self, _: &Request, p: &[u8; 6]) -> Result<(), PinError> {
        assert_eq!(*p, [0; 6]);
        Ok(())
    }
}
struct Verifier;
impl SecurePinVerifier for Verifier {
    fn check_security_state(&mut self) -> Result<(), PolicyError> {
        Ok(())
    }
    fn verify_device_pin(&mut self, r: &Request) -> Result<VerifiedPinRequest, PolicyError> {
        let mut p = PinEntry::begin(*r, || 1).unwrap();
        for _ in 0..6 {
            p.press(Digit::Zero, Press::Short, 1).unwrap();
        }
        Ok(p.submit(&mut Local, || 1).unwrap())
    }
    fn confirm_on_device(&mut self, _: &VerifiedPinRequest) -> Result<(), PolicyError> {
        Ok(())
    }
}
struct Root;
impl State for Root {
    fn generation(&self) -> u64 {
        0
    }
    fn check_security_state(&mut self) -> Result<(), PolicyError> {
        Ok(())
    }
}
fn scope(hotp: bool) -> Execution {
    let mut h = Sha256::new();
    h.update(b"OpenKeys/OTP/display/v1");
    h.update([if hotp { 2 } else { 1 }, 0, 0x18]);
    let r = Request {
        operation: if hotp {
            Operation::ShowHotp
        } else {
            Operation::ShowTotp
        },
        digest: h.finalize().into(),
        channel: [2; 32],
    };
    let mut g = Gate::new(Verifier);
    g.approve(r, || 1).unwrap();
    g.consume_authorized(&r, || 1)
        .unwrap()
        .activate(&r, &mut Root, || 1)
        .unwrap()
}
#[derive(Default)]
struct Db {
    counter: u64,
    macs: usize,
    calls: usize,
    closed: usize,
    lost_ack: bool,
    malformed_mac: bool,
    bad_type: bool,
}
type Shared = Rc<RefCell<Db>>;

#[path = "otp_provision_tests.rs"]
mod provision;
struct Io(Shared);
impl crate::private::Sealed for Io {}
impl crate::private::PinBound for Io {}
impl PinBoundExchange for Io {}
impl Exchange for Io {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        let mut d = self.0.borrow_mut();
        d.calls += 1;
        assert_eq!(c.class, 0x80);
        assert_eq!(c.le, Some(0));
        assert_eq!(&c.data[..2], &[0x41, 4]);
        let id = u32::from_be_bytes(c.data[2..6].try_into().unwrap());
        assert!(id == SEED_BASE || id == HOTP_BASE);
        let mut r = [0; 70];
        let n = match (c.instruction, c.p1, c.p2) {
            (2, 0, 0x26) => {
                r[..6].copy_from_slice(&[
                    0x41,
                    1,
                    if d.bad_type {
                        0
                    } else if id == SEED_BASE {
                        0x11
                    } else {
                        0x0d
                    },
                    0x42,
                    1,
                    1,
                ]);
                6
            }
            (2, 0, 7) => {
                r[..4].copy_from_slice(&[0x41, 2, 0, if id == SEED_BASE { 20 } else { 8 }]);
                4
            }
            (2, 0, 0) => {
                assert_eq!(id, HOTP_BASE);
                r[..2].copy_from_slice(&[0x41, 8]);
                r[2..10].copy_from_slice(&d.counter.to_be_bytes());
                10
            }
            (1, 8, 0) => {
                assert_eq!(id, HOTP_BASE);
                assert_eq!(c.data.len(), 6);
                d.counter += 1;
                if d.lost_ack {
                    return Err(Error::Channel);
                }
                0
            }
            (3, 0x0d, 0x45) => {
                assert_eq!(id, SEED_BASE);
                assert_eq!(c.data.len(), 19);
                assert_eq!(&c.data[6..11], &[0x42, 1, 0x18, 0x43, 8]);
                d.macs += 1;
                r[..2].copy_from_slice(&[0x41, 20]);
                // RFC4226 HMAC for ASCII "12345678901234567890", counter0.
                r[2..22].copy_from_slice(&[
                    0xcc, 0x93, 0xcf, 0x18, 0x50, 0x8d, 0x94, 0x93, 0x4c, 0x64, 0xb6, 0x5d, 0x8b,
                    0xa7, 0x66, 0x7f, 0xb7, 0xcd, 0xe4, 0xb0,
                ]);
                if d.malformed_mac { 3 } else { 22 }
            }
            _ => panic!("Unexpected APDU"),
        };
        out[..n].copy_from_slice(&r[..n]);
        Ok(n)
    }
    fn close(&mut self) {
        self.0.borrow_mut().closed += 1;
    }
}

#[test]
fn se_oneshot_uses_exact_hmac_opcode_and_never_reads_seed() {
    let db = Shared::default();
    let mut k = OtpKeys::new(Io(db.clone()));
    let m = k.hmac(&scope(false), 0, Algorithm::Sha1, 0).unwrap();
    assert_eq!(m[0], 0xcc);
    assert_eq!(m[19], 0xb0);
    assert_eq!(&m[20..], &[0; 44]);
    assert_eq!(db.borrow().macs, 1);
    assert_eq!(db.borrow().calls, 3);
}
#[test]
fn hotp_requires_one_durable_exact_counter_and_cannot_reuse_it() {
    let db = Shared::default();
    let mut k = OtpKeys::new(Io(db.clone()));
    let s = scope(true);
    assert_eq!(k.reserve_hotp(&s, 0, Algorithm::Sha1).unwrap(), 0);
    assert_eq!(db.borrow().counter, 1);
    k.hmac(&s, 0, Algorithm::Sha1, 0).unwrap();
    assert!(k.hmac(&s, 0, Algorithm::Sha1, 0).is_err());
    assert_eq!(db.borrow().macs, 1);
    let mut k = OtpKeys::new(Io(db.clone()));
    assert_eq!(k.reserve_hotp(&scope(true), 0, Algorithm::Sha1).unwrap(), 1);
}
#[test]
fn lost_increment_ack_or_counter_substitution_never_returns_mac() {
    let db = Shared::default();
    db.borrow_mut().lost_ack = true;
    let mut k = OtpKeys::new(Io(db.clone()));
    let s = scope(true);
    assert!(k.reserve_hotp(&s, 0, Algorithm::Sha1).is_err());
    assert_eq!(db.borrow().counter, 1);
    assert!(k.hmac(&s, 0, Algorithm::Sha1, 0).is_err());
    assert_eq!(db.borrow().macs, 0);
    db.borrow_mut().lost_ack = false;
    let mut k = OtpKeys::new(Io(db.clone()));
    assert_eq!(k.reserve_hotp(&s, 0, Algorithm::Sha1).unwrap(), 1);
    assert!(k.hmac(&s, 0, Algorithm::Sha1, 0).is_err());
    assert_eq!(db.borrow().macs, 0);
}
#[test]
fn wrong_authority_namespace_schema_and_response_fail_closed() {
    for mode in 0..5 {
        let db = Shared::default();
        let mut k = OtpKeys::new(Io(db.clone()));
        let result = match mode {
            0 => k.hmac(&scope(false), 32, Algorithm::Sha1, 0),
            1 => k.hmac(&scope(false), 0, Algorithm::Sha256, 0),
            2 => k.hmac(&scope(true), 0, Algorithm::Sha1, 0),
            3 => {
                db.borrow_mut().bad_type = true;
                k.hmac(&scope(false), 0, Algorithm::Sha1, 0)
            }
            _ => {
                db.borrow_mut().malformed_mac = true;
                k.hmac(&scope(false), 0, Algorithm::Sha1, 0)
            }
        };
        assert!(result.is_err());
        assert!(db.borrow().closed > 0);
        assert!(k.hmac(&scope(false), 0, Algorithm::Sha1, 0).is_err());
    }
}
