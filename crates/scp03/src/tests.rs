extern crate std;

use super::*;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use cmac::{Cmac, Mac};

#[path = "test_vectors.rs"]
mod vectors;

// Software crypto exists only in cfg(test), with public synthetic keys. The
// target build never links this backend, the OpenSSL CLI or a software AES.
struct Software {
    challenge: [u8; 8],
    fail: bool,
    decrypt_calls: usize,
}

impl Software {
    fn new() -> Self {
        Self {
            challenge: [1, 2, 3, 4, 5, 6, 7, 8],
            fail: false,
            decrypt_calls: 0,
        }
    }
    fn check(&self) -> Result<(), Error> {
        if self.fail {
            Err(Error::Crypto)
        } else {
            Ok(())
        }
    }
}

macro_rules! backend {
    ($size:expr, $aes:ty) => {
        impl Crypto<$size> for Software {
            fn random_challenge(&mut self, out: &mut [u8; 8]) -> Result<(), Error> {
                self.check()?;
                *out = self.challenge;
                Ok(())
            }
            fn cmac(
                &mut self,
                key: &[u8; $size],
                message: &[u8],
                out: &mut [u8; 16],
            ) -> Result<(), Error> {
                self.check()?;
                let mut mac = <Cmac<$aes> as Mac>::new_from_slice(key).unwrap();
                mac.update(message);
                out.copy_from_slice(&mac.finalize().into_bytes());
                Ok(())
            }
            fn encrypt_block(
                &mut self,
                key: &[u8; $size],
                input: &[u8; 16],
                out: &mut [u8; 16],
            ) -> Result<(), Error> {
                self.check()?;
                let cipher = <$aes>::new_from_slice(key).unwrap();
                let mut block = (*input).into();
                cipher.encrypt_block(&mut block);
                out.copy_from_slice(&block);
                Ok(())
            }
            fn encrypt_cbc(
                &mut self,
                key: &[u8; $size],
                iv: &[u8; 16],
                bytes: &mut [u8],
            ) -> Result<(), Error> {
                self.check()?;
                if !bytes.len().is_multiple_of(16) {
                    return Err(Error::Input);
                }
                let mut previous = *iv;
                for block in bytes.chunks_exact_mut(16) {
                    let mut input = [0; 16];
                    for (i, byte) in input.iter_mut().enumerate() {
                        *byte = block[i] ^ previous[i];
                    }
                    <Self as Crypto<$size>>::encrypt_block(self, key, &input, &mut previous)?;
                    block.copy_from_slice(&previous);
                }
                Ok(())
            }
            fn decrypt_cbc(
                &mut self,
                key: &[u8; $size],
                iv: &[u8; 16],
                bytes: &mut [u8],
            ) -> Result<(), Error> {
                self.check()?;
                self.decrypt_calls += 1;
                if !bytes.len().is_multiple_of(16) {
                    return Err(Error::Input);
                }
                let cipher = <$aes>::new_from_slice(key).unwrap();
                let mut previous = *iv;
                for block in bytes.chunks_exact_mut(16) {
                    let saved: [u8; 16] = block.try_into().unwrap();
                    let mut input = saved.into();
                    cipher.decrypt_block(&mut input);
                    for (i, byte) in block.iter_mut().enumerate() {
                        *byte = input[i] ^ previous[i];
                    }
                    previous = saved;
                }
                Ok(())
            }
        }
    };
}
backend!(16, aes::Aes128);
backend!(32, aes::Aes256);

const AID: [u8; 8] = [0xa0, 0, 0, 1, 0x51, 0, 0, 0];

fn masters<const N: usize>() -> StaticKeys<N> {
    StaticKeys::new(
        Zeroizing::new(core::array::from_fn(|i| i as u8)),
        Zeroizing::new(core::array::from_fn(|i| i as u8 + 32)),
    )
    .unwrap()
}
fn command(data: &[u8]) -> Command<'_> {
    Command {
        class: 0x80,
        instruction: 0xca,
        p1: 0,
        p2: 0,
        data,
        le: Some(0),
    }
}
fn established() -> (Software, Channel<16>) {
    let mut crypto = Software::new();
    let keys = masters();
    let handshake = Handshake::start(&mut crypto, &keys, 1, &AID).unwrap();
    let pending = handshake
        .accept(&mut crypto, &vectors::aes128_random::IU_RESPONSE)
        .unwrap();
    let channel = pending.finish(&[0x90, 0]).unwrap();
    (crypto, channel)
}
fn after_empty() -> (Software, Channel<16>) {
    let (mut crypto, mut channel) = established();
    channel.wrap(&mut crypto, &command(&[])).unwrap();
    channel
        .unwrap(&mut crypto, &vectors::aes128_random::RESPONSE_1)
        .unwrap();
    (crypto, channel)
}

#[test]
fn primitive_cmac_matches_nist_sp800_38b() {
    let key = [
        0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f,
        0x3c,
    ];
    let mut crypto = Software::new();
    let mut tag = [0; 16];
    crypto.cmac(&key, &[], &mut tag).unwrap();
    assert_eq!(
        tag,
        [
            0xbb, 0x1d, 0x69, 0x29, 0xe9, 0x59, 0x37, 0x28, 0x7f, 0xa3, 0x7d, 0x12, 0x9b, 0x75,
            0x67, 0x46
        ]
    );
    crypto
        .cmac(
            &key,
            &[
                0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
                0x17, 0x2a,
            ],
            &mut tag,
        )
        .unwrap();
    assert_eq!(
        tag,
        [
            0x07, 0x0a, 0x16, 0xb4, 0x6b, 0x4d, 0x41, 0x44, 0xf7, 0x9b, 0xdd, 0x9d, 0xd0, 0x4a,
            0x28, 0x7c
        ]
    );
}

macro_rules! transcript {
    ($test:ident,$size:expr,$module:ident) => {
        #[test]
        fn $test() {
            use vectors::$module as v;
            let keys = masters::<$size>();
            let mut crypto = Software::new();
            let h = Handshake::start(&mut crypto, &keys, 1, &AID).unwrap();
            assert_eq!(h.initialize_update(), v::INITIALIZE);
            let p = h.accept(&mut crypto, &v::IU_RESPONSE).unwrap();
            assert_eq!(p.external_authenticate(), &v::EXTERNAL);
            assert_eq!(&p.keys.enc[..], v::SESSION_ENC);
            assert_eq!(&p.keys.mac[..], v::SESSION_MAC);
            assert_eq!(&p.keys.rmac[..], v::SESSION_RMAC);
            let mut c = p.finish(&[0x90, 0]).unwrap();
            assert_eq!(
                c.wrap(&mut crypto, &command(&[])).unwrap().wire_bytes(),
                v::COMMAND_1
            );
            assert!(
                c.unwrap(&mut crypto, &v::RESPONSE_1)
                    .unwrap()
                    .expose()
                    .is_empty()
            );
            assert_eq!(
                c.wrap(&mut crypto, &command(b"public test data"))
                    .unwrap()
                    .wire_bytes(),
                v::COMMAND_2
            );
            assert_eq!(
                c.unwrap(&mut crypto, &v::RESPONSE_2).unwrap().expose(),
                b"public response!"
            );
            let data: [u8; 239] = core::array::from_fn(|i| i as u8);
            assert_eq!(
                c.wrap(&mut crypto, &command(&data)).unwrap().wire_bytes(),
                v::COMMAND_3
            );
            let expected: [u8; 239] = core::array::from_fn(|i| 238 - i as u8);
            assert_eq!(
                c.unwrap(&mut crypto, &v::RESPONSE_3).unwrap().expose(),
                expected
            );
            assert_eq!(c.counter, 3);
            assert!(!c.pending);
        }
    };
}
transcript!(aes128_random_openssl_transcript, 16, aes128_random);
transcript!(aes128_pseudo_openssl_transcript, 16, aes128_pseudo);
transcript!(aes256_random_openssl_transcript, 32, aes256_random);

#[test]
fn handshake_rejects_downgrade_replay_wrong_aid_and_forgery() {
    let keys = masters::<16>();
    let mut crypto = Software::new();
    for field in [10, 11, 12, 13, 20, 21, 28] {
        for bit in 0..8 {
            let mut response = vectors::aes128_random::IU_RESPONSE;
            response[field] ^= 1 << bit;
            let h = Handshake::start(&mut crypto, &keys, 1, &AID).unwrap();
            assert!(h.accept(&mut crypto, &response).is_err());
        }
    }
    for changed in [0, 7] {
        let mut aid = AID;
        aid[changed] ^= 1;
        let h = Handshake::start(&mut crypto, &keys, 1, &aid).unwrap();
        assert!(matches!(
            h.accept(&mut crypto, &vectors::aes128_pseudo::IU_RESPONSE),
            Err(Error::Integrity)
        ));
    }
    crypto.challenge[0] ^= 1;
    let h = Handshake::start(&mut crypto, &keys, 1, &AID).unwrap();
    assert!(matches!(
        h.accept(&mut crypto, &vectors::aes128_random::IU_RESPONSE),
        Err(Error::Integrity)
    ));
    assert!(Handshake::start(&mut crypto, &keys, 0, &AID).is_err());
    assert!(Handshake::start(&mut crypto, &keys, 1, &[0; 4]).is_err());
    crypto.fail = true;
    assert!(matches!(
        Handshake::start(&mut crypto, &keys, 1, &AID),
        Err(Error::Crypto)
    ));
}

#[test]
fn every_response_bit_is_checked_before_decryption_and_failure_is_terminal() {
    for byte in 0..vectors::aes128_random::RESPONSE_2.len() {
        for bit in 0..8 {
            let (mut crypto, mut channel) = after_empty();
            channel
                .wrap(&mut crypto, &command(b"public test data"))
                .unwrap();
            let mut response = vectors::aes128_random::RESPONSE_2;
            response[byte] ^= 1 << bit;
            assert!(channel.unwrap(&mut crypto, &response).is_err());
            assert_eq!(crypto.decrypt_calls, 0);
            assert!(
                channel
                    .keys
                    .enc
                    .iter()
                    .chain(channel.keys.mac.iter())
                    .chain(channel.keys.rmac.iter())
                    .all(|b| *b == 0)
            );
            assert!(matches!(
                channel.unwrap(&mut crypto, &vectors::aes128_random::RESPONSE_2),
                Err(Error::State)
            ));
            assert!(matches!(
                channel.wrap(&mut crypto, &command(&[])),
                Err(Error::State)
            ));
        }
    }
}

#[test]
fn replay_reorder_bare_errors_and_pipelining_never_succeed() {
    for response in [
        &[0x90, 0][..],
        &[0x6a, 0x88],
        &[0x62, 0],
        &vectors::aes128_random::RESPONSE_2,
    ] {
        let (mut crypto, mut channel) = established();
        channel.wrap(&mut crypto, &command(&[])).unwrap();
        assert!(channel.unwrap(&mut crypto, response).is_err());
    }
    let (mut crypto, mut channel) = after_empty();
    channel
        .wrap(&mut crypto, &command(b"public test data"))
        .unwrap();
    assert!(
        channel
            .unwrap(&mut crypto, &vectors::aes128_random::RESPONSE_1)
            .is_err()
    );
    let (mut crypto, mut channel) = established();
    channel.wrap(&mut crypto, &command(&[])).unwrap();
    assert!(matches!(
        channel.wrap(&mut crypto, &command(&[])),
        Err(Error::State)
    ));
    let (mut crypto, mut channel) = established();
    assert!(matches!(
        channel.unwrap(&mut crypto, &vectors::aes128_random::RESPONSE_1),
        Err(Error::State)
    ));
}

#[test]
fn authenticated_bad_padding_is_terminal_and_counter_never_wraps() {
    let (mut crypto, mut channel) = established();
    channel.wrap(&mut crypto, &command(&[])).unwrap();
    let iv = channel.iv(&mut crypto, true).unwrap();
    let mut response = [0; 26];
    crypto
        .encrypt_cbc(&channel.keys.enc, &iv, &mut response[..16])
        .unwrap();
    let mut input = [0; 34];
    input[..16].copy_from_slice(&channel.chain[..]);
    input[16..32].copy_from_slice(&response[..16]);
    input[32..].copy_from_slice(&[0x90, 0]);
    let mut tag = [0; 16];
    crypto.cmac(&channel.keys.rmac, &input, &mut tag).unwrap();
    response[16..24].copy_from_slice(&tag[..8]);
    response[24..].copy_from_slice(&[0x90, 0]);
    assert!(matches!(
        channel.unwrap(&mut crypto, &response),
        Err(Error::Integrity)
    ));
    assert_eq!(crypto.decrypt_calls, 1);
    for count in [MAX_COMMANDS, u32::MAX] {
        let (mut crypto, mut channel) = established();
        channel.counter = count;
        assert!(matches!(
            channel.wrap(&mut crypto, &command(&[])),
            Err(Error::Exhausted)
        ));
        assert!(channel.faulted);
    }
}

#[test]
fn invalid_inputs_and_primitive_errors_latch_closed() {
    for class in [4, 0x40, 0x81, 0xff] {
        let (mut crypto, mut channel) = established();
        let mut c = command(&[]);
        c.class = class;
        assert!(matches!(channel.wrap(&mut crypto, &c), Err(Error::Input)));
    }
    let (mut crypto, mut channel) = established();
    assert!(matches!(
        channel.wrap(&mut crypto, &command(&[0; 240])),
        Err(Error::Input)
    ));
    let (mut crypto, mut channel) = established();
    crypto.fail = true;
    assert!(matches!(
        channel.wrap(&mut crypto, &command(b"secret")),
        Err(Error::Crypto)
    ));
    assert!(channel.faulted);
    assert!(channel.chain.iter().all(|b| *b == 0));
}

struct ScriptedLink {
    calls: std::rc::Rc<core::cell::Cell<usize>>,
    fail_private: bool,
    oversize_reply: bool,
}
impl client::Link for ScriptedLink {
    fn exchange(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, Error> {
        let at = self.calls.get();
        self.calls.set(at + 1);
        let (expected, reply): (&[u8], &[u8]) = match at {
            0 => (
                &vectors::aes128_random::INITIALIZE,
                &vectors::aes128_random::IU_RESPONSE,
            ),
            1 => (&vectors::aes128_random::EXTERNAL, &[0x90, 0]),
            2 => (
                &vectors::aes128_random::COMMAND_1,
                &vectors::aes128_random::RESPONSE_1,
            ),
            3 => (
                &vectors::aes128_random::COMMAND_2,
                &vectors::aes128_random::RESPONSE_2,
            ),
            _ => panic!("an uncertain private operation was replayed"),
        };
        assert_eq!(input, expected);
        if at >= 2 && self.fail_private {
            return Err(Error::Link);
        }
        if at >= 2 && self.oversize_reply {
            return Ok(output.len() + 1);
        }
        output[..reply.len()].copy_from_slice(reply);
        Ok(reply.len())
    }
}
struct LiveGuard {
    calls: usize,
    revoke_at: Option<usize>,
}
impl LiveGuard {
    fn live() -> Self {
        Self {
            calls: 0,
            revoke_at: None,
        }
    }
}
impl client::Guard for LiveGuard {
    fn check(&mut self) -> Result<(), Error> {
        self.calls += 1;
        if self.revoke_at == Some(self.calls) {
            Err(Error::Authorization)
        } else {
            Ok(())
        }
    }
}
fn profile() -> client::Profile<'static> {
    client::Profile {
        key_version: 1,
        full_selected_aid: &AID,
        qualified_applet_version: 0x0702_0000,
    }
}
type TestClient = client::Client<ScriptedLink, 16>;
fn client() -> (TestClient, std::rc::Rc<core::cell::Cell<usize>>) {
    let calls = std::rc::Rc::new(core::cell::Cell::new(0));
    let link = ScriptedLink {
        calls: calls.clone(),
        fail_private: false,
        oversize_reply: false,
    };
    let c = TestClient::establish(
        link,
        &mut Software::new(),
        &masters(),
        &profile(),
        &mut LiveGuard::live(),
    )
    .unwrap();
    (c, calls)
}

#[test]
fn private_client_checks_live_authorization_even_after_decryption() {
    for step in 1..=4 {
        let (mut c, calls) = client();
        let mut guard = LiveGuard {
            calls: 0,
            revoke_at: Some(step),
        };
        assert!(matches!(
            c.exchange(&mut Software::new(), &command(&[]), &mut guard),
            Err(Error::Authorization)
        ));
        assert_eq!(calls.get(), if step <= 2 { 2 } else { 3 });
        assert!(matches!(
            c.exchange(&mut Software::new(), &command(&[]), &mut LiveGuard::live()),
            Err(Error::State)
        ));
    }
    let (mut c, calls) = client();
    let mut guard = LiveGuard::live();
    assert!(
        c.exchange(&mut Software::new(), &command(&[]), &mut guard)
            .unwrap()
            .expose()
            .is_empty()
    );
    assert_eq!(guard.calls, 4);
    assert_eq!(
        c.exchange(
            &mut Software::new(),
            &command(b"public test data"),
            &mut guard
        )
        .unwrap()
        .expose(),
        b"public response!"
    );
    assert_eq!(calls.get(), 4);
}

#[test]
fn private_client_does_not_replay_ambiguous_link_or_trust_an_impossible_length() {
    for oversize in [false, true] {
        let calls = std::rc::Rc::new(core::cell::Cell::new(0));
        let link = ScriptedLink {
            calls: calls.clone(),
            fail_private: !oversize,
            oversize_reply: oversize,
        };
        let mut c = TestClient::establish(
            link,
            &mut Software::new(),
            &masters(),
            &profile(),
            &mut LiveGuard::live(),
        )
        .unwrap();
        assert!(matches!(
            c.exchange(&mut Software::new(), &command(&[]), &mut LiveGuard::live()),
            Err(Error::Link)
        ));
        assert!(matches!(
            c.exchange(&mut Software::new(), &command(&[]), &mut LiveGuard::live()),
            Err(Error::State)
        ));
        assert_eq!(calls.get(), 3);
    }
}

#[test]
fn establishment_requires_qualified_profile_and_each_guard_checkpoint() {
    for step in 1..=5 {
        let calls = std::rc::Rc::new(core::cell::Cell::new(0));
        let link = ScriptedLink {
            calls: calls.clone(),
            fail_private: false,
            oversize_reply: false,
        };
        let mut guard = LiveGuard {
            calls: 0,
            revoke_at: Some(step),
        };
        assert!(matches!(
            TestClient::establish(
                link,
                &mut Software::new(),
                &masters(),
                &profile(),
                &mut guard
            ),
            Err(Error::Authorization)
        ));
        assert_eq!(
            calls.get(),
            match step {
                1 | 2 => 0,
                3 | 4 => 1,
                _ => 2,
            }
        );
    }
    let calls = std::rc::Rc::new(core::cell::Cell::new(0));
    let link = ScriptedLink {
        calls: calls.clone(),
        fail_private: false,
        oversize_reply: false,
    };
    let mut p = profile();
    p.qualified_applet_version = 0x0402_ffff;
    assert!(matches!(
        TestClient::establish(
            link,
            &mut Software::new(),
            &masters(),
            &p,
            &mut LiveGuard::live()
        ),
        Err(Error::Unsupported)
    ));
    assert_eq!(calls.get(), 0);
}
