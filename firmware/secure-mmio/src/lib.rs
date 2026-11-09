#![no_std]
#![deny(unsafe_code)]
//! Small explicitly reviewed MMIO boundary for H573 secure aliases and the
//! RNG_NSCR missing from metapac21.0.0. See README for the six fixed-address
//! constructors. No unsafe Rust in the protocol/policy/crypto state machines.
#[cfg(target_arch = "arm")]
mod ports;
#[cfg(target_arch = "arm")]
pub use ports::{RngPort, SaesPort, SecureSeI2c, initialize, initialize_se_i2c};
