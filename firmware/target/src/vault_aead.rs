//! Actual STM32H573 SAES AES-256-GCM adapter, never a host crypto oracle.
//! Requires correct secure peripheral/RAM/DMA attribution, a running watchdog,
//! authenticated provisioning of a unique purpose-specific vault key and the
//! durable SE epoch ledger. None is supplied by the diagnostic image.
//! Do not derive this key from the public MCU UID or use a firmware default.

use device_vault::{Aead, Error, PAYLOAD_BYTES};
use embassy_stm32::{
    mode::Blocking,
    pac, peripherals,
    saes::{AesGcm, Direction, Saes},
};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

pub struct HardwareVaultAead<'d> {
    hardware: Saes<'d, peripherals::SAES, Blocking>,
    key: Zeroizing<[u8; 32]>,
    faulted: bool,
}

impl<'d> HardwareVaultAead<'d> {
    /// Key must arrive through the protected provisioning/unwrapping path.
    /// No key readback, debug formatting, raw host API or fallback is exposed.
    pub fn new(hardware: Saes<'d, peripherals::SAES, Blocking>, key: Zeroizing<[u8; 32]>) -> Self {
        let mut this = Self {
            hardware,
            key,
            faulted: false,
        };
        this.clear_registers();
        this
    }

    fn clear_registers(&mut self) {
        let peripheral = pac::SAES;
        // Wait before changing CR, as required by the SAES register contract.
        // A stuck peripheral cannot preserve an unlocked device indefinitely.
        for _ in 0..500_000 {
            if !peripheral.sr().read().busy() {
                peripheral.cr().modify(|r| r.set_en(false));
                peripheral.cr().modify(|r| r.set_iprst(true));
                peripheral.cr().modify(|r| r.set_iprst(false));
                if peripheral.sr().read().keyvalid() || peripheral.cr().read().en() {
                    cortex_m::peripheral::SCB::sys_reset();
                }
                return;
            }
        }
        cortex_m::peripheral::SCB::sys_reset();
    }

    fn crypt(
        &mut self,
        direction: Direction,
        nonce: &[u8; 12],
        aad: &[u8; 48],
        input: &[u8; PAYLOAD_BYTES],
        output: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<[u8; 16], Error> {
        if self.faulted {
            return Err(Error::Faulted);
        }
        let result = (|| {
            let cipher = AesGcm::new(&self.key, nonce);
            let mut context = self.hardware.start(&cipher, direction);
            self.hardware
                .aad_blocking(&mut context, aad, true)
                .map_err(|_| Error::Crypto)?;
            self.hardware
                .payload_blocking(&mut context, input, output, true)
                .map_err(|_| Error::Crypto)?;
            self.hardware
                .finish_blocking(context)
                .map_err(|_| Error::Crypto)?
                .ok_or(Error::Crypto)
        })();
        // finish_blocking disables EN but does not itself erase key registers.
        self.clear_registers();
        if result.is_err() {
            self.faulted = true;
            self.key.zeroize();
            output.fill(0);
        }
        result
    }
}

impl Aead for HardwareVaultAead<'_> {
    fn seal(
        &mut self,
        nonce: &[u8; 12],
        aad: &[u8; 48],
        bytes: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<[u8; 16], Error> {
        let mut output = Zeroizing::new([0; PAYLOAD_BYTES]);
        match self.crypt(Direction::Encrypt, nonce, aad, bytes, &mut output) {
            Ok(tag) => {
                bytes.copy_from_slice(&output[..]);
                Ok(tag)
            }
            Err(error) => {
                bytes.fill(0);
                Err(error)
            }
        }
    }
    fn open(
        &mut self,
        nonce: &[u8; 12],
        aad: &[u8; 48],
        tag: &[u8; 16],
        bytes: &mut [u8; PAYLOAD_BYTES],
    ) -> Result<(), Error> {
        let mut output = Zeroizing::new([0; PAYLOAD_BYTES]);
        let calculated = self.crypt(Direction::Decrypt, nonce, aad, bytes, &mut output);
        if let Ok(calculated) = calculated
            && bool::from(calculated.ct_eq(tag))
        {
            // No tentative plaintext is copied to the caller before this.
            bytes.copy_from_slice(&output[..]);
            return Ok(());
        }
        bytes.fill(0);
        self.faulted = true;
        self.key.zeroize();
        Err(Error::Crypto)
    }
}

impl Drop for HardwareVaultAead<'_> {
    fn drop(&mut self) {
        self.clear_registers();
        self.key.zeroize();
    }
}
