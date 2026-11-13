#![no_std]
#![forbid(unsafe_code)]

//! Actual SE050 object APDUs, durable retry/tag budgets and two-head vault
//! ledger. Production I/O is sealed to authenticated SCP03; private-key/OTP
//! providers additionally require a native PIN session. No factory/default
//! keys or RAM/NOR counter fallback. Typed key/OTP provisioning is separate
//! from persistent-state open, which never creates or repairs missing objects.
//! The chip's atomic/monotonic/persistent behavior and commissioning ACLs must
//! still be qualified on the selected personality (R05 C2/Applet7.2); model
//! tests do not qualify a real device.

pub mod keys;
pub mod native_pin;
#[cfg(feature = "native-uid-qualification")]
pub mod native_uid;
mod objects;
pub mod otp;
pub mod pin_only;
mod record;
mod state;
pub use objects::{Exchange, PinBoundExchange, ScpSession};
pub use state::{BIO_MAX_FAILURES, Service, State, StateRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Closed,
    Channel,
    Schema,
    Counter,
    Exhausted,
    Record,
    Authorization,
    Borrow,
    EraseRequired,
}

mod private {
    pub trait Sealed {}
    pub trait PinBound {}
}

#[cfg(test)]
mod tests;
