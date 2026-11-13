//! Qualification of actual H573 readbacks before using its non-exportable
//! DHUK. This is a register contract, not evidence of an authenticated image,
//! a working sensor or complete Root/memory attribution.
use crate::Error;

const INTERNAL_TAMPERS: u32 = 0x5dff;
const EXTERNAL_TAMPERS: u32 = 0b101; // R05 PC13/IN1 case, PC1/IN3 fault.
const CRYPTO_ATTRIBUTES: u32 = (1 << 18) | (1 << 19); // RNG, SAES.

/// A fresh privileged Secure read of these registers is mandatory. Production
/// ports must never construct this from a host response, cached boot flags or
/// an unauthenticated option-byte manifest. No register is written here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub flash_options: u32,
    pub flash_options2: u32,
    pub flash_secure_boot: u32,
    pub flash_secure_epoch: u32,
    pub flash_privilege: u32,
    pub hdpl: u32,
    pub next_hdpl: u32,
    pub epoch_select: u32,
    pub sbs_security: u32,
    pub memory_erase_status: u32,
    pub tamper_control: [u32; 3],
    pub tamper_security: u32,
    pub tamper_privilege: u32,
    pub tamper_filter: u32,
    pub tamper_active: u32,
    pub tamper_status: u32,
    pub crypto_security: u32,
    pub crypto_privilege: u32,
    pub unprivileged: bool,
    pub debugger_attached: bool,
}

/// Qualified, stable derivation context. Kept for the peripheral lifetime;
/// even a change to another individually valid epoch/lifecycle is terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceKeyContext(Snapshot);

impl Snapshot {
    pub fn qualify(self) -> Result<DeviceKeyContext, Error> {
        let product = (self.flash_options >> 8) & 0xff;
        let boot = (self.flash_options >> 22) & 0xff;
        let external = self.tamper_control[0] & 0xff;
        let internal = (self.tamper_control[0] >> 16) & INTERNAL_TAMPERS;
        if !matches!(product, 0x72 | 0x5c) // CLOSED or LOCKED, never OPEN.
            || boot != 0xc3 // STiRoT, not an arbitrary OEM/reset entry.
            || self.flash_options2 >> 24 != 0xb4 // TZEN.
            // All SRAM cleared on reset, SRAM2/3 and backup ECC enabled.
            || self.flash_options2 & 0x7c != 0
            || self.flash_secure_boot & 0xff != 0xb4 // SECBOOT_LOCK.
            || self.flash_secure_epoch & !0x00ff_ffff != 0
            || self.flash_privilege & 1 == 0
            // UM3125 Rev2 pp19/23: authenticated secure-image jump at HDPL2.
            // This Root stays there; a HDPL3/NS handoff is not this profile.
            || self.hdpl != 0x8a
            || self.next_hdpl != 0
            || self.epoch_select != 1 // SEC_EPOCH; never forced-zero/NS.
            || self.sbs_security & 0xb != 0xb
            // Boot must preserve MCLR; this module never clears it/flags.
            || self.memory_erase_status & 1 == 0
            || external & EXTERNAL_TAMPERS != EXTERNAL_TAMPERS
            || internal != INTERNAL_TAMPERS
            // Immediate hardware erasure, not potential/no-erase or masked
            // mode. BKBLOCK/BKERASE deny even if other readbacks look valid.
            || self.tamper_control[1] & (external | (EXTERNAL_TAMPERS << 16) | (3 << 22)) != 0
            || self.tamper_control[2] & internal != 0
            || self.tamper_security & (1 << 31) == 0
            || self.tamper_privilege & (1 << 31) == 0
            // These two PCB inputs are passive, not active-output loops.
            || self.tamper_active & EXTERNAL_TAMPERS != 0
            || self.tamper_status != 0
            || self.crypto_security & CRYPTO_ATTRIBUTES != CRYPTO_ATTRIBUTES
            || self.crypto_privilege & CRYPTO_ATTRIBUTES != CRYPTO_ATTRIBUTES
            || self.unprivileged
            || self.debugger_attached
        {
            return Err(Error::Hardware);
        }
        Ok(DeviceKeyContext(self))
    }
}

#[cfg(test)]
pub(crate) fn fixture() -> Snapshot {
    Snapshot {
        flash_options: (0xc3 << 22) | (0x72 << 8),
        flash_options2: 0xb4 << 24,
        flash_secure_boot: 0xb4,
        flash_secure_epoch: 0,
        flash_privilege: 1,
        hdpl: 0x8a,
        next_hdpl: 0,
        epoch_select: 1,
        sbs_security: 0xb,
        memory_erase_status: 1,
        tamper_control: [(INTERNAL_TAMPERS << 16) | EXTERNAL_TAMPERS, 0, 0],
        tamper_security: 1 << 31,
        tamper_privilege: 1 << 31,
        tamper_filter: 0,
        tamper_active: 0,
        tamper_status: 0,
        crypto_security: CRYPTO_ATTRIBUTES,
        crypto_privilege: CRYPTO_ATTRIBUTES,
        unprivileged: false,
        debugger_attached: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_open_debug_dummy_epoch_unerased_memory_and_weakened_protection() {
        let base = fixture();
        assert!(base.qualify().is_ok());
        let mut locked = base;
        locked.flash_options ^= (0x72 ^ 0x5c) << 8;
        assert!(locked.qualify().is_ok());
        let mutations: &[fn(&mut Snapshot)] = &[
            |s| s.flash_options &= !(0xff << 8),
            |s| s.flash_options ^= (0xc3 ^ 0xb4) << 22,
            |s| s.flash_options2 ^= (0xb4 ^ 0xc3) << 24,
            |s| s.flash_secure_boot = 0xc3,
            |s| s.flash_secure_epoch = 1 << 24,
            |s| s.flash_privilege = 0,
            |s| s.hdpl = 0x6f,
            |s| s.next_hdpl = 1,
            |s| s.epoch_select = 0,
            |s| s.epoch_select = 2,
            |s| s.epoch_select = 3,
            |s| s.sbs_security = 0,
            |s| s.memory_erase_status = 0,
            |s| s.tamper_security = 0,
            |s| s.tamper_privilege = 0,
            |s| s.tamper_status = 1,
            |s| s.tamper_status = 1 << 24,
            |s| s.tamper_active = 1,
            |s| s.tamper_active = 4,
            |s| s.crypto_security = 0,
            |s| s.crypto_privilege = 0,
            |s| s.unprivileged = true,
            |s| s.debugger_attached = true,
        ];
        for mutation in mutations {
            let mut s = base;
            mutation(&mut s);
            assert_eq!(s.qualify(), Err(Error::Hardware));
        }
        for bit in 2..=6 {
            let mut s = base;
            s.flash_options2 |= 1 << bit;
            assert_eq!(s.qualify(), Err(Error::Hardware));
        }
        for bit in 0..32 {
            if base.tamper_control[0] & (1 << bit) != 0 {
                let mut s = base;
                s.tamper_control[0] &= !(1 << bit);
                assert_eq!(s.qualify(), Err(Error::Hardware));
                let mut s = base;
                s.tamper_control[if bit < 16 { 1 } else { 2 }] |= 1 << (bit % 16);
                assert_eq!(s.qualify(), Err(Error::Hardware));
            }
        }
        for bit in [16, 18, 22, 23] {
            let mut s = base;
            s.tamper_control[1] |= 1 << bit;
            assert_eq!(s.qualify(), Err(Error::Hardware));
        }
    }
}
