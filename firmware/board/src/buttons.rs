//! Debounced physical releases, never host-injected button events. Discard
//! chords, held keys at a context change, clock rollback and polling gaps.
use device_policy::pin::{Button, Press};

const DEBOUNCE_MS: u64 = 25;
const LONG_MS: u64 = 800;
const MAX_POLL_GAP_MS: u64 = 100;
const MAX_HOLD_MS: u64 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Clock,
    InvalidSample,
}

pub struct Buttons {
    last_time: u64,
    candidate: u8,
    candidate_since: u64,
    stable: u8,
    pressed_at: Option<u64>,
    blocked: bool,
    faulted: bool,
}

impl Buttons {
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
    /// Invoke before fingerprint/PIN/confirmation, including after cancellation.
    pub fn discard_pending(&mut self, now: u64) {
        if now < self.last_time {
            self.faulted = true;
        }
        self.last_time = now;
        self.candidate_since = now;
        self.pressed_at = None;
        self.blocked = true;
    }

    /// `pressed` is the active-low GPIO sample converted into bits0..3.
    /// Poll at least every100ms using a live protected monotonic clock.
    /// Only a stable release can emit exactly one short OR long event.
    pub fn sample(&mut self, now: u64, pressed: u8) -> Result<Option<(Button, Press)>, Error> {
        if self.faulted || now < self.last_time {
            self.faulted = true;
            self.pressed_at = None;
            return Err(Error::Clock);
        }
        if pressed & !0x0f != 0 {
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
                let button = match previous {
                    1 => Button::One,
                    2 => Button::Two,
                    4 => Button::Three,
                    8 => Button::Four,
                    _ => return Ok(None),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounce_has_one_release_and_no_autorepeat() {
        let mut b = Buttons::new(0);
        for (time, mask) in [(25, 0), (30, 1), (35, 0), (40, 1), (65, 1), (70, 0)] {
            assert!(b.sample(time, mask).unwrap().is_none());
        }
        assert!(matches!(
            b.sample(95, 0),
            Ok(Some((Button::One, Press::Short)))
        ));
        assert!(b.sample(120, 0).unwrap().is_none());
    }

    #[test]
    fn hold_emits_long_only_on_release() {
        let mut b = Buttons::new(0);
        b.sample(25, 0).unwrap();
        b.sample(30, 8).unwrap();
        for t in (55..=855).step_by(25) {
            assert!(b.sample(t, 8).unwrap().is_none());
        }
        b.sample(880, 0).unwrap();
        assert!(matches!(
            b.sample(905, 0),
            Ok(Some((Button::Four, Press::Long)))
        ));
        assert!(b.sample(930, 0).unwrap().is_none());
    }

    #[test]
    fn chords_context_changes_and_polling_gaps_cannot_confirm() {
        let mut b = Buttons::new(0);
        for (t, mask) in [
            (25, 0),
            (30, 4),
            (55, 4),
            (60, 5),
            (85, 5),
            (90, 0),
            (115, 0),
        ] {
            assert!(b.sample(t, mask).unwrap().is_none());
        }
        b.sample(120, 4).unwrap();
        b.sample(145, 4).unwrap();
        b.discard_pending(150);
        assert!(b.sample(155, 0).unwrap().is_none());
        assert!(b.sample(180, 0).unwrap().is_none());
        b.sample(185, 4).unwrap();
        b.sample(210, 4).unwrap();
        assert!(b.sample(500, 0).unwrap().is_none());
        assert!(b.sample(525, 0).unwrap().is_none());
        assert!(matches!(b.sample(524, 0), Err(Error::Clock)));
        assert!(matches!(b.sample(526, 0), Err(Error::Clock)));
    }
}
