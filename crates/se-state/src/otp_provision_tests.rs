use super::*;
use openkeys_otp_format::{Algorithm as ImportAlgorithm, Kind};
use std::{collections::VecDeque, vec, vec::Vec};

struct Step {
    header: [u8; 3],
    data: Vec<u8>,
    response: Result<Vec<u8>, Error>,
}
#[derive(Default)]
struct Trace {
    steps: VecDeque<Step>,
    calls: usize,
    closed: usize,
}
struct Script(Rc<RefCell<Trace>>);
impl crate::private::Sealed for Script {}
impl crate::private::PinBound for Script {}
impl PinBoundExchange for Script {}
impl Exchange for Script {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        let mut t = self.0.borrow_mut();
        t.calls += 1;
        let step = t.steps.pop_front().expect("unexpected APDU/retry");
        assert_eq!([c.instruction, c.p1, c.p2], step.header);
        assert_eq!(c.class, 0x80);
        assert_eq!(c.le, Some(0));
        assert_eq!(c.data, step.data);
        // No ReadObject(seed), including during import verification/deletion.
        if c.instruction == 2 && c.p2 == 0 {
            assert_eq!(&c.data[..6], &hex("41044f4b0702"));
        }
        let response = step.response?;
        assert!(response.len() <= out.len());
        out[..response.len()].copy_from_slice(&response);
        Ok(response.len())
    }
    fn close(&mut self) {
        self.0.borrow_mut().closed += 1;
    }
}
fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u8::from_str_radix(core::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
fn step(header: [u8; 3], data: &str, response: &str) -> Step {
    Step {
        header,
        data: hex(data),
        response: Ok(hex(response)),
    }
}
fn scope(request: Request) -> Execution {
    let mut g = Gate::new(Verifier);
    g.approve(request, || 1).unwrap();
    g.consume_authorized(&request, || 1)
        .unwrap()
        .activate(&request, &mut Root, || 1)
        .unwrap()
}
fn input() -> Import<'static> {
    Import::new(
        Kind::Hotp,
        ImportAlgorithm::Sha1,
        "Example",
        "alice",
        42,
        b"12345678901234567890",
    )
    .unwrap()
}
fn install_steps() -> Vec<Step> {
    let mut steps = vec![
        step([4, 0, 0x27], "41044f4b0602", "410102"),
        step([4, 0, 0x27], "41044f4b0702", "410102"),
        step(
            [1, 8, 0],
            "1112084f4b08010036000008000000002000000041044f4b07024308000000000000002a",
            "",
        ),
        step([2, 0, 0x26], "41044f4b0702", "41010d420101"),
        step([2, 0, 7], "41044f4b0702", "41020008"),
        step([2, 0, 0], "41044f4b0702", "4108000000000000002a"),
    ];
    let mut seed = hex("1112084f4b08011006000008000000002000000041044f4b06024314");
    seed.extend_from_slice(b"12345678901234567890");
    steps.push(Step {
        header: [1, 5, 0],
        data: seed,
        response: Ok(vec![]),
    });
    steps.extend([
        step([2, 0, 0x26], "41044f4b0602", "410111420101"),
        step([2, 0, 7], "41044f4b0602", "41020014"),
    ]);
    steps
}
fn backend(steps: Vec<Step>) -> (OtpKeys<Script>, Rc<RefCell<Trace>>) {
    let t = Rc::new(RefCell::new(Trace {
        steps: steps.into(),
        ..Trace::default()
    }));
    (OtpKeys::new(Script(t.clone())), t)
}

#[test]
fn import_has_explicit_native_pin_acl_no_export_and_confirmed_persistent_counter() {
    let input = input();
    let s = scope(import_request(2, 7, &input, [2; 32]).unwrap());
    let (mut k, t) = backend(install_steps());
    k.install(&s, 2, 7, &input).unwrap();
    assert!(t.borrow().steps.is_empty());
    assert_eq!(t.borrow().calls, 9);
}
#[test]
fn orphan_existing_object_never_gets_default_policy_overwrite_or_retry() {
    let input = input();
    let s = scope(import_request(2, 7, &input, [2; 32]).unwrap());
    for index in [0, 1] {
        let mut steps = install_steps();
        steps[index].response = Ok(hex("410101"));
        steps.truncate(index + 1);
        let (mut k, t) = backend(steps);
        assert_eq!(k.install(&s, 2, 7, &input), Err(Error::Record));
        let calls = t.borrow().calls;
        assert!(k.install(&s, 2, 7, &input).is_err());
        assert_eq!(t.borrow().calls, calls);
        assert!(t.borrow().closed > 0);
    }
}
#[test]
fn seed_account_slot_or_generation_swap_is_denied_before_any_apdu() {
    let original = input();
    let s = scope(import_request(2, 7, &original, [2; 32]).unwrap());
    let changed = Import::new(
        Kind::Hotp,
        ImportAlgorithm::Sha1,
        "Example",
        "mallory",
        42,
        b"12345678901234567890",
    )
    .unwrap();
    for (slot, generation, input) in [(3, 7, &original), (2, 8, &original), (2, 7, &changed)] {
        let (mut k, t) = backend(vec![]);
        assert_eq!(
            k.install(&s, slot, generation, input),
            Err(Error::Authorization)
        );
        assert_eq!(t.borrow().calls, 0);
    }
}
#[test]
fn uncertain_import_response_never_publishes_success_or_replays_creation() {
    for at in 0..install_steps().len() {
        let mut steps = install_steps();
        steps[at].response = Err(Error::Channel);
        steps.truncate(at + 1);
        let (mut k, t) = backend(steps);
        let input = input();
        let s = scope(import_request(2, 7, &input, [2; 32]).unwrap());
        assert!(k.install(&s, 2, 7, &input).is_err());
        let calls = t.borrow().calls;
        assert!(k.install(&s, 2, 7, &input).is_err());
        assert_eq!(t.borrow().calls, calls);
        assert_eq!(calls, at + 1);
    }
}
#[test]
fn deletion_is_seed_first_and_resumes_only_from_authenticated_absence() {
    for absent in [false, true] {
        let mut steps = vec![step(
            [4, 0, 0x27],
            "41044f4b0602",
            if absent { "410102" } else { "410101" },
        )];
        if !absent {
            steps.extend([
                step([4, 0, 0x28], "41044f4b0602", ""),
                step([4, 0, 0x27], "41044f4b0602", "410102"),
            ]);
        }
        steps.extend([
            step([4, 0, 0x27], "41044f4b0702", "410101"),
            step([4, 0, 0x28], "41044f4b0702", ""),
            step([4, 0, 0x27], "41044f4b0702", "410102"),
        ]);
        let (mut k, t) = backend(steps);
        let s = scope(delete_request(2, 7, [2; 32]).unwrap());
        k.delete(&s, 2, 7).unwrap();
        assert!(t.borrow().steps.is_empty());
    }
}
