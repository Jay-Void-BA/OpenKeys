//! Host checks of the maintained token/Mutex API used by the Cortex-M patch.
//! The hardware PRIMASK backend still needs target testing; these checks do
//! not simulate interrupt timing, voltage faults or TrustZone attribution.
use core::cell::Cell;
use cortex_m::interrupt::{self, Mutex};

#[test]
fn nested_scopes_and_thread_exclusion_use_the_maintained_backend() {
    static VALUE: Mutex<Cell<u32>> = Mutex::new(Cell::new(0));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                for _ in 0..1000 {
                    interrupt::free(|token| {
                        let value = VALUE.borrow(token).get();
                        interrupt::free(|nested| VALUE.borrow(nested).set(value + 1));
                    });
                }
            });
        }
    });
    interrupt::free(|token| assert_eq!(VALUE.borrow(token).get(), 2000));
}

#[test]
fn unwinding_releases_the_host_critical_section() {
    assert!(std::panic::catch_unwind(|| interrupt::free(|_| panic!("test only"))).is_err());
    interrupt::free(|token| {
        let value = Mutex::new(Cell::new(42));
        assert_eq!(value.borrow(token).get(), 42);
    });
}
