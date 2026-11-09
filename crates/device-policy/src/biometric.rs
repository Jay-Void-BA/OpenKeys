//! FPC2534 protected-result policy, not a sensor driver or finished authenticator.
//!
//! FPC protocol v4 has an authenticated 16-bit Identify operation tag. Every tag
//! is reserved durably BEFORE sending, never reused under the same pairing key,
//! and bound to one request/channel/deadline. Random GCM IVs alone do not prevent
//! replay. At 65,535 attempts the pairing must stop biometric authentication.
//!
//! Production requires SE-backed counters/retry limits and authenticated GCM
//! under a per-device pairing key. There is deliberately no permissive default.

use crate::Request;
use zeroize::Zeroizing;

const VERSION: u16 = 4;
const RESPONSE: u16 = 0x12;
const EVENT: u16 = 0x13;
const APP_SECURE: u16 = 0x0041;
const STATUS: u16 = 0x0040;
const IDENTIFY: u16 = 0x0055;
const MATCH: u16 = 0x61ec;
const SPECIFIED: u16 = 0x3034;
const MAX_TAG: u32 = u16::MAX as u32;
const DEADLINE_MS: u64 = 15_000;
const FRAME_SIZE: usize = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Storage,
    Exhausted,
    InvalidReservation,
    RecoveryRequired,
    Locked,
    Expired,
    TimeWentBackwards,
    WrongRequest,
    InvalidFrame,
    Unauthenticated,
    UnexpectedResponse,
    NoMatch,
    WrongTag,
    WrongTemplate,
}

/// Implement inside the protected device domain using authenticated durable SE
/// state. An ordinary Flash/host integer is not an implementation of this trait.
pub trait ProtectedAttempts {
    /// Durably pre-charge a persistent biometric retry attempt, then reserve
    /// a never-reused tag in 1..=65535 for the current pairing key. Return only
    /// after BOTH commits are confirmed; ordered writes are permitted (no
    /// cross-object transaction or rollback is assumed). An ambiguous
    /// power/transport outcome must return Err and must not issue a sensor request.
    /// Exhaustion, reset and PIN entry must NEVER wrap/reset the tag counter.
    fn reserve_identify_attempt(&mut self, request: &Request) -> Result<u32, Error>;

    /// Persist successful verification / retry credit AFTER authenticated match.
    /// Must not reset the lifetime operation-tag counter. Error denies release.
    fn record_match(&mut self, proof: &AuthenticatedMatch) -> Result<(), Error>;
}

/// Minted only by the protected frame verifier after GCM, result, template,
/// tag, request and live-deadline checks. This is not a user grant yet: the
/// persistent retry update and outer device confirmation must still succeed.
/// No public constructor, Clone/Copy, Debug or host Boolean conversion exists.
pub struct AuthenticatedMatch {
    request: Request,
    tag: u16,
}
impl AuthenticatedMatch {
    pub fn request(&self) -> &Request {
        &self.request
    }
    pub fn tag(&self) -> u16 {
        self.tag
    }
}

/// Verify GCM before making plaintext usable. The exact 8-byte frame header is
/// AAD. Keys stay in the protected hardware boundary. Implementations must use
/// a vetted GCM primitive, constant-time tag checking and fixed bounded lengths.
/// No driver is supplied here, so no software match Boolean can authorize use.
pub trait ProtectedGcm {
    fn decrypt_identify_payload(
        &mut self,
        header: &[u8; 8],
        iv: &[u8; 12],
        tag: &[u8; 16],
        payload: &mut [u8; 12],
    ) -> Result<(), Error>;
}

/// Outbound provider for the paired runtime. Generate a fresh 12-byte IV with
/// the protected entropy source; never accept a host IV or use a fixed/test IV.
/// The durable 16-bit operation tag is anti-replay context, NOT a replacement
/// for GCM nonce safety. Real sensor/host RNG fault qualification is mandatory.
pub trait ProtectedIdentifyGcm: ProtectedGcm {
    fn seal_identify_payload(
        &mut self,
        header: &[u8; 8],
        payload: &mut [u8; 10],
    ) -> Result<([u8; 12], [u8; 16]), Error>;
}

/// A precharged, request-bound encrypted request, not a match or user grant.
/// No public constructor/Clone/Debug/host conversion. The protected root must
/// install real counter/AEAD providers; Rust types do not isolate a hostile TCB.
pub struct EncryptedIdentify {
    wire: [u8; 46],
    request: Request,
    expires: u64,
}
impl EncryptedIdentify {
    pub fn wire_bytes(&self) -> &[u8; 46] {
        &self.wire
    }
    pub fn request(&self) -> &Request {
        &self.request
    }
    pub fn expires_ms(&self) -> u64 {
        self.expires
    }
}

#[derive(Clone, Copy)]
struct Pending {
    request: Request,
    tag: u16,
    template: u16,
    expires: u64,
}

/// Created only after a final authenticated matching Identify event and a
/// successful durable retry update. Device confirmation is still required by
/// the outer SecureVerifier/Gate; this type alone does not sign or expose data.
pub struct VerifiedRequest {
    pub(crate) request: Request,
    pub(crate) expires: u64,
}
impl VerifiedRequest {
    pub fn request(&self) -> &Request {
        &self.request
    }
}

pub struct Session<C, A> {
    counter: C,
    aead: A,
    pending: Option<Pending>,
    last_tag: u32,
    last_time: u64,
    recovery_required: bool,
}

impl<C: ProtectedAttempts, A: ProtectedGcm> Session<C, A> {
    pub const fn new(counter: C, aead: A) -> Self {
        Self {
            counter,
            aead,
            pending: None,
            last_tag: 0,
            last_time: 0,
            recovery_required: false,
        }
    }

    fn time(&mut self, now: u64) -> Result<(), Error> {
        if self.recovery_required {
            return Err(Error::RecoveryRequired);
        }
        if now < self.last_time {
            self.pending = None;
            self.recovery_required = true;
            return Err(Error::TimeWentBackwards);
        }
        self.last_time = now;
        Ok(())
    }

    /// Call after local intent (e.g. CONFIRM), never just upon host discovery.
    /// Returns the operation tag to include in the encrypted Identify request.
    pub fn begin(&mut self, request: Request, template: u16, now: u64) -> Result<u16, Error> {
        self.pending = None;
        self.time(now)?;
        let expires = now.checked_add(DEADLINE_MS).ok_or(Error::Expired)?;
        let reserved = match self.counter.reserve_identify_attempt(&request) {
            Ok(tag) => tag,
            Err(error) => {
                self.recovery_required = true;
                return Err(error);
            }
        };
        if reserved == 0 || reserved <= self.last_tag {
            self.recovery_required = true;
            return Err(Error::InvalidReservation);
        }
        self.last_tag = reserved;
        if reserved > MAX_TAG {
            self.recovery_required = true;
            return Err(Error::Exhausted);
        }
        let tag = u16::try_from(reserved).map_err(|_| Error::Exhausted)?;
        self.pending = Some(Pending {
            request,
            tag,
            template,
            expires,
        });
        Ok(tag)
    }

    /// A bounded secure Status or Identify packet. Status packets cannot grant
    /// access or extend the deadline. Any malformed, wrong, expired or rejected
    /// packet revokes this attempt; cancellation burns the already reserved tag.
    /// The clock callback must read trusted device monotonic time, including
    /// after blocking hardware calls; never capture a host/snapshot timestamp.
    pub fn receive(
        &mut self,
        frame: &[u8],
        request: &Request,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Option<VerifiedRequest>, Error> {
        let result = self.receive_inner(frame, request, &mut clock);
        if result.is_err() || matches!(&result, Ok(Some(_))) {
            self.pending = None;
        }
        result
    }

    fn receive_inner(
        &mut self,
        frame: &[u8],
        request: &Request,
        clock: &mut impl FnMut() -> u64,
    ) -> Result<Option<VerifiedRequest>, Error> {
        let now = clock();
        self.time(now)?;
        let pending = self.pending.ok_or(Error::Locked)?;
        if now >= pending.expires {
            return Err(Error::Expired);
        }
        if *request != pending.request {
            return Err(Error::WrongRequest);
        }
        if frame.len() != FRAME_SIZE {
            return Err(Error::InvalidFrame);
        }
        let header: &[u8; 8] = frame[..8].try_into().map_err(|_| Error::InvalidFrame)?;
        let kind = word(header, 2);
        if word(header, 0) != VERSION
            || !matches!(kind, RESPONSE | EVENT)
            || word(header, 4) != APP_SECURE
            || word(header, 6) != 40
        {
            return Err(Error::InvalidFrame);
        }
        let iv = frame[8..20].try_into().map_err(|_| Error::InvalidFrame)?;
        let mac = frame[20..36].try_into().map_err(|_| Error::InvalidFrame)?;
        let mut body = Zeroizing::new([0_u8; 12]);
        body.copy_from_slice(&frame[36..]);
        self.aead
            .decrypt_identify_payload(header, iv, mac, &mut body)
            .map_err(|_| Error::Unauthenticated)?;
        // Re-read trusted time after blocking cryptography; a stale caller
        // timestamp must not extend authorization across a slow hardware call.
        let after_crypto = clock();
        self.time(after_crypto)?;
        if after_crypto >= pending.expires {
            return Err(Error::Expired);
        }
        if word(&body[..], 2) != kind {
            return Err(Error::UnexpectedResponse);
        }
        if word(&body[..], 0) == STATUS {
            let state = word(&body[..], 6);
            // Ready + secure interface; no reported application/system failure.
            if state & 3 != 3 || state & 0x0400 != 0 || word(&body[..], 8) != 0 {
                return Err(Error::UnexpectedResponse);
            }
            return Ok(None);
        }
        if word(&body[..], 0) != IDENTIFY || kind != EVENT {
            return Err(Error::UnexpectedResponse);
        }
        if word(&body[..], 10) != pending.tag {
            return Err(Error::WrongTag);
        }
        if word(&body[..], 6) != SPECIFIED || word(&body[..], 8) != pending.template {
            return Err(Error::WrongTemplate);
        }
        if word(&body[..], 4) != MATCH {
            return Err(Error::NoMatch);
        }
        let proof = AuthenticatedMatch {
            request: pending.request,
            tag: pending.tag,
        };
        self.counter.record_match(&proof).map_err(|_| {
            self.recovery_required = true;
            Error::Storage
        })?;
        let after_commit = clock();
        self.time(after_commit)?;
        if after_commit >= pending.expires {
            return Err(Error::Expired);
        }
        Ok(Some(VerifiedRequest {
            request: pending.request,
            expires: pending.expires,
        }))
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }
}

impl<C: ProtectedAttempts, A: ProtectedIdentifyGcm> Session<C, A> {
    /// Production sender: local intent -> durable debit/tag -> live clock ->
    /// fixed Identify body -> protected GCM -> live clock. No packet leaves on
    /// any storage/entropy/crypto/deadline error. Transport must recheck again
    /// immediately before and after its blocking write and never retry it.
    pub fn begin_encrypted(
        &mut self,
        request: Request,
        template: u16,
        mut clock: impl FnMut() -> u64,
    ) -> Result<EncryptedIdentify, Error> {
        let result = (|| {
            let tag = self.begin(request, template, clock())?;
            let pending = self.pending.ok_or(Error::Locked)?;
            self.time(clock())?;
            if self.last_time >= pending.expires {
                return Err(Error::Expired);
            }
            let header = [4, 0, 0x11, 0, 0x11, 0, 38, 0];
            let mut body = Zeroizing::new([0u8; 10]);
            for (index, word) in [IDENTIFY, 0x11, SPECIFIED, template, tag]
                .into_iter()
                .enumerate()
            {
                body[2 * index..2 * index + 2].copy_from_slice(&word.to_le_bytes());
            }
            let (iv, mac) = self
                .aead
                .seal_identify_payload(&header, &mut body)
                .map_err(|_| {
                    self.recovery_required = true;
                    Error::RecoveryRequired
                })?;
            self.time(clock())?;
            if self.last_time >= pending.expires {
                return Err(Error::Expired);
            }
            let mut wire = [0u8; 46];
            wire[..8].copy_from_slice(&header);
            wire[8..20].copy_from_slice(&iv);
            wire[20..36].copy_from_slice(&mac);
            wire[36..].copy_from_slice(&body[..]);
            Ok(EncryptedIdentify {
                wire,
                request,
                expires: pending.expires,
            })
        })();
        if result.is_err() {
            self.pending = None;
        }
        result
    }
}

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::Operation;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, Tag, aead::AeadInPlace};
    use core::sync::atomic::{AtomicU64, Ordering};
    use std::{cell::Cell, rc::Rc};

    struct Counter {
        value: Rc<Cell<u32>>,
        uncertain: bool,
        fail_commit: bool,
    }
    impl ProtectedAttempts for Counter {
        fn reserve_identify_attempt(&mut self, _: &Request) -> Result<u32, Error> {
            self.value.set(self.value.get() + 1);
            if self.uncertain {
                Err(Error::Storage)
            } else {
                Ok(self.value.get())
            }
        }
        fn record_match(&mut self, _: &AuthenticatedMatch) -> Result<(), Error> {
            if self.fail_commit {
                Err(Error::Storage)
            } else {
                Ok(())
            }
        }
    }
    // Public deterministic test key only; never linked as a production backend.
    struct TestGcm;
    impl ProtectedGcm for TestGcm {
        fn decrypt_identify_payload(
            &mut self,
            header: &[u8; 8],
            iv: &[u8; 12],
            tag: &[u8; 16],
            payload: &mut [u8; 12],
        ) -> Result<(), Error> {
            Aes256Gcm::new_from_slice(&[0x42; 32])
                .unwrap()
                .decrypt_in_place_detached(
                    Nonce::from_slice(iv),
                    header,
                    payload,
                    Tag::from_slice(tag),
                )
                .map_err(|_| Error::Unauthenticated)
        }
    }
    fn req() -> Request {
        Request {
            operation: Operation::RevealRecoveryCode,
            digest: [3; 32],
            channel: [4; 32],
        }
    }
    fn session(value: Rc<Cell<u32>>) -> Session<Counter, TestGcm> {
        Session::new(
            Counter {
                value,
                uncertain: false,
                fail_commit: false,
            },
            TestGcm,
        )
    }
    struct Outbound {
        calls: Rc<Cell<u32>>,
        fail: bool,
    }
    impl ProtectedGcm for Outbound {
        fn decrypt_identify_payload(
            &mut self,
            header: &[u8; 8],
            iv: &[u8; 12],
            tag: &[u8; 16],
            payload: &mut [u8; 12],
        ) -> Result<(), Error> {
            Aes256Gcm::new_from_slice(&[0x35; 32])
                .unwrap()
                .decrypt_in_place_detached(
                    Nonce::from_slice(iv),
                    header,
                    payload,
                    Tag::from_slice(tag),
                )
                .map_err(|_| Error::Unauthenticated)
        }
    }
    impl ProtectedIdentifyGcm for Outbound {
        fn seal_identify_payload(
            &mut self,
            header: &[u8; 8],
            payload: &mut [u8; 10],
        ) -> Result<([u8; 12], [u8; 16]), Error> {
            self.calls.set(self.calls.get() + 1);
            if self.fail {
                return Err(Error::Unauthenticated);
            }
            // Public deterministic outbound fixture key/nonce, TEST ONLY.
            let iv = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
            let mac = Aes256Gcm::new_from_slice(&[0x35; 32])
                .unwrap()
                .encrypt_in_place_detached(Nonce::from_slice(&iv), header, payload)
                .unwrap();
            Ok((iv, mac.into()))
        }
    }
    fn outbound() -> Session<Counter, Outbound> {
        Session::new(
            Counter {
                value: Rc::new(Cell::new(0)),
                uncertain: false,
                fail_commit: false,
            },
            Outbound {
                calls: Rc::new(Cell::new(0)),
                fail: false,
            },
        )
    }
    #[test]
    fn outbound_wire_matches_independent_aes_gcm_fixture_and_keeps_request_binding() {
        let mut s = outbound();
        let command = s.begin_encrypted(req(), 7, || 100).unwrap();
        // Generated independently with Python cryptography 50.0.0 AESGCM.
        let expected = [
            0x04, 0x00, 0x11, 0x00, 0x11, 0x00, 0x26, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
            0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x9b, 0x98, 0x62, 0xa0, 0x9a, 0x16, 0x3b, 0x21,
            0x7b, 0xfb, 0x77, 0x61, 0x1c, 0x2d, 0x28, 0x56, 0x6d, 0x35, 0xf4, 0x5c, 0xf3, 0xc3,
            0x4d, 0xfc, 0xe6, 0x33,
        ];
        assert_eq!(command.wire_bytes(), &expected);
        assert_eq!(command.request(), &req());
        assert_eq!(command.expires_ms(), 15_100);
        assert_eq!(s.counter.value.get(), 1);
        // Same public key in both directions, distinct IV. Independent Python
        // AESGCM result for the matching template7/tag1 event.
        let response = [
            0x04, 0x00, 0x13, 0x00, 0x41, 0x00, 0x28, 0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
            0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x30, 0x40, 0x4c, 0x6a, 0x46, 0x5f, 0x45, 0xd2,
            0xf2, 0x58, 0x9b, 0x7f, 0xae, 0x6e, 0x82, 0x74, 0x8f, 0x67, 0x8e, 0x2c, 0x8b, 0xc5,
            0x8f, 0x3a, 0xf0, 0x8c, 0xe4, 0x7a,
        ];
        let proof = s.receive(&response, &req(), || 101).unwrap().unwrap();
        assert_eq!(proof.request(), &req());
    }
    #[test]
    fn slow_reservation_or_sealing_returns_no_command_and_burns_the_tag() {
        for ticks in [[0, 15_000, 15_000], [0, 1, 15_000]] {
            let mut s = outbound();
            let mut clock = ticks.into_iter();
            assert!(matches!(
                s.begin_encrypted(req(), 7, || clock.next().unwrap()),
                Err(Error::Expired)
            ));
            assert_eq!(s.counter.value.get(), 1);
            assert!(matches!(
                s.receive(&matched(1), &req(), || 15_001),
                Err(Error::Locked)
            ));
            // Restarting the operation reserves a new tag, never a refund.
            s.begin_encrypted(req(), 7, || 15_002).unwrap();
            assert_eq!(s.counter.value.get(), 2);
        }
    }
    #[test]
    fn uncertain_storage_or_crypto_fault_cannot_emit_or_retry_a_command() {
        let mut s = outbound();
        s.counter.uncertain = true;
        assert!(matches!(
            s.begin_encrypted(req(), 7, || 0),
            Err(Error::Storage)
        ));
        assert_eq!(s.aead.calls.get(), 0);
        s.counter.uncertain = false;
        assert!(matches!(
            s.begin_encrypted(req(), 7, || 1),
            Err(Error::RecoveryRequired)
        ));
        let mut s = outbound();
        s.aead.fail = true;
        assert!(matches!(
            s.begin_encrypted(req(), 7, || 0),
            Err(Error::RecoveryRequired)
        ));
        assert_eq!(s.counter.value.get(), 1);
        s.aead.fail = false;
        assert!(matches!(
            s.begin_encrypted(req(), 7, || 1),
            Err(Error::RecoveryRequired)
        ));
        assert_eq!(s.aead.calls.get(), 1);
        assert_eq!(s.counter.value.get(), 1);
    }
    #[test]
    fn backwards_clock_during_outbound_crypto_requires_recovery() {
        let mut s = outbound();
        let mut clock = [100, 101, 99].into_iter();
        assert!(matches!(
            s.begin_encrypted(req(), 7, || clock.next().unwrap()),
            Err(Error::TimeWentBackwards)
        ));
        assert!(matches!(
            s.receive(&matched(1), &req(), || 102),
            Err(Error::RecoveryRequired)
        ));
        assert!(matches!(
            s.begin_encrypted(req(), 7, || 102),
            Err(Error::RecoveryRequired)
        ));
    }
    fn packet(cmd: u16, kind: u16, fields: [u16; 4]) -> [u8; 48] {
        let mut output = [0_u8; 48];
        for (i, value) in [VERSION, kind, APP_SECURE, 40].iter().enumerate() {
            output[i * 2..i * 2 + 2].copy_from_slice(&value.to_le_bytes());
        }
        static TEST_NONCE: AtomicU64 = AtomicU64::new(1);
        let nonce = TEST_NONCE.fetch_add(1, Ordering::Relaxed);
        output[8..16].copy_from_slice(&nonce.to_le_bytes());
        // Unique test nonces for this public fixture key; no production RNG claim.
        let mut body = [0; 12];
        for (i, value) in [cmd, kind, fields[0], fields[1], fields[2], fields[3]]
            .iter()
            .enumerate()
        {
            body[i * 2..i * 2 + 2].copy_from_slice(&value.to_le_bytes());
        }
        let tag = Aes256Gcm::new_from_slice(&[0x42; 32])
            .unwrap()
            .encrypt_in_place_detached(Nonce::from_slice(&output[8..20]), &output[..8], &mut body)
            .unwrap();
        output[20..36].copy_from_slice(&tag);
        output[36..].copy_from_slice(&body);
        output
    }
    fn matched(tag: u16) -> [u8; 48] {
        packet(IDENTIFY, EVENT, [MATCH, SPECIFIED, 7, tag])
    }
    #[test]
    fn authenticated_match_is_bound_and_single_use() {
        let mut s = session(Rc::new(Cell::new(0)));
        let tag = s.begin(req(), 7, 100).unwrap();
        let proof = s.receive(&matched(tag), &req(), || 101).unwrap().unwrap();
        assert_eq!(proof.request(), &req());
        assert!(matches!(
            s.receive(&matched(tag), &req(), || 102),
            Err(Error::Locked)
        ));
    }
    #[test]
    fn every_packet_bit_mutation_denies_and_revokes() {
        for bit in 0..48 * 8 {
            let mut s = session(Rc::new(Cell::new(0)));
            let tag = s.begin(req(), 7, 0).unwrap();
            let original = matched(tag);
            let mut changed = original;
            changed[bit / 8] ^= 1 << (bit % 8);
            assert!(s.receive(&changed, &req(), || 1).is_err(), "bit {bit}");
            assert!(matches!(
                s.receive(&original, &req(), || 2),
                Err(Error::Locked)
            ));
        }
    }
    #[test]
    fn intercepted_match_cannot_be_replayed_after_power_loss() {
        let durable = Rc::new(Cell::new(0));
        let mut before = session(durable.clone());
        let old = before.begin(req(), 7, 0).unwrap();
        let intercepted = matched(old); // Never delivered to the host before reset.
        drop(before);
        let mut after = session(durable);
        let fresh = after.begin(req(), 7, 0).unwrap();
        assert_ne!(old, fresh);
        assert!(matches!(
            after.receive(&intercepted, &req(), || 1),
            Err(Error::WrongTag)
        ));
    }
    #[test]
    fn exhaustion_never_wraps_and_uncertain_storage_requires_recovery() {
        let mut s = session(Rc::new(Cell::new(MAX_TAG - 1)));
        assert_eq!(s.begin(req(), 7, 0), Ok(u16::MAX));
        s.cancel();
        assert_eq!(s.begin(req(), 7, 1), Err(Error::Exhausted));
        let mut s = session(Rc::new(Cell::new(0)));
        s.counter.uncertain = true;
        assert_eq!(s.begin(req(), 7, 0), Err(Error::Storage));
        s.counter.uncertain = false;
        assert_eq!(s.begin(req(), 7, 1), Err(Error::RecoveryRequired));
    }
    #[test]
    fn valid_crypto_does_not_bypass_context_or_template() {
        for case in 0..5 {
            let mut s = session(Rc::new(Cell::new(0)));
            let tag = s.begin(req(), 7, 0).unwrap();
            let mut request = req();
            let frame = match case {
                0 => {
                    request.digest[0] ^= 1;
                    matched(tag)
                }
                1 => {
                    request.channel[0] ^= 1;
                    matched(tag)
                }
                2 => packet(IDENTIFY, EVENT, [MATCH, SPECIFIED, 8, tag]),
                3 => packet(IDENTIFY, EVENT, [0xbaad, SPECIFIED, 7, tag]),
                _ => matched(tag + 1),
            };
            assert!(s.receive(&frame, &request, || 1).is_err());
        }
    }
    #[test]
    fn status_cannot_authorize_or_extend_deadline() {
        let mut s = session(Rc::new(Cell::new(0)));
        let tag = s.begin(req(), 7, 10).unwrap();
        let status = packet(STATUS, RESPONSE, [0, 0x2003, 0, 0]);
        assert!(s.receive(&status, &req(), || 14_999).unwrap().is_none());
        assert!(matches!(
            s.receive(&matched(tag), &req(), || 15_010),
            Err(Error::Expired)
        ));
    }
    #[test]
    fn rolled_back_reservation_and_bad_packet_lengths_fail_closed() {
        let durable = Rc::new(Cell::new(0));
        let mut s = session(durable.clone());
        s.begin(req(), 7, 0).unwrap();
        s.cancel();
        durable.set(0); // Broken/rolled-back adapter must never produce a proof.
        assert_eq!(s.begin(req(), 7, 1), Err(Error::InvalidReservation));
        for length in [0, 7, 8, 35, 47, 49, 4096] {
            let mut s = session(Rc::new(Cell::new(0)));
            s.begin(req(), 7, 0).unwrap();
            let data = std::vec![0; length];
            assert!(matches!(
                s.receive(&data, &req(), || 1),
                Err(Error::InvalidFrame)
            ));
        }
    }
    #[test]
    fn slow_crypto_or_commit_cannot_make_expired_proof() {
        for ticks in [[1, 15_000, 15_000], [1, 2, 15_000]] {
            let mut s = session(Rc::new(Cell::new(0)));
            let tag = s.begin(req(), 7, 0).unwrap();
            let mut time = ticks.into_iter();
            assert!(matches!(
                s.receive(&matched(tag), &req(), || time.next().unwrap()),
                Err(Error::Expired)
            ));
            assert!(matches!(
                s.receive(&matched(tag), &req(), || 15_001),
                Err(Error::Locked)
            ));
        }
    }
    #[test]
    fn rollback_and_failed_retry_commit_cannot_authorize() {
        let mut s = session(Rc::new(Cell::new(0)));
        let tag = s.begin(req(), 7, 100).unwrap();
        assert!(matches!(
            s.receive(&matched(tag), &req(), || 99),
            Err(Error::TimeWentBackwards)
        ));
        let mut s = session(Rc::new(Cell::new(0)));
        let tag = s.begin(req(), 7, 0).unwrap();
        s.counter.fail_commit = true;
        assert!(matches!(
            s.receive(&matched(tag), &req(), || 1),
            Err(Error::Storage)
        ));
        assert_eq!(s.begin(req(), 7, 2), Err(Error::RecoveryRequired));
    }
}
