use device_policy::Request;
use embedded_hal::{
    delay::DelayNs,
    digital::{ErrorType as DigitalErrorType, InputPin, OutputPin},
    spi::{ErrorKind, ErrorType as SpiErrorType, SpiBus},
};
use embedded_hal_bus::spi::ExclusiveDevice;
use openkeys_board::fingerprint::{Error, Guard, RECEIVE_BYTES, Transport};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Cs(bool),
    Reset(bool),
    Delay(u32),
    Read,
    Write(Vec<u8>),
    Flush,
}
struct Model {
    now: u64,
    allowed: bool,
    ready: bool,
    io_error: bool,
    irq_error: bool,
    reset_error: bool,
    expire_after_io: bool,
    io_elapsed: u64,
    frames: VecDeque<[u8; RECEIVE_BYTES]>,
    events: Vec<Event>,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            now: 0,
            allowed: true,
            ready: false,
            io_error: false,
            irq_error: false,
            reset_error: false,
            expire_after_io: false,
            io_elapsed: 0,
            frames: VecDeque::new(),
            events: Vec::new(),
        }
    }
}
type Shared = Rc<RefCell<Model>>;
struct Bus(Shared);
impl Bus {
    fn finish(&self) -> Result<(), ErrorKind> {
        let mut state = self.0.borrow_mut();
        state.now += state.io_elapsed;
        if state.expire_after_io {
            state.allowed = false;
        }
        if state.io_error {
            Err(ErrorKind::Other)
        } else {
            Ok(())
        }
    }
}
impl SpiErrorType for Bus {
    type Error = ErrorKind;
}
impl SpiBus for Bus {
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let mut state = self.0.borrow_mut();
        state.events.push(Event::Read);
        assert_eq!(bytes.len(), RECEIVE_BYTES);
        bytes.copy_from_slice(&state.frames.pop_front().unwrap());
        drop(state);
        self.finish()
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0
            .borrow_mut()
            .events
            .push(Event::Write(bytes.to_vec()));
        self.finish()
    }
    fn transfer(&mut self, _: &mut [u8], _: &[u8]) -> Result<(), Self::Error> {
        panic!("unexpected duplex operation")
    }
    fn transfer_in_place(&mut self, _: &mut [u8]) -> Result<(), Self::Error> {
        panic!("unexpected in-place operation")
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.0.borrow_mut().events.push(Event::Flush);
        Ok(())
    }
}
struct Pin(Shared, bool);
impl DigitalErrorType for Pin {
    type Error = embedded_hal::digital::ErrorKind;
}
impl Pin {
    fn set(&self, high: bool) -> Result<(), embedded_hal::digital::ErrorKind> {
        let mut state = self.0.borrow_mut();
        state.events.push(if self.1 {
            Event::Reset(high)
        } else {
            Event::Cs(high)
        });
        if self.1 && state.reset_error {
            Err(embedded_hal::digital::ErrorKind::Other)
        } else {
            Ok(())
        }
    }
}
impl OutputPin for Pin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.set(false)
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set(true)
    }
}
struct Irq(Shared);
impl DigitalErrorType for Irq {
    type Error = embedded_hal::digital::ErrorKind;
}
impl InputPin for Irq {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        let state = self.0.borrow();
        if state.irq_error {
            Err(embedded_hal::digital::ErrorKind::Other)
        } else {
            Ok(state.ready)
        }
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.is_high().map(|high| !high)
    }
}
struct Delay(Shared);
impl DelayNs for Delay {
    fn delay_ns(&mut self, ns: u32) {
        let mut state = self.0.borrow_mut();
        state.events.push(Event::Delay(ns));
        state.now += u64::from(ns).div_ceil(1000);
    }
}
struct LiveGuard(Shared);
impl Guard for LiveGuard {
    fn check(&mut self) -> Result<(), Error> {
        if self.0.borrow().allowed {
            Ok(())
        } else {
            Err(Error::Authorization)
        }
    }
    fn now_us(&mut self) -> u64 {
        self.0.borrow().now
    }
    fn check_request(&mut self, request: &Request) -> Result<(), Error> {
        if request == &fixture_request() {
            self.check()
        } else {
            Err(Error::Authorization)
        }
    }
}

fn fixture_request() -> Request {
    Request {
        operation: device_policy::Operation::RevealRecoveryCode,
        digest: [3; 32],
        channel: [4; 32],
    }
}
type Port = Transport<ExclusiveDevice<Bus, Pin, Delay>, Irq, Pin>;
fn held(state: &Shared) -> Port {
    let device = ExclusiveDevice::new(
        Bus(state.clone()),
        Pin(state.clone(), false),
        Delay(state.clone()),
    )
    .unwrap();
    Transport::new(device, Irq(state.clone()), Pin(state.clone(), true)).unwrap()
}
fn active(state: &Shared) -> Port {
    let mut port = held(state);
    port.release(&mut Delay(state.clone()), &mut LiveGuard(state.clone()))
        .unwrap();
    state.borrow_mut().events.clear();
    port
}
fn request() -> [u8; 46] {
    let mut frame = [0x65; 46]; // Public arbitrary ciphertext, not a valid MAC.
    frame[..8].copy_from_slice(&[4, 0, 0x11, 0, 0x11, 0, 38, 0]);
    frame
}
fn reply() -> [u8; RECEIVE_BYTES] {
    let mut frame = [0x37; RECEIVE_BYTES];
    frame[..8].copy_from_slice(&[4, 0, 0x13, 0, 0x41, 0, 40, 0]);
    frame
}
fn spi_count(state: &Shared) -> usize {
    state
        .borrow()
        .events
        .iter()
        .filter(|event| matches!(event, Event::Read | Event::Write(_)))
        .count()
}
fn encrypted_identify(request: Request) -> device_policy::biometric::EncryptedIdentify {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::AeadInPlace};
    use device_policy::biometric::{
        AuthenticatedMatch, Error as BioError, ProtectedAttempts, ProtectedGcm,
        ProtectedIdentifyGcm, Session,
    };
    // Private model providers for public fixture data only. No runtime fake
    // provider or host authorization Boolean is exported by the board crate.
    struct Counter;
    impl ProtectedAttempts for Counter {
        fn reserve_identify_attempt(&mut self, _: &Request) -> Result<u32, BioError> {
            Ok(1)
        }
        fn record_match(&mut self, _: &AuthenticatedMatch) -> Result<(), BioError> {
            Err(BioError::Storage)
        }
    }
    struct Aead;
    impl ProtectedGcm for Aead {
        fn decrypt_identify_payload(
            &mut self,
            _: &[u8; 8],
            _: &[u8; 12],
            _: &[u8; 16],
            _: &mut [u8; 12],
        ) -> Result<(), BioError> {
            Err(BioError::Unauthenticated)
        }
    }
    impl ProtectedIdentifyGcm for Aead {
        fn seal_identify_payload(
            &mut self,
            header: &[u8; 8],
            body: &mut [u8; 10],
        ) -> Result<([u8; 12], [u8; 16]), BioError> {
            let iv = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
            let tag = Aes256Gcm::new_from_slice(&[0x35; 32])
                .unwrap()
                .encrypt_in_place_detached(Nonce::from_slice(&iv), header, body)
                .unwrap();
            Ok((iv, tag.into()))
        }
    }
    Session::new(Counter, Aead)
        .begin_encrypted(request, 7, || 10)
        .unwrap()
}
#[test]
fn policy_minted_command_is_consumed_with_request_and_deadline_checks() {
    let state = Shared::default();
    let mut port = active(&state);
    let command = encrypted_identify(fixture_request());
    let expected = *command.wire_bytes();
    port.send_identify(command, &mut LiveGuard(state.clone()))
        .unwrap();
    assert!(
        state
            .borrow()
            .events
            .contains(&Event::Write(expected.to_vec()))
    );
    assert_eq!(spi_count(&state), 1);
    let state = Shared::default();
    let mut port = active(&state);
    let mut other = fixture_request();
    other.channel[0] ^= 1;
    assert_eq!(
        port.send_identify(encrypted_identify(other), &mut LiveGuard(state.clone())),
        Err(Error::Authorization)
    );
    assert_eq!(spi_count(&state), 0);
    assert_eq!(state.borrow().events.last(), Some(&Event::Reset(false)));
}
#[test]
fn a_policy_command_cannot_be_sent_or_return_success_after_its_deadline() {
    for already_expired in [true, false] {
        let state = Shared::default();
        let mut port = active(&state);
        let command = encrypted_identify(fixture_request()); // expires at15010 ms
        state.borrow_mut().now = if already_expired {
            15_010_000
        } else {
            15_009_900
        };
        assert_eq!(
            port.send_identify(command, &mut LiveGuard(state.clone())),
            Err(Error::Time)
        );
        assert_eq!(spi_count(&state), usize::from(!already_expired));
        assert_eq!(state.borrow().events.last(), Some(&Event::Reset(false)));
    }
}
#[test]
fn reset_release_is_explicit_guarded_and_held_before_any_clock() {
    let state = Shared::default();
    let mut port = held(&state);
    assert_eq!(
        state.borrow().events,
        [Event::Cs(true), Event::Reset(false)]
    );
    port.release(&mut Delay(state.clone()), &mut LiveGuard(state.clone()))
        .unwrap();
    assert_eq!(
        &state.borrow().events[2..],
        &[Event::Delay(10_000_000), Event::Reset(true)]
    );
    assert_eq!(spi_count(&state), 0);
    assert!(
        port.release(&mut Delay(state.clone()), &mut LiveGuard(state.clone()))
            .is_err()
    );
    assert_eq!(state.borrow().events.last(), Some(&Event::Reset(false)));
}
#[test]
fn cs_remains_asserted_for_the_wake_delay_and_the_whole_frame() {
    let state = Shared::default();
    let mut port = active(&state);
    let mut guard = LiveGuard(state.clone());
    port.write(&request(), &mut guard).unwrap();
    assert_eq!(
        state.borrow().events,
        [
            Event::Cs(false),
            Event::Flush,
            Event::Delay(600_000),
            Event::Write(request().to_vec()),
            Event::Flush,
            Event::Cs(true)
        ]
    );
    state.borrow_mut().events.clear();
    assert!(port.poll(&mut guard).unwrap().is_none());
    assert_eq!(spi_count(&state), 0);
    state.borrow_mut().ready = true;
    state.borrow_mut().frames.push_back(reply());
    let frame = port.poll(&mut guard).unwrap().unwrap();
    assert_eq!(frame.wire_bytes(), &reply());
    assert_eq!(
        state.borrow().events,
        [
            Event::Cs(false),
            Event::Flush,
            Event::Delay(600_000),
            Event::Read,
            Event::Flush,
            Event::Cs(true)
        ]
    );
    // Framing is not authentication: the arbitrary tag was NOT a match proof.
}
#[test]
fn disabled_or_revoked_links_never_clock_or_recover() {
    let state = Shared::default();
    let mut port = held(&state);
    let mut guard = LiveGuard(state.clone());
    assert!(matches!(port.poll(&mut guard), Err(Error::Disabled)));
    assert_eq!(port.write(&request(), &mut guard), Err(Error::Faulted));
    assert_eq!(spi_count(&state), 0);
    let state = Shared::default();
    let mut port = active(&state);
    state.borrow_mut().allowed = false;
    assert_eq!(
        port.write(&request(), &mut LiveGuard(state.clone())),
        Err(Error::Authorization)
    );
    state.borrow_mut().allowed = true;
    assert_eq!(
        port.write(&request(), &mut LiveGuard(state.clone())),
        Err(Error::Faulted)
    );
    assert_eq!(spi_count(&state), 0);
}
#[test]
fn malformed_or_plaintext_requests_are_refused_before_spi() {
    for length in 0..=49 {
        let state = Shared::default();
        let mut port = active(&state);
        if length == 46 {
            continue;
        }
        let mut bytes = request().to_vec();
        bytes.resize(length, 0);
        assert_eq!(
            port.write(&bytes, &mut LiveGuard(state.clone())),
            Err(Error::Frame)
        );
        assert_eq!(spi_count(&state), 0);
    }
    for index in 0..8 {
        let state = Shared::default();
        let mut port = active(&state);
        let mut frame = request();
        frame[index] ^= 1;
        assert_eq!(
            port.write(&frame, &mut LiveGuard(state.clone())),
            Err(Error::Frame)
        );
        assert_eq!(spi_count(&state), 0);
    }
}
#[test]
fn partial_io_errors_are_not_retried_and_cs_is_deasserted() {
    for read in [false, true] {
        let state = Shared::default();
        let mut port = active(&state);
        state.borrow_mut().ready = true;
        state.borrow_mut().frames.push_back(reply());
        state.borrow_mut().io_error = true;
        let mut guard = LiveGuard(state.clone());
        if read {
            assert!(matches!(port.poll(&mut guard), Err(Error::Io)));
        } else {
            assert_eq!(port.write(&request(), &mut guard), Err(Error::Io));
        }
        assert_eq!(spi_count(&state), 1);
        {
            let state = state.borrow();
            let events = &state.events;
            assert_eq!(
                &events[events.len() - 3..],
                &[Event::Flush, Event::Cs(true), Event::Reset(false)]
            );
        }
        state.borrow_mut().io_error = false;
        assert!(matches!(port.poll(&mut guard), Err(Error::Faulted)));
        assert_eq!(spi_count(&state), 1);
    }
}
#[test]
fn post_io_revocation_or_slow_io_cannot_return_an_untrusted_frame() {
    for mode in 0..3 {
        let state = Shared::default();
        let mut port = active(&state);
        let mut guard = LiveGuard(state.clone());
        state.borrow_mut().ready = true;
        state.borrow_mut().frames.push_back(reply());
        match mode {
            0 => state.borrow_mut().expire_after_io = true,
            1 => state.borrow_mut().io_elapsed = 100_000,
            2 => state.borrow_mut().now = 0, // below last release timestamp
            _ => unreachable!(),
        }
        assert!(port.poll(&mut guard).is_err());
        assert_eq!(state.borrow().events.last(), Some(&Event::Reset(false)));
        assert!(matches!(port.poll(&mut guard), Err(Error::Faulted)));
    }
}
#[test]
fn malformed_reply_and_frame_flood_permanently_stop_the_sensor() {
    for index in 0..8 {
        let state = Shared::default();
        let mut port = active(&state);
        state.borrow_mut().ready = true;
        let mut frame = reply();
        frame[index] ^= if index == 2 { 4 } else { 1 };
        state.borrow_mut().frames.push_back(frame);
        assert!(matches!(
            port.poll(&mut LiveGuard(state.clone())),
            Err(Error::Frame)
        ));
    }
    let state = Shared::default();
    let mut port = active(&state);
    state.borrow_mut().ready = true;
    state.borrow_mut().frames.extend([reply(); 32]);
    let mut guard = LiveGuard(state.clone());
    for _ in 0..32 {
        assert!(port.poll(&mut guard).unwrap().is_some());
    }
    assert!(matches!(port.poll(&mut guard), Err(Error::Budget)));
    assert_eq!(spi_count(&state), 32);
    assert_eq!(state.borrow().events.last(), Some(&Event::Reset(false)));
}
#[test]
fn irq_or_reset_failure_is_terminal_and_drop_asserts_reset() {
    let state = Shared::default();
    let mut port = active(&state);
    state.borrow_mut().irq_error = true;
    assert!(matches!(
        port.poll(&mut LiveGuard(state.clone())),
        Err(Error::Io)
    ));
    assert_eq!(spi_count(&state), 0);
    let state = Shared::default();
    let mut port = held(&state);
    state.borrow_mut().reset_error = true;
    assert_eq!(
        port.release(&mut Delay(state.clone()), &mut LiveGuard(state.clone())),
        Err(Error::Io)
    );
    state.borrow_mut().reset_error = false;
    assert_eq!(
        port.release(&mut Delay(state.clone()), &mut LiveGuard(state.clone())),
        Err(Error::Faulted)
    );
    let state = Shared::default();
    drop(active(&state));
    assert_eq!(state.borrow().events.last(), Some(&Event::Reset(false)));
}
