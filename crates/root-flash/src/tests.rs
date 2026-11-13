extern crate std;
use super::*;
use openkeys_h573_crypto::device_key::Snapshot;
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    vec::Vec,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Register(Register, u32),
    Read(u8, usize, usize),
    Program(u8, usize),
    Erase(u8),
}
struct Model {
    cr: u32,
    sr: u32,
    correction: u32,
    detection: u32,
    key_step: bool,
    bytes: [[u8; 8192]; 2],
    snapshot: Snapshot,
    layout: Layout,
    events: Vec<Event>,
    commits: Vec<(usize, u8, usize)>,
    calls: usize,
    cut: Option<(usize, bool)>,
    change_context: Option<usize>,
    resets: usize,
    no_eop: bool,
    torn_phrase: bool,
    incomplete_erase: bool,
}
#[derive(Clone)]
struct ModelPort(Rc<RefCell<Model>>);
impl ModelPort {
    fn call<T>(&self, action: impl FnOnce(&mut Model) -> T) -> Result<T, Error> {
        let mut m = self.0.borrow_mut();
        m.calls += 1;
        let cut = m.cut.filter(|&(at, _)| at == m.calls);
        if cut == Some((m.calls, false)) {
            return Err(Error::Io);
        }
        if m.change_context == Some(m.calls) {
            m.snapshot.flash_secure_epoch += 1;
        }
        let out = action(&mut m);
        if cut.is_some() {
            Err(Error::Io)
        } else {
            Ok(out)
        }
    }
}
impl Port for ModelPort {
    fn read(&mut self, register: Register) -> Result<u32, Error> {
        self.call(|m| match register {
            Register::Status => m.sr,
            Register::Control => m.cr,
            Register::Correction => m.correction,
            Register::Detection => m.detection,
            _ => panic!("invalid internal register read"),
        })
    }
    fn write(&mut self, register: Register, value: u32) -> Result<(), Error> {
        self.call(|m| {
            m.events.push(Event::Register(register, value));
            match register {
                Register::Key => {
                    assert_eq!(m.cr, 1, "unlock an already unlocked controller");
                    if m.key_step {
                        assert_eq!(value, 0xcdef_89ab);
                        m.key_step = false;
                        m.cr = 0;
                    } else {
                        assert_eq!(value, 0x4567_0123);
                        m.key_step = true;
                    }
                }
                Register::Clear => {
                    assert_eq!(value, 0x0001_0000, "may clear EOP only, never errors");
                    m.sr &= !value;
                }
                Register::Control => {
                    assert_eq!(m.sr & 11, 0, "control write while buffer/controller busy");
                    assert!(matches!(
                        value,
                        0 | 1 | 2 | 0x8000_1f84 | 0x8000_1fa4 | 0x8000_1fc4 | 0x8000_1fe4
                    ));
                    if m.cr & 1 != 0 {
                        return; // LOCK cannot be cleared with a control write.
                    }
                    m.cr = value;
                    if value & 0x8000_0024 == 0x8000_0024 {
                        let sector = (value >> 6) & 0x7f;
                        assert!(matches!(sector, 126 | 127));
                        let slot = (sector - 126) as usize;
                        m.events.push(Event::Erase(slot as u8));
                        m.bytes[slot].fill(0xff);
                        if m.incomplete_erase {
                            m.bytes[slot][8191] = 0;
                        }
                        m.cr &= !0x20;
                        if !m.no_eop {
                            m.sr |= 0x10000;
                        }
                    }
                }
                _ => panic!("invalid register write"),
            }
        })
    }
    fn context(&mut self) -> Result<DeviceKeyContext, Error> {
        self.call(|m| m.snapshot.qualify().map_err(|_| Error::Authorization))?
    }
    fn layout(&mut self) -> Result<Layout, Error> {
        self.call(|m| m.layout)
    }
    fn read_bytes(&mut self, slot: u8, offset: usize, out: &mut [u8]) -> Result<(), Error> {
        self.call(|m| {
            assert_eq!(m.sr & 11, 0);
            assert!(slot < 2 && offset <= 8192 && out.len() <= 8192 - offset);
            m.events.push(Event::Read(slot, offset, out.len()));
            out.copy_from_slice(&m.bytes[usize::from(slot)][offset..offset + out.len()]);
        })
    }
    fn program_phrase(&mut self, slot: u8, offset: usize, data: &[u8; 16]) -> Result<(), Error> {
        self.call(|m| {
            assert_eq!(m.cr, 2);
            assert_eq!(m.sr & 0x007e_000b, 0);
            assert!(slot < 2 && offset.is_multiple_of(16) && offset <= 8176);
            assert_eq!(
                &m.bytes[usize::from(slot)][offset..offset + 16],
                &[0xff; 16]
            );
            m.events.push(Event::Program(slot, offset));
            m.commits.push((m.calls, slot, offset));
            let count = if m.torn_phrase { 8 } else { 16 };
            m.bytes[usize::from(slot)][offset..offset + count].copy_from_slice(&data[..count]);
            if !m.no_eop {
                m.sr |= 0x10000;
            }
        })
    }
    fn emergency_reset(&mut self) -> ! {
        self.0.borrow_mut().resets += 1;
        panic!("whole MCU reset");
    }
}
fn fixture() -> ModelPort {
    ModelPort(Rc::new(RefCell::new(Model {
        cr: 1,
        sr: 0,
        correction: 0,
        detection: 0,
        key_step: false,
        bytes: [[0xff; 8192]; 2],
        snapshot: Snapshot {
            flash_options: (0xc3 << 22) | (0x72 << 8),
            flash_options2: 0xb4 << 24,
            flash_secure_boot: 0xb4,
            flash_secure_epoch: 0,
            flash_privilege: 1,
            hdpl: 0x8a,
            next_hdpl: 0,
            epoch_select: 1,
            sbs_security: 0xb,
            memory_erase_status: 1,
            tamper_control: [0x5dff_0005, 0, 0],
            tamper_security: 1 << 31,
            tamper_privilege: 1 << 31,
            tamper_filter: 0,
            tamper_active: 0,
            tamper_status: 0,
            crypto_security: 0x000c_0000,
            crypto_privilege: 0x000c_0000,
            unprivileged: false,
            debugger_attached: false,
        },
        layout: Layout {
            options: (0xc3 << 22) | (0x72 << 8),
            secure_watermark: 0x007f_007e,
            secure_tail: 0xc000_0000,
            privileged_tail: 0xc000_0000,
            edata: 0,
            icache_control: 0,
            icache_status: 0,
            dcache_control: 0,
            dcache_status: 0,
        },
        events: Vec::new(),
        commits: Vec::new(),
        calls: 0,
        cut: None,
        change_context: None,
        resets: 0,
        no_eop: false,
        torn_phrase: false,
        incomplete_erase: false,
    })))
}
fn engine() -> (Flash<ModelPort>, ModelPort) {
    let port = fixture();
    let flash = Flash::new(port.clone()).unwrap();
    port.0.borrow_mut().calls = 0;
    (flash, port)
}
#[test]
fn fixed_secure_sectors_ordered_phrase_readback_full_erase_and_relock() {
    assert_eq!(SECURE_BASE, 0x0c1f_c000);
    assert_eq!(NONSECURE_BASE, 0x081f_c000);
    let (mut flash, port) = engine();
    flash.program(0, 240, &[0x39; 16]).unwrap();
    flash.program(1, 8176, &[0x52; 16]).unwrap();
    let mut out = [0; 14];
    flash.read(1, 8177, &mut out).unwrap();
    assert_eq!(out, [0x52; 14]);
    let before = port.0.borrow().events.len();
    flash.erase(1).unwrap();
    let m = port.0.borrow();
    assert_eq!(m.cr, 1);
    assert_eq!(&m.bytes[0][240..256], &[0x39; 16]);
    assert!(m.bytes[1].iter().all(|&b| b == 0xff));
    let erases: Vec<_> = m
        .events
        .iter()
        .filter(|e| matches!(e, Event::Erase(_)))
        .collect();
    assert_eq!(erases, [&Event::Erase(1)]);
    let reads: Vec<_> = m.events[before..]
        .iter()
        .filter_map(|e| match e {
            Event::Read(1, offset, 256) => Some(*offset),
            _ => None,
        })
        .collect();
    assert_eq!(reads, (0..8192).step_by(256).collect::<Vec<_>>());
    assert!(m.events.windows(3).any(|w| w
        == [
            Event::Register(Register::Key, 0x4567_0123),
            Event::Register(Register::Key, 0xcdef_89ab),
            Event::Register(Register::Clear, 0x10000),
        ]));
}
#[test]
fn no_change_phrase_does_not_program_ecc_and_nonvirgin_or_outside_writes_close() {
    let (mut flash, port) = engine();
    flash.program(0, 0, &[0xff; 16]).unwrap();
    assert!(
        port.0
            .borrow()
            .events
            .iter()
            .all(|e| !matches!(e, Event::Program(..)))
    );
    flash.program(0, 0, &[0x31; 16]).unwrap();
    assert_eq!(flash.program(0, 0, &[0x11; 16]), Err(Error::Integrity));
    assert_eq!(flash.erase(0), Err(Error::Closed));
    for (slot, offset) in [
        (2, 0),
        (255, 0),
        (0, 1),
        (0, 8180),
        (1, 8192),
        (0, usize::MAX),
    ] {
        let (mut flash, port) = engine();
        assert_eq!(flash.program(slot, offset, &[0x11; 16]), Err(Error::Input));
        assert_eq!(port.0.borrow().cr, 1);
        assert!(
            port.0
                .borrow()
                .events
                .iter()
                .all(|e| !matches!(e, Event::Program(..) | Event::Erase(..)))
        );
        let mut out = [0x55; 16];
        assert_eq!(flash.read(0, 0, &mut out), Err(Error::Closed));
        assert_eq!(out, [0; 16]);
    }
}
fn operation(flash: &mut Flash<ModelPort>, op: u8) -> Result<(), Error> {
    match op {
        0 => flash.program(0, 0, &[0x35; 16]),
        1 => flash.erase(1),
        _ => flash.read(0, 0, &mut [0; 19]),
    }
}
#[test]
fn every_io_failure_before_or_after_never_returns_an_acknowledged_operation() {
    for op in 0..3 {
        let (mut baseline, port) = engine();
        operation(&mut baseline, op).unwrap();
        let calls = port.0.borrow().calls;
        assert!(calls > 15);
        for after in [false, true] {
            for at in 1..=calls {
                let (mut flash, port) = engine();
                port.0.borrow_mut().cut = Some((at, after));
                let result = catch_unwind(AssertUnwindSafe(|| operation(&mut flash, op)));
                assert!(
                    !matches!(result, Ok(Ok(()))),
                    "op={op}, call={at}, after={after}"
                );
                let m = port.0.borrow();
                assert!(m.resets > 0 || m.cr == 1);
                drop(m);
                if result.is_ok() {
                    let mut out = [0x55; 16];
                    assert_eq!(flash.read(0, 0, &mut out), Err(Error::Closed));
                    assert_eq!(out, [0; 16]);
                }
            }
        }
    }
}
#[test]
fn context_or_tamper_change_during_an_operation_cannot_retain_old_authority() {
    let (mut baseline, port) = engine();
    baseline.program(0, 0, &[0x35; 16]).unwrap();
    let calls = port.0.borrow().calls;
    for at in 1..=calls {
        let (mut flash, port) = engine();
        port.0.borrow_mut().change_context = Some(at);
        assert!(flash.program(0, 0, &[0x35; 16]).is_err());
        assert_eq!(port.0.borrow().cr, 1);
    }
    let (mut flash, port) = engine();
    port.0.borrow_mut().snapshot.tamper_status = 1;
    let mut out = [0x55; 20];
    assert_eq!(flash.read(0, 0, &mut out), Err(Error::Authorization));
    assert_eq!(out, [0; 20]);
}
#[test]
fn stale_eop_false_completion_torn_phrase_and_last_byte_erase_are_rejected() {
    for op in 0..4 {
        let (mut flash, port) = engine();
        let mut m = port.0.borrow_mut();
        m.sr = 0x10000; // stale completion must be cleared first.
        match op {
            0 | 1 => m.no_eop = true,
            2 => m.torn_phrase = true,
            _ => m.incomplete_erase = true,
        }
        drop(m);
        let result = if op == 1 || op == 3 {
            flash.erase(1)
        } else {
            flash.program(0, 0, &[0x35; 16])
        };
        assert!(result.is_err());
        assert_eq!(port.0.borrow().cr, 1);
    }
}
#[test]
fn hardware_error_and_ecc_flags_are_never_cleared_into_success() {
    for bit in 17..=24 {
        let (mut flash, port) = engine();
        let mut m = port.0.borrow_mut();
        match bit {
            23 => m.correction = 1 << 30,
            24 => m.detection = 1 << 31,
            _ => m.sr = 1 << bit,
        }
        drop(m);
        assert_eq!(flash.program(0, 0, &[0x35; 16]), Err(Error::Io));
        assert!(
            port.0
                .borrow()
                .events
                .iter()
                .all(|e| !matches!(e, Event::Register(Register::Clear, _)))
        );
    }
}
#[test]
fn busy_or_incomplete_write_buffer_forces_a_bounded_whole_device_reset() {
    for active in [1, 2, 8] {
        let (mut flash, port) = engine();
        port.0.borrow_mut().sr = active;
        let result = catch_unwind(AssertUnwindSafe(|| flash.erase(0)));
        assert!(result.is_err());
        let mut m = port.0.borrow_mut();
        assert_eq!(m.resets, 1);
        assert!(m.events.is_empty());
        // The real reset does not return or run Drop. The fixture clears the
        // model's transient controller state solely so the test can deallocate.
        m.sr = 0;
        m.cr = 1;
    }
}
#[test]
fn unqualified_mapping_or_cache_state_is_not_a_secure_journal_store() {
    let mutations: &[fn(&mut Layout)] = &[
        |l| l.options |= 1 << 31,
        |l| l.secure_watermark = 0x007f_007f,
        |l| l.secure_watermark = 0x007e_0000,
        |l| l.secure_tail &= !(1 << 30),
        |l| l.privileged_tail &= !(1 << 31),
        |l| l.edata = 1 << 15,
        |l| l.icache_control = 1,
        |l| l.icache_status = 1,
        |l| l.icache_status = 4,
        |l| l.dcache_control = 1,
        |l| l.dcache_status = 1,
        |l| l.dcache_status = 4,
        |l| l.dcache_status = 8,
    ];
    for mutation in mutations {
        let port = fixture();
        mutation(&mut port.0.borrow_mut().layout);
        assert!(matches!(
            Flash::new(port.clone()),
            Err(Error::Authorization)
        ));
        assert!(port.0.borrow().events.is_empty());
    }
}

#[test]
fn constructor_io_uncertainty_never_provides_a_flash_owner_or_initializes_data() {
    let port = fixture();
    let baseline = Flash::new(port.clone()).unwrap();
    let calls = port.0.borrow().calls;
    drop(baseline);
    for after in [false, true] {
        for at in 1..=calls {
            let port = fixture();
            port.0.borrow_mut().cut = Some((at, after));
            let result = catch_unwind(AssertUnwindSafe(|| Flash::new(port.clone())));
            assert!(!matches!(result, Ok(Ok(_))), "call={at}, after={after}");
            let m = port.0.borrow();
            assert!(m.resets > 0 || m.cr == 1);
            assert!(
                m.events
                    .iter()
                    .all(|e| !matches!(e, Event::Program(..) | Event::Erase(..)))
            );
        }
    }
}

// These HMAC/nonce/authorization fixtures exist ONLY in the synthetic model.
// The real target uses DHUK, a verified boot Root and actual wipe evidence.
struct TestMac;
impl openkeys_root_journal::Mac for TestMac {
    fn tag(&mut self, body: &[u8; 224]) -> Result<[u8; 16], Error> {
        use hmac::Mac;
        let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&[0x57; 32]).unwrap();
        mac.update(body);
        Ok(mac.finalize().into_bytes()[..16].try_into().unwrap())
    }
}
struct TestGuard(u8);
impl openkeys_root_journal::Guard for TestGuard {
    fn check(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn fresh_owner_nonce(&mut self) -> Result<[u8; 32], Error> {
        self.0 += 1;
        Ok([self.0; 32])
    }
    fn authorize_transition(
        &mut self,
        _: &openkeys_root_journal::State,
        _: &openkeys_root_journal::State,
    ) -> Result<(), Error> {
        Ok(())
    }
    fn authorize_factory_initialize(
        &mut self,
        _: &openkeys_root_journal::Context,
    ) -> Result<(), Error> {
        Ok(())
    }
}
fn journal_context() -> openkeys_root_journal::Context {
    openkeys_root_journal::Context {
        device: [1; 32],
        board: [2; 32],
        profile: [3; 32],
        policy: [4; 32],
    }
}
#[test]
fn real_journal_core_composes_with_fixed_page_controller_across_reopen_and_epochs() {
    use openkeys_root_journal::{Journal, Phase};
    let (flash, port) = engine();
    let mut guard = TestGuard(0);
    let mut journal =
        Journal::factory_initialize(flash, TestMac, journal_context(), &mut guard).unwrap();
    for phase in [
        Phase::PinInstalling,
        Phase::PinInstalled,
        Phase::ApplicationsInstalling,
        Phase::Ready,
    ] {
        journal.advance(phase, &mut guard).unwrap();
    }
    let previous = journal.state().unwrap();
    drop(journal);
    let flash = Flash::new(port.clone()).unwrap();
    let mut journal = Journal::open(flash, TestMac, journal_context(), &mut guard).unwrap();
    assert_eq!(journal.state().unwrap(), previous);
    let target = journal.advance(Phase::ErasePending, &mut guard).unwrap();
    assert_ne!(target.owner, previous.owner);
    assert_eq!(target.generation, previous.generation + 1);
    // Metadata integration only: TestGuard does NOT prove whole-device wipe.
    journal.advance(Phase::FactoryBlank, &mut guard).unwrap();
    drop(journal);
    port.0.borrow_mut().bytes[0][8191] = 0;
    let flash = Flash::new(port.clone()).unwrap();
    assert!(matches!(
        Journal::open(flash, TestMac, journal_context(), &mut guard),
        Err(Error::Integrity)
    ));
}
#[test]
fn lost_plan_seal_ack_requires_exact_pending_metadata_recovery_never_old_ready() {
    use openkeys_root_journal::{Journal, Phase};
    let (flash, port) = engine();
    let mut guard = TestGuard(0);
    let mut baseline =
        Journal::factory_initialize(flash, TestMac, journal_context(), &mut guard).unwrap();
    port.0.borrow_mut().calls = 0;
    port.0.borrow_mut().commits.clear();
    baseline.advance(Phase::PinInstalling, &mut guard).unwrap();
    let at = port
        .0
        .borrow()
        .commits
        .iter()
        .find(|&&(_, slot, offset)| slot == 0 && offset == 496)
        .unwrap()
        .0;
    drop(baseline);

    let (flash, port) = engine();
    let mut guard = TestGuard(0);
    let mut journal =
        Journal::factory_initialize(flash, TestMac, journal_context(), &mut guard).unwrap();
    port.0.borrow_mut().calls = 0;
    port.0.borrow_mut().cut = Some((at, true));
    assert!(journal.advance(Phase::PinInstalling, &mut guard).is_err());
    assert_eq!(journal.state(), Err(Error::Closed));
    drop(journal);
    port.0.borrow_mut().cut = None;
    let flash = Flash::new(port.clone()).unwrap();
    let mut recovered = Journal::open(flash, TestMac, journal_context(), &mut guard).unwrap();
    assert_eq!(recovered.state(), Err(Error::Pending));
    assert_eq!(
        recovered.pending_target().unwrap().unwrap().phase,
        Phase::PinInstalling
    );
    assert_eq!(
        recovered.recover(&mut guard).unwrap().phase,
        Phase::PinInstalling
    );
}
