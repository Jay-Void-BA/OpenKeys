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

// Shared ledger/attempt adapters for the protected runtime; this re-export
// does not commission/enable the SE or turn diagnostics into an authenticator.
#[cfg(target_arch = "arm")]
pub use openkeys_se_state as persistent_state;

#[cfg(target_arch = "arm")]
pub mod fingerprint_aead;

#[cfg(target_arch = "arm")]
pub mod fingerprint_link;
