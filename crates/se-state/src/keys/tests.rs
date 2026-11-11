extern crate std;
use super::*;
use crate::{Exchange, private};
use aes_gcm::{Aes256Gcm, KeyInit, aead::AeadInPlace};
use device_policy::{Gate, Request, SecureVerifier, biometric};
use std::{cell::RefCell, collections::VecDeque, rc::Rc, vec, vec::Vec};

fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u8::from_str_radix(core::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
fn public() -> [u8; 65] {
    hex(concat!(
        "04",
        "60fed4ba255a9d31c961eb74c6356d68c049b8923b61fa6ce669622e60f29fb6",
        "7903fe1008b8bc99a41ae9e95628bc64f2f1b20c2d7e9f5177a3c294d4462299"
    ))
    .try_into()
    .unwrap()
}
fn signature() -> Vec<u8> {
    hex(concat!(
        "3046022100",
        "efd48b2aacb6a8fd1140dd9cd45e81d69d2c877b56aaf991c34d0ea84eaf3716",
        "022100",
        "f7cb1c942d657c41d436c7a1b6e29f65f3e900dbb9aff4064dc4ab2f843acda8"
    ))
}
struct Step {
    header: [u8; 3],
    data: Vec<u8>,
    reply: Result<Vec<u8>, Error>,
}
#[derive(Default)]
struct Trace {
    steps: VecDeque<Step>,
    calls: usize,
    closes: usize,
}
struct Model(Rc<RefCell<Trace>>);
impl private::Sealed for Model {}
impl private::PinBound for Model {}
impl PinBoundExchange for Model {}
impl Exchange for Model {
    fn exchange(&mut self, c: &Command<'_>, out: &mut [u8]) -> Result<usize, Error> {
        let mut t = self.0.borrow_mut();
        t.calls += 1;
        let step = t.steps.pop_front().expect("unexpected APDU");
        assert_eq!(c.class, 0x80);
        assert_eq!(c.le, Some(0));
        assert_eq!([c.instruction, c.p1, c.p2], step.header);
        assert_eq!(c.data, step.data);
        let reply = step.reply?;
        assert!(reply.len() <= out.len());
        out[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }
    fn close(&mut self) {
        self.0.borrow_mut().closes += 1;
    }
}
fn model(steps: Vec<Step>) -> (Keys<Model>, Rc<RefCell<Trace>>) {
    let trace = Rc::new(RefCell::new(Trace {
        steps: steps.into(),
        ..Trace::default()
    }));
    (Keys::new(Model(trace.clone())), trace)
}
fn step(header: [u8; 3], data: &[u8], reply: &[u8]) -> Step {
    Step {
        header,
        data: data.into(),
        reply: Ok(reply.into()),
    }
}
fn key_steps() -> Vec<Step> {
    let mut reply = vec![0x41, 65];
    reply.extend(public());
    vec![
        step([2, 0, 0x26], &hex("41044f4b1007"), &hex("410129420101")),
        step([2, 0, 7], &hex("41044f4b1007"), &hex("41020020")),
        step([2, 0, 0], &hex("41044f4b1007"), &reply),
    ]
}

// Only test code can implement the sealed model transport. Obtain the opaque
// authority through the real policy gate and authenticated FPC frame parser.
struct Bio;
impl biometric::ProtectedAttempts for Bio {
    fn reserve_identify_attempt(&mut self, _: &Request) -> Result<u32, biometric::Error> {
        Ok(1)
    }
    fn record_match(&mut self, _: &biometric::AuthenticatedMatch) -> Result<(), biometric::Error> {
        Ok(())
    }
}
impl biometric::ProtectedGcm for Bio {
    fn decrypt_identify_payload(
        &mut self,
        h: &[u8; 8],
        iv: &[u8; 12],
        tag: &[u8; 16],
        b: &mut [u8; 12],
    ) -> Result<(), biometric::Error> {
        Aes256Gcm::new_from_slice(&[0x42; 32])
            .unwrap()
            .decrypt_in_place_detached(iv.into(), h, b, tag.into())
            .map_err(|_| biometric::Error::Unauthenticated)
    }
}
impl SecureVerifier for Bio {
    fn check_security_state(&mut self) -> Result<(), device_policy::Error> {
        Ok(())
    }
    fn verify_fingerprint(
        &mut self,
        r: &Request,
    ) -> Result<biometric::VerifiedRequest, device_policy::Error> {
        let mut s = biometric::Session::new(Bio, Bio);
        let tag = s.begin(*r, 7, 1).unwrap();
        let h = [4, 0, 0x13, 0, 0x41, 0, 40, 0];
        let iv = [0x93; 12];
        let mut b = [0; 12];
        for (i, w) in [0x55u16, 0x13, 0x61ec, 0x3034, 7, tag]
            .into_iter()
            .enumerate()
        {
            b[i * 2..i * 2 + 2].copy_from_slice(&w.to_le_bytes());
        }
        let mac = Aes256Gcm::new_from_slice(&[0x42; 32])
            .unwrap()
            .encrypt_in_place_detached((&iv).into(), &h, &mut b)
            .unwrap();
        let mut frame = h.to_vec();
        frame.extend(iv);
        frame.extend(mac);
        frame.extend(b);
        s.receive(&frame, r, || 1)
            .unwrap()
            .ok_or(device_policy::Error::Rejected)
    }
    fn verify_additional_pin(
        &mut self,
        _: &biometric::VerifiedRequest,
    ) -> Result<(), device_policy::Error> {
        Ok(())
    }
    fn confirm_on_device(
        &mut self,
        _: &biometric::VerifiedRequest,
    ) -> Result<(), device_policy::Error> {
        Ok(())
    }
}
pub(crate) fn auth(operation: Operation) -> Authorization {
    let request = Request {
        operation,
        digest: [1; 32],
        channel: [2; 32],
    };
    let mut gate = Gate::new(Bio);
    gate.approve(request, || 1).unwrap();
    gate.consume_authorized(&request, || 1).unwrap()
}

#[test]
fn generated_key_has_explicit_restrictive_policy_and_never_contains_a_private_key_tlv() {
    let mut steps = vec![
        step([4, 0, 0x27], &hex("41044f4b1007"), &hex("410102")),
        step(
            [1, 0x61, 0],
            &hex("1112084f4b08011026000008000000002000000041044f4b1007420103"),
            &[],
        ),
    ];
    steps.extend(key_steps());
    let (mut keys, trace) = model(steps);
    let key = keys.generate(&auth(Operation::FidoRegister), 7).unwrap();
    assert_eq!(key.public_key(), &public());
    assert_eq!(key.slot(), 7);
    assert!(trace.borrow().steps.is_empty());
}
#[test]
fn existing_or_ambiguous_object_never_gets_overwritten_or_retried() {
    for reply in [
        Ok(hex("410101")),
        Ok(vec![]),
        Ok(hex("41015a")),
        Err(Error::Channel),
    ] {
        let (mut keys, trace) = model(vec![Step {
            header: [4, 0, 0x27],
            data: hex("41044f4b1007"),
            reply,
        }]);
        assert!(keys.generate(&auth(Operation::FidoRegister), 7).is_err());
        assert!(keys.generate(&auth(Operation::FidoRegister), 7).is_err());
        assert_eq!(trace.borrow().calls, 1);
        assert!(trace.borrow().closes > 0);
    }
}
#[test]
fn rfc6979_signature_is_checked_against_the_bound_public_key_and_single_sha256() {
    let mut steps = key_steps();
    let mut reply = vec![0x41, 72];
    reply.extend(signature());
    steps.push(step([3,0x0c,9], &hex("41044f4b10074201214320af2bdbe1aa9b6ec1e2ade1d694f41fc71a831d0268e9891562113d8a62add1bf"), &reply));
    let (mut keys, trace) = model(steps);
    let auth = auth(Operation::FidoSign);
    let key = keys.restore(&auth, 7, &public()).unwrap();
    assert_eq!(
        keys.sign(&auth, &key, b"sample")
            .unwrap()
            .to_der()
            .as_bytes(),
        signature()
    );
    assert!(trace.borrow().steps.is_empty());
}
#[test]
fn every_single_bit_signature_corruption_is_rejected_and_closes_the_channel() {
    let valid = signature();
    for bit in 0..valid.len() * 8 {
        let mut corrupt = valid.clone();
        corrupt[bit / 8] ^= 1 << (bit % 8);
        let mut reply = vec![0x41, 72];
        reply.extend(corrupt);
        let mut steps = key_steps();
        steps.push(step([3,0x0c,9], &hex("41044f4b10074201214320af2bdbe1aa9b6ec1e2ade1d694f41fc71a831d0268e9891562113d8a62add1bf"), &reply));
        let (mut keys, trace) = model(steps);
        let auth = auth(Operation::FidoSign);
        let key = keys.restore(&auth, 7, &public()).unwrap();
        assert!(keys.sign(&auth, &key, b"sample").is_err(), "bit {bit}");
        assert!(trace.borrow().closes > 0);
    }
}
#[test]
fn hmac_root_stays_in_se_and_exact_hkdf_salt_info_and_length_are_sent() {
    let expected = hex("ca6556cdf639f4c9c416cf20fd3b5908f01eab1e5ebdd5dd716be97c5ed99e42");
    for (has_uv, id) in [(true, "41044f4b0201"), (false, "41044f4b0200")] {
        let mut command = hex(id);
        command.extend(hex("4201044320000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f440a6372656452616e646f6d45020020"));
        let mut reply = vec![0x41, 32];
        reply.extend(&expected);
        let (mut keys, trace) = model(vec![
            step([2, 0, 0x26], &hex(id), &hex("410111420101")),
            step([2, 0, 7], &hex(id), &hex("41020020")),
            step([3, 0, 0x2d], &command, &reply),
        ]);
        let salt = core::array::from_fn(|i| i as u8);
        assert_eq!(
            &keys
                .derive_cred_random(&auth(Operation::DeriveHmacSecret), has_uv, &salt)
                .unwrap()[..],
            expected
        );
        assert_eq!(trace.borrow().calls, 3);
        assert!(trace.borrow().steps.is_empty());
    }
}
#[test]
fn unrelated_authority_and_out_of_namespace_ids_never_reach_the_bus() {
    let (mut keys, trace) = model(vec![]);
    for operation in [
        Operation::RevealNote,
        Operation::ListPrivateMetadata,
        Operation::PairHost,
        Operation::FirmwareUpdate,
    ] {
        let a = auth(operation);
        assert_eq!(keys.generate(&a, 7).err(), Some(Error::Authorization));
        assert_eq!(
            keys.derive_cred_random(&a, true, &[0; 32]).err(),
            Some(Error::Authorization)
        );
    }
    assert!(
        keys.generate(&auth(Operation::FidoRegister), KEY_SLOTS)
            .is_err()
    );
    assert_eq!(trace.borrow().calls, 0);
}
#[test]
fn wrong_type_width_transience_changed_key_and_off_curve_point_are_terminal() {
    for mode in 0..6 {
        let mut steps = key_steps();
        match mode {
            0 => steps[0].reply = Ok(hex("410101420101")), // generic curve not enough
            1 => steps[0].reply = Ok(hex("410129420102")), // transient
            2 => steps[1].reply = Ok(hex("41020100")),     // bits rather than bytes
            3 => steps[2].reply.as_mut().unwrap()[5] ^= 1,
            4 => steps[2].reply.as_mut().unwrap()[2] = 0,
            _ => steps[0].reply.as_mut().unwrap().push(0),
        }
        // Failures before the public read deliberately leave later steps unused.
        let (mut keys, trace) = model(steps);
        assert!(
            keys.restore(&auth(Operation::FidoSign), 7, &public())
                .is_err()
        );
        assert!(trace.borrow().closes > 0);
    }
}
#[test]
fn malformed_derived_secret_is_never_returned() {
    for reply in [vec![], hex("410100"), vec![0x41; 34], vec![0; 35]] {
        let mut command = hex("41044f4b02014201044320");
        command.extend([0; 32]);
        command.extend(hex("440a6372656452616e646f6d45020020"));
        let (mut keys, trace) = model(vec![
            step([2, 0, 0x26], &hex("41044f4b0201"), &hex("410111420101")),
            step([2, 0, 7], &hex("41044f4b0201"), &hex("41020020")),
            step([3, 0, 0x2d], &command, &reply),
        ]);
        assert!(
            keys.derive_cred_random(&auth(Operation::DeriveHmacSecret), true, &[0; 32])
                .is_err()
        );
        assert!(trace.borrow().closes > 0);
    }
}
