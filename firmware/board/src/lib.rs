#![no_std]
#![forbid(unsafe_code)]
//! Shared drivers, plus explicit R04 legacy and R05 PIN/LCD modules.
//! Physical pin contracts are version-specific. These execute only within
//! the protected device domain; they do not authenticate a host or a finger.

pub mod buttons;
pub mod dfr0555;
pub mod fingerprint;
pub mod flash;
pub mod lcd;
pub mod otp;
pub mod pin_ui;
pub mod pins;
pub mod se_i2c;
pub mod se_io;
pub mod storage;
pub mod ten_keys;

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
