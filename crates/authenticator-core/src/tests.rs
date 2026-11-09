extern crate std;
use super::*;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, Tag, aead::AeadInPlace};
use alloc::{vec, vec::Vec};
use device_policy::{
    Authorization, Gate, Request, SecureVerifier,
    biometric::{self, ProtectedAttempts, ProtectedGcm, Session},
};
use opensk::{
    api::{fingerprint::Fingerprint, persist::Persist},
    ctap::Channel,
    env::{Env, test::TestEnv},
};
use sk_cbor::{Value, cbor_array, cbor_map};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
const BINDING: [u8; 32] = [0xa5; 32];
const CHANNEL: Channel = Channel::MainHid([0, 0, 0, 7]);
#[derive(Default)]
struct Stats {
    begins: u32,
    ends: u32,
    checks: u32,
    fail_check: Option<u32>,
    active: bool,
    generation: u64,
    revoke_check: Option<u32>,
}
struct Model {
    inner: TestEnv,
    stats: Rc<RefCell<Stats>>,
}
impl Model {
    fn new() -> (Self, Rc<RefCell<Stats>>) {
        let mut inner = TestEnv::default();
        inner.fingerprint().prepare_enrollment().unwrap();
        let _ = inner.fingerprint().capture_sample(None).unwrap();
        let (_, remaining, template) = inner.fingerprint().capture_sample(None).unwrap();
        assert_eq!(remaining, 0);
        inner
            .persist()
            .store_template_id(template.unwrap())
            .unwrap();
        let stats = Rc::new(RefCell::new(Stats::default()));
        (
            Self {
                inner,
                stats: stats.clone(),
            },
            stats,
        )
    }
}
// This fixture is compiled only in tests. The production crate has no model
// Env, software signer or device authorization success switch.
impl Env for Model {
    type Rng = <TestEnv as Env>::Rng;
    type UserPresence = <TestEnv as Env>::UserPresence;
    type Persist = <TestEnv as Env>::Persist;
    type KeyStore = <TestEnv as Env>::KeyStore;
    type Write = <TestEnv as Env>::Write;
    type Customization = <TestEnv as Env>::Customization;
    type HidConnection = <TestEnv as Env>::HidConnection;
    type Clock = <TestEnv as Env>::Clock;
    type Crypto = <TestEnv as Env>::Crypto;
    type Fingerprint = <TestEnv as Env>::Fingerprint;
    fn rng(&mut self) -> &mut Self::Rng {
        self.inner.rng()
    }
    fn user_presence(&mut self) -> &mut Self::UserPresence {
        self.inner.user_presence()
    }
    fn persist(&mut self) -> &mut Self::Persist {
        self.inner.persist()
    }
    fn key_store(&mut self) -> &mut Self::KeyStore {
        self.inner.key_store()
    }
    fn clock(&mut self) -> &mut Self::Clock {
        self.inner.clock()
    }
    fn fingerprint(&mut self) -> &mut Self::Fingerprint {
        self.inner.fingerprint()
    }
    fn write(&mut self) -> Self::Write {
        self.inner.write()
    }
    fn customization(&self) -> &Self::Customization {
        self.inner.customization()
    }
    fn hid_connection(&mut self) -> &mut Self::HidConnection {
        self.inner.hid_connection()
    }
    fn boots_after_soft_reset(&self) -> bool {
        false
    }
}
impl Platform for Model {
    fn check_boot_and_channel(&mut self, binding: &[u8; 32]) -> Result<(), Error> {
        let mut stats = self.stats.borrow_mut();
        stats.checks += 1;
        if stats.revoke_check == Some(stats.checks) {
            stats.generation += 1;
        }
        if *binding != BINDING || stats.fail_check == Some(stats.checks) {
            Err(Error::State)
        } else {
            Ok(())
        }
    }
    fn authorization_generation(&self) -> u64 {
        self.stats.borrow().generation
    }
    fn begin_authorized(&mut self, _: device_policy::execution::Execution) -> Result<(), Error> {
        let mut stats = self.stats.borrow_mut();
        stats.begins += 1;
        assert!(!stats.active);
        stats.active = true;
        Ok(())
    }
    fn end_authorized(&mut self) {
        let mut stats = self.stats.borrow_mut();
        stats.ends += 1;
        stats.active = false;
    }
    fn reset_fido_authorized(&mut self) -> Result<Vec<u8>, Error> {
        Err(Error::Unsupported)
    }
}
struct Counter;
impl ProtectedAttempts for Counter {
    fn reserve_identify_attempt(&mut self, _: &Request) -> Result<u32, biometric::Error> {
        Ok(1)
    }
    fn record_match(&mut self, _: &biometric::AuthenticatedMatch) -> Result<(), biometric::Error> {
        Ok(())
    }
}
struct Gcm;
impl ProtectedGcm for Gcm {
    fn decrypt_identify_payload(
        &mut self,
        h: &[u8; 8],
        iv: &[u8; 12],
        tag: &[u8; 16],
        body: &mut [u8; 12],
    ) -> Result<(), biometric::Error> {
        Aes256Gcm::new_from_slice(&[0x72; 32])
            .unwrap()
            .decrypt_in_place_detached(Nonce::from_slice(iv), h, body, Tag::from_slice(tag))
            .map_err(|_| biometric::Error::Unauthenticated)
    }
}
struct Verifier {
    pin_calls: Rc<Cell<usize>>,
}
impl SecureVerifier for Verifier {
    fn check_security_state(&mut self) -> Result<(), device_policy::Error> {
        Ok(())
    }
    fn verify_fingerprint(
        &mut self,
        r: &Request,
    ) -> Result<biometric::VerifiedRequest, device_policy::Error> {
        let mut session = Session::new(Counter, Gcm);
        let tag = session.begin(*r, 7, 10).unwrap();
        let header = [4, 0, 0x13, 0, 0x41, 0, 40, 0];
        let iv = [0x39; 12];
        let mut body = [0u8; 12];
        for (i, word) in [0x55u16, 0x13, 0x61ec, 0x3034, 7, tag]
            .into_iter()
            .enumerate()
        {
            body[2 * i..2 * i + 2].copy_from_slice(&word.to_le_bytes());
        }
        let mac = Aes256Gcm::new_from_slice(&[0x72; 32])
            .unwrap()
            .encrypt_in_place_detached(Nonce::from_slice(&iv), &header, &mut body)
            .unwrap();
        let mut wire = header.to_vec();
        wire.extend(iv);
        wire.extend(mac);
        wire.extend(body);
        session
            .receive(&wire, r, || 10)
            .unwrap()
            .ok_or(device_policy::Error::Rejected)
    }
    fn verify_additional_pin(
        &mut self,
        _: &biometric::VerifiedRequest,
    ) -> Result<(), device_policy::Error> {
        self.pin_calls.set(self.pin_calls.get() + 1);
        Ok(())
    }
    fn confirm_on_device(
        &mut self,
        _: &biometric::VerifiedRequest,
    ) -> Result<(), device_policy::Error> {
        Ok(())
    }
}
fn authority(r: &Request) -> Authorization {
    let mut gate = Gate::new(Verifier {
        pin_calls: Rc::new(Cell::new(0)),
    });
    gate.approve(*r, || 10).unwrap();
    gate.consume_authorized(r, || 10).unwrap()
}
fn encode(cmd: u8, value: Value) -> Vec<u8> {
    let mut bytes = vec![cmd];
    opensk::ctap::cbor_write(value, &mut bytes).unwrap();
    bytes
}
fn make_credential() -> Vec<u8> {
    encode(
        1,
        cbor_map! {
            1=>vec![0xc1u8;32],2=>cbor_map!{"id"=>"example.com","name"=>"attacker.invalid"},
            3=>cbor_map!{"id"=>vec![7u8],"name"=>"public-fixture"},
            4=>cbor_array![cbor_map!{"alg"=>-7,"type"=>"public-key"}],
            6=>cbor_map!{"hmac-secret"=>true},7=>cbor_map!{"rk"=>true,"uv"=>false}
        },
    )
}
fn get_assertion(hash: u8) -> Vec<u8> {
    encode(
        2,
        cbor_map! {1=>"example.com",2=>vec![hash;32],5=>cbor_map!{"up"=>false,"uv"=>false}},
    )
}
fn run(core: &mut Processor<Model>, bytes: &[u8]) -> Vec<u8> {
    let prepared = prepare_ctap2(bytes, BINDING, CHANNEL).unwrap();
    let auth = authority(prepared.request().unwrap());
    core.authorized(prepared, auth, || 10).unwrap().to_vec()
}
fn response_map(bytes: &[u8]) -> Vec<(Value, Value)> {
    assert_eq!(bytes[0], 0, "CTAP status {:#x}", bytes[0]);
    opensk::ctap::cbor_read(&bytes[1..])
        .unwrap()
        .extract_map()
        .unwrap()
}
fn get(map: &[(Value, Value)], key: i64) -> Value {
    map.iter()
        .find(|(k, _)| *k == Value::integer(key))
        .unwrap()
        .1
        .clone()
}

#[test]
fn next_assertions_need_fresh_bound_authority_and_cannot_replay_a_consumed_cursor() {
    let (env, _) = Model::new();
    let mut core = Processor::new(env);
    run(&mut core, &make_credential());
    let raw = make_credential();
    let mut map = opensk::ctap::cbor_read(&raw[1..])
        .unwrap()
        .extract_map()
        .unwrap();
    let user = map
        .iter_mut()
        .find(|(k, _)| *k == Value::unsigned(3))
        .unwrap();
    user.1 = cbor_map! {"id"=>vec![8u8],"name"=>"second-public-fixture"};
    run(&mut core, &encode(1, Value::map(map)));
    let first = run(&mut core, &get_assertion(0x51));
    assert_eq!(get(&response_map(&first), 5).extract_unsigned(), Some(2));
    assert!(core.prepare(&[8], [1; 32], CHANNEL, 10).is_err());
    assert!(
        core.prepare(&[8], BINDING, Channel::MainHid([0, 0, 0, 8]), 10)
            .is_err()
    );
    let next = core.prepare(&[8], BINDING, CHANNEL, 10).unwrap();
    let stale = core.prepare(&[8], BINDING, CHANNEL, 10).unwrap();
    let stale_authority = authority(stale.request().unwrap());
    assert!(matches!(next.review(),Review::Website(site) if site=="example.com"));
    let auth = authority(next.request().unwrap());
    assert_eq!(core.authorized(next, auth, || 10).unwrap()[0], 0);
    assert!(matches!(
        core.authorized(stale, stale_authority, || 10),
        Err(Error::Authorization)
    ));
    assert!(core.prepare(&[8], BINDING, CHANNEL, 10).is_err());
}

#[test]
fn u2f_authentication_is_fingerprint_gated_and_signature_verifies() {
    use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier as _};
    use sha2::{Digest, Sha256};
    let (env, _) = Model::new();
    let mut core = Processor::new(env);
    // U2F key handles are non-discoverable wrapped credentials, not the
    // random IDs used to look up resident/passkey records.
    let raw = make_credential();
    let mut map = opensk::ctap::cbor_read(&raw[1..])
        .unwrap()
        .extract_map()
        .unwrap();
    map.iter_mut()
        .find(|(k, _)| *k == Value::unsigned(7))
        .unwrap()
        .1 = cbor_map! {"rk"=>false,"uv"=>false};
    let response = run(&mut core, &encode(1, Value::map(map)));
    let map = response_map(&response);
    let data = get(&map, 2).extract_byte_string().unwrap();
    let id_len = u16::from_be_bytes(data[53..55].try_into().unwrap()) as usize;
    let id = &data[55..55 + id_len];
    let cose = opensk::ctap::cbor_read(&data[55 + id_len..55 + id_len + 77])
        .unwrap()
        .extract_map()
        .unwrap();
    let mut public = vec![4];
    public.extend(get(&cose, -2).extract_byte_string().unwrap());
    public.extend(get(&cose, -3).extract_byte_string().unwrap());
    let app: [u8; 32] = Sha256::digest(b"example.com").into();
    let challenge = [0x71; 32];
    let mut body = challenge.to_vec();
    body.extend(app);
    body.push(u8::try_from(id.len()).unwrap());
    body.extend(id);
    let mut apdu = vec![0, 2, 8, 0, 0];
    apdu.extend(u16::try_from(body.len()).unwrap().to_be_bytes());
    apdu.extend(body);
    let request = prepare_u2f(&apdu, BINDING, CHANNEL).unwrap();
    assert!(matches!(request.review(),Review::ApplicationHash(hash) if hash==&app));
    assert!(core.public(request).is_err());
    let request = prepare_u2f(&apdu, BINDING, CHANNEL).unwrap();
    let auth = authority(request.request().unwrap());
    let response = core.authorized(request, auth, || 10).unwrap();
    assert_eq!(&response[response.len() - 2..], &[0x90, 0]);
    assert_eq!(response[0], 1);
    let mut signed = app.to_vec();
    signed.extend(&response[..5]);
    signed.extend(challenge);
    VerifyingKey::from_sec1_bytes(&public)
        .unwrap()
        .verify(
            &signed,
            &Signature::from_der(&response[5..response.len() - 2]).unwrap(),
        )
        .unwrap();
}

#[test]
fn u2f_version_is_public_but_vendor_and_malformed_apdus_are_not() {
    let (env, _) = Model::new();
    let mut core = Processor::new(env);
    let request = prepare_u2f(&[0, 3, 0, 0, 0, 0, 0], BINDING, CHANNEL).unwrap();
    assert_eq!(core.public(request).unwrap().as_slice(), b"U2F_V2\x90\x00");
    for bytes in [
        &[0, 0x40, 0, 0][..],
        &[0, 1, 0, 0, 0, 0, 64],
        &[1, 3, 0, 0],
        &[0, 2, 3, 0, 0, 0, 255],
    ] {
        assert!(prepare_u2f(bytes, BINDING, CHANNEL).is_err());
    }
}

#[test]
fn bounded_untrusted_request_corpus_never_panics() {
    let mut random = 0x9137u64;
    for len in 0..1024 {
        let mut raw = vec![0; len];
        for byte in &mut raw {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            *byte = (random >> 32) as u8;
        }
        let _ = prepare_ctap2(&raw, BINDING, CHANNEL);
        let _ = prepare_u2f(&raw, BINDING, CHANNEL);
    }
}

#[test]
fn real_make_credential_and_assertion_have_forced_uv_and_a_verifiable_signature() {
    use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier as _};
    let (env, stats) = Model::new();
    let mut core = Processor::new(env);
    let prepared = prepare_ctap2(&make_credential(), BINDING, CHANNEL).unwrap();
    assert!(matches!(prepared.review(),Review::Website(s) if s=="example.com"));
    let auth = authority(prepared.request().unwrap());
    let make = core.authorized(prepared, auth, || 10).unwrap();
    let make = response_map(&make);
    let auth_data = get(&make, 2).extract_byte_string().unwrap();
    assert_eq!(auth_data[32] & 5, 5); // Both UP and UV despite host flags=false.
    let id_len = u16::from_be_bytes(auth_data[53..55].try_into().unwrap()) as usize;
    let credential_id = &auth_data[55..55 + id_len];
    // ES256 COSE key is77 bytes; extension output follows it when ED is set.
    let cose = opensk::ctap::cbor_read(&auth_data[55 + id_len..55 + id_len + 77])
        .unwrap()
        .extract_map()
        .unwrap();
    let mut public = vec![4];
    public.extend(get(&cose, -2).extract_byte_string().unwrap());
    public.extend(get(&cose, -3).extract_byte_string().unwrap());
    let assertion = run(&mut core, &get_assertion(0x51));
    let assertion = response_map(&assertion);
    let credential = get(&assertion, 1).extract_map().unwrap();
    assert!(
        credential
            .iter()
            .any(|(k, v)| *k == Value::text_string("id".into())
                && v.clone().extract_byte_string().as_deref() == Some(credential_id))
    );
    let data = get(&assertion, 2).extract_byte_string().unwrap();
    assert_eq!(data[32] & 5, 5);
    let signature = get(&assertion, 3).extract_byte_string().unwrap();
    let mut signed = data;
    signed.extend([0x51; 32]);
    VerifyingKey::from_sec1_bytes(&public)
        .unwrap()
        .verify(&signed, &Signature::from_der(&signature).unwrap())
        .unwrap();
    assert_eq!(stats.borrow().begins, 2);
    assert!(!stats.borrow().active);
}

#[test]
fn reset_dispatch_cannot_partially_delete_shared_fingerprints_via_upstream_fallback() {
    let (env, stats) = Model::new();
    let mut core = Processor::new(env);
    assert_eq!(run(&mut core, &make_credential())[0], 0);
    let reset = prepare_ctap2(&[7], BINDING, CHANNEL).unwrap();
    let authorization = authority(reset.request().unwrap());
    assert!(matches!(
        core.authorized(reset, authorization, || 10),
        Err(Error::Unsupported)
    ));
    assert!(!stats.borrow().active);
    assert_eq!(run(&mut core, &get_assertion(0x51))[0], 0);
}

#[test]
fn multi_account_limit_uses_the_trusted_platform_capacity_instead_of_a_smaller_magic_number() {
    let (env, _) = Model::new();
    let mut core = Processor::new(env);
    for account in 0u8..101 {
        let raw = make_credential();
        let mut map = opensk::ctap::cbor_read(&raw[1..])
            .unwrap()
            .extract_map()
            .unwrap();
        let user = map
            .iter_mut()
            .find(|(k, _)| *k == Value::unsigned(3))
            .unwrap();
        user.1 = cbor_map! {"id"=>vec![account],"name"=>"fixture"};
        assert_eq!(run(&mut core, &encode(1, Value::map(map)))[0], 0);
    }
    let response = run(&mut core, &get_assertion(0x51));
    let map = response_map(&response);
    assert_eq!(get(&map, 5).extract_unsigned(), Some(101));
    let next = core.prepare(&[8], BINDING, CHANNEL, 11).unwrap();
    let authorization = authority(next.request().unwrap());
    assert_eq!(core.authorized(next, authorization, || 11).unwrap()[0], 0);
}

#[test]
fn hmac_secret_v1_and_v2_round_trip_and_authenticate_salts_after_fingerprint() {
    use opensk::{
        api::crypto::{
            aes256::Aes256,
            ecdh::{PublicKey as _, SecretKey as _, SharedSecret as _},
            hkdf256::Hkdf256,
            hmac256::Hmac256,
            sha256::Sha256,
        },
        ctap::data_formats::CoseKey,
        env::{AesKey, EcdhPk, EcdhSk, Hkdf, Hmac, Sha},
    };
    for protocol_number in [1, 2] {
        let (env, _) = Model::new();
        let mut core = Processor::new(env);
        assert_eq!(run(&mut core, &make_credential())[0], 0);
        let mut client_env = TestEnv::default();
        let client = EcdhSk::<TestEnv>::random(client_env.rng());
        let client_public = CoseKey::from_ecdh_public_key::<TestEnv>(client.public_key());
        let key_request = encode(6, cbor_map! {1=>protocol_number,2=>2});
        let response = core
            .public(prepare_ctap2(&key_request, BINDING, CHANNEL).unwrap())
            .unwrap();
        let server_key = get(&response_map(&response), 1).extract_map().unwrap();
        let x: [u8; 32] = get(&server_key, -2)
            .extract_byte_string()
            .unwrap()
            .try_into()
            .unwrap();
        let y: [u8; 32] = get(&server_key, -3)
            .extract_byte_string()
            .unwrap()
            .try_into()
            .unwrap();
        let peer = EcdhPk::<TestEnv>::from_coordinates(&x, &y).unwrap();
        let mut handshake = [0u8; 32];
        client
            .diffie_hellman(&peer)
            .raw_secret_bytes(&mut handshake);
        // Client protocol orchestration is independent of the server's
        // private pin_protocol module, including version-specific KDF/IV/MAC.
        let (aes_key, mac_key) = if protocol_number == 1 {
            let key = Sha::<TestEnv>::digest(&handshake);
            (key, key)
        } else {
            let mut aes = [0; 32];
            let mut mac = [0; 32];
            Hkdf::<TestEnv>::hkdf_256(&handshake, &[0; 32], b"CTAP2 AES key", &mut aes);
            Hkdf::<TestEnv>::hkdf_256(&handshake, &[0; 32], b"CTAP2 HMAC key", &mut mac);
            (aes, mac)
        };
        let aes = AesKey::<TestEnv>::new(&aes_key);
        let salt = vec![0x63; 64];
        let iv = if protocol_number == 1 {
            [0; 16]
        } else {
            [0x27; 16]
        };
        let mut encrypted = salt.clone();
        aes.encrypt_cbc(&iv, &mut encrypted);
        if protocol_number == 2 {
            let mut prefixed = iv.to_vec();
            prefixed.extend(encrypted);
            encrypted = prefixed;
        }
        let mut mac = [0; 32];
        Hmac::<TestEnv>::mac(&mac_key, &encrypted, &mut mac);
        let authentication = mac[..if protocol_number == 1 { 16 } else { 32 }].to_vec();
        let mut expected = None;
        for pass in 0..3 {
            let mut auth = authentication.clone();
            if pass == 2 {
                auth[0] ^= 1;
            }
            let request = encode(
                2,
                cbor_map! {
                    1=>"example.com",2=>vec![0x31u8;32],
                    4=>cbor_map!{"hmac-secret"=>cbor_map!{
                        1=>Value::from(client_public.clone()),2=>encrypted.clone(),3=>auth,4=>protocol_number
                    }},5=>cbor_map!{"uv"=>false}
                },
            );
            let response = run(&mut core, &request);
            if pass == 2 {
                assert_ne!(response[0], 0);
                assert_eq!(response.len(), 1);
                continue;
            }
            let map = response_map(&response);
            let data = get(&map, 2).extract_byte_string().unwrap();
            assert_ne!(data[32] & 0x80, 0);
            let extensions = opensk::ctap::cbor_read(&data[37..])
                .unwrap()
                .extract_map()
                .unwrap();
            let cipher = extensions
                .iter()
                .find(|(k, _)| *k == Value::text_string("hmac-secret".into()))
                .unwrap()
                .1
                .clone()
                .extract_byte_string()
                .unwrap();
            let (iv, mut plain) = if protocol_number == 1 {
                ([0; 16], cipher)
            } else {
                (cipher[..16].try_into().unwrap(), cipher[16..].to_vec())
            };
            aes.decrypt_cbc(&iv, &mut plain);
            assert_eq!(plain.len(), 64);
            assert_eq!(&plain[..32], &plain[32..]); // equal salts -> equal secrets.
            if let Some(before) = &expected {
                assert_eq!(&plain, before);
            } else {
                expected = Some(plain);
            }
        }
    }
}
#[test]
fn public_info_hides_private_capacity_and_does_not_advertise_host_pin() {
    let (env, stats) = Model::new();
    let mut core = Processor::new(env);
    let info = core
        .public(prepare_ctap2(&[4], BINDING, CHANNEL).unwrap())
        .unwrap();
    let map = response_map(&info);
    assert!(!map.iter().any(|(k, _)| *k == Value::unsigned(20)));
    let options = get(&map, 4).extract_map().unwrap();
    assert!(
        !options
            .iter()
            .any(|(k, _)| *k == Value::text_string("clientPin".into()))
    );
    assert!(
        options
            .iter()
            .any(|(k, v)| *k == Value::text_string("alwaysUv".into())
                && *v == Value::bool_value(true))
    );
    assert_eq!(stats.borrow().begins, 0);
    assert!(matches!(
        core.public(prepare_ctap2(&make_credential(), BINDING, CHANNEL).unwrap()),
        Err(Error::Authorization)
    ));
}
#[test]
fn changed_payload_channel_or_expired_authority_never_enters_crypto_scope() {
    let (env, stats) = Model::new();
    let mut core = Processor::new(env);
    let prepared = prepare_ctap2(&make_credential(), BINDING, CHANNEL).unwrap();
    let auth = authority(prepared.request().unwrap());
    let other = prepare_ctap2(&get_assertion(3), BINDING, CHANNEL).unwrap();
    assert!(matches!(
        core.authorized(other, auth, || 10),
        Err(Error::Authorization)
    ));
    let prepared = prepare_ctap2(&make_credential(), [1; 32], CHANNEL).unwrap();
    let auth = authority(prepared.request().unwrap());
    assert!(matches!(
        core.authorized(prepared, auth, || 10),
        Err(Error::State)
    ));
    let prepared = prepare_ctap2(&make_credential(), BINDING, CHANNEL).unwrap();
    let auth = authority(prepared.request().unwrap());
    assert!(matches!(
        core.authorized(prepared, auth, || 15_010),
        Err(Error::Time)
    ));
    assert_eq!(stats.borrow().begins, 0);
}
#[test]
fn rollback_or_late_platform_fault_erases_the_response_and_ends_scope() {
    for fail in [false, true] {
        let (env, stats) = Model::new();
        let mut core = Processor::new(env);
        let prepared = prepare_ctap2(&make_credential(), BINDING, CHANNEL).unwrap();
        let auth = authority(prepared.request().unwrap());
        if fail {
            stats.borrow_mut().fail_check = Some(3);
        }
        let mut ticks = [10, 10, 10, 20, 15].into_iter();
        assert!(
            core.authorized(prepared, auth, || ticks.next().unwrap())
                .is_err()
        );
        assert!(!stats.borrow().active);
        assert_eq!(stats.borrow().ends, 1);
    }
}

#[test]
fn stale_generation_before_activation_or_during_dispatch_never_releases_a_response() {
    for mode in 0..3 {
        let (env, stats) = Model::new();
        let mut core = Processor::new(env);
        let prepared = prepare_ctap2(&make_credential(), BINDING, CHANNEL).unwrap();
        let authorization = authority(prepared.request().unwrap());
        if mode == 0 {
            stats.borrow_mut().generation = 1;
        } else if mode == 2 {
            stats.borrow_mut().revoke_check = Some(3);
        }
        let mut revoked = false;
        let response = core.authorized(prepared, authorization, || {
            if mode == 1 && stats.borrow().active && !revoked {
                stats.borrow_mut().generation += 1;
                revoked = true;
            }
            10
        });
        assert!(matches!(response, Err(Error::Authorization)));
        assert!(!stats.borrow().active);
        assert_eq!(stats.borrow().ends, 1);
        assert_eq!(stats.borrow().begins, if mode == 0 { 0 } else { 1 });
    }
}

#[test]
fn rollback_after_the_last_activation_check_prevents_the_first_private_operation() {
    let (env, stats) = Model::new();
    let mut core = Processor::new(env);
    let prepared = prepare_ctap2(&make_credential(), BINDING, CHANNEL).unwrap();
    let authorization = authority(prepared.request().unwrap());
    let mut ticks = [10, 20, 25, 24, 26].into_iter();
    assert!(matches!(
        core.authorized(prepared, authorization, || ticks.next().unwrap()),
        Err(Error::Time)
    ));
    assert!(!stats.borrow().active);
    assert_eq!(stats.borrow().ends, 1);
    // The early failure did not register a credential before suppressing output.
    assert_eq!(run(&mut core, &get_assertion(0x51))[0], 0x2e);
}
#[test]
fn forbidden_host_pin_mutations_bad_rp_ids_and_noncanonical_requests_are_rejected() {
    for subcmd in [1, 3, 4, 5, 7, 8, 9] {
        assert!(matches!(
            prepare_ctap2(&encode(6, cbor_map! {1=>1,2=>subcmd}), BINDING, CHANNEL),
            Err(Error::Unsupported)
        ));
    }
    for id in [
        "https://example.com",
        "example.com:443",
        "evil@example.com",
        "-example.com",
        "example..com",
        "\u{0435}xample.com",
    ] {
        assert!(
            prepare_ctap2(
                &encode(2, cbor_map! {1=>id,2=>vec![0u8;32]}),
                BINDING,
                CHANNEL
            )
            .is_err()
        );
    }
    for bytes in [
        vec![4, 0],
        vec![2, 0xa2, 1, 0, 1, 0],
        vec![2, 0xbf, 0xff],
        vec![0x0d, 0xa0],
    ] {
        assert!(prepare_ctap2(&bytes, BINDING, CHANNEL).is_err());
    }
}
