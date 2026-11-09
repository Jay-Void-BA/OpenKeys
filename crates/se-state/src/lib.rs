#![no_std]
#![forbid(unsafe_code)]
//! Actual SE050 object APDUs, durable retry/tag budgets and two-head vault
//! ledger. Production I/O is sealed to the authenticated SCP03 client. No
//! object creation/deletion, factory/default keys or RAM/NOR counter fallback.
//! The chip's atomic/monotonic/persistent behavior and commissioning ACLs must
//! still be qualified on the exact E2 personality; model tests do not do so.

pub mod keys;
mod objects;
mod record;
mod state;
pub use objects::{Exchange, ScpSession};
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
}

mod private {
    pub trait Sealed {}
}

#[cfg(test)]
mod tests;
