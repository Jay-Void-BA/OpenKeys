use openkeys_se_transport::transport::{Clock, Io, IoError, Transport};
use openkeys_se_transport::{Control, Direction, Error, Kind, MAX_INF, ReceiveError, WireFrame};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

const ATR: &[u8] = &[
    0, 0xa0, 0, 0, 3, 0x96, 4, 3, 0xe8, 0, 0xfe, 2, 11, 3, 0xe8, 8, 1, 0, 0, 0, 0, 100, 0, 0, 10,
    b'J', b'C', b'O', b'P', b'4', b' ', b'A', b'T', b'P', b'O',
];
#[derive(Default)]
struct Script {
    reads: VecDeque<Result<Vec<u8>, IoError>>,
    writes: Vec<Vec<u8>>,
    fail_write: bool,
}
type Shared = Rc<RefCell<Script>>;
fn queue(s: &Shared, kind: Kind, info: &[u8]) {
    let f = WireFrame::encode(Direction::SeToHost, kind, info).unwrap();
    let bytes = f.bytes();
    let mut s = s.borrow_mut();
    s.reads.push_back(Ok(bytes[..3].to_vec()));
    if !info.is_empty() {
        s.reads.push_back(Ok(info.to_vec()));
    }
    s.reads.push_back(Ok(bytes[bytes.len() - 2..].to_vec()));
}
struct Bus(Shared);
impl Io for Bus {
    fn write(&mut self, bytes: &[u8]) -> Result<(), IoError> {
        let mut s = self.0.borrow_mut();
        s.writes.push(bytes.to_vec());
        if s.fail_write {
            Err(IoError::Fault)
        } else {
            Ok(())
        }
    }
    fn read(&mut self, out: &mut [u8]) -> Result<(), IoError> {
        let bytes = self
            .0
            .borrow_mut()
            .reads
            .pop_front()
            .expect("Unexpected read")?;
        assert_eq!(out.len(), bytes.len());
        out.copy_from_slice(&bytes);
        Ok(())
    }
}
struct Time(Rc<Cell<u64>>);
impl Clock for Time {
    fn now_us(&mut self) -> u64 {
        self.0.get()
    }
    fn wait_us(&mut self, us: u32) {
        self.0.set(self.0.get() + u64::from(us));
    }
}
fn setup() -> (Transport<Bus, Time>, Shared, Rc<Cell<u64>>) {
    let s = Shared::default();
    let t = Rc::new(Cell::new(0));
    queue(
        &s,
        Kind::Supervisory {
            response: true,
            control: Control::Resync,
        },
        &[],
    );
    queue(
        &s,
        Kind::Supervisory {
            response: true,
            control: Control::InterfaceReset,
        },
        ATR,
    );
    let mut transport = Transport::new(Bus(s.clone()), Time(t.clone()));
    transport.synchronize(0).unwrap();
    (transport, s, t)
}
fn info(sequence: bool, more: bool) -> Kind {
    Kind::Information { sequence, more }
}

#[test]
fn consecutive_exchanges_preserve_both_sequence_bits() {
    let (mut t, s, _) = setup();
    let mut out = [0; 1024];
    for sequence in [false, true, false] {
        queue(&s, info(sequence, false), &[0x90, 0]);
        assert_eq!(
            t.exchange_untrusted(&[0, 0xa4, 4, 0], &mut out)
                .unwrap()
                .wire_bytes(),
            [0x90, 0]
        );
        let packet = s.borrow().writes.last().unwrap().clone();
        assert_eq!(packet[1], if sequence { 0x40 } else { 0 });
    }
}
#[test]
fn chained_command_ack_and_response_are_strict() {
    let (mut t, s, _) = setup();
    queue(
        &s,
        Kind::Receive {
            next_sequence: true,
            error: ReceiveError::None,
        },
        &[],
    );
    queue(&s, info(false, true), &[0x77; MAX_INF]);
    queue(&s, info(true, false), &[0x90, 0]);
    let mut out = [0; 1024];
    assert_eq!(
        t.exchange_untrusted(&[0x55; MAX_INF + 1], &mut out)
            .unwrap()
            .wire_bytes()
            .len(),
        MAX_INF + 2
    );
    let s = s.borrow();
    assert_eq!(s.writes[2][1], 0x20);
    assert_eq!(s.writes[3][1], 0x40);
    assert_eq!(s.writes[4][1], 0x90);
}
#[test]
fn valid_crc_wrong_sequence_is_rejected_and_session_latches() {
    let (mut t, s, _) = setup();
    queue(&s, info(true, false), &[0x90, 0]);
    let mut out = [0xa5; 1024];
    assert!(matches!(
        t.exchange_untrusted(&[0x80, 0xca, 0, 0], &mut out),
        Err(Error::Sequence)
    ));
    assert!(out.iter().all(|x| *x == 0));
    let calls = s.borrow().writes.len();
    assert!(matches!(
        t.exchange_untrusted(&[0x80, 0xca, 0, 0], &mut out),
        Err(Error::Faulted)
    ));
    assert_eq!(s.borrow().writes.len(), calls);
}
#[test]
fn wrong_chain_ack_is_never_only_a_warning() {
    let (mut t, s, _) = setup();
    queue(
        &s,
        Kind::Receive {
            next_sequence: false,
            error: ReceiveError::None,
        },
        &[],
    );
    assert!(matches!(
        t.exchange_untrusted(&[0; MAX_INF + 1], &mut [0; 1024]),
        Err(Error::Sequence)
    ));
}
#[test]
fn waiting_time_extensions_have_fixed_budget() {
    let (mut t, s, _) = setup();
    for _ in 0..5 {
        queue(
            &s,
            Kind::Supervisory {
                response: false,
                control: Control::Wtx,
            },
            &[1],
        );
    }
    assert!(matches!(
        t.exchange_untrusted(&[0x80, 0xca, 0, 0], &mut [0; 1024]),
        Err(Error::Timeout)
    ));
    assert_eq!(s.borrow().writes.len(), 2 + 1 + 4);
}
#[test]
fn partial_io_and_clock_rollback_cannot_restart_a_private_command() {
    let (mut t, s, _) = setup();
    s.borrow_mut().fail_write = true;
    assert!(matches!(
        t.exchange_untrusted(&[0x80, 0xca, 0, 0], &mut [0; 1024]),
        Err(Error::Io)
    ));
    s.borrow_mut().fail_write = false;
    assert!(matches!(
        t.exchange_untrusted(&[0x80, 0xca, 0, 0], &mut [0; 1024]),
        Err(Error::Faulted)
    ));
    let (mut t, _, clock) = setup();
    clock.set(0);
    assert!(matches!(
        t.exchange_untrusted(&[0x80, 0xca, 0, 0], &mut [0; 1024]),
        Err(Error::Timeout)
    ));
}
#[test]
fn extended_length_and_mid_frame_nack_do_not_shift_the_stream() {
    let (mut t, s, _) = setup();
    s.borrow_mut().reads.push_back(Ok(vec![0xa5, 0, 255]));
    assert!(matches!(
        t.exchange_untrusted(&[0x80, 0xca, 0, 0], &mut [0; 1024]),
        Err(Error::Length)
    ));
    let (mut t, s, _) = setup();
    s.borrow_mut().reads.push_back(Ok(vec![0xa5, 0, 2]));
    s.borrow_mut()
        .reads
        .push_back(Err(IoError::AddressNotReady));
    assert!(matches!(
        t.exchange_untrusted(&[0x80, 0xca, 0, 0], &mut [0; 1024]),
        Err(Error::Io)
    ));
    assert!(matches!(t.synchronize(0), Err(Error::Faulted)));
}
