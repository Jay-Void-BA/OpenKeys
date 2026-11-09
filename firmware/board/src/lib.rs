#![no_std]
#![forbid(unsafe_code)]
//! Drivers for the frozen R04 electrical PCB. These must execute only within
//! the protected device domain; they do not authenticate a host or a finger.

pub mod buttons;
pub mod flash;
pub mod lcd;
pub mod pins;
pub mod storage;

#[cfg(test)]
extern crate std;

/// A hardware error latches the affected driver. Never turn a failed I/O into
/// an authorization success or expose peripheral command APIs to a host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Io,
    Faulted,
    InvalidInput,
    WrongPart,
    Timeout,
    Verify,
}
