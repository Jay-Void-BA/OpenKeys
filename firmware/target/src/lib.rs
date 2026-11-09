#![no_std]
#![forbid(unsafe_code)]
//! Hardware adapters for the R04 protected runtime. A compiled adapter is not
//! proof that secure boot, TrustZone attribution or provisioning is deployed.

#[cfg(target_arch = "arm")]
pub mod vault_aead;

#[cfg(target_arch = "arm")]
pub mod se_link;

#[cfg(target_arch = "arm")]
pub mod scp_crypto;
