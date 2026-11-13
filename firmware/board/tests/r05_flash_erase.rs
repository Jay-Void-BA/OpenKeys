use embedded_hal::{
    delay::DelayNs,
    spi::{ErrorType, Operation, SpiDevice},
};
use openkeys_board::{
    Error,
    flash::{CAPACITY, Flash, PAGE, SECTOR},
    flash_erase::{Checkpoint, Eraser, Progress},
};
use std::{cell::RefCell, rc::Rc};

struct Db {
    bytes: Vec<u8>,
    wel: bool,
    calls: usize,
    erased: Vec<u32>,
    imperfect_erase: bool,
    lose_erase_ack: bool,
    corrupt_after_last_erase: bool,
}
impl Default for Db {
    fn default() -> Self {
        Self {
            bytes: vec![0xa5; CAPACITY as usize],
            wel: false,
            calls: 0,
            erased: Vec::new(),
            imperfect_erase: false,
            lose_erase_ack: false,
            corrupt_after_last_erase: false,
        }
    }
}
type Shared = Rc<RefCell<Db>>;
struct Spi(Shared);
impl ErrorType for Spi {
    type Error = embedded_hal::spi::ErrorKind;
}
impl SpiDevice for Spi {
    fn transaction(&mut self, ops: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        let mut d = self.0.borrow_mut();
        d.calls += 1;
        let header = match ops[0] {
            Operation::Write(h) => h,
            _ => panic!("missing opcode"),
        };
        let opcode = header[0];
        let address = if header.len() == 4 {
            u32::from_be_bytes([0, header[1], header[2], header[3]]) as usize
        } else {
            0
        };
        match opcode {
            0xab => {}
            0x06 => d.wel = true,
            0x20 => {
                assert!(d.wel);
                assert!(address.is_multiple_of(SECTOR as usize));
                assert!(address + SECTOR as usize <= d.bytes.len());
                d.wel = false;
                d.bytes[address..address + SECTOR as usize].fill(0xff);
                d.erased.push(address as u32);
                if d.imperfect_erase {
                    d.bytes[address] = 0;
                }
                if d.corrupt_after_last_erase && address + SECTOR as usize == d.bytes.len() {
                    d.bytes[0] = 0x80;
                }
                if d.lose_erase_ack {
                    d.lose_erase_ack = false;
                    return Err(embedded_hal::spi::ErrorKind::Other);
                }
            }
            0x05 | 0x9f | 0x03 => {}
            _ => panic!("erase may not program/write status/chip-reset: {opcode:02x}"),
        }
        for op in ops {
            if let Operation::Read(out) = op {
                match opcode {
                    0x05 => out.copy_from_slice(&[if d.wel { 2 } else { 0 }]),
                    0x9f => out.copy_from_slice(&[0xef, 0x40, 0x17]),
                    0x03 => out.copy_from_slice(&d.bytes[address..address + out.len()]),
                    _ => panic!("unexpected read"),
                }
            }
        }
        Ok(())
    }
}
struct Delay;
impl DelayNs for Delay {
    fn delay_ns(&mut self, _: u32) {}
}
#[derive(Default)]
struct Root {
    checks: usize,
    deny_at: Option<usize>,
}
impl Checkpoint for Root {
    fn check(&mut self) -> Result<(), Error> {
        self.checks += 1;
        if self.deny_at == Some(self.checks) {
            Err(Error::Faulted)
        } else {
            Ok(())
        }
    }
}
fn flash(db: &Shared) -> Flash<Spi> {
    let mut f = Flash::new(Spi(db.clone()));
    f.identify(&mut Delay).unwrap();
    f
}
fn erase_sectors(erase: &mut Eraser<Spi>, root: &mut Root) {
    for n in 0..CAPACITY / SECTOR {
        assert_eq!(
            erase.progress().unwrap(),
            Progress::Erasing {
                sectors_done: n as u16
            }
        );
        erase.step(&mut Delay, root).unwrap();
    }
    assert_eq!(
        erase.progress().unwrap(),
        Progress::Verifying { pages_done: 0 }
    );
}
fn verify_pages(erase: &mut Eraser<Spi>, root: &mut Root) {
    for n in 0..CAPACITY / PAGE as u32 {
        assert_eq!(
            erase.progress().unwrap(),
            Progress::Verifying { pages_done: n }
        );
        erase.step(&mut Delay, root).unwrap();
    }
    assert_eq!(erase.progress().unwrap(), Progress::Complete);
}

#[test]
fn complete_wipe_covers_both_banks_and_requires_a_second_full_pass() {
    let db = Shared::default();
    let mut root = Root::default();
    let mut erase = Eraser::begin(flash(&db), &mut root).unwrap();
    erase_sectors(&mut erase, &mut root);
    assert!(db.borrow().bytes.iter().all(|&b| b == 0xff));
    assert_eq!(
        db.borrow().erased,
        (0..CAPACITY).step_by(SECTOR as usize).collect::<Vec<_>>()
    );
    verify_pages(&mut erase, &mut root);
    let receipt = erase.finish(&mut root).unwrap();
    let mut flash = receipt.into_flash();
    let mut bytes = [0; PAGE];
    flash
        .read(CAPACITY - PAGE as u32, &mut bytes, &mut Delay)
        .unwrap();
    assert_eq!(bytes, [0xff; PAGE]);
}
#[test]
fn final_verification_catches_an_earlier_sector_corrupted_during_later_work() {
    let db = Shared::default();
    db.borrow_mut().corrupt_after_last_erase = true;
    let mut root = Root::default();
    let mut erase = Eraser::begin(flash(&db), &mut root).unwrap();
    erase_sectors(&mut erase, &mut root);
    assert_eq!(erase.step(&mut Delay, &mut root), Err(Error::Verify));
    let calls = db.borrow().calls;
    assert_eq!(erase.step(&mut Delay, &mut root), Err(Error::Faulted));
    assert_eq!(db.borrow().calls, calls);
    assert!(erase.finish(&mut root).is_err());
}
#[test]
fn denied_root_checks_and_incomplete_progress_cannot_mint_a_receipt() {
    let db = Shared::default();
    let f = flash(&db);
    let calls = db.borrow().calls;
    let mut root = Root {
        deny_at: Some(1),
        ..Root::default()
    };
    assert!(Eraser::begin(f, &mut root).is_err());
    assert_eq!(db.borrow().calls, calls);
    assert!(Eraser::begin(Flash::new(Spi(db.clone())), &mut Root::default()).is_err());
    for deny_at in [2, 3] {
        let mut root = Root {
            deny_at: Some(deny_at),
            ..Root::default()
        };
        let mut erase = Eraser::begin(flash(&db), &mut root).unwrap();
        assert!(erase.step(&mut Delay, &mut root).is_err());
        assert_eq!(erase.progress(), Err(Error::Faulted));
        assert!(erase.finish(&mut root).is_err());
    }
    let mut root = Root::default();
    let erase = Eraser::begin(flash(&db), &mut root).unwrap();
    assert!(erase.finish(&mut root).is_err());
}
#[test]
fn imperfect_erase_or_lost_ack_closes_owner_and_recovery_restarts_at_zero() {
    for imperfect in [false, true] {
        let db = Shared::default();
        db.borrow_mut().imperfect_erase = imperfect;
        db.borrow_mut().lose_erase_ack = !imperfect;
        let mut root = Root::default();
        let mut erase = Eraser::begin(flash(&db), &mut root).unwrap();
        assert!(erase.step(&mut Delay, &mut root).is_err());
        assert!(erase.finish(&mut root).is_err());
        assert_eq!(db.borrow().erased, [0]);
        db.borrow_mut().imperfect_erase = false;
        // Model of trusted reboot: the durable erase intent must still exist.
        // The same physical device is reopened, without a persisted host offset.
        let mut erase = Eraser::begin(flash(&db), &mut root).unwrap();
        assert_eq!(
            erase.progress().unwrap(),
            Progress::Erasing { sectors_done: 0 }
        );
        erase.step(&mut Delay, &mut root).unwrap();
        assert_eq!(db.borrow().erased, [0, 0]);
    }
}
#[test]
fn hardware_fault_after_all_readbacks_still_prevents_final_receipt() {
    let db = Shared::default();
    let mut root = Root::default();
    let mut erase = Eraser::begin(flash(&db), &mut root).unwrap();
    erase_sectors(&mut erase, &mut root);
    verify_pages(&mut erase, &mut root);
    root.deny_at = Some(root.checks + 1);
    assert!(erase.finish(&mut root).is_err());
}
