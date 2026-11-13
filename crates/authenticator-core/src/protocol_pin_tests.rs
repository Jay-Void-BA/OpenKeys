//! Synthetic client-side CTAP1/2 PIN protocol interoperability. No production
//! software key store, PIN constant or authorization bypass is exported.
use super::*;
use opensk::{
    api::crypto::{
        aes256::Aes256,
        ecdh::{PublicKey as _, SecretKey as _, SharedSecret as _},
        hkdf256::Hkdf256,
        hmac256::Hmac256,
        sha256::Sha256,
    },
    ctap::{data_formats::CoseKey, status_code::Ctap2StatusCode as Status},
    env::{AesKey, EcdhPk, EcdhSk, Hkdf, Hmac, LocalPin, Sha},
};

const HOST_PIN: &[u8] = b"host-PIN-only-8765";
struct HostPin {
    protocol: u64,
    public: CoseKey,
    aes: [u8; 32],
    mac: [u8; 32],
}
impl HostPin {
    fn new(core: &mut Processor<Model>, protocol: u64) -> Self {
        let mut client_env = TestEnv::default();
        let client = EcdhSk::<TestEnv>::random(client_env.rng());
        let public = CoseKey::from_ecdh_public_key::<TestEnv>(client.public_key());
        let request = encode(6, cbor_map! {1=>protocol,2=>2});
        let response = core
            .public(prepare_ctap2(&request, BINDING, CHANNEL).unwrap())
            .unwrap();
        let server = get(&response_map(&response), 1).extract_map().unwrap();
        let x = get(&server, -2)
            .extract_byte_string()
            .unwrap()
            .try_into()
            .unwrap();
        let y = get(&server, -3)
            .extract_byte_string()
            .unwrap()
            .try_into()
            .unwrap();
        let peer = EcdhPk::<TestEnv>::from_coordinates(&x, &y).unwrap();
        let mut shared = [0; 32];
        client.diffie_hellman(&peer).raw_secret_bytes(&mut shared);
        let (aes, mac) = if protocol == 1 {
            let key = Sha::<TestEnv>::digest(&shared);
            (key, key)
        } else {
            let mut aes = [0; 32];
            let mut mac = [0; 32];
            Hkdf::<TestEnv>::hkdf_256(&shared, &[0; 32], b"CTAP2 AES key", &mut aes);
            Hkdf::<TestEnv>::hkdf_256(&shared, &[0; 32], b"CTAP2 HMAC key", &mut mac);
            (aes, mac)
        };
        Self {
            protocol,
            public,
            aes,
            mac,
        }
    }
    fn encrypt(&self, plain: &[u8]) -> Vec<u8> {
        let iv = if self.protocol == 1 {
            [0; 16]
        } else {
            [0x41; 16]
        };
        let mut encrypted = plain.to_vec();
        AesKey::<TestEnv>::new(&self.aes).encrypt_cbc(&iv, &mut encrypted);
        if self.protocol == 2 {
            let mut result = iv.to_vec();
            result.extend(encrypted);
            result
        } else {
            encrypted
        }
    }
    fn decrypt(&self, encrypted: &[u8]) -> Vec<u8> {
        let (iv, body) = if self.protocol == 1 {
            ([0; 16], encrypted)
        } else {
            (encrypted[..16].try_into().unwrap(), &encrypted[16..])
        };
        let mut plain = body.to_vec();
        AesKey::<TestEnv>::new(&self.aes).decrypt_cbc(&iv, &mut plain);
        plain
    }
    fn auth(&self, key: &[u8; 32], message: &[u8]) -> Vec<u8> {
        let mut mac = [0; 32];
        Hmac::<TestEnv>::mac(key, message, &mut mac);
        mac[..if self.protocol == 1 { 16 } else { 32 }].to_vec()
    }
    fn set_pin(&self) -> Vec<u8> {
        let mut plain = [0; 64];
        plain[..HOST_PIN.len()].copy_from_slice(HOST_PIN);
        let encrypted = self.encrypt(&plain);
        encode(
            6,
            cbor_map! {1=>self.protocol,2=>3,3=>self.public.clone(),4=>self.auth(&self.mac,&encrypted),5=>encrypted},
        )
    }
    fn token(&self, pin: &[u8]) -> Vec<u8> {
        let hash = Sha::<TestEnv>::digest(pin);
        encode(
            6,
            cbor_map! {1=>self.protocol,2=>9,3=>self.public.clone(),6=>self.encrypt(&hash[..16]),9=>3,10=>"example.com"},
        )
    }
    fn local_token(&self) -> Vec<u8> {
        encode(
            6,
            cbor_map! {1=>self.protocol,2=>6,3=>self.public.clone(),9=>3,10=>"example.com"},
        )
    }
    fn change_pin(&self, old: &[u8], new: &[u8]) -> Vec<u8> {
        let mut plain = [0; 64];
        plain[..new.len()].copy_from_slice(new);
        let new_enc = self.encrypt(&plain);
        let old_enc = self.encrypt(&Sha::<TestEnv>::digest(old)[..16]);
        let mut both = new_enc.clone();
        both.extend(&old_enc);
        encode(
            6,
            cbor_map! {1=>self.protocol,2=>4,3=>self.public.clone(),4=>self.auth(&self.mac,&both),5=>new_enc,6=>old_enc},
        )
    }
    fn credential(&self, token: &[u8; 32]) -> Vec<u8> {
        let raw = make_credential();
        let mut map = opensk::ctap::cbor_read(&raw[1..])
            .unwrap()
            .extract_map()
            .unwrap();
        map.push((
            Value::unsigned(8),
            Value::byte_string(self.auth(token, &[0xc1; 32])),
        ));
        map.push((Value::unsigned(9), Value::unsigned(self.protocol)));
        encode(1, Value::map(map))
    }
    fn assertion(&self, token: &[u8; 32]) -> Vec<u8> {
        let raw = get_assertion(0x53);
        let mut map = opensk::ctap::cbor_read(&raw[1..])
            .unwrap()
            .extract_map()
            .unwrap();
        map.push((
            Value::unsigned(6),
            Value::byte_string(self.auth(token, &[0x53; 32])),
        ));
        map.push((Value::unsigned(7), Value::unsigned(self.protocol)));
        encode(2, Value::map(map))
    }
}
fn private(core: &mut Processor<Model>, bytes: &[u8], stats: &Rc<RefCell<Stats>>) -> Vec<u8> {
    let prepared = prepare_ctap2(bytes, BINDING, CHANNEL).unwrap();
    let auth = authority_at(prepared.request().unwrap(), stats.borrow().generation);
    core.authorized(prepared, auth, || 10).unwrap().to_vec()
}

#[test]
fn independent_host_pin_requires_both_proofs_and_v1_v2_tokens_work() {
    for protocol in [1, 2] {
        let (env, stats) = Model::new();
        let mut core = Processor::new(env);
        let client = HostPin::new(&mut core, protocol);
        assert_eq!(private(&mut core, &make_credential(), &stats)[0], 0);
        assert_eq!(private(&mut core, &client.set_pin(), &stats), [0]);
        assert_eq!(
            private(&mut core, &make_credential(), &stats),
            [Status::CTAP2_ERR_PUAT_REQUIRED as u8]
        );
        let response = private(&mut core, &client.token(HOST_PIN), &stats);
        let token: [u8; 32] = client
            .decrypt(
                &get(&response_map(&response), 2)
                    .extract_byte_string()
                    .unwrap(),
            )
            .try_into()
            .unwrap();
        let credential = client.credential(&token);
        stats.borrow_mut().deny_local_pin = true;
        let prepared = prepare_ctap2(&credential, BINDING, CHANNEL).unwrap();
        let auth = authority_at(prepared.request().unwrap(), core.authorization_generation());
        assert!(matches!(
            core.authorized(prepared, auth, || 10),
            Err(Error::Authorization)
        ));
        assert!(!stats.borrow().active);
        stats.borrow_mut().deny_local_pin = false;
        // A failed live hardware-PIN check must also revoke the earlier host
        // token. A new hardware proof cannot silently revive the other layer.
        assert_eq!(
            private(&mut core, &client.assertion(&token), &stats),
            [Status::CTAP2_ERR_PIN_AUTH_INVALID as u8]
        );
        let client = HostPin::new(&mut core, protocol);
        let response = private(&mut core, &client.token(HOST_PIN), &stats);
        let token: [u8; 32] = client
            .decrypt(
                &get(&response_map(&response), 2)
                    .extract_byte_string()
                    .unwrap(),
            )
            .try_into()
            .unwrap();
        // Setting the HOST protocol PIN must not erase an existing credential
        // or mutate the separate hardware-PIN namespace.
        assert_eq!(private(&mut core, &client.assertion(&token), &stats)[0], 0);
        // Upstream consumes the CTAP token permissions after a private use;
        // management-session leases must not silently revive those flags.
        let next = private(&mut core, &client.token(HOST_PIN), &stats);
        let next_token: [u8; 32] = client
            .decrypt(&get(&response_map(&next), 2).extract_byte_string().unwrap())
            .try_into()
            .unwrap();
        assert_eq!(
            private(&mut core, &client.credential(&next_token), &stats)[0],
            0
        );
        let info = core
            .public(prepare_ctap2(&[4], BINDING, CHANNEL).unwrap())
            .unwrap();
        let map = response_map(&info);
        let options = get(&map, 4).extract_map().unwrap();
        for (name, expected) in [
            ("clientPin", true),
            ("uv", false),
            ("alwaysUv", true),
            ("makeCredUvNotRqd", false),
        ] {
            assert!(
                options
                    .iter()
                    .any(|(k, v)| *k == Value::text_string(name.into())
                        && *v == Value::bool_value(expected))
            );
        }
        assert_eq!(get(&map, 18).extract_unsigned(), Some(4));
        assert!(
            !options
                .iter()
                .any(|(k, _)| *k == Value::text_string("bioEnroll".into()))
        );
    }
}
#[test]
fn lock_invalidates_tokens_and_captured_encrypted_protocol_pin() {
    for protocol in [1, 2] {
        let (env, stats) = Model::new();
        let mut core = Processor::new(env);
        let client = HostPin::new(&mut core, protocol);
        assert_eq!(private(&mut core, &client.set_pin(), &stats), [0]);
        let captured = client.token(HOST_PIN);
        let response = private(&mut core, &captured, &stats);
        let token: [u8; 32] = client
            .decrypt(
                &get(&response_map(&response), 2)
                    .extract_byte_string()
                    .unwrap(),
            )
            .try_into()
            .unwrap();
        core.lock();
        assert_eq!(
            private(&mut core, &client.credential(&token), &stats),
            [Status::CTAP2_ERR_PIN_AUTH_INVALID as u8]
        );
        assert_ne!(private(&mut core, &captured, &stats)[0], 0);
        let fresh = HostPin::new(&mut core, protocol);
        assert_eq!(private(&mut core, &fresh.token(HOST_PIN), &stats)[0], 0);
    }
}
#[test]
fn changing_protocol_pin_revokes_old_tokens_and_preserves_credentials() {
    for protocol in [1, 2] {
        let (env, stats) = Model::new();
        let mut core = Processor::new(env);
        assert_eq!(private(&mut core, &make_credential(), &stats)[0], 0);
        let client = HostPin::new(&mut core, protocol);
        assert_eq!(private(&mut core, &client.set_pin(), &stats), [0]);
        let response = private(&mut core, &client.token(HOST_PIN), &stats);
        let old_token: [u8; 32] = client
            .decrypt(
                &get(&response_map(&response), 2)
                    .extract_byte_string()
                    .unwrap(),
            )
            .try_into()
            .unwrap();
        let new = b"second-protocol-PIN";
        assert_eq!(
            private(&mut core, &client.change_pin(HOST_PIN, new), &stats),
            [0]
        );
        assert_eq!(
            private(&mut core, &client.assertion(&old_token), &stats),
            [Status::CTAP2_ERR_PIN_AUTH_INVALID as u8]
        );
        assert_eq!(
            private(&mut core, &client.token(HOST_PIN), &stats),
            [Status::CTAP2_ERR_PIN_INVALID as u8]
        );
        // An incorrect protocol PIN rotates the server ECDH key. A correct
        // next attempt must obtain a fresh agreement rather than replay it.
        let next_client = HostPin::new(&mut core, protocol);
        let response = private(&mut core, &next_client.token(new), &stats);
        let fresh: [u8; 32] = next_client
            .decrypt(
                &get(&response_map(&response), 2)
                    .extract_byte_string()
                    .unwrap(),
            )
            .try_into()
            .unwrap();
        assert_eq!(
            private(&mut core, &next_client.assertion(&fresh), &stats)[0],
            0
        );
    }
}
#[test]
fn locking_cannot_refund_consecutive_wrong_protocol_pin_attempts() {
    for protocol in [1, 2] {
        let (env, stats) = Model::new();
        let mut core = Processor::new(env);
        let first = HostPin::new(&mut core, protocol);
        assert_eq!(private(&mut core, &first.set_pin(), &stats), [0]);
        for attempt in 0..3 {
            core.lock();
            let client = HostPin::new(&mut core, protocol);
            let result = private(&mut core, &client.token(b"wrong-host-PIN"), &stats);
            assert_eq!(
                result,
                [if attempt == 2 {
                    Status::CTAP2_ERR_PIN_AUTH_BLOCKED
                } else {
                    Status::CTAP2_ERR_PIN_INVALID
                } as u8]
            );
        }
        core.lock();
        let client = HostPin::new(&mut core, protocol);
        assert_eq!(
            private(&mut core, &client.token(HOST_PIN), &stats),
            [Status::CTAP2_ERR_PIN_AUTH_BLOCKED as u8]
        );
    }
}
#[test]
fn protocol_pin_commands_never_use_public_or_biometric_authorization() {
    for subcommand in [1, 3, 4, 5, 6, 9] {
        let bytes = encode(6, cbor_map! {1=>2,2=>subcommand,9=>3});
        let prepared = prepare_ctap2(&bytes, BINDING, CHANNEL).unwrap();
        assert_eq!(
            prepared.request().unwrap().operation,
            device_policy::Operation::ProtocolPin
        );
        let (env, _) = Model::new();
        assert!(matches!(
            Processor::new(env).public(prepared),
            Err(Error::Authorization)
        ));
    }
    for command in [9, 0x40] {
        assert!(matches!(
            prepare_ctap2(&encode(command, cbor_map! {1=>1,2=>1}), BINDING, CHANNEL),
            Err(Error::Unsupported)
        ));
    }
    for subcommand in [7, 8] {
        assert!(
            prepare_ctap2(&encode(6, cbor_map! {1=>2,2=>subcommand}), BINDING, CHANNEL).is_err()
        );
    }
    for permissions in [0, 8, 16, 32, 255] {
        assert!(
            prepare_ctap2(
                &encode(6, cbor_map! {1=>2,2=>9,9=>permissions}),
                BINDING,
                CHANNEL
            )
            .is_err()
        );
    }
}

#[test]
fn local_uv_tokens_work_only_when_no_independent_host_pin_is_configured() {
    for protocol in [1, 2] {
        let (env, stats) = Model::new();
        let mut core = Processor::new(env);
        let client = HostPin::new(&mut core, protocol);
        let response = private(&mut core, &client.local_token(), &stats);
        let token: [u8; 32] = client
            .decrypt(
                &get(&response_map(&response), 2)
                    .extract_byte_string()
                    .unwrap(),
            )
            .try_into()
            .unwrap();
        assert_eq!(private(&mut core, &client.credential(&token), &stats)[0], 0);
        let pending = private(&mut core, &client.local_token(), &stats);
        let pending_token: [u8; 32] = client
            .decrypt(
                &get(&response_map(&pending), 2)
                    .extract_byte_string()
                    .unwrap(),
            )
            .try_into()
            .unwrap();
        assert_eq!(private(&mut core, &client.set_pin(), &stats), [0]);
        assert_eq!(
            private(&mut core, &client.credential(&pending_token), &stats),
            [Status::CTAP2_ERR_PIN_AUTH_INVALID as u8]
        );
        assert_eq!(
            private(&mut core, &client.local_token(), &stats),
            [Status::CTAP2_ERR_PUAT_REQUIRED as u8]
        );
    }
}

#[test]
fn failed_protocol_verification_revokes_an_older_valid_token() {
    for protocol in [1, 2] {
        let (env, stats) = Model::new();
        let mut core = Processor::new(env);
        let client = HostPin::new(&mut core, protocol);
        assert_eq!(private(&mut core, &client.set_pin(), &stats), [0]);
        let response = private(&mut core, &client.token(HOST_PIN), &stats);
        let old: [u8; 32] = client
            .decrypt(
                &get(&response_map(&response), 2)
                    .extract_byte_string()
                    .unwrap(),
            )
            .try_into()
            .unwrap();
        assert_eq!(
            private(&mut core, &client.token(b"wrong-protocol-pin"), &stats),
            [Status::CTAP2_ERR_PIN_INVALID as u8]
        );
        assert_eq!(
            private(&mut core, &client.credential(&old), &stats),
            [Status::CTAP2_ERR_PIN_AUTH_INVALID as u8]
        );
    }
}
#[test]
fn default_test_environment_cannot_assert_a_local_pin_proof() {
    let mut env = TestEnv::default();
    assert!(!env.local_pin().configured().unwrap());
    assert!(env.local_pin().verify_active_request(CHANNEL).is_err());
}

#[test]
fn public_info_does_not_query_private_catalog_or_pin_records_before_redaction() {
    let (env, stats) = Model::new();
    let mut core = Processor::new(env);
    let before = stats.borrow().persist_calls;
    let info = core
        .public(prepare_ctap2(&[4], BINDING, CHANNEL).unwrap())
        .unwrap();
    // The one persistence method is the public AAGUID. PIN capability comes
    // from the Root's separate public flag, never a private PIN-hash read.
    assert_eq!(stats.borrow().persist_calls - before, 1);
    let map = response_map(&info);
    assert!(!map.iter().any(|(key, _)| *key == Value::unsigned(20)));
    assert_eq!(get(&map, 13).extract_unsigned(), Some(4));
    assert_eq!(get(&map, 12), Value::bool_value(false));
    assert_eq!(stats.borrow().begins, 0);
}

#[test]
fn native_channel_change_revokes_protocol_tokens_before_new_dispatch() {
    let (env, stats) = Model::new();
    let mut core = Processor::new(env);
    let client = HostPin::new(&mut core, 2);
    assert_eq!(private(&mut core, &client.set_pin(), &stats), [0]);
    assert_eq!(private(&mut core, &client.token(HOST_PIN), &stats)[0], 0);
    let changed = Channel::MainHid([0, 0, 0, 8]);
    assert!(matches!(
        core.public(prepare_ctap2(&[4], BINDING, changed).unwrap()),
        Err(Error::Authorization)
    ));
    assert_eq!(stats.borrow().generation, 1);
    assert!(!stats.borrow().active);
    let info = core
        .public(prepare_ctap2(&[4], BINDING, changed).unwrap())
        .unwrap();
    assert_eq!(info[0], 0);
}

#[test]
fn destroying_the_processor_revokes_the_root_session() {
    let (env, stats) = Model::new();
    let core = Processor::new(env);
    let before = stats.borrow().generation;
    drop(core);
    assert_eq!(stats.borrow().generation, before + 1);
    assert!(!stats.borrow().active);
}
