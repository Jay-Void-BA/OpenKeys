use super::*;
use alloc::{vec, vec::Vec};
use device_policy::session::Transport;

struct Rng(u8);
impl Entropy for Rng {
    fn fill(&mut self, output: &mut [u8; 32]) -> Result<(), Error> {
        self.0 = self.0.checked_add(1).ok_or(Error::Entropy)?;
        output.fill(self.0);
        Ok(())
    }
}
struct Pins(Vec<[u8; 32]>);
impl PeerStore for Pins {
    fn contains(&mut self, peer: &[u8; 32]) -> Result<bool, Error> {
        Ok(self.0.contains(peer))
    }
    fn insert(&mut self, peer: &[u8; 32]) -> Result<(), Error> {
        self.0.push(*peer);
        Ok(())
    }
}
fn link(epoch: u64) -> NativeLink {
    NativeLink::new(Transport::Nfc, epoch).unwrap()
}
fn identities() -> (Identity, Identity) {
    let mut r = Rng(10);
    (
        Identity::generate(&mut r).unwrap(),
        Identity::generate(&mut r).unwrap(),
    )
}
fn handshakes() -> (Handshake, Handshake, Identity, Identity) {
    let (a, b) = identities();
    let mut rng = Rng(50);
    (
        Handshake::begin(Role::Manager, &a, &mut rng, link(1), 0).unwrap(),
        Handshake::begin(Role::Key, &b, &mut rng, link(8), 0).unwrap(),
        a,
        b,
    )
}
fn candidates() -> (PairingCandidate, PairingCandidate, Identity, Identity) {
    let (mut a, mut b, ai, bi) = handshakes();
    let mut buffer = [0; 96];
    let n = a.write(link(1), 1, &mut buffer).unwrap();
    b.read(link(8), 1, &buffer[..n]).unwrap();
    let n = b.write(link(8), 2, &mut buffer).unwrap();
    a.read(link(1), 2, &buffer[..n]).unwrap();
    let n = a.write(link(1), 3, &mut buffer).unwrap();
    b.read(link(8), 3, &buffer[..n]).unwrap();
    (
        a.finish(link(1), 4).unwrap(),
        b.finish(link(8), 4).unwrap(),
        ai,
        bi,
    )
}
fn channels() -> (Channel, Channel) {
    let (a, b, ai, bi) = candidates();
    let a = a
        .accept_known(&mut Pins(vec![bi.public_key()]), link(1), || 5)
        .unwrap();
    let b = b
        .accept_known(&mut Pins(vec![ai.public_key()]), link(8), || 5)
        .unwrap();
    (a, b)
}

#[test]
fn xx_mutual_pins_transcript_comparison_and_local_epochs() {
    let (a, b, ai, bi) = candidates();
    assert_eq!(a.context().peer(), bi.public_key());
    assert_eq!(b.context().peer(), ai.public_key());
    assert_eq!(a.context().transcript(), b.context().transcript());
    assert_eq!(a.comparison(), b.comparison());
    assert_eq!(a.pairing_request(), b.pairing_request());
    let a = a
        .accept_known(&mut Pins(vec![bi.public_key()]), link(1), || 5)
        .unwrap();
    assert_eq!(a.binding().unwrap().channel(), a.context().transcript());
    assert!(b.accept_known(&mut Pins(vec![]), link(8), || 5).is_err());
}

#[test]
fn ordered_encryption_both_directions_and_no_plaintext_on_wire() {
    let (mut a, mut b) = channels();
    let secret = b"a synthetic recovery secret";
    for n in 5..30 {
        let encrypted = a.seal(link(1), n, secret).unwrap();
        assert!(!encrypted.windows(secret.len()).any(|x| x == secret));
        assert_eq!(&**b.open(link(8), n, &encrypted).unwrap(), secret);
        let reply = b.seal(link(8), n, b"ack").unwrap();
        assert_eq!(&**a.open(link(1), n, &reply).unwrap(), b"ack");
    }
}

#[test]
fn every_wire_byte_is_authenticated_and_any_failure_closes_both_keys() {
    let (mut a, _) = channels();
    let record = a.seal(link(1), 5, b"private").unwrap();
    for index in 0..record.len() {
        let (_, mut receiver) = channels();
        let mut mutated = record.clone();
        mutated[index] ^= 1;
        assert!(receiver.open(link(8), 5, &mutated).is_err(), "byte {index}");
        assert!(matches!(
            receiver.open(link(8), 6, &record),
            Err(Error::Closed)
        ));
        assert_eq!(receiver.seal(link(8), 6, b"ack"), Err(Error::Closed));
    }
}

#[test]
fn replay_reorder_reflection_and_previous_connection_fail() {
    let (mut a, mut b) = channels();
    let one = a.seal(link(1), 5, b"one").unwrap();
    b.open(link(8), 5, &one).unwrap();
    assert!(matches!(b.open(link(8), 6, &one), Err(Error::Replay)));
    let (mut a, mut b) = channels();
    let one = a.seal(link(1), 5, b"one").unwrap();
    let two = a.seal(link(1), 6, b"two").unwrap();
    assert!(matches!(b.open(link(8), 6, &two), Err(Error::Replay)));
    assert!(matches!(a.open(link(1), 6, &one), Err(Error::Replay)));
    let (mut a, mut b) = channels();
    let packet = a.seal(link(1), 7, b"data").unwrap();
    assert!(matches!(b.open(link(9), 7, &packet), Err(Error::Link)));

    // Even the synthetic repeated RNG fixture cannot repeat the transport
    // keys when native epochs change. Real firmware still needs checked RNG.
    let (ai, bi) = identities();
    let mut rng = Rng(50);
    let mut new_a = Handshake::begin(Role::Manager, &ai, &mut rng, link(2), 0).unwrap();
    let mut new_b = Handshake::begin(Role::Key, &bi, &mut rng, link(9), 0).unwrap();
    let mut wire = [0; 96];
    let n = new_a.write(link(2), 1, &mut wire).unwrap();
    new_b.read(link(9), 1, &wire[..n]).unwrap();
    let n = new_b.write(link(9), 2, &mut wire).unwrap();
    new_a.read(link(2), 2, &wire[..n]).unwrap();
    let n = new_a.write(link(2), 3, &mut wire).unwrap();
    new_b.read(link(9), 3, &wire[..n]).unwrap();
    let mut new_b = new_b
        .finish(link(9), 4)
        .unwrap()
        .accept_known(&mut Pins(vec![ai.public_key()]), link(9), || 5)
        .unwrap();
    assert!(matches!(
        new_b.open(link(9), 7, &packet),
        Err(Error::Crypto)
    ));
}

#[test]
fn link_reset_suspend_field_loss_and_reselection_cannot_keep_keys() {
    for changed in [
        NativeLink::new(Transport::Nfc, 9).unwrap(),
        NativeLink::new(Transport::Usb, 8).unwrap(),
    ] {
        let (mut a, mut b) = channels();
        let packet = a.seal(link(1), 5, b"data").unwrap();
        assert!(matches!(b.open(changed, 5, &packet), Err(Error::Link)));
        assert_eq!(b.seal(link(8), 6, b"ack"), Err(Error::Closed));
    }
    assert_eq!(NativeLink::new(Transport::Nfc, 0), Err(Error::Link));
}

#[test]
fn fixed_time_and_byte_budgets_never_renew_on_records() {
    let (mut a, _) = channels();
    a.seal(link(1), 600_003, b"last").unwrap();
    assert_eq!(a.seal(link(1), 600_004, b"too late"), Err(Error::Expired));
    let (mut a, _) = channels();
    a.seal(link(1), 10, b"ok").unwrap();
    assert_eq!(a.seal(link(1), 9, b"rollback"), Err(Error::Time));
    let (mut a, _) = channels();
    let block = vec![0xaa; MAX_PLAINTEXT];
    for _ in 0..(16_777_216 / MAX_PLAINTEXT) {
        a.seal(link(1), 10, &block).unwrap();
    }
    assert_eq!(a.seal(link(1), 10, &block), Err(Error::Expired));
}

#[test]
fn handshake_turns_lengths_timeouts_and_low_order_points_fail_closed() {
    let (mut a, mut b, _, _) = handshakes();
    assert_eq!(b.write(link(8), 1, &mut [0; 96]), Err(Error::State));
    assert_eq!(b.read(link(8), 1, &[0; 32]), Err(Error::Closed));
    assert_eq!(
        a.write(link(1), HANDSHAKE_MS, &mut [0; 32]),
        Err(Error::Expired)
    );
    let (_, mut b, _, _) = handshakes();
    b.read(link(8), 1, &[0; 32]).unwrap(); // low-order e rejected on DH in msg2
    let mut out = [0xaa; 96];
    assert_eq!(b.write(link(8), 2, &mut out), Err(Error::Crypto));
    assert_eq!(out, [0; 96]);
    for n in 0..100 {
        if n == 32 {
            continue;
        }
        let (_, mut b, _, _) = handshakes();
        assert_eq!(b.read(link(8), 1, &vec![7; n]), Err(Error::Bounds));
    }
}

#[test]
fn oversized_or_empty_records_and_incomplete_handshakes_have_no_output() {
    let (a, _, _, _) = handshakes();
    assert!(a.finish(link(1), 1).is_err());
    for n in [0, MAX_PLAINTEXT + 1] {
        let (mut a, _) = channels();
        assert_eq!(a.seal(link(1), 5, &vec![1; n]), Err(Error::Bounds));
    }
    for n in 0..33 {
        let (_, mut b) = channels();
        assert!(b.open(link(8), 5, &vec![0; n]).is_err());
    }
}

#[test]
fn independent_snow_implementation_agrees_on_xx_wire_and_transcript() {
    let (a, b) = identities();
    let mut ours = Handshake::begin(Role::Manager, &a, &mut Rng(50), link(1), 0).unwrap();
    let mut other = b.seal_for_storage(|key| {
        snow::Builder::new(SUITE.parse().unwrap())
            .local_private_key(key)
            .unwrap()
            .prologue(&handshake::prologue(link(8)))
            .unwrap()
            .build_responder()
            .unwrap()
    });
    let mut buffer = [0; 128];
    let n = ours.write(link(1), 1, &mut buffer).unwrap();
    assert_eq!(other.read_message(&buffer[..n], &mut []).unwrap(), 0);
    let n = other.write_message(&[], &mut buffer).unwrap();
    ours.read(link(1), 2, &buffer[..n]).unwrap();
    let n = ours.write(link(1), 3, &mut buffer).unwrap();
    assert_eq!(other.read_message(&buffer[..n], &mut []).unwrap(), 0);
    let candidate = ours.finish(link(1), 4).unwrap();
    assert_eq!(
        candidate.context().transcript().as_slice(),
        other.get_handshake_hash()
    );
    assert_eq!(other.get_remote_static().unwrap(), a.public_key());
    assert_eq!(candidate.context().peer(), b.public_key());
    let mut ours = candidate
        .accept_known(&mut Pins(vec![b.public_key()]), link(1), || 5)
        .unwrap();
    // Test-only extraction from the independent implementation. Production
    // never compiles snow or exposes raw split/transport keys.
    let (send_key, receive_key) = other.dangerously_get_raw_split();
    let send_key = Zeroizing::new(send_key);
    let receive_key = Zeroizing::new(receive_key);
    use snow::resolvers::{CryptoResolver, DefaultResolver};
    let mut receiver = DefaultResolver
        .resolve_cipher(&snow::params::CipherChoice::ChaChaPoly)
        .unwrap();
    receiver.set(&send_key);
    let packet = ours.seal(link(1), 5, b"fixture").unwrap();
    let mut ad = [0; 48];
    ad[..16].copy_from_slice(&packet[..16]);
    ad[16..].copy_from_slice(other.get_handshake_hash());
    let mut plaintext = Zeroizing::new([0; 7]);
    assert_eq!(
        receiver
            .decrypt(0, &ad, &packet[16..], &mut *plaintext)
            .unwrap(),
        7
    );
    assert_eq!(&*plaintext, b"fixture");
    let mut sender = DefaultResolver
        .resolve_cipher(&snow::params::CipherChoice::ChaChaPoly)
        .unwrap();
    sender.set(&receive_key);
    let mut reply = vec![b'O', b'K', 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0];
    ad[..16].copy_from_slice(&reply);
    let mut ciphertext = [0; 19];
    assert_eq!(sender.encrypt(0, &ad, b"ack", &mut ciphertext), 19);
    reply.extend(ciphertext);
    assert_eq!(&**ours.open(link(1), 6, &reply).unwrap(), b"ack");
}

#[test]
fn failures_of_entropy_and_pins_have_no_fallback() {
    struct Bad;
    impl Entropy for Bad {
        fn fill(&mut self, _: &mut [u8; 32]) -> Result<(), Error> {
            Err(Error::Entropy)
        }
    }
    assert!(matches!(Identity::generate(&mut Bad), Err(Error::Entropy)));
    let (a, _) = identities();
    assert!(matches!(
        Handshake::begin(Role::Key, &a, &mut Bad, link(1), 0),
        Err(Error::Entropy)
    ));
    let (a, _, _, _) = candidates();
    struct Fault;
    impl PeerStore for Fault {
        fn contains(&mut self, _: &[u8; 32]) -> Result<bool, Error> {
            Err(Error::Persistence)
        }
        fn insert(&mut self, _: &[u8; 32]) -> Result<(), Error> {
            Err(Error::Persistence)
        }
    }
    assert!(matches!(
        a.accept_known(&mut Fault, link(1), || 5),
        Err(Error::Persistence)
    ));
}

#[test]
fn no_pin_or_code_export_command_and_noncanonical_requests_are_rejected() {
    use wire::Request;
    let mut request = [0; 16];
    request[0] = 1;
    for code in 0..=255u8 {
        request[1] = code;
        let accepted = Request::parse(&request).is_ok();
        assert_eq!(accepted, matches!(code, 1 | 2 | 12));
    }
    request[1] = 1;
    request[3] = 1;
    assert!(Request::parse(&request).is_err());
    for n in 0..16 {
        assert!(Request::parse(&request[..n]).is_err());
    }
}

mod pairing_tests {
    use super::*;
    use device_policy::{
        Request,
        execution::State,
        pin::{Error as PinError, Press, ProtectedPin},
        pin_only::{Digit, Gate, PinEntry, SecurePinVerifier, VerifiedPinRequest},
    };
    struct Local;
    impl ProtectedPin<6> for Local {
        fn reserve_attempt(&mut self, _: &Request) -> Result<(), PinError> {
            Ok(())
        }
        fn verify(&mut self, _: &Request, pin: &[u8; 6]) -> Result<(), PinError> {
            if pin == &[0; 6] {
                Ok(())
            } else {
                Err(PinError::Incorrect)
            }
        }
    }
    struct Review;
    impl SecurePinVerifier for Review {
        fn check_security_state(&mut self) -> Result<(), device_policy::Error> {
            Ok(())
        }
        fn verify_device_pin(
            &mut self,
            r: &Request,
        ) -> Result<VerifiedPinRequest, device_policy::Error> {
            let mut entry = PinEntry::begin(*r, || 1).unwrap();
            for _ in 0..6 {
                entry.press(Digit::Zero, Press::Short, 1).unwrap();
            }
            Ok(entry.submit(&mut Local, || 1).unwrap())
        }
        fn confirm_on_device(
            &mut self,
            _: &VerifiedPinRequest,
        ) -> Result<(), device_policy::Error> {
            Ok(())
        }
    }
    struct Root {
        generation: u64,
    }
    impl State for Root {
        fn generation(&self) -> u64 {
            self.generation
        }
        fn check_security_state(&mut self) -> Result<(), device_policy::Error> {
            Ok(())
        }
    }
    fn authority(request: Request) -> device_policy::Authorization {
        let mut gate = Gate::new(Review);
        gate.approve(request, || 1).unwrap();
        gate.consume_authorized(&request, || 1).unwrap()
    }
    #[test]
    fn key_pairing_requires_exact_fresh_local_authority_and_readback() {
        let (_, key, manager, _) = candidates();
        let request = key.pairing_request();
        let mut store = Pins(vec![]);
        let (channel, _completed) = key
            .accept_new(
                authority(request),
                &mut Root { generation: 0 },
                &mut store,
                link(8),
                || 5,
            )
            .unwrap();
        assert_eq!(channel.context().peer(), manager.public_key());
        assert_eq!(store.0, [manager.public_key()]);
        for wrong in 0..3 {
            let (_, key, _, _) = candidates();
            let mut request = key.pairing_request();
            let mut store = Pins(vec![]);
            if wrong == 0 {
                request.digest[0] ^= 1;
            }
            let result = key.accept_new(
                authority(request),
                &mut Root {
                    generation: u64::from(wrong == 1),
                },
                &mut store,
                link(8),
                || if wrong == 2 { 600_004 } else { 5 },
            );
            assert!(result.is_err());
            assert!(store.0.is_empty());
        }
    }
    #[test]
    fn host_consent_cannot_create_key_authority_or_ignore_store_failure() {
        struct Deny;
        impl ManagerPairingReview for Deny {
            fn compare_with_key(&mut self, _: &[u8; 20], _: &[u8; 32]) -> Result<(), Error> {
                Err(Error::Pairing)
            }
        }
        let (manager, _, _, _) = candidates();
        let mut store = Pins(vec![]);
        assert!(
            manager
                .accept_new_manager(&mut store, &mut Deny, link(1), || 5)
                .is_err()
        );
        assert!(store.0.is_empty());
        let (_, key, _, _) = candidates();
        assert!(
            key.accept_new_manager(&mut store, &mut Deny, link(8), || 5)
                .is_err()
        );
        struct Lost;
        impl PeerStore for Lost {
            fn contains(&mut self, _: &[u8; 32]) -> Result<bool, Error> {
                Ok(false)
            }
            fn insert(&mut self, _: &[u8; 32]) -> Result<(), Error> {
                Ok(())
            }
        }
        let (_, key, _, _) = candidates();
        let request = key.pairing_request();
        assert!(matches!(
            key.accept_new(
                authority(request),
                &mut Root { generation: 0 },
                &mut Lost,
                link(8),
                || 5
            ),
            Err(Error::Persistence)
        ));
    }
}

mod apdu_tests {
    use super::*;
    use crate::apdu::{Client, Endpoint, Handler, Transport as ApduTransport};
    use std::{cell::RefCell, rc::Rc};
    #[derive(Default)]
    struct Stats {
        calls: usize,
        revokes: usize,
        active: bool,
    }
    struct Key {
        identity: Identity,
        peer: [u8; 32],
        handshake: Option<Handshake>,
        channel: Option<Channel>,
        epoch: u64,
        stats: Rc<RefCell<Stats>>,
    }
    impl Key {
        fn link(&self) -> NativeLink {
            link(self.epoch)
        }
    }
    impl Handler for Key {
        fn handshake(&mut self, message: &[u8]) -> Result<Vec<u8>, Error> {
            if message.len() == 32 {
                if self.handshake.is_some() || self.channel.is_some() {
                    return Err(Error::State);
                }
                let mut h =
                    Handshake::begin(Role::Key, &self.identity, &mut Rng(70), self.link(), 0)?;
                h.read(self.link(), 1, message)?;
                let mut response = vec![0; 96];
                h.write(self.link(), 2, &mut response)?;
                self.handshake = Some(h);
                Ok(response)
            } else {
                let mut h = self.handshake.take().ok_or(Error::State)?;
                h.read(self.link(), 3, message)?;
                let candidate = h.finish(self.link(), 4)?;
                self.channel =
                    Some(candidate.accept_known(&mut Pins(vec![self.peer]), self.link(), || 5)?);
                self.stats.borrow_mut().active = true;
                Ok(Vec::new())
            }
        }
        fn encrypted_record(&mut self, record: &[u8]) -> Result<Vec<u8>, Error> {
            let link = self.link();
            let channel = self.channel.as_mut().ok_or(Error::Closed)?;
            let plaintext = channel.open(link, 6, record)?;
            self.stats.borrow_mut().calls += 1;
            // Synthetic loopback only. Production handlers must parse wire::
            // Request and authorize each operation, including private lists.
            channel.seal(link, 6, &plaintext)
        }
        fn revoke(&mut self) {
            self.handshake = None;
            self.channel = None;
            self.epoch += 1;
            let mut stats = self.stats.borrow_mut();
            stats.revokes += 1;
            stats.active = false;
        }
    }
    struct Link {
        endpoint: Rc<RefCell<Endpoint<Key>>>,
        closed: bool,
    }
    impl ApduTransport for Link {
        fn transmit(&mut self, command: &[u8], out: &mut [u8]) -> Result<usize, Error> {
            if self.closed {
                return Err(Error::Closed);
            }
            let response = self.endpoint.borrow_mut().receive(command, || 0);
            if response.len() > out.len() {
                return Err(Error::Bounds);
            }
            out[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }
        fn close(&mut self) {
            self.closed = true;
            self.endpoint.borrow_mut().reset();
        }
    }
    type Connected = (
        Client<Link>,
        Channel,
        Rc<RefCell<Endpoint<Key>>>,
        Rc<RefCell<Stats>>,
    );
    fn connected() -> Connected {
        let (manager, key) = identities();
        let public = key.public_key();
        let stats = Rc::new(RefCell::new(Stats::default()));
        let endpoint = Rc::new(RefCell::new(Endpoint::new(Key {
            identity: key,
            peer: manager.public_key(),
            handshake: None,
            channel: None,
            epoch: 8,
            stats: stats.clone(),
        })));
        let mut client = Client::new(Link {
            endpoint: endpoint.clone(),
            closed: false,
        });
        client.select().unwrap();
        let mut h = Handshake::begin(Role::Manager, &manager, &mut Rng(50), link(1), 0).unwrap();
        let mut message = [0; 96];
        let n = h.write(link(1), 1, &mut message).unwrap();
        let response = client.handshake(&message[..n]).unwrap();
        h.read(link(1), 2, &response).unwrap();
        let n = h.write(link(1), 3, &mut message).unwrap();
        assert!(client.handshake(&message[..n]).unwrap().is_empty());
        let channel = h
            .finish(link(1), 4)
            .unwrap()
            .accept_known(&mut Pins(vec![public]), link(1), || 5)
            .unwrap();
        (client, channel, endpoint, stats)
    }
    #[test]
    fn ccid_nfc_same_full_xx_and_aead_exchange_at_every_fragment_boundary() {
        let (mut client, mut channel, _, _) = connected();
        for n in [1, 2, 205, 206, 207, 208, 239, 240, 241, 1000, MAX_PLAINTEXT] {
            let secret = Zeroizing::new(vec![0x5a; n]);
            let ciphertext = channel.seal(link(1), 6, &secret).unwrap();
            let reply = client.exchange(&ciphertext).unwrap();
            assert_eq!(&**channel.open(link(1), 6, &reply).unwrap(), &**secret);
        }
    }
    fn command(ins: u8, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        let mut out = vec![0x80, ins, p1, p2, data.len() as u8];
        out.extend_from_slice(data);
        out
    }
    #[test]
    fn partial_replay_wrong_order_overrun_and_timeout_never_reach_handler() {
        let first = command(0x72, 0, 0, &[0, 100, 7]);
        for bad in [
            command(0x72, 0, 0, &[0, 100, 7]),
            command(0x72, 1, 2, &[0; 99]),
            command(0x72, 1, 1, &[0; 100]),
            command(0x73, 0, 0, &[0, 0]),
        ] {
            let (_client, _channel, endpoint, stats) = connected();
            let mut endpoint = endpoint.borrow_mut();
            assert_eq!(endpoint.receive(&first, || 0), [0x90, 0]);
            assert_eq!(endpoint.receive(&bad, || 1), [0x69, 0x85]);
            assert_eq!(endpoint.receive(&first, || 2), [0x69, 0x85]);
            assert_eq!(stats.borrow().calls, 0);
            assert!(!stats.borrow().active);
        }
        let (_client, _channel, endpoint, stats) = connected();
        let mut endpoint = endpoint.borrow_mut();
        assert_eq!(endpoint.receive(&first, || 0), [0x90, 0]);
        assert_eq!(endpoint.tick(apdu::MESSAGE_MS), Err(Error::Expired));
        assert_eq!(stats.borrow().calls, 0);
        assert!(!stats.borrow().active);
    }
    #[test]
    fn reselect_and_native_reset_revoke_both_crypto_and_pending_fragments() {
        let (mut client, _channel, endpoint, stats) = connected();
        assert!(stats.borrow().active);
        client.select().unwrap();
        assert!(!stats.borrow().active);
        assert_eq!(stats.borrow().revokes, 2);
        endpoint.borrow_mut().reset();
        assert_eq!(
            endpoint
                .borrow_mut()
                .receive(&command(0x72, 0, 0, &[0, 33, 7]), || 0),
            [0x69, 0x85]
        );
    }
    #[test]
    fn bounded_malformed_apdus_do_not_panic_or_leave_selection_open() {
        let (client, _channel, endpoint, stats) = connected();
        let mut seed = 0x1742_6521u32;
        for n in 0..600usize {
            let mut bytes = vec![0; n % 261];
            for b in &mut bytes {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *b = seed as u8;
            }
            endpoint.borrow_mut().receive(&bytes, || 0);
        }
        drop(client);
        assert!(!stats.borrow().active);
    }
    #[test]
    fn response_fragment_deadline_uses_fresh_time_after_a_slow_local_review() {
        use std::cell::Cell;
        struct Slow {
            now: Rc<Cell<u64>>,
            rollback: bool,
        }
        impl Handler for Slow {
            fn handshake(&mut self, _: &[u8]) -> Result<Vec<u8>, Error> {
                Err(Error::State)
            }
            fn encrypted_record(&mut self, _: &[u8]) -> Result<Vec<u8>, Error> {
                self.now.set(if self.rollback {
                    self.now.get() - 1
                } else {
                    self.now.get() + 60_000
                });
                Ok(vec![0; 33]) // synthetic ciphertext, never a production handler
            }
            fn revoke(&mut self) {}
        }
        for rollback in [false, true] {
            let now = Rc::new(Cell::new(50));
            let mut endpoint = Endpoint::new(Slow {
                now: now.clone(),
                rollback,
            });
            let mut select = command(0xa4, 4, 0, apdu::AID);
            select[0] = 0;
            assert_eq!(endpoint.receive(&select, || now.get()), [0x90, 0]);
            let mut data = vec![0, 33];
            data.extend_from_slice(&[0; 33]);
            let reply = endpoint.receive(&command(0x72, 0, 0, &data), || now.get());
            if rollback {
                assert_eq!(reply, [0x69, 0x85]);
                assert_eq!(endpoint.tick(50), Err(Error::Closed));
            } else {
                assert_eq!(reply, [0, 33, 0x90, 0]);
                now.set(now.get() + 1);
                let reply = endpoint.receive(&command(0x73, 0, 0, &[0, 0]), || now.get());
                assert_eq!(reply.len(), 35);
                assert_eq!(reply[33..], [0x90, 0]);
            }
        }
    }
}
