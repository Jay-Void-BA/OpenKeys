use device_policy::{
    Operation, Request,
    lcd::{Decision, Error as ReviewError, Review},
    pin::{Error as PinError, Press, ProtectedPin},
    pin_only::{Digit, PIN_DIGITS},
};
use openkeys_board::pin_ui::{PinUi, Update};

#[derive(Default)]
struct Backend {
    attempts: usize,
    verifies: usize,
    storage_failure: bool,
}
impl ProtectedPin<PIN_DIGITS> for Backend {
    fn reserve_attempt(&mut self, _: &Request) -> Result<(), PinError> {
        self.attempts += 1;
        if self.storage_failure {
            Err(PinError::Storage)
        } else {
            Ok(())
        }
    }
    fn verify(&mut self, _: &Request, digits: &[u8; PIN_DIGITS]) -> Result<(), PinError> {
        self.verifies += 1;
        // Synthetic fixture only. No default production PIN exists.
        if *digits == [0; PIN_DIGITS] {
            Ok(())
        } else {
            Err(PinError::Incorrect)
        }
    }
}
fn request() -> Request {
    Request {
        operation: Operation::FidoSign,
        digest: [1; 32],
        channel: [2; 32],
    }
}
fn release(ui: &mut PinUi, pin: &mut Backend, t: &mut u64, bit: u32) -> Update {
    for mask in [0, 0, 1 << bit, 1 << bit, 0] {
        *t += 30;
        ui.poll(mask, pin, || *t).unwrap();
    }
    *t += 30;
    ui.poll(0, pin, || *t).unwrap()
}

#[test]
fn host_requests_timeouts_and_waiting_cancel_never_spend_attempts() {
    let mut ui = PinUi::new(0);
    let mut pin = Backend::default();
    ui.request(request(), 0).unwrap();
    assert!(ui.request(request(), 1).is_err());
    assert!(matches!(
        ui.poll(0, &mut pin, || 60_000),
        Ok(Update::Cancelled)
    ));
    ui.request(request(), 60_001).unwrap();
    let mut t = 60_001;
    assert!(matches!(
        release(&mut ui, &mut pin, &mut t, 9),
        Update::Cancelled
    ));
    assert_eq!(pin.attempts, 0);
    assert_eq!(pin.verifies, 0);
}

#[test]
fn physical_start_then_six_decimal_zeroes_yields_one_request_receipt() {
    let mut ui = PinUi::new(0);
    let mut pin = Backend::default();
    ui.request(request(), 0).unwrap();
    let mut t = 0;
    assert!(matches!(
        release(&mut ui, &mut pin, &mut t, 4),
        Update::Frame(_)
    ));
    assert_eq!(pin.attempts, 0);
    for count in 1..PIN_DIGITS {
        match release(&mut ui, &mut pin, &mut t, 9) {
            Update::Frame(frame) => {
                assert_eq!(frame[1].iter().filter(|&&c| c == b'*').count(), count);
                assert!(!frame.iter().flatten().any(u8::is_ascii_digit));
            }
            _ => panic!("receipt must require six digits"),
        }
    }
    let Update::Verified(receipt) = release(&mut ui, &mut pin, &mut t, 9) else {
        panic!("missing local PIN proof");
    };
    assert_eq!(*receipt.request(), request());
    assert_eq!(pin.verifies, 1);
    assert_eq!(pin.attempts, 1);
    assert!(matches!(ui.poll(0, &mut pin, || t + 1), Ok(Update::Quiet)));
    let mut review = Review::for_pin(&receipt, b"accounts.example.attacker.test", t).unwrap();
    assert_eq!(
        review.press_digit(Digit::Five, Press::Short, t),
        Err(ReviewError::NotReviewed)
    );
    review.frame(t);
    review.displayed();
    assert_eq!(
        review.press_digit(Digit::Five, Press::Short, t),
        Err(ReviewError::NotReviewed)
    );
    assert_eq!(
        review.press_digit(Digit::Eight, Press::Short, t),
        Ok(Decision::Pending)
    );
    review.frame(t);
    review.displayed();
    assert_eq!(
        review.press_digit(Digit::Five, Press::Long, t),
        Ok(Decision::Pending)
    );
    assert_eq!(
        review.press_digit(Digit::Five, Press::Short, t),
        Ok(Decision::Confirmed(request()))
    );
    assert_eq!(
        review.press_digit(Digit::Five, Press::Short, t),
        Ok(Decision::Cancelled)
    );
}

#[test]
fn held_start_input_timeouts_storage_errors_and_faults_do_not_verify() {
    let mut ui = PinUi::new(0);
    let mut pin = Backend::default();
    ui.poll(16, &mut pin, || 1).unwrap();
    ui.poll(16, &mut pin, || 26).unwrap();
    ui.request(request(), 27).unwrap();
    ui.poll(0, &mut pin, || 28).unwrap();
    ui.poll(0, &mut pin, || 53).unwrap();
    assert_eq!(pin.attempts, 0);
    let mut t = 53;
    release(&mut ui, &mut pin, &mut t, 4);
    assert_eq!(pin.attempts, 0);
    assert!(matches!(
        ui.poll(0, &mut pin, || t + 60_000),
        Ok(Update::Cancelled)
    ));
    ui.request(request(), t + 60_001).unwrap();
    t += 60_001;
    pin.storage_failure = true;
    release(&mut ui, &mut pin, &mut t, 4);
    for _ in 0..PIN_DIGITS - 1 {
        release(&mut ui, &mut pin, &mut t, 9);
    }
    assert!(matches!(
        release(&mut ui, &mut pin, &mut t, 9),
        Update::Denied
    ));
    assert_eq!(pin.verifies, 0);
    assert!(ui.poll(0, &mut pin, || t - 1).is_err());
    assert!(ui.request(request(), t + 1).is_err());
}
