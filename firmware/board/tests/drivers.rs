use embedded_hal::{
    delay::DelayNs,
    digital::{ErrorType as DigitalErrorType, OutputPin},
    i2c::{ErrorType as I2cErrorType, I2c, Operation},
    spi::{ErrorType as SpiErrorType, Operation as SpiOperation, SpiDevice},
};
use openkeys_board::{Error, flash::Flash, lcd::Lcd};
use std::{cell::RefCell, collections::VecDeque, convert::Infallible, rc::Rc};

#[derive(Default)]
struct DisplayState {
    reset_high: bool,
    light_on: bool,
    delay_ns: u64,
    fail_bus: bool,
    writes: Vec<Vec<u8>>,
}
type Shared = Rc<RefCell<DisplayState>>;
struct Bus(Shared);
impl I2cErrorType for Bus {
    type Error = embedded_hal::i2c::ErrorKind;
}
impl I2c for Bus {
    fn transaction(&mut self, address: u8, ops: &mut [Operation<'_>]) -> Result<(), Self::Error> {
        assert_eq!(address, 0x3e);
        let mut state = self.0.borrow_mut();
        if state.fail_bus {
            return Err(embedded_hal::i2c::ErrorKind::Other);
        }
        for op in ops {
            match op {
                Operation::Write(data) => state.writes.push(data.to_vec()),
                Operation::Read(_) => panic!("LCD is write-only"),
            }
        }
        Ok(())
    }
}
struct Pin(Shared, bool);
impl DigitalErrorType for Pin {
    type Error = Infallible;
}
impl OutputPin for Pin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.set(false);
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set(true);
        Ok(())
    }
}
impl Pin {
    fn set(&self, value: bool) {
        let mut state = self.0.borrow_mut();
        if self.1 {
            state.reset_high = value;
        } else {
            state.light_on = value;
        }
    }
}
struct Delay(Shared);
impl DelayNs for Delay {
    fn delay_ns(&mut self, ns: u32) {
        self.0.borrow_mut().delay_ns += u64::from(ns);
    }
}
fn display(state: &Shared) -> Lcd<Bus, Pin, Pin, Delay> {
    Lcd::new(
        Bus(state.clone()),
        Pin(state.clone(), true),
        Pin(state.clone(), false),
        Delay(state.clone()),
    )
    .unwrap()
}

#[test]
fn lcd_full_rows_fault_lock_and_no_fault_recovery() {
    let state = Shared::default();
    let mut lcd = display(&state);
    assert!(!state.borrow().reset_high && !state.borrow().light_on);
    lcd.initialize().unwrap();
    assert!(!state.borrow().light_on);
    let frame = [*b"example.com     ", *b"Confirm / Cancel"];
    lcd.show(&frame).unwrap();
    assert!(state.borrow().light_on);
    let packets: Vec<_> = state
        .borrow()
        .writes
        .iter()
        .filter(|p| p[0] == 0x40)
        .cloned()
        .collect();
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0][1..], frame[0]);
    assert_eq!(packets[1][1..], frame[1]);
    state.borrow_mut().fail_bus = true;
    assert_eq!(lcd.show(&frame), Err(Error::Io));
    assert!(!state.borrow().reset_high && !state.borrow().light_on);
    state.borrow_mut().fail_bus = false;
    assert_eq!(lcd.initialize(), Err(Error::Faulted));
    assert_eq!(lcd.show(&frame), Err(Error::Faulted));
}

#[test]
fn lcd_rejects_controls_and_clears_before_drop() {
    let state = Shared::default();
    let mut lcd = display(&state);
    lcd.initialize().unwrap();
    lcd.show(&device_policy::lcd::locked()).unwrap();
    let before = state.borrow().delay_ns;
    drop(lcd);
    assert!(!state.borrow().reset_high && !state.borrow().light_on);
    assert!(state.borrow().delay_ns >= before + 2_000_000);
    let mut lcd = display(&state);
    lcd.initialize().unwrap();
    let mut bad = device_policy::lcd::locked();
    bad[1][0] = 0;
    assert_eq!(lcd.show(&bad), Err(Error::InvalidInput));
    assert!(!state.borrow().reset_high && !state.borrow().light_on);
}

struct Script {
    steps: VecDeque<(u8, Vec<u8>)>,
    calls: usize,
}
type SpiState = Rc<RefCell<Script>>;
struct Spi(SpiState);
impl SpiErrorType for Spi {
    type Error = embedded_hal::spi::ErrorKind;
}
impl SpiDevice for Spi {
    fn transaction(&mut self, ops: &mut [SpiOperation<'_, u8>]) -> Result<(), Self::Error> {
        let mut state = self.0.borrow_mut();
        state.calls += 1;
        let (expected, reply) = state.steps.pop_front().expect("Unexpected SPI operation");
        match &ops[0] {
            SpiOperation::Write(header) => assert_eq!(header[0], expected),
            _ => panic!("Missing command"),
        }
        for op in ops {
            if let SpiOperation::Read(out) = op {
                out.copy_from_slice(&reply);
            }
        }
        Ok(())
    }
}
struct NoDelay;
impl DelayNs for NoDelay {
    fn delay_ns(&mut self, _: u32) {}
}
fn flash(extra: &[(u8, &[u8])]) -> (Flash<Spi>, SpiState) {
    let mut steps = VecDeque::from([
        (0xab, vec![]),
        (0x05, vec![0]),
        (0x9f, vec![0xef, 0x40, 0x17]),
    ]);
    steps.extend(extra.iter().map(|(code, reply)| (*code, reply.to_vec())));
    let state = Rc::new(RefCell::new(Script { steps, calls: 0 }));
    let mut flash = Flash::new(Spi(state.clone()));
    flash.identify(&mut NoDelay).unwrap();
    (flash, state)
}

#[test]
fn flash_rejects_overflow_page_wrap_and_misaligned_erase_before_io() {
    let (mut f, s) = flash(&[]);
    let mut output = [0; 257];
    assert_eq!(
        f.read(u32::MAX, &mut output[..1], &mut NoDelay),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        f.read(0, &mut output, &mut NoDelay),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        f.program_page(255, &[1, 2], &mut NoDelay),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        f.program_page(0, &[], &mut NoDelay),
        Err(Error::InvalidInput)
    );
    assert_eq!(f.erase_sector(1, &mut NoDelay), Err(Error::InvalidInput));
    assert_eq!(s.borrow().calls, 3);
}

#[test]
fn flash_wel_failure_and_readback_corruption_latch_fault() {
    let (mut f, s) = flash(&[(0x05, &[0]), (0x06, &[]), (0x05, &[0])]);
    assert_eq!(f.program_page(0, &[0xa5], &mut NoDelay), Err(Error::Verify));
    let calls = s.borrow().calls;
    assert_eq!(
        f.program_page(0, &[0xa5], &mut NoDelay),
        Err(Error::Faulted)
    );
    assert_eq!(s.borrow().calls, calls);
    let (mut f, _) = flash(&[
        (0x05, &[0]),
        (0x06, &[]),
        (0x05, &[2]),
        (0x02, &[]),
        (0x05, &[0]),
        (0x05, &[0]),
        (0x03, &[0xa4]),
    ]);
    assert_eq!(f.program_page(0, &[0xa5], &mut NoDelay), Err(Error::Verify));
    assert_eq!(f.read(0, &mut [0; 1], &mut NoDelay), Err(Error::Faulted));
}
