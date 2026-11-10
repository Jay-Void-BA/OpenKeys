//! RFC public test secrets only. No owner seed, PIN, device or transport.
use device_policy::{
    Authorization, Error as PolicyError, Request,
    execution::{Execution, State},
    lcd::Frame,
    pin::{Error as PinError, Press, ProtectedPin},
    pin_only::{Digit, Gate, PinEntry, SecurePinVerifier, VerifiedPinRequest},
};
use hmac::{Hmac, Mac};
use openkeys_board::{
    Error,
    otp::{Algorithm, Display, Mode, ProtectedOtp, Slot, TrustedScreen, request},
};
use std::{cell::RefCell, rc::Rc};

struct LocalPin;
impl ProtectedPin<6> for LocalPin {
    fn reserve_attempt(&mut self, _: &Request) -> Result<(), PinError> {
        Ok(())
    }
    fn verify(&mut self, _: &Request, d: &[u8; 6]) -> Result<(), PinError> {
        assert_eq!(*d, [0; 6]);
        Ok(())
    }
}
struct Verifier;
impl SecurePinVerifier for Verifier {
    fn check_security_state(&mut self) -> Result<(), PolicyError> {
        Ok(())
    }
    fn verify_device_pin(&mut self, r: &Request) -> Result<VerifiedPinRequest, PolicyError> {
        let mut e = PinEntry::begin(*r, || 1).unwrap();
        for _ in 0..6 {
            e.press(Digit::Zero, Press::Short, 1).unwrap();
        }
        Ok(e.submit(&mut LocalPin, || 1).unwrap())
    }
    fn confirm_on_device(&mut self, _: &VerifiedPinRequest) -> Result<(), PolicyError> {
        Ok(())
    }
}
fn auth(mode: Mode, alg: Algorithm) -> Authorization {
    let r = request(mode, Slot::new(0).unwrap(), alg, [2; 32]);
    let mut gate = Gate::new(Verifier);
    gate.approve(r, || 1).unwrap();
    gate.consume_authorized(&r, || 1).unwrap()
}
#[derive(Default)]
struct Db {
    counter: u64,
    utc: u64,
    generation: u64,
    macs: usize,
    frames: Vec<Frame>,
    clears: usize,
    fail_counter: bool,
    fail_mac: bool,
    fail_display: bool,
    invalid_time: bool,
    wrong_mac_length: bool,
    rollover: bool,
}
type Shared = Rc<RefCell<Db>>;
struct Backend(Shared);
impl State for Backend {
    fn generation(&self) -> u64 {
        self.0.borrow().generation
    }
    fn check_security_state(&mut self) -> Result<(), PolicyError> {
        Ok(())
    }
}
impl ProtectedOtp for Backend {
    fn utc_millis(&mut self) -> Result<u64, Error> {
        let d = self.0.borrow();
        if d.invalid_time {
            Err(Error::Faulted)
        } else {
            Ok(d.utc)
        }
    }
    fn reserve_hotp(
        &mut self,
        execution: &Execution,
        slot: Slot,
        algorithm: Algorithm,
    ) -> Result<u64, Error> {
        assert_eq!(
            execution.request(),
            &request(Mode::Hotp, slot, algorithm, [2; 32])
        );
        assert_eq!(slot.index(), 0);
        let mut d = self.0.borrow_mut();
        let old = d.counter;
        d.counter = old.checked_add(1).ok_or(Error::Faulted)?;
        if d.fail_counter {
            Err(Error::Io)
        } else {
            Ok(old)
        }
    }
    fn hmac(
        &mut self,
        execution: &Execution,
        slot: Slot,
        alg: Algorithm,
        counter: &[u8; 8],
        out: &mut [u8; 64],
    ) -> Result<usize, Error> {
        assert_eq!(slot.index(), 0);
        assert_eq!(execution.request().channel, [2; 32]);
        let mut d = self.0.borrow_mut();
        d.macs += 1;
        if d.fail_mac {
            return Err(Error::Io);
        }
        let seed = match alg {
            Algorithm::Sha1 => b"12345678901234567890".as_slice(),
            Algorithm::Sha256 => b"12345678901234567890123456789012",
            Algorithm::Sha512 => {
                b"1234567890123456789012345678901234567890123456789012345678901234"
            }
        };
        let value = match alg {
            Algorithm::Sha1 => {
                let mut h = Hmac::<sha1::Sha1>::new_from_slice(seed).unwrap();
                h.update(counter);
                h.finalize().into_bytes().to_vec()
            }
            Algorithm::Sha256 => {
                let mut h = Hmac::<sha2::Sha256>::new_from_slice(seed).unwrap();
                h.update(counter);
                h.finalize().into_bytes().to_vec()
            }
            Algorithm::Sha512 => {
                let mut h = Hmac::<sha2::Sha512>::new_from_slice(seed).unwrap();
                h.update(counter);
                h.finalize().into_bytes().to_vec()
            }
        };
        out[..value.len()].copy_from_slice(&value);
        if d.rollover {
            d.utc += 30_000;
        }
        Ok(if d.wrong_mac_length { 3 } else { value.len() })
    }
}
struct Screen(Shared);
impl TrustedScreen for Screen {
    fn write_private_frame(&mut self, f: &Frame) -> Result<(), Error> {
        let mut d = self.0.borrow_mut();
        if d.fail_display {
            Err(Error::Io)
        } else {
            d.frames.push(*f);
            Ok(())
        }
    }
    fn clear_private_frame(&mut self) -> Result<(), Error> {
        self.0.borrow_mut().clears += 1;
        Ok(())
    }
}
fn display(db: &Shared) -> Display<Backend, Screen> {
    Display::new(Backend(db.clone()), Screen(db.clone()), 0)
}
fn show(d: &mut Display<Backend, Screen>, mode: Mode, alg: Algorithm) -> Result<(), Error> {
    d.show_once(
        auth(mode, alg),
        mode,
        Slot::new(0).unwrap(),
        alg,
        [2; 32],
        || 1,
    )
    .map(|_| ())
}
fn code(db: &Shared) -> u32 {
    let d = db.borrow();
    let f = d.frames.last().unwrap();
    std::str::from_utf8(&f[1][..6]).unwrap().parse().unwrap()
}

#[test]
fn rfc4226_all_ten_hotp_values_advance_before_display() {
    let db = Shared::default();
    let mut d = display(&db);
    for expected in [
        755224, 287082, 359152, 969429, 338314, 254676, 287922, 162583, 399871, 520489,
    ] {
        show(&mut d, Mode::Hotp, Algorithm::Sha1).unwrap();
        assert_eq!(code(&db), expected);
    }
    assert_eq!(db.borrow().counter, 10);
    assert_eq!(db.borrow().macs, 10);
}
#[test]
fn rfc6238_sha1_sha256_sha512_render_exactly_six_digits() {
    for (seconds, values) in [
        (59, [94287082, 46119246, 90693936]),
        (1111111109, [7081804, 68084774, 25091201]),
        (1111111111, [14050471, 67062674, 99943326]),
        (1234567890, [89005924, 91819424, 93441116]),
        (2000000000, [69279037, 90698825, 38618901]),
        (20000000000_u64, [65353130, 77737706, 47863826]),
    ] {
        for (alg, expected) in [Algorithm::Sha1, Algorithm::Sha256, Algorithm::Sha512]
            .into_iter()
            .zip(values)
        {
            let db = Shared::default();
            db.borrow_mut().utc = seconds * 1000;
            let mut d = display(&db);
            show(&mut d, Mode::Totp, alg).unwrap();
            assert_eq!(code(&db), expected % 1_000_000);
            assert_eq!(&db.borrow().frames.last().unwrap()[1][6..], b"          ");
            assert_eq!(db.borrow().counter, 0);
        }
    }
}
#[test]
fn ambiguous_counter_write_or_failed_mac_burns_counter_without_output() {
    let db = Shared::default();
    db.borrow_mut().fail_counter = true;
    assert!(show(&mut display(&db), Mode::Hotp, Algorithm::Sha1).is_err());
    assert_eq!(db.borrow().counter, 1);
    assert_eq!(db.borrow().macs, 0);
    assert!(db.borrow().frames.is_empty());
    db.borrow_mut().fail_counter = false;
    db.borrow_mut().fail_mac = true;
    assert!(show(&mut display(&db), Mode::Hotp, Algorithm::Sha1).is_err());
    assert_eq!(db.borrow().counter, 2);
    assert!(db.borrow().frames.is_empty());
    db.borrow_mut().fail_mac = false;
    show(&mut display(&db), Mode::Hotp, Algorithm::Sha1).unwrap();
    assert_eq!(code(&db), 359152);
}
#[test]
fn substituted_slot_algorithm_or_channel_never_reaches_private_provider() {
    for mode in 0..3 {
        let db = Shared::default();
        let mut d = display(&db);
        let a = auth(Mode::Hotp, Algorithm::Sha1);
        let result = d.show_once(
            a,
            Mode::Hotp,
            Slot::new(if mode == 0 { 1 } else { 0 }).unwrap(),
            if mode == 1 {
                Algorithm::Sha256
            } else {
                Algorithm::Sha1
            },
            if mode == 2 { [3; 32] } else { [2; 32] },
            || 1,
        );
        assert!(result.is_err());
        assert_eq!(db.borrow().counter, 0);
        assert_eq!(db.borrow().macs, 0);
        assert!(db.borrow().frames.is_empty());
    }
    assert_eq!(Slot::new(32), Err(Error::InvalidInput));
}
#[test]
fn invalid_time_rollover_wrong_digest_length_and_display_failure_clear() {
    for mode in 0..4 {
        let db = Shared::default();
        db.borrow_mut().utc = 59_000;
        {
            let mut s = db.borrow_mut();
            match mode {
                0 => s.invalid_time = true,
                1 => s.rollover = true,
                2 => s.wrong_mac_length = true,
                _ => s.fail_display = true,
            }
        }
        let mut d = display(&db);
        assert!(show(&mut d, Mode::Totp, Algorithm::Sha1).is_err());
        assert!(db.borrow().frames.is_empty());
        assert!(db.borrow().clears > 0);
    }
}
#[test]
fn expiry_clock_rollback_disconnect_rtc_step_and_drop_clear_without_regenerating() {
    for mode in 0..5 {
        let db = Shared::default();
        db.borrow_mut().utc = 59_000;
        let mut d = display(&db);
        show(&mut d, Mode::Totp, Algorithm::Sha1).unwrap();
        match mode {
            0 => d.tick(1001).unwrap(),
            1 => assert!(d.tick(0).is_err()),
            2 => {
                db.borrow_mut().generation = 1;
                d.tick(2).unwrap()
            }
            3 => {
                db.borrow_mut().utc = 60_000;
                d.tick(2).unwrap()
            }
            _ => drop(d),
        }
        assert!(db.borrow().clears > 0);
        assert_eq!(db.borrow().macs, 1);
    }
}
