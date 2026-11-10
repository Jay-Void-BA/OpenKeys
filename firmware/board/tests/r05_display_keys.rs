use device_policy::{pin::Press, pin_only::Digit};
use embedded_hal::{
    delay::DelayNs,
    digital::{ErrorType as DigitalErrorType, OutputPin},
    i2c::{ErrorType, I2c, Operation},
};
use openkeys_board::{
    Error,
    dfr0555::{Lcd, Revision},
    ten_keys::Keypad,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug)]
enum Event {
    Pin(bool, bool),
    Delay(u32),
    Write(u8, Vec<u8>),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    calls: usize,
    fail_at: Option<usize>,
    gpio_failure: bool,
    release: bool,
    arm: bool,
}
type Shared = Rc<RefCell<State>>;
struct Bus(Shared);
impl ErrorType for Bus {
    type Error = embedded_hal::i2c::ErrorKind;
}
impl I2c for Bus {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let mut s = self.0.borrow_mut();
        let call = s.calls;
        s.calls += 1;
        assert!(
            s.release && s.arm,
            "no bus traffic while display power is held off"
        );
        if s.fail_at == Some(call) {
            return Err(embedded_hal::i2c::ErrorKind::Other);
        }
        for operation in operations {
            match operation {
                Operation::Write(bytes) => s.events.push(Event::Write(address, bytes.to_vec())),
                Operation::Read(_) => panic!("DFR0555 profile is write-only"),
            }
        }
        Ok(())
    }
}
struct Pin(Shared, bool);
impl DigitalErrorType for Pin {
    type Error = embedded_hal::digital::ErrorKind;
}
impl OutputPin for Pin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.set(false)
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set(true)
    }
}
impl Pin {
    fn set(&self, value: bool) -> Result<(), embedded_hal::digital::ErrorKind> {
        let mut s = self.0.borrow_mut();
        s.events.push(Event::Pin(self.1, value));
        if s.gpio_failure {
            return Err(embedded_hal::digital::ErrorKind::Other);
        }
        if self.1 {
            s.release = value;
        } else {
            s.arm = value;
        }
        Ok(())
    }
}
struct Delay(Shared);
impl DelayNs for Delay {
    fn delay_ns(&mut self, ns: u32) {
        self.0.borrow_mut().events.push(Event::Delay(ns));
    }
}
type Display = Lcd<Bus, Pin, Pin, Delay>;
fn display(s: &Shared, revision: Revision) -> Display {
    Lcd::new(
        Bus(s.clone()),
        Pin(s.clone(), true),
        Pin(s.clone(), false),
        Delay(s.clone()),
        revision,
    )
    .unwrap()
}
fn frame() -> device_policy::lcd::Frame {
    [*b"accounts.example", *b"Confirm / Cancel"]
}

#[test]
fn both_revisions_have_explicit_addresses_cold_start_and_exact_full_rows() {
    for revision in [Revision::V1_0, Revision::V1_1] {
        let s = Shared::default();
        let mut lcd = display(&s, revision);
        assert_eq!(lcd.show(&frame()), Err(Error::InvalidInput));
        lcd.initialize().unwrap();
        let first_arm = s
            .borrow()
            .events
            .iter()
            .position(|e| matches!(e, Event::Pin(false, true)))
            .unwrap();
        assert!(
            s.borrow().events[..first_arm]
                .iter()
                .any(|e| matches!(e,Event::Delay(n) if *n>=500_000_000))
        );
        let first_write = s
            .borrow()
            .events
            .iter()
            .position(|e| matches!(e, Event::Write(..)))
            .unwrap();
        assert!(
            s.borrow().events[first_arm..first_write]
                .iter()
                .any(|e| matches!(e,Event::Delay(n) if *n>=200_000_000))
        );
        let at = s.borrow().events.len();
        lcd.show(&frame()).unwrap();
        let state = s.borrow();
        let packets: Vec<_> = state.events[at..]
            .iter()
            .filter_map(|e| {
                if let Event::Write(a, b) = e {
                    Some((*a, b))
                } else {
                    None
                }
            })
            .collect();
        assert!(
            packets
                .iter()
                .all(|(a, _)| *a == 0x3e || *a == revision.backlight_address())
        );
        let chars: Vec<_> = packets
            .iter()
            .filter(|(a, b)| *a == 0x3e && b[0] == 0x40)
            .map(|(_, b)| b[1])
            .collect();
        assert_eq!(chars, frame().into_iter().flatten().collect::<Vec<_>>());
        let on = packets
            .iter()
            .position(|(a, b)| *a == 0x3e && b.as_slice() == [0x80, 0x0c])
            .unwrap();
        assert!(
            packets[..on]
                .iter()
                .filter(|(a, b)| *a == 0x3e && b[0] == 0x40)
                .count()
                == 32
        );
    }
}
#[test]
fn every_initialization_and_frame_io_failure_cuts_power_and_never_retries() {
    for revision in [Revision::V1_0, Revision::V1_1] {
        let baseline = Shared::default();
        let mut lcd = display(&baseline, revision);
        lcd.initialize().unwrap();
        let init_calls = baseline.borrow().calls;
        lcd.show(&frame()).unwrap();
        let total = baseline.borrow().calls;
        for fail in 0..total {
            let s = Shared::default();
            s.borrow_mut().fail_at = Some(fail);
            let mut lcd = display(&s, revision);
            if fail < init_calls {
                assert_eq!(lcd.initialize(), Err(Error::Io));
            } else {
                lcd.initialize().unwrap();
                assert_eq!(lcd.show(&frame()), Err(Error::Io));
            }
            assert!(!s.borrow().arm && !s.borrow().release);
            let calls = s.borrow().calls;
            s.borrow_mut().fail_at = None;
            assert_eq!(lcd.initialize(), Err(Error::Faulted));
            assert_eq!(lcd.show(&frame()), Err(Error::Faulted));
            assert_eq!(s.borrow().calls, calls);
        }
    }
}
#[test]
fn invalid_characters_lock_drop_and_gpio_failures_do_not_leave_a_power_request() {
    let s = Shared::default();
    let mut lcd = display(&s, Revision::V1_1);
    lcd.initialize().unwrap();
    let mut bad = frame();
    bad[1][3] = 0x1b;
    assert_eq!(lcd.show(&bad), Err(Error::InvalidInput));
    assert!(!s.borrow().release && !s.borrow().arm);
    drop(lcd);
    let s = Shared::default();
    let mut lcd = display(&s, Revision::V1_1);
    lcd.initialize().unwrap();
    drop(lcd);
    assert!(!s.borrow().release && !s.borrow().arm);
    let s = Shared::default();
    s.borrow_mut().gpio_failure = true;
    assert!(
        Lcd::new(
            Bus(s.clone()),
            Pin(s.clone(), true),
            Pin(s.clone(), false),
            Delay(s.clone()),
            Revision::V1_1
        )
        .is_err()
    );
    assert!(
        s.borrow()
            .events
            .iter()
            .any(|e| matches!(e, Event::Pin(false, false)))
    );
    assert!(
        s.borrow()
            .events
            .iter()
            .any(|e| matches!(e, Event::Pin(true, false)))
    );
}
#[test]
fn all_ten_gpio_bits_map_to_one_release_and_chords_never_emit() {
    for bit in 0..10 {
        let mut k = Keypad::new(0);
        k.sample(25, 0).unwrap();
        k.sample(30, 1 << bit).unwrap();
        k.sample(55, 1 << bit).unwrap();
        k.sample(60, 0).unwrap();
        assert!(k.sample(85, 0) == Ok(Some((Digit::from_gpio_index(bit).unwrap(), Press::Short))));
        assert!(k.sample(86, 0).unwrap().is_none());
    }
    for mask in 1_u16..=0x3ff {
        if mask.count_ones() < 2 {
            continue;
        }
        let mut k = Keypad::new(0);
        for (t, m) in [(25, 0), (30, mask), (55, mask), (60, 0), (85, 0)] {
            assert!(k.sample(t, m).unwrap().is_none());
        }
    }
}
#[test]
fn held_keys_scene_changes_gaps_and_rollback_never_become_confirmations() {
    let mut k = Keypad::new(0);
    k.sample(25, 0).unwrap();
    k.sample(30, 16).unwrap();
    k.sample(55, 16).unwrap();
    k.discard_pending(60);
    assert!(k.sample(65, 0).unwrap().is_none());
    assert!(k.sample(90, 0).unwrap().is_none());
    k.sample(95, 16).unwrap();
    k.sample(120, 16).unwrap();
    assert!(k.sample(300, 0).unwrap().is_none());
    assert!(k.sample(325, 0).unwrap().is_none());
    assert!(k.sample(324, 0).is_err());
    assert!(k.sample(326, 0).is_err());
    let mut k = Keypad::new(0);
    assert!(k.sample(1, 0x400).is_err());
    assert!(k.sample(2, 0).is_err());
}
