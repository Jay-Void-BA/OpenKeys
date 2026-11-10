//! Debounced physical releases, never host-injected button events. Discard
//! Ten dedicated GPIOs, no matrix. Discard chords, held keys at a context change, clock rollback and polling gaps.
use device_policy::{pin::Press, pin_only::Digit};

const DEBOUNCE_MS: u64 = 25;
const LONG_MS: u64 = 800;
const MAX_POLL_GAP_MS: u64 = 100;
const MAX_HOLD_MS: u64 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Clock,
    InvalidSample,
}

pub struct Keypad {
    last_time: u64,
    candidate: u16,
    candidate_since: u64,
    stable: u16,
    pressed_at: Option<u64>,
    blocked: bool,
    faulted: bool,
}

impl Keypad {
    pub const fn new(now: u64) -> Self {
        Self {
            last_time: now,
            candidate: 0,
            candidate_since: now,
            stable: 0,
            pressed_at: None,
            blocked: true,
            faulted: false,
        }
    }

    /// Require all keys to be released before accepting input in a new scene.
    /// Invoke before PIN/confirmation, including after cancellation.
    pub fn discard_pending(&mut self, now: u64) {
        if now < self.last_time {
            self.faulted = true;
        }
        self.last_time = now;
        self.candidate_since = now;
        self.pressed_at = None;
        self.blocked = true;
    }

    /// `pressed` is the active-low GPIO sample converted into bits0..9.
    /// Poll at least every100ms using a live protected monotonic clock.
    /// Only a stable release can emit exactly one short OR long event.
    pub fn sample(&mut self, now: u64, pressed: u16) -> Result<Option<(Digit, Press)>, Error> {
        if self.faulted || now < self.last_time {
            self.faulted = true;
            self.pressed_at = None;
            return Err(Error::Clock);
        }
        if pressed & !0x03ff != 0 {
            self.faulted = true;
            self.pressed_at = None;
            return Err(Error::InvalidSample);
        }
        if now - self.last_time > MAX_POLL_GAP_MS {
            self.discard_pending(now);
        }
        self.last_time = now;
        if pressed.count_ones() > 1 {
            self.blocked = true;
            self.pressed_at = None;
        }
        if pressed != self.candidate {
            self.candidate = pressed;
            self.candidate_since = now;
        }
        if now - self.candidate_since < DEBOUNCE_MS {
            return Ok(None);
        }
        if self.blocked {
            if pressed == 0 {
                self.blocked = false;
                self.stable = 0;
                self.pressed_at = None;
            }
            return Ok(None);
        }
        if self
            .pressed_at
            .is_some_and(|start| now - start > MAX_HOLD_MS)
        {
            self.blocked = true;
            self.pressed_at = None;
            return Ok(None);
        }
        if self.stable == pressed {
            return Ok(None);
        }
        let previous = self.stable;
        self.stable = pressed;
        if previous == 0 && pressed.count_ones() == 1 {
            self.pressed_at = Some(self.candidate_since);
            return Ok(None);
        }
        if pressed == 0 {
            if let Some(start) = self.pressed_at.take() {
                let Some(button) = Digit::from_gpio_index(previous.trailing_zeros()) else {
                    return Ok(None);
                };
                let kind = if self.candidate_since - start >= LONG_MS {
                    Press::Long
                } else {
                    Press::Short
                };
                return Ok(Some((button, kind)));
            }
        } else {
            // Rolling directly from one key to another is not a fresh press.
            self.blocked = true;
            self.pressed_at = None;
        }
        Ok(None)
    }
}
