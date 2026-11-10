use super::*;
use crate::pin::{Error as PinError, Press};
use core::cell::Cell;

fn request() -> Request {
    Request {
        operation: Operation::FidoSign,
        digest: [1; 32],
        channel: [2; 32],
    }
}
// This test double is intentionally not a production verifier or PIN store.
struct Pin {
    used: u8,
    deny_storage: bool,
    digits: [u8; PIN_DIGITS],
}
impl ProtectedPin<PIN_DIGITS> for Pin {
    fn reserve_attempt(&mut self, _: &Request) -> Result<(), PinError> {
        if self.deny_storage || self.used == 11 {
            return Err(PinError::Storage);
        }
        self.used += 1;
        Ok(())
    }
    fn verify(&mut self, _: &Request, digits: &[u8; PIN_DIGITS]) -> Result<(), PinError> {
        if digits == &self.digits {
            Ok(())
        } else {
            Err(PinError::Incorrect)
        }
    }
}
fn pin() -> Pin {
    Pin {
        used: 0,
        deny_storage: false,
        digits: [0, 1, 2, 3, 0, 1],
    }
}
fn enter(p: &mut Pin, r: Request, now: u64) -> VerifiedPinRequest {
    let mut e = PinEntry::begin(r, || now).unwrap();
    for value in p.digits {
        let digit =
            Digit::from_gpio_index(if value == 0 { 9 } else { u32::from(value - 1) }).unwrap();
        e.press(digit, Press::Short, now).unwrap();
    }
    e.submit(p, || now).unwrap()
}

#[test]
fn decimal_zero_is_data_and_length_is_not_a_terminator() {
    let mut p = pin();
    let r = request();
    let mut e = PinEntry::begin(r, || 0).unwrap();
    e.press(Digit::Zero, Press::Short, 1).unwrap();
    assert_eq!(e.filled(), 1);
    e.press(Digit::Nine, Press::Long, 2).unwrap();
    assert_eq!(e.filled(), 0);
    for digit in [
        Digit::Zero,
        Digit::One,
        Digit::Two,
        Digit::Three,
        Digit::Zero,
        Digit::One,
    ] {
        e.press(digit, Press::Short, 3).unwrap();
    }
    assert_eq!(
        e.press(Digit::Two, Press::Short, 4),
        Err(PinError::Incomplete)
    );
    assert_eq!(e.submit(&mut p, || 5).unwrap().request(), &r);
}
#[test]
fn incomplete_cancellation_spends_no_attempt_and_completed_verify_requires_credit() {
    let mut p = pin();
    for _ in 0..30 {
        let mut e = PinEntry::begin(request(), || 0).unwrap();
        e.press(Digit::One, Press::Short, 1).unwrap();
        assert_eq!(
            e.press(Digit::Zero, Press::Long, 2),
            Err(PinError::Cancelled)
        );
        assert!(e.digits.iter().all(|&d| d == 0));
    }
    assert_eq!(p.used, 0);
    p.deny_storage = true;
    let mut e = PinEntry::begin(request(), || 0).unwrap();
    for _ in 0..PIN_DIGITS {
        e.press(Digit::One, Press::Short, 1).unwrap();
    }
    assert!(matches!(e.submit(&mut p, || 2), Err(PinError::Storage)));
    assert_eq!(p.used, 0);
}
#[test]
fn entry_rejects_rollback_expiry_incomplete_wrong_pin_and_counter_overflow() {
    let mut p = pin();
    let mut e = PinEntry::begin(request(), || 10).unwrap();
    assert_eq!(e.press(Digit::One, Press::Short, 9), Err(PinError::Expired));
    assert!(e.digits.iter().all(|&d| d == 0));
    let mut e = PinEntry::begin(request(), || 0).unwrap();
    assert_eq!(
        e.press(Digit::One, Press::Short, ENTRY_MS),
        Err(PinError::Expired)
    );
    let e = PinEntry::begin(request(), || 0).unwrap();
    assert!(matches!(e.submit(&mut p, || 1), Err(PinError::Incomplete)));
    let mut e = PinEntry::begin(request(), || 0).unwrap();
    for _ in 0..PIN_DIGITS {
        e.press(Digit::One, Press::Short, 1).unwrap();
    }
    assert!(matches!(e.submit(&mut p, || 2), Err(PinError::Incorrect)));
    assert!(matches!(
        PinEntry::begin(request(), || u64::MAX),
        Err(PinError::Expired)
    ));
}
#[test]
fn review_after_pin_requires_all_pages_and_locks_to_the_pin_prompt() {
    let mut p = pin();
    let proof = enter(&mut p, request(), 0);
    let mut review =
        crate::lcd::Review::for_pin(&proof, b"accounts.example.attacker.test", 1).unwrap();
    review.frame(2);
    review.displayed();
    assert_eq!(review.confirm(3), Err(crate::lcd::Error::NotReviewed));
    review.next_page();
    review.frame(4);
    review.displayed();
    assert_eq!(
        review.confirm(5),
        Ok(crate::lcd::Decision::Confirmed(request()))
    );
    assert_eq!(review.frame(6), crate::lcd::locked_pin());
    assert!(matches!(
        crate::lcd::Review::for_pin(&proof, b"example.test", REVIEW_MS),
        Err(crate::lcd::Error::Expired)
    ));
}

struct Verifier<'a> {
    pin: Pin,
    clock: &'a Cell<u64>,
    confirmed: bool,
    fault: bool,
    wrong_request: bool,
}
impl SecurePinVerifier for Verifier<'_> {
    fn check_security_state(&mut self) -> Result<(), Error> {
        if self.fault {
            Err(Error::HardwareFault)
        } else {
            Ok(())
        }
    }
    fn verify_device_pin(&mut self, r: &Request) -> Result<VerifiedPinRequest, Error> {
        let mut exact = *r;
        if self.wrong_request {
            exact.channel[0] ^= 1;
        }
        Ok(enter(&mut self.pin, exact, self.clock.get()))
    }
    fn confirm_on_device(&mut self, _: &VerifiedPinRequest) -> Result<(), Error> {
        if self.confirmed {
            Ok(())
        } else {
            Err(Error::Rejected)
        }
    }
}
fn verifier(clock: &Cell<u64>) -> Verifier<'_> {
    Verifier {
        pin: pin(),
        clock,
        confirmed: true,
        fault: false,
        wrong_request: false,
    }
}
#[test]
fn grants_are_exact_single_use_and_consumed_even_on_wrong_request() {
    let clock = Cell::new(0);
    let mut gate = Gate::new(verifier(&clock));
    let r = request();
    gate.approve(r, || clock.get()).unwrap();
    let mut wrong = r;
    wrong.channel[0] ^= 1;
    assert!(matches!(
        gate.consume_authorized(&wrong, || clock.get()),
        Err(Error::WrongRequest)
    ));
    assert!(matches!(
        gate.consume_authorized(&r, || clock.get()),
        Err(Error::Locked)
    ));
    gate.approve(r, || clock.get()).unwrap();
    let grant = gate.consume_authorized(&r, || clock.get()).unwrap();
    assert_eq!(grant.request(), &r);
    assert!(matches!(
        gate.consume_authorized(&r, || clock.get()),
        Err(Error::Locked)
    ));
}
#[test]
fn no_confirmation_wrong_binding_expiry_fault_and_disconnect_deny() {
    let clock = Cell::new(0);
    let r = request();
    let mut v = verifier(&clock);
    v.confirmed = false;
    assert_eq!(Gate::new(v).approve(r, || 0), Err(Error::Rejected));
    let mut v = verifier(&clock);
    v.wrong_request = true;
    assert_eq!(Gate::new(v).approve(r, || 0), Err(Error::WrongRequest));
    let mut gate = Gate::new(verifier(&clock));
    gate.approve(r, || 0).unwrap();
    gate.lock();
    assert!(matches!(
        gate.consume_authorized(&r, || 0),
        Err(Error::Locked)
    ));
    gate.approve(r, || 0).unwrap();
    assert!(matches!(
        gate.consume_authorized(&r, || REVIEW_MS),
        Err(Error::Expired)
    ));
    let mut gate = Gate::new(verifier(&clock));
    gate.approve(r, || 10).unwrap();
    assert!(matches!(
        gate.consume_authorized(&r, || 9),
        Err(Error::TimeWentBackwards)
    ));
    assert!(matches!(gate.approve(r, || 11), Err(Error::HardwareFault)));
    let mut v = verifier(&clock);
    v.fault = true;
    let mut gate = Gate::new(v);
    assert_eq!(gate.approve(r, || 0), Err(Error::HardwareFault));
    assert_eq!(gate.approve(r, || 1), Err(Error::HardwareFault));
}
#[test]
fn removed_biometric_commands_and_unprovisioned_profile_never_unlock() {
    let clock = Cell::new(0);
    let mut gate = Gate::new(verifier(&clock));
    for operation in [Operation::EnrollFingerprint, Operation::DeleteFingerprint] {
        let mut r = request();
        r.operation = operation;
        assert_eq!(gate.approve(r, || 0), Err(Error::Rejected));
    }
    assert_eq!(gate.verifier.pin.used, 0);
    assert!(Gate::new(Unprovisioned).approve(request(), || 0).is_err());
}
