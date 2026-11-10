//! R05 physical PIN flow. A host request alone NEVER spends a PIN attempt.
//! Protected Root must own this object, its ten GPIO inputs, clock and backend.
//! This generic implementation is not a deployed secure boot/TrustZone policy.
use crate::{Error, ten_keys::Keypad};
use device_policy::{
    Request,
    lcd::{Frame, pin_only_progress as pin_progress},
    pin::{Press, ProtectedPin},
    pin_only::{Digit, PIN_DIGITS, PinEntry, VerifiedPinRequest},
};

pub enum Update {
    Quiet,
    Frame(Frame),
    Verified(VerifiedPinRequest),
    Cancelled,
    Denied,
}
enum State {
    Idle,
    Waiting { request: Request, expires: u64 },
    Entering(PinEntry),
}
pub struct PinUi {
    keys: Keypad,
    state: State,
    faulted: bool,
}
impl PinUi {
    pub const fn new(now: u64) -> Self {
        Self {
            keys: Keypad::new(now),
            state: State::Idle,
            faulted: false,
        }
    }
    /// Admit one canonical request; no private identity is displayed yet.
    /// A second host request cannot replace an active local PIN entry.
    pub fn request(&mut self, request: Request, now: u64) -> Result<Frame, Error> {
        if self.faulted || !matches!(self.state, State::Idle) {
            return Err(Error::Faulted);
        }
        let expires = now.checked_add(60_000).ok_or(Error::Timeout)?;
        self.keys.discard_pending(now);
        self.state = State::Waiting { request, expires };
        Ok([*b"PIN required    ", *b"5=Start 0=Cancel"])
    }
    pub fn cancel(&mut self, now: u64) {
        // Dropping PinEntry zeroizes its digits; persistent debits are retained.
        self.state = State::Idle;
        self.keys.discard_pending(now);
    }
    pub fn fault(&mut self, now: u64) {
        self.cancel(now);
        self.faulted = true;
    }
    /// `clock` is a live protected monotonic timer; `pressed` comes ONLY from
    /// direct secure GPIO sampling in electrical bit order 1..9,0.
    pub fn poll(
        &mut self,
        pressed: u16,
        pin: &mut impl ProtectedPin<PIN_DIGITS>,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Update, Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        let now = clock();
        let event = match self.keys.sample(now, pressed) {
            Ok(event) => event,
            Err(_) => {
                self.fault(now);
                return Err(Error::Faulted);
            }
        };
        let state = core::mem::replace(&mut self.state, State::Idle);
        match state {
            State::Idle => Ok(Update::Quiet),
            State::Waiting { request, expires } => {
                if now >= expires {
                    self.cancel(now);
                    return Ok(Update::Cancelled);
                }
                match event {
                    Some((Digit::Zero, _)) => {
                        self.cancel(now);
                        Ok(Update::Cancelled)
                    }
                    Some((Digit::Five, Press::Short)) => {
                        // A NEW physical start release is required. The full
                        // guess is precharged on submit, before verification.
                        match PinEntry::begin(request, &mut clock) {
                            Ok(entry) => {
                                self.state = State::Entering(entry);
                                self.keys.discard_pending(clock());
                                Ok(Update::Frame(pin_progress(0)))
                            }
                            Err(_) => {
                                self.cancel(clock());
                                Ok(Update::Denied)
                            }
                        }
                    }
                    _ => {
                        self.state = State::Waiting { request, expires };
                        Ok(Update::Quiet)
                    }
                }
            }
            State::Entering(mut entry) => {
                if entry.tick(now).is_err() {
                    self.cancel(now);
                    return Ok(Update::Cancelled);
                }
                let Some((digit, press)) = event else {
                    self.state = State::Entering(entry);
                    return Ok(Update::Quiet);
                };
                if entry.press(digit, press, now).is_err() {
                    self.cancel(now);
                    return Ok(Update::Cancelled);
                }
                if entry.filled() == PIN_DIGITS {
                    let result = entry.submit(pin, &mut clock);
                    self.keys.discard_pending(clock());
                    return Ok(match result {
                        Ok(receipt) => Update::Verified(receipt),
                        Err(_) => Update::Denied,
                    });
                }
                let frame = pin_progress(entry.filled());
                self.state = State::Entering(entry);
                Ok(Update::Frame(frame))
            }
        }
    }
}
