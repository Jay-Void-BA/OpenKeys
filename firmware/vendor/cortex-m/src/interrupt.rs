//! Interrupts

pub use critical_section::{CriticalSection, Mutex};
#[cfg(cortex_m)]
use core::arch::asm;
#[cfg(cortex_m)]
use core::sync::atomic::{Ordering, compiler_fence};
use cortex_m_macros::asm_cfg;

/// Trait for enums of external interrupt numbers.
///
/// This trait should be implemented by a peripheral access crate (PAC)
/// on its enum of available external interrupts for a specific device.
/// Each variant must convert to a u16 of its interrupt number,
/// which is its exception number - 16.
///
/// # Safety
///
/// This trait must only be implemented on enums of device interrupts. Each
/// enum variant must represent a distinct value (no duplicates are permitted),
/// and must always return the same value (do not change at runtime).
///
/// These requirements ensure safe nesting of critical sections.
pub unsafe trait InterruptNumber: Copy {
    /// Return the interrupt number associated with this variant.
    ///
    /// See trait documentation for safety requirements.
    fn number(self) -> u16;
}

/// Disables all interrupts
///
/// Forms a read/write memory barrier for the compiler.
#[inline]
#[asm_cfg(cortex_m)]
pub fn disable() {
    unsafe { asm!("cpsid i", options(nostack, preserves_flags)) };

    // Ensure no subsequent memory accesses are reordered to before interrupts are disabled.
    compiler_fence(Ordering::SeqCst);
}

/// Enables all the interrupts
///
/// Forms a read/write memory barrier for the compiler.
///
/// # Safety
///
/// - Do not call this function inside an `interrupt::free` critical section
#[inline]
#[asm_cfg(cortex_m)]
pub unsafe fn enable() {
    // Ensure no preceeding memory accesses are reordered to after interrupts are enabled.
    compiler_fence(Ordering::SeqCst);

    unsafe { asm!("cpsie i", options(nostack, preserves_flags)) };
}

/// Execute closure `f` in an interrupt-free context.
///
/// This as also known as a "critical section".
#[inline]
pub fn free<F, R>(f: F) -> R
where
    F: FnOnce(CriticalSection<'_>) -> R,
{
    // OpenKeys patch: use the maintained lifetime-scoped critical-section
    // implementation. The selected single-core backend retains the upstream
    // PRIMASK save/restore and compiler-fence implementation unchanged.
    critical_section::with(f)
}
