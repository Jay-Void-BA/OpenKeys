#![no_std]
#![forbid(unsafe_code)]
//! Protected CTAP core; physical transports and the hardware Env are composed
//! by the verified root. No host Boolean or software unlock implementation.
extern crate alloc;
pub use opensk;

mod limits;
mod processor;
mod request;
pub use processor::{Platform, Processor};
#[cfg(test)]
mod tests;
pub use request::{Prepared, Review, prepare_ctap2, prepare_u2f};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Input,
    Unsupported,
    Authorization,
    State,
    Time,
    Limit,
}
