//! One-shot, request-bound execution after local authorization and confirmation.
//! A use-by deadline authorizes only one exact operation, not blanket access.
use crate::{Authorization, Error, Request};

/// Trusted root state only: generation changes on cancel/disconnect/suspend/
/// lock, and check covers latched hardware/boot faults. Never implement from a
/// host packet Boolean or a host clock. Time is a live secure callback below.
pub trait State {
    fn generation(&self) -> u64;
    fn check_security_state(&mut self) -> Result<(), Error>;
}
/// Non-clonable and not constructible/deserializable by a host. The root owns
/// this object during the call, and every private provider uses check before
/// and after blocking work and before output/commit. No renewal API exists.
pub struct Execution {
    authorization: Authorization,
    started: u64,
    deadline: u64,
    last_time: u64,
    closed: bool,
}
/// Successful end of one checked execution. Not cloneable, serializable or
/// constructible by a host. Only a matching active session may consume it to
/// renew idle time; status/keepalive packets cannot produce this capability.
pub struct Completed {
    pub(crate) authorization: Authorization,
    pub(crate) finished_ms: u64,
}
impl Authorization {
    /// Consumes the original authority before its use-by deadline. Extends
    /// only this exact approved operation to its fixed device execution budget.
    pub fn activate(
        self,
        expected: &Request,
        state: &mut impl State,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Execution, Error> {
        if self.request() != expected {
            return Err(Error::WrongRequest);
        }
        if self.generation() == u64::MAX {
            return Err(Error::CounterExhausted);
        }
        let start = clock();
        if start < self.issued_ms() {
            return Err(Error::TimeWentBackwards);
        }
        if start >= self.expires_ms() {
            return Err(Error::Expired);
        }
        if state.generation() != self.generation() {
            return Err(Error::Rejected);
        }
        state
            .check_security_state()
            .map_err(|_| Error::HardwareFault)?;
        let now = clock();
        if now < start {
            return Err(Error::TimeWentBackwards);
        }
        if now >= self.expires_ms() {
            return Err(Error::Expired);
        }
        if state.generation() != self.generation() {
            return Err(Error::Rejected);
        }
        let deadline = start
            .checked_add(expected.operation.execution_budget_ms())
            .ok_or(Error::CounterExhausted)?;
        let deadline = self
            .execution_deadline
            .map_or(deadline, |fixed| deadline.min(fixed));
        if now >= deadline {
            return Err(Error::Expired);
        }
        Ok(Execution {
            authorization: self,
            started: start,
            deadline,
            last_time: now,
            closed: false,
        })
    }
}
impl Execution {
    pub fn finish(
        mut self,
        expected: &Request,
        state: &mut impl State,
        clock: impl FnMut() -> u64,
    ) -> Result<Completed, Error> {
        self.check(expected, state, clock)?;
        self.closed = true;
        Ok(Completed {
            authorization: self.authorization,
            finished_ms: self.last_time,
        })
    }
    pub fn authorization(&self) -> &Authorization {
        &self.authorization
    }
    pub fn request(&self) -> &Request {
        self.authorization.request()
    }
    pub fn started_ms(&self) -> u64 {
        self.started
    }
    pub fn expires_ms(&self) -> u64 {
        self.deadline
    }
    pub fn checked_at_ms(&self) -> u64 {
        self.last_time
    }
    /// Root continuation policy may only narrow the original deadline. A new
    /// local proof cannot extend a previous multi-response transaction.
    pub fn restrict_deadline(mut self, until: u64) -> Result<Self, Error> {
        self.deadline = self.deadline.min(until);
        if self.deadline <= self.last_time {
            return Err(Error::Expired);
        }
        Ok(self)
    }
    pub fn cancel(&mut self) {
        self.closed = true;
    }
    fn time(&mut self, now: u64) -> Result<(), Error> {
        if now < self.last_time {
            return Err(Error::TimeWentBackwards);
        }
        if now >= self.deadline {
            return Err(Error::Expired);
        }
        self.last_time = now;
        Ok(())
    }
    pub fn check(
        &mut self,
        expected: &Request,
        state: &mut impl State,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(), Error> {
        if self.closed {
            return Err(Error::Locked);
        }
        let result = (|| {
            self.time(clock())?;
            if self.request() != expected {
                return Err(Error::WrongRequest);
            }
            if self.authorization.generation() == u64::MAX {
                return Err(Error::CounterExhausted);
            }
            if state.generation() != self.authorization.generation() {
                return Err(Error::Rejected);
            }
            state
                .check_security_state()
                .map_err(|_| Error::HardwareFault)?;
            self.time(clock())?;
            if state.generation() != self.authorization.generation() {
                return Err(Error::Rejected);
            }
            Ok(())
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Operation;
    use core::cell::Cell;
    struct Live<'a> {
        generation: Cell<u64>,
        clock: &'a Cell<u64>,
        delay: u64,
        fault: bool,
        revoke: bool,
    }
    impl State for Live<'_> {
        fn generation(&self) -> u64 {
            self.generation.get()
        }
        fn check_security_state(&mut self) -> Result<(), Error> {
            self.clock.set(self.clock.get() + self.delay);
            if self.revoke {
                self.generation.set(self.generation.get() + 1);
            }
            if self.fault {
                Err(Error::HardwareFault)
            } else {
                Ok(())
            }
        }
    }
    fn authorization(operation: Operation) -> Authorization {
        Authorization {
            request: Request {
                operation,
                digest: [1; 32],
                channel: [2; 32],
            },
            issued: 10,
            expires: 20,
            generation: 3,
            execution_deadline: None,
        }
    }
    fn live(clock: &Cell<u64>) -> Live<'_> {
        Live {
            generation: Cell::new(3),
            clock,
            delay: 0,
            fault: false,
            revoke: false,
        }
    }
    #[test]
    fn slow_exact_import_can_finish_after_capture_use_by_but_cannot_renew_its_execution() {
        let clock = Cell::new(11);
        let mut state = live(&clock);
        let a = authorization(Operation::Import);
        let request = *a.request();
        let mut e = a.activate(&request, &mut state, || clock.get()).unwrap();
        assert_eq!(e.expires_ms(), 300_011);
        for t in [21, 15_020, 100_000, 300_010] {
            clock.set(t);
            e.check(&request, &mut state, || clock.get()).unwrap();
            assert_eq!(e.expires_ms(), 300_011);
        }
        clock.set(300_011);
        assert_eq!(
            e.check(&request, &mut state, || clock.get()),
            Err(Error::Expired)
        );
        clock.set(300_010);
        assert_eq!(
            e.check(&request, &mut state, || clock.get()),
            Err(Error::Locked)
        );
    }
    #[test]
    fn expired_wrong_request_channel_generation_or_late_start_never_activates() {
        for mode in 0..6 {
            let clock = Cell::new(11);
            let mut state = live(&clock);
            let a = authorization(Operation::Import);
            let mut request = *a.request();
            match mode {
                0 => clock.set(20),
                1 => clock.set(9),
                2 => request.digest[0] ^= 1,
                3 => request.channel[0] ^= 1,
                4 => state.generation.set(4),
                _ => state.delay = 9,
            };
            assert!(a.activate(&request, &mut state, || clock.get()).is_err());
        }
    }
    #[test]
    fn every_live_revoke_fault_clock_rollback_and_cancel_is_terminal() {
        for mode in 0..6 {
            let clock = Cell::new(11);
            let mut state = live(&clock);
            let a = authorization(Operation::FidoSign);
            let request = *a.request();
            let mut e = a.activate(&request, &mut state, || clock.get()).unwrap();
            assert_eq!(e.expires_ms(), 15_011);
            match mode {
                0 => state.generation.set(4),
                1 => state.fault = true,
                2 => clock.set(10),
                3 => state.revoke = true,
                4 => state.delay = 15_000,
                _ => e.cancel(),
            };
            assert!(e.check(&request, &mut state, || clock.get()).is_err());
            state.generation.set(3);
            state.fault = false;
            state.delay = 0;
            state.revoke = false;
            clock.set(12);
            assert_eq!(
                e.check(&request, &mut state, || clock.get()),
                Err(Error::Locked)
            );
        }
    }
    #[test]
    fn a_different_private_operation_or_object_cannot_use_a_long_import_scope() {
        let clock = Cell::new(11);
        let mut state = live(&clock);
        let a = authorization(Operation::Import);
        let request = *a.request();
        let mut e = a.activate(&request, &mut state, || clock.get()).unwrap();
        let mut wrong = request;
        wrong.operation = Operation::RevealRecoveryCode;
        assert_eq!(
            e.check(&wrong, &mut state, || clock.get()),
            Err(Error::WrongRequest)
        );
        assert_eq!(
            e.check(&request, &mut state, || clock.get()),
            Err(Error::Locked)
        );
    }
    #[test]
    fn continuation_deadline_can_only_shorten_even_with_a_fresh_authority() {
        let clock = Cell::new(11);
        let mut state = live(&clock);
        let a = authorization(Operation::FidoSign);
        let request = *a.request();
        let e = a
            .activate(&request, &mut state, || clock.get())
            .unwrap()
            .restrict_deadline(u64::MAX)
            .unwrap();
        assert_eq!(e.expires_ms(), 15_011);
        let mut e = e.restrict_deadline(14).unwrap();
        clock.set(14);
        assert_eq!(
            e.check(&request, &mut state, || clock.get()),
            Err(Error::Expired)
        );
    }
    #[test]
    fn terminal_generation_is_never_usable_even_if_a_provider_reports_the_same_value() {
        let clock = Cell::new(11);
        let mut state = live(&clock);
        state.generation.set(u64::MAX);
        let mut a = authorization(Operation::Import);
        a.generation = u64::MAX;
        let request = *a.request();
        assert!(matches!(
            a.activate(&request, &mut state, || clock.get()),
            Err(Error::CounterExhausted)
        ));
    }
}
