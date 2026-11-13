//! Independent fixed provisioning transcript plus the native applet model.
use super::*;
use crate::{
    commissioning::{Installer, Root},
    inventory::{Baseline, ExpectedObject},
};
use device_policy::{
    pin::Press,
    pin_only::Digit,
    pin_setup::{Error as SetupError, Progress, ProtectedCommissioning, Setup},
};
use std::collections::VecDeque;

const FACTORY: [ExpectedObject; 1] = [ExpectedObject {
    id: 0x7fff_1001,
    kind: 0x0b,
}];
#[derive(Default)]
struct Journal {
    phase: u8, // 0 explicitly trusted Blank,1 Installing,2 PIN milestone only.
    checks: usize,
    fail_check: Option<usize>,
    lose_begin: bool,
    lose_commit: bool,
    untrusted: bool,
}
type JournalDb = Rc<RefCell<Journal>>;
struct ProtectedRoot(JournalDb, Shared);
impl Root for ProtectedRoot {
    fn check(&mut self) -> Result<(), Error> {
        let mut j = self.0.borrow_mut();
        j.checks += 1;
        if j.untrusted || j.fail_check == Some(j.checks) {
            Err(Error::Authorization)
        } else {
            Ok(())
        }
    }
    fn qualified_blank(&mut self) -> Result<(), Error> {
        if self.0.borrow().phase != 0 {
            Err(Error::Record)
        } else {
            Ok(())
        }
    }
    fn begin_pin_install(&mut self, profile: &[u8; 32], inventory: &[u8; 32]) -> Result<(), Error> {
        self.qualified_blank()?;
        assert_eq!(profile, &[0xa7; 32]);
        assert_ne!(inventory, &[0; 32]);
        let mut j = self.0.borrow_mut();
        j.phase = 1; // Persistence happens before its possibly lost ACK.
        if j.lose_begin {
            Err(Error::Channel)
        } else {
            Ok(())
        }
    }
    fn commit_pin_namespace(&mut self) -> Result<(), Error> {
        assert_eq!(self.0.borrow().phase, 1);
        assert!(!self.1.borrow().native_verified);
        assert!(self.1.borrow().cuts > 0);
        let mut j = self.0.borrow_mut();
        j.phase = 2;
        if j.lose_commit {
            Err(Error::Channel)
        } else {
            Ok(())
        }
    }
}
struct Step {
    header: [u8; 3],
    data: Vec<u8>,
    response: Vec<u8>,
}
#[derive(Default)]
struct Trace {
    steps: VecDeque<Step>,
    calls: usize,
    closes: usize,
    mutations: usize,
    fail: Option<(usize, bool)>,
    leftover: bool,
}
type TraceDb = Rc<RefCell<Trace>>;
struct CommissionIo {
    trace: TraceDb,
    journal: JournalDb,
    native: Io,
}
impl private::Sealed for CommissionIo {}
impl Exchange for CommissionIo {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        let mut t = self.trace.borrow_mut();
        t.calls += 1;
        if t.fail == Some((t.calls, false)) {
            return Err(Error::Channel);
        }
        let response = if c.p2 == 0x25 {
            assert_eq!(self.journal.borrow().phase, 0);
            assert_eq!(c.data, [0x41, 2, 0, 0, 0x42, 1, 0xff]);
            Some(if t.leftover {
                vec![0x41, 1, 1, 0x42, 8, 0x7f, 0xff, 0x10, 1, 0x4f, 0x4b, 8, 1]
            } else {
                vec![0x41, 1, 1, 0x42, 4, 0x7f, 0xff, 0x10, 1]
            })
        } else if c.p2 == 0x26 && c.data == [0x41, 4, 0x7f, 0xff, 0x10, 1] {
            Some(vec![0x41, 1, 0x0b, 0x42, 1, 1])
        } else if let Some(step) = t.steps.pop_front() {
            assert_eq!(
                self.journal.borrow().phase,
                1,
                "SE mutation before durable Installing"
            );
            assert_eq!([c.instruction, c.p1, c.p2], step.header);
            assert_eq!(c.le, None);
            assert_eq!(c.data, step.data);
            t.mutations += usize::from(c.instruction & 0x1f == 1);
            Some(step.response)
        } else {
            None
        };
        let n = if let Some(response) = response {
            assert!(response.len() <= out.len());
            out[..response.len()].copy_from_slice(&response);
            response.len()
        } else {
            self.native.exchange(c, out)?
        };
        if t.fail == Some((t.calls, true)) {
            return Err(Error::Channel);
        }
        Ok(n)
    }
    fn close(&mut self) {
        self.trace.borrow_mut().closes += 1;
        self.native.close();
    }
}
fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u8::from_str_radix(core::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect()
}
fn transcript() -> TraceDb {
    // Policies/fields are literal independent wire fixtures. PIN is ASCII,
    // finite maxAttempts and INS_AUTH_OBJECT are explicit. Marker is last.
    let mut steps = VecDeque::new();
    steps.push_back(Step {
        header: [0x41, 7, 0],
        data: hex("11090800000000200200001202000b41044f4b08014206393237303436"),
        response: vec![],
    });
    for (id, policy) in [
        ("4f4b0501", "1109080000000000320000"),
        ("4f4b0502", "1112084f4b080100320000080000000000220000"),
        ("4f4b0507", "1109080000000000320000"),
    ] {
        steps.push_back(Step {
            header: [1, 8, 0],
            data: hex(&std::format!("{policy}4104{id}42020008")),
            response: vec![],
        });
        steps.push_back(Step {
            header: [2, 0, 0],
            data: hex(&std::format!("4104{id}")),
            response: vec![0x41, 8, 0, 0, 0, 0, 0, 0, 0, 0],
        });
    }
    steps.push_back(Step {
        header: [1, 6, 0],
        data: hex(concat!(
            "110908000000000022000041044f4b050642020000430200204420",
            "ba65388a90faa904cd64b2c297ee2648947e281f5dd98f8c4bc6b8d11bc34583"
        )),
        response: vec![],
    });
    Rc::new(RefCell::new(Trace {
        steps,
        ..Trace::default()
    }))
}
fn installer(
    trace: &TraceDb,
    journal: &JournalDb,
    native: &Shared,
) -> Installer<'static, CommissionIo, Power, ProtectedRoot> {
    Installer::new(
        CommissionIo {
            trace: trace.clone(),
            journal: journal.clone(),
            native: Io {
                db: native.clone(),
                closed: false,
            },
        },
        Power(native.clone()),
        ProtectedRoot(journal.clone(), native.clone()),
        Baseline::from_qualified_profile(&FACTORY, [0xa7; 32]).unwrap(),
    )
}
fn setup(trace: &TraceDb, journal: &JournalDb, native: &Shared) -> Result<(), SetupError> {
    let mut installer = installer(trace, journal, native);
    let mut setup = Setup::begin(&mut installer, || 1)?;
    let mut progress = Progress::Entering;
    for digit in PIN.into_iter().cycle().take(12) {
        let key =
            Digit::from_gpio_index(if digit == 0 { 9 } else { u32::from(digit - 1) }).unwrap();
        progress = setup.press(key, Press::Short, &mut installer, || 1)?;
    }
    assert_eq!(progress, Progress::Installed);
    Ok(())
}
#[test]
fn first_boot_uses_two_local_entries_finite_immutable_pin_and_marker_last() {
    let trace = transcript();
    let journal = JournalDb::default();
    let native = Shared::default();
    setup(&trace, &journal, &native).unwrap();
    assert_eq!(trace.borrow().mutations, 5);
    assert!(trace.borrow().steps.is_empty());
    assert_eq!(journal.borrow().phase, 2); // NOT global Ready.
    assert_eq!((native.borrow().spent, native.borrow().credit), (1, 1));
    assert!(!native.borrow().native_verified);
    assert!(native.borrow().cuts > 0);
}
#[test]
fn untrusted_boot_or_any_old_object_refuses_before_se_mutation() {
    for leftover in [false, true] {
        let trace = transcript();
        let journal = JournalDb::default();
        let native = Shared::default();
        trace.borrow_mut().leftover = leftover;
        journal.borrow_mut().untrusted = !leftover;
        assert!(setup(&trace, &journal, &native).is_err());
        assert_eq!(trace.borrow().mutations, 0);
        assert_eq!(journal.borrow().phase, 0);
        assert_eq!(native.borrow().native_verifies, 0);
    }
}
#[test]
fn interrupted_installation_never_reopens_as_blank_or_repeats_pin_mutation() {
    let baseline = transcript();
    let j = JournalDb::default();
    let n = Shared::default();
    setup(&baseline, &j, &n).unwrap();
    let calls = baseline.borrow().calls;
    for at in 1..=calls {
        for after in [false, true] {
            let trace = transcript();
            let journal = JournalDb::default();
            let native = Shared::default();
            trace.borrow_mut().fail = Some((at, after));
            assert!(
                setup(&trace, &journal, &native).is_err(),
                "lost APDU {at}/{after}"
            );
            assert_ne!(journal.borrow().phase, 2);
            assert!(!native.borrow().native_verified);
            if journal.borrow().phase == 1 {
                let mutations = trace.borrow().mutations;
                trace.borrow_mut().fail = None;
                let mut retry = installer(&trace, &journal, &native);
                assert!(retry.qualified_factory_blank().is_err());
                assert_eq!(trace.borrow().mutations, mutations);
                assert_eq!(journal.borrow().phase, 1);
            }
        }
    }
}
#[test]
fn journal_ambiguity_or_power_cut_failure_is_not_successful_initialization() {
    for mode in 0..3 {
        let trace = transcript();
        let journal = JournalDb::default();
        let native = Shared::default();
        match mode {
            0 => journal.borrow_mut().lose_begin = true,
            1 => journal.borrow_mut().lose_commit = true,
            _ => native.borrow_mut().fail_power = true,
        }
        assert!(setup(&trace, &journal, &native).is_err());
        assert_ne!(journal.borrow().phase, 0);
        if mode == 0 {
            assert_eq!(trace.borrow().mutations, 0);
        }
        if mode != 1 {
            assert_eq!(journal.borrow().phase, 1);
        }
        let mutations = trace.borrow().mutations;
        assert!(
            installer(&trace, &journal, &native)
                .qualified_factory_blank()
                .is_err()
        );
        assert_eq!(trace.borrow().mutations, mutations);
    }
}
