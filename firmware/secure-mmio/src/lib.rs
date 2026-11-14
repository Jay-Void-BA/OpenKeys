#![no_std]
#![deny(unsafe_code)]
//! Small explicitly reviewed MMIO boundary for H573 secure aliases and the
//! RNG_NSCR missing from metapac21.0.0. See README for the twelve fixed-address
//! constructors and two-sector journal word boundary. No unsafe Rust in the
//! protocol/policy/crypto/persistence state machines.
#[cfg(target_arch = "arm")]
mod ports;
#[cfg(target_arch = "arm")]
mod root_flash;
#[cfg(target_arch = "arm")]
pub use ports::{RngPort, SaesPort, SecureSeI2c, initialize, initialize_se_i2c};
#[cfg(target_arch = "arm")]
pub use root_flash::{RootFlashPort, initialize_root_flash};
