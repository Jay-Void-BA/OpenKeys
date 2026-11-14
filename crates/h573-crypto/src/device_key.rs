//! Qualification of actual H573 readbacks before using its non-exportable
//! DHUK. This is a register contract, not evidence of an authenticated image,
//! a working sensor or complete Root/memory attribution.
use crate::Error;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

const INTERNAL_TAMPERS: u32 = 0x5dff;
const EXTERNAL_TAMPERS: u32 = 1; // R05 PC13/IN1 active return, PA1/OUT4.
const CRYPTO_ATTRIBUTES: u32 = (1 << 18) | (1 << 19); // RNG, SAES.
pub const ACTIVE_TAMP_CONFIG: u32 = 0xc003_0301;
pub const ACTIVE_TAMP_OUTPUTS: u32 = 0x300;
pub const BACKUP_ANCHOR_FIRST: usize = 8; // Never use BHK registers0..7.
pub const BACKUP_ANCHOR_WORDS: usize = 18;
pub const BACKUP_ANCHOR_TAG: u32 = u32::from_le_bytes(*b"OKB1");

/// Public continuity metadata, not a private key or factory-blank proof.
/// Encoding does not write hardware. Only the verified, destructive factory /
/// owner-epoch transition may write these words; ordinary boot never does.
pub fn encode_backup_owner(owner: &[u8; 32]) -> Result<[u32; BACKUP_ANCHOR_WORDS], Error> {
    if *owner == [0; 32] || *owner == [0xff; 32] {
        return Err(Error::Input);
    }
    let mut words = [0; BACKUP_ANCHOR_WORDS];
    for index in 0..8 {
        words[index] = u32::from_le_bytes(owner[index * 4..index * 4 + 4].try_into().unwrap());
        words[index + 8] = !words[index];
    }
    words[16] = BACKUP_ANCHOR_TAG;
    words[17] = !BACKUP_ANCHOR_TAG;
    Ok(words)
}

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
    pub tamper_active_outputs: u32,
    pub tamper_active_state: u32,
    pub tamper_options: u32,
    pub tamper_resources: u32,
    pub backup_anchor: [u32; BACKUP_ANCHOR_WORDS],
    pub backup_power: u32,
    pub backup_clock: u32,
    pub power_security: u32,
    pub power_privilege: u32,
    pub clock_security: u32,
    pub clock_privilege: u32,
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

impl DeviceKeyContext {
    /// The Root must compare its AUTHENTICATED selected journal owner here,
    /// then check the independent immutable SE owner epoch. The anchor alone
    /// is neither a MAC nor permission to recreate a missing journal.
    pub fn verify_backup_owner(&self, owner: &[u8; 32]) -> Result<(), Error> {
        let mut observed = Zeroizing::new([0; 32]);
        for index in 0..8 {
            observed[index * 4..index * 4 + 4]
                .copy_from_slice(&self.0.backup_anchor[index].to_le_bytes());
        }
        if bool::from(observed.as_ref().ct_eq(owner)) {
            Ok(())
        } else {
            Err(Error::Hardware)
        }
    }
}

impl Snapshot {
    pub fn qualify(mut self) -> Result<DeviceKeyContext, Error> {
        // PRNG output changes legitimately; it is not an immutable key
        // context. INITS and SEEDF MUST still qualify. LSIRDY is also a live
        // status bit (IWDG can start LSI); configuration bits remain pinned.
        self.tamper_active_state &= !0xff;
        self.backup_clock &= !(1 << 27);
        let product = (self.flash_options >> 8) & 0xff;
        let boot = (self.flash_options >> 22) & 0xff;
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
            || self.tamper_control[0] != ((INTERNAL_TAMPERS << 16) | EXTERNAL_TAMPERS)
            // Immediate hardware erasure, not potential/no-erase or masked
            // mode. BKBLOCK/BKERASE deny even if other readbacks look valid.
            || self.tamper_control[1] != 0
            || self.tamper_control[2] != 0
            // All32 backup registers Secure read/write and privileged. BHK
            // lock0..7 is independent; this anchor occupies8..25 only.
            || self.tamper_security & 0x80ff_80ff != 0x8020_8020
            || self.tamper_privilege & 0xe000_8000 != 0xe000_8000
            || self.tamper_filter != 0x80 // No unused passive pull-ups.
            || self.tamper_active != ACTIVE_TAMP_CONFIG
            || self.tamper_active_outputs != ACTIVE_TAMP_OUTPUTS
            || self.tamper_active_state != 1 << 15 // Initialized, not seeding.
            || self.tamper_options != 0 // Default PC13/PA1 mappings.
            || self.tamper_resources != 1 // Include backup SRAM erasure too.
            // MONEN=1; BREN=0, VBE=0, VBRS=0. Primary must NEVER charge.
            || self.backup_power != 2
            || self.backup_clock & 0x8323 != 0x8123 // LSE ready, CSS, RTC=LSE.
            || self.backup_clock & 0x18 != 8 // Qualified budget uses medium-low drive.
            || self.backup_clock & ((1 << 24) | (1 << 16) | 0xc4) != 0
            || self.power_security & 0xf8ff != 0xf8ff
            || self.power_privilege & 3 != 3
            || self.clock_security & 0x3bff != 0x3bff
            || self.clock_privilege & 3 != 3
            || self.backup_anchor[16] != BACKUP_ANCHOR_TAG
            || self.backup_anchor[17] != !BACKUP_ANCHOR_TAG
            || self.backup_anchor[..8] == [0; 8]
            || self.backup_anchor[..8] == [u32::MAX; 8]
            || (0..8).any(|index| self.backup_anchor[index + 8] != !self.backup_anchor[index])
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
        tamper_security: 0x8020_8020,
        tamper_privilege: 0xe000_8000,
        tamper_filter: 0x80,
        tamper_active: ACTIVE_TAMP_CONFIG,
        tamper_active_outputs: ACTIVE_TAMP_OUTPUTS,
        tamper_active_state: 1 << 15,
        tamper_options: 0,
        tamper_resources: 1,
        backup_anchor: encode_backup_owner(&[0x46; 32]).unwrap(),
        backup_power: 2,
        backup_clock: 0x812b,
        power_security: 0xf8ff,
        power_privilege: 3,
        clock_security: 0x3bff,
        clock_privilege: 3,
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
    fn backup_loss_corruption_old_passive_profile_and_weak_monitoring_deny() {
        let base = fixture();
        for index in 0..BACKUP_ANCHOR_WORDS {
            for bit in 0..32 {
                let mut snapshot = base;
                snapshot.backup_anchor[index] ^= 1 << bit;
                assert_eq!(snapshot.qualify(), Err(Error::Hardware));
            }
        }
        let mut lost = base;
        lost.backup_anchor.fill(0);
        assert_eq!(lost.qualify(), Err(Error::Hardware));
        let mut passive = base;
        passive.tamper_control[0] = 0x5dff_0005;
        passive.tamper_active = 0;
        assert_eq!(passive.qualify(), Err(Error::Hardware));
        let changes: &[fn(&mut Snapshot)] = &[
            |s| s.backup_power = 0,
            |s| s.backup_power |= 1, // Backup SRAM accidentally retained.
            |s| s.backup_power |= 1 << 8, // Primary charging enabled.
            |s| s.backup_clock &= !2,
            |s| s.backup_clock &= !(1 << 5),
            |s| s.backup_clock ^= 3 << 8, // RTC switched to LSI.
            |s| s.backup_clock |= 1 << 6,
            |s| s.backup_clock |= 1 << 16,
            |s| s.tamper_security = 1 << 31, // NS backup registers.
            |s| s.tamper_security ^= 1,      // 33 is not a valid zone offset.
            |s| s.tamper_privilege = 1 << 31,
            |s| s.tamper_filter = 0,
            |s| s.tamper_active_outputs = 1 << 8, // OUT2, not PA1/OUT4.
            |s| s.tamper_active_state |= 1 << 14,
            |s| s.tamper_active_state = 0,
            |s| s.tamper_options = 1,
            |s| s.tamper_resources = 0,
            |s| s.power_security &= !(1 << 14),
            |s| s.power_privilege = 0,
            |s| s.clock_security &= !(1 << 4),
            |s| s.clock_privilege = 0,
        ];
        for change in changes {
            let mut snapshot = base;
            change(&mut snapshot);
            assert_eq!(snapshot.qualify(), Err(Error::Hardware));
        }
        assert_eq!(encode_backup_owner(&[0; 32]), Err(Error::Input));
        assert_eq!(encode_backup_owner(&[0xff; 32]), Err(Error::Input));
    }

    #[test]
    fn live_prng_and_lsi_status_do_not_change_identity_but_owner_does() {
        let base = fixture();
        let context = base.qualify().unwrap();
        for byte in 0..=255 {
            let mut live = base;
            live.tamper_active_state |= byte;
            live.backup_clock |= 1 << 27;
            assert_eq!(live.qualify(), Ok(context));
        }
        context.verify_backup_owner(&[0x46; 32]).unwrap();
        assert_eq!(
            context.verify_backup_owner(&[0x47; 32]),
            Err(Error::Hardware)
        );
        let mut changed = base;
        changed.backup_anchor = encode_backup_owner(&[0x47; 32]).unwrap();
        assert_ne!(changed.qualify().unwrap(), context);
    }

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
