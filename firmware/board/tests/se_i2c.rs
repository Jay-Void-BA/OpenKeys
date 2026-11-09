use openkeys_board::se_i2c::{
    BUSY, Error, FAULTS, Master, NACK, PE, RXNE, Register, Registers, STOP, TCR, TXE, TXIS,
};
use openkeys_se_transport::{
    MAX_FRAME,
    transport::{Clock, Io, IoError},
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

struct Model {
    cr1: u32,
    timing: u32,
    status: u32,
    now: u64,
    tick: u64,
    bytes: Vec<u8>,
    rx: VecDeque<u8>,
    cr2: Vec<u32>,
    chunk: usize,
    reload: bool,
    read: bool,
    progress: usize,
    address_nack: bool,
    nack_after: Option<usize>,
    fault_after: Option<usize>,
    timing_after: Option<usize>,
    rollback_after: Option<usize>,
    sticky_stop: bool,
    stalled: bool,
    reads: u64,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            cr1: PE,
            timing: 0x00b0_3fdb,
            status: TXE,
            now: 0,
            tick: 1,
            bytes: Vec::new(),
            rx: VecDeque::new(),
            cr2: Vec::new(),
            chunk: 0,
            reload: false,
            read: false,
            progress: 0,
            address_nack: false,
            nack_after: None,
            fault_after: None,
            timing_after: None,
            rollback_after: None,
            sticky_stop: false,
            stalled: false,
            reads: 0,
        }
    }
}
impl Model {
    fn data_status(&mut self) {
        self.status = TXE | BUSY | if self.read { RXNE } else { TXIS };
    }
    fn advanced(&mut self) {
        self.progress += 1;
        self.chunk -= 1;
        if self.rollback_after == Some(self.progress) {
            self.now = 0;
        }
        if self.nack_after == Some(self.progress) {
            self.status = TXE | NACK | STOP;
        } else if self.fault_after == Some(self.progress) {
            self.status = TXE | FAULTS;
        } else if self.chunk == 0 {
            self.status = TXE | if self.reload { BUSY | TCR } else { STOP };
        } else {
            self.data_status();
        }
    }
}
type Shared = Rc<RefCell<Model>>;
struct Port(Shared);
impl Registers for Port {
    fn read(&mut self, reg: Register) -> Result<u32, Error> {
        let mut m = self.0.borrow_mut();
        Ok(match reg {
            Register::Cr1 => m.cr1,
            Register::Timing => {
                if m.timing_after == Some(m.progress) {
                    m.timing ^ 1
                } else {
                    m.timing
                }
            }
            Register::Isr => {
                m.reads += 1;
                if m.stalled && !m.cr2.is_empty() {
                    TXE | BUSY
                } else {
                    m.status
                }
            }
            Register::Rxdr => {
                assert!(m.read);
                let byte = m.rx.pop_front().expect("Unexpected RXDR read");
                m.advanced();
                u32::from(byte)
            }
            _ => panic!("Unexpected register read"),
        })
    }
    fn write(&mut self, reg: Register, value: u32) -> Result<(), Error> {
        let mut m = self.0.borrow_mut();
        match reg {
            Register::Cr1 => m.cr1 = value,
            Register::Cr2 => {
                assert_eq!(value & 0x3ff, 0x48 << 1);
                assert_eq!(value & (1 << 11), 0); // 7-bit only.
                let start = value & (1 << 13) != 0;
                if start {
                    m.progress = 0;
                } else {
                    assert_ne!(m.status & TCR, 0);
                }
                m.cr2.push(value);
                m.chunk = ((value >> 16) & 255) as usize;
                assert_ne!(m.chunk, 0);
                m.reload = value & (1 << 24) != 0;
                assert_ne!(value & ((1 << 24) | (1 << 25)), 0);
                m.read = value & (1 << 10) != 0;
                if m.address_nack {
                    m.status = TXE | NACK | STOP;
                } else {
                    m.data_status();
                }
            }
            Register::Icr => {
                if !m.sticky_stop {
                    m.status &= !value;
                }
            }
            Register::Txdr => {
                assert!(!m.read);
                m.bytes.push(value as u8);
                m.advanced();
            }
            _ => panic!("Unexpected register write"),
        }
        Ok(())
    }
}
struct Time(Shared);
impl Clock for Time {
    fn now_us(&mut self) -> u64 {
        let mut m = self.0.borrow_mut();
        m.now = m.now.saturating_add(m.tick);
        m.now
    }
    fn wait_us(&mut self, us: u32) {
        self.0.borrow_mut().now += u64::from(us);
    }
}
fn master(m: &Shared) -> Master<Port, Time> {
    Master::new(Port(m.clone()), Time(m.clone())).unwrap()
}
fn data(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 256) as u8).collect()
}

#[test]
fn complete_transfers_cover_nbytes_and_reload_boundaries() {
    for len in [1, 3, 254, 255, 256, MAX_FRAME] {
        for read in [false, true] {
            let m = Shared::default();
            let mut bus = master(&m);
            let expected = data(len);
            if read {
                m.borrow_mut().rx.extend(expected.iter().copied());
                let mut out = vec![0x55; len];
                bus.read(&mut out).unwrap();
                assert_eq!(out, expected);
            } else {
                bus.write(&expected).unwrap();
                assert_eq!(m.borrow().bytes, expected);
            }
            let m = m.borrow();
            assert_eq!(m.status, TXE);
            assert_eq!(m.cr2.len(), if len > 255 { 2 } else { 1 });
            assert_ne!(m.cr2[0] & (1 << 13), 0);
            if len > 255 {
                assert_ne!(m.cr2[0] & (1 << 24), 0);
                assert_eq!(m.cr2[1] & (1 << 13), 0);
            }
            assert_ne!(m.cr2.last().unwrap() & (1 << 25), 0);
            assert_ne!(m.cr1 & PE, 0);
        }
    }
}
#[test]
fn only_zero_progress_address_nack_can_retry_after_confirmed_stop() {
    for read in [false, true] {
        let m = Shared::default();
        let mut bus = master(&m);
        m.borrow_mut().address_nack = true;
        let mut output = [0x55; 4];
        let result = if read {
            bus.read(&mut output)
        } else {
            bus.write(&[1, 2, 3, 4])
        };
        assert_eq!(result, Err(IoError::AddressNotReady));
        assert!(m.borrow().bytes.is_empty());
        assert_eq!(m.borrow().status, TXE);
        assert_ne!(m.borrow().cr1 & PE, 0);
        if read {
            assert_eq!(output, [0; 4]);
        }
        m.borrow_mut().address_nack = false;
        m.borrow_mut().rx.extend([4, 3, 2, 1]);
        if read {
            bus.read(&mut output).unwrap();
            assert_eq!(output, [4, 3, 2, 1]);
        } else {
            bus.write(&[1, 2, 3, 4]).unwrap();
            assert_eq!(m.borrow().bytes, [1, 2, 3, 4]);
        }
    }
}
#[test]
fn every_data_nack_is_terminal_and_read_output_is_erased() {
    for read in [false, true] {
        for cut in 1..=MAX_FRAME {
            let m = Shared::default();
            let mut bus = master(&m);
            m.borrow_mut().nack_after = Some(cut);
            m.borrow_mut().rx.extend(data(MAX_FRAME));
            let mut out = vec![0x55; MAX_FRAME];
            let result = if read {
                bus.read(&mut out)
            } else {
                bus.write(&data(MAX_FRAME))
            };
            assert_eq!(result, Err(IoError::Fault), "cut {cut}");
            if read {
                assert!(out.iter().all(|b| *b == 0));
            }
            assert_eq!(m.borrow().cr1 & PE, 0);
            let starts = m.borrow().cr2.len();
            m.borrow_mut().nack_after = None;
            assert_eq!(bus.write(&[1]), Err(IoError::Fault));
            assert_eq!(m.borrow().cr2.len(), starts);
        }
    }
}
#[test]
fn late_bus_fault_clock_rollback_or_configuration_change_cannot_release_data() {
    for case in 0..3 {
        let m = Shared::default();
        let mut bus = master(&m);
        m.borrow_mut().rx.extend([1, 2, 3]);
        match case {
            0 => m.borrow_mut().fault_after = Some(3),
            1 => m.borrow_mut().rollback_after = Some(3),
            2 => m.borrow_mut().timing_after = Some(3),
            _ => unreachable!(),
        }
        let mut out = [0x55; 3];
        assert_eq!(bus.read(&mut out), Err(IoError::Fault));
        assert_eq!(out, [0; 3]);
        assert_eq!(m.borrow().cr1 & PE, 0);
    }
}
#[test]
fn stuck_clock_and_status_flags_have_independent_finite_poll_budget() {
    let m = Shared::default();
    let mut bus = master(&m);
    m.borrow_mut().tick = 0;
    m.borrow_mut().stalled = true;
    assert_eq!(bus.write(&[1]), Err(IoError::Fault));
    assert!(m.borrow().reads <= 2_000_002);
    assert_eq!(m.borrow().cr1 & PE, 0);
}
#[test]
fn live_time_budget_expires_without_waiting_for_poll_budget() {
    let m = Shared::default();
    let mut bus = master(&m);
    m.borrow_mut().tick = 10_000;
    m.borrow_mut().stalled = true;
    let mut out = [0x55];
    assert_eq!(bus.read(&mut out), Err(IoError::Fault));
    assert_eq!(out, [0]);
    assert!(m.borrow().reads < 20);
    assert_eq!(m.borrow().cr1 & PE, 0);
}
#[test]
fn uncleared_stop_after_address_nack_is_not_retryable() {
    let m = Shared::default();
    let mut bus = master(&m);
    m.borrow_mut().address_nack = true;
    m.borrow_mut().sticky_stop = true;
    assert_eq!(bus.write(&[1]), Err(IoError::Fault));
    assert_eq!(m.borrow().cr1 & PE, 0);
}
#[test]
fn stale_or_faulted_initial_controller_is_never_reset_into_success() {
    for status in [BUSY, FAULTS, NACK, STOP, RXNE, TXIS, TCR, 0] {
        let m = Shared::default();
        m.borrow_mut().status = status;
        assert!(Master::new(Port(m.clone()), Time(m.clone())).is_err());
        assert!(m.borrow().cr2.is_empty());
        assert_eq!(m.borrow().cr1 & PE, 0);
    }
}
#[test]
fn interrupt_dma_or_disabled_profiles_are_rejected_before_start() {
    for cr1 in [0, PE | 2, PE | (1 << 14), PE | (1 << 15)] {
        let m = Shared::default();
        m.borrow_mut().cr1 = cr1;
        assert!(Master::new(Port(m.clone()), Time(m.clone())).is_err());
        assert!(m.borrow().cr2.is_empty());
    }
}
#[test]
fn invalid_lengths_or_new_stale_state_do_not_start_a_transaction() {
    for len in [0, MAX_FRAME + 1] {
        let m = Shared::default();
        let mut bus = master(&m);
        let mut out = vec![0x55; len];
        assert_eq!(bus.read(&mut out), Err(IoError::Fault));
        assert!(out.iter().all(|b| *b == 0));
        assert!(m.borrow().cr2.is_empty());
    }
    let m = Shared::default();
    let mut bus = master(&m);
    m.borrow_mut().status = BUSY | TXE;
    assert_eq!(bus.write(&[1]), Err(IoError::Fault));
    assert!(m.borrow().cr2.is_empty());
}
#[test]
fn the_actual_t1_core_uses_progress_aware_reads_and_writes() {
    use openkeys_se_transport::{Control, Direction, Kind, WireFrame, transport::Transport};
    const ATR: &[u8] = &[
        0, 0xa0, 0, 0, 3, 0x96, 4, 3, 0xe8, 0, 0xfe, 2, 11, 3, 0xe8, 8, 1, 0, 0, 0, 0, 100, 0, 0,
        10, b'J', b'C', b'O', b'P', b'4', b' ', b'A', b'T', b'P', b'O',
    ];
    let m = Shared::default();
    let bus = master(&m);
    for (control, info) in [(Control::Resync, &[][..]), (Control::InterfaceReset, ATR)] {
        let frame = WireFrame::encode(
            Direction::SeToHost,
            Kind::Supervisory {
                response: true,
                control,
            },
            info,
        )
        .unwrap();
        m.borrow_mut().rx.extend(frame.bytes());
    }
    let mut transport = Transport::new(bus, Time(m.clone()));
    transport.synchronize(0).unwrap();
    let frame = WireFrame::encode(
        Direction::SeToHost,
        Kind::Information {
            sequence: false,
            more: false,
        },
        &[0x90, 0],
    )
    .unwrap();
    m.borrow_mut().rx.extend(frame.bytes());
    let mut output = [0x55; 1024];
    assert_eq!(
        transport
            .exchange_untrusted(&[0x80, 0xca, 0, 0], &mut output)
            .unwrap()
            .wire_bytes(),
        [0x90, 0]
    );
    assert!(m.borrow().rx.is_empty());
}
