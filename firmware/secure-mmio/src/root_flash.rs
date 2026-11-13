//! Exclusive fixed-page secure FLASH boundary. The signed production boot/OTA
//! layout must reserve bank2 sectors126/127; diagnostics never call this.
use crate::ports::{Aliases, aliases};
use core::sync::atomic::{Ordering, compiler_fence};
use embassy_stm32::{Peri, pac, peripherals};
use openkeys_h573_crypto::device_key::DeviceKeyContext;
use openkeys_root_flash::{Flash, Layout, Port, Register, SECURE_BASE};
use openkeys_root_journal::{Error, PHRASE_BYTES, SECTOR_BYTES};

pub struct RootFlashPort<'d> {
    _owned: Peri<'d, peripherals::FLASH>,
    readbacks: Aliases,
}

/// Privileged verified Secure Root only. Requires the real closed-profile,
/// memory attribution, cache-off/quiet state and boot/OTA exclusion of both
/// sectors BEFORE this call. No fuses, remap, attribution or cache setup here.
/// No production boot caller/reserved linker image is supplied by this port.
pub fn initialize_root_flash(
    flash: Peri<'_, peripherals::FLASH>,
) -> Result<Flash<RootFlashPort<'_>>, Error> {
    Flash::new(RootFlashPort {
        _owned: flash,
        readbacks: aliases(),
    })
}

#[allow(unsafe_code)]
fn word(slot: u8, offset: usize) -> Result<pac::common::Reg<u32, pac::common::RW>, Error> {
    if slot > 1 || offset >= SECTOR_BYTES || !offset.is_multiple_of(4) {
        return Err(Error::Input);
    }
    let address = SECURE_BASE as usize + usize::from(slot) * SECTOR_BYTES + offset;
    // SAFETY: Only one of4096 aligned32-bit words in the two fixed secure
    // flash sectors0x0C1FC000..0x0C1FFFFF can be selected. No caller pointer,
    // reference, slice or arbitrary address. FLASH ownership is moved into
    // RootFlashPort, whose raw owner is never returned. Readbacks qualify
    // secure mapping/no-swap/no-EDATA/cache-off; the protected boot/OTA/MPU
    // reservation and exclusive bus use remain Root obligations. The one
    // phrase writer below issues four ordered stores; other use is reads.
    Ok(unsafe { pac::common::Reg::from_ptr(address as *mut u32) })
}

impl Port for RootFlashPort<'_> {
    fn read(&mut self, register: Register) -> Result<u32, Error> {
        let p = self.readbacks.flash;
        match register {
            Register::Status => Ok(p.secsr().read().0),
            Register::Control => Ok(p.seccr().read().0),
            Register::Correction => Ok(p.ecccorr().read().0),
            Register::Detection => Ok(p.eccdetr().read().0),
            _ => Err(Error::Input),
        }
    }
    fn write(&mut self, register: Register, value: u32) -> Result<(), Error> {
        let p = self.readbacks.flash;
        // Never permit INV, FW, bank/mass erase, option/OBK keys, error/ECC
        // clears, interrupts or a different physical sector even internally.
        match register {
            Register::Control
                if matches!(
                    value,
                    0 | 1 | 2 | 0x8000_1f84 | 0x8000_1fa4 | 0x8000_1fc4 | 0x8000_1fe4
                ) =>
            {
                p.seccr().write_value(pac::flash::regs::Seccr(value));
            }
            Register::Key if matches!(value, 0x4567_0123 | 0xcdef_89ab) => {
                p.seckeyr().write_value(value);
            }
            Register::Clear if value == 0x0001_0000 => {
                p.secccr().write_value(pac::flash::regs::Secccr(value));
            }
            _ => return Err(Error::Input),
        }
        Ok(())
    }
    fn context(&mut self) -> Result<DeviceKeyContext, Error> {
        self.readbacks
            .device_context()
            .map_err(|_| Error::Authorization)
    }
    fn layout(&mut self) -> Result<Layout, Error> {
        let a = &self.readbacks;
        Ok(Layout {
            options: a.flash.optsr_cur().read().0,
            secure_watermark: a.flash.secwm2r_cur().read().0,
            secure_tail: a.flash.secbb2r4().read().0,
            privileged_tail: a.flash.privbb2r4().read().0,
            edata: a.flash.edata2r_cur().read().0,
            icache_control: a.icache.cr().read().0,
            icache_status: a.icache.sr().read().0,
            dcache_control: a.dcache.cr().read().0,
            dcache_status: a.dcache.sr().read().0,
        })
    }
    fn read_bytes(&mut self, slot: u8, offset: usize, out: &mut [u8]) -> Result<(), Error> {
        if slot > 1 || offset > SECTOR_BYTES || out.len() > SECTOR_BYTES - offset {
            return Err(Error::Input);
        }
        cortex_m::asm::dsb();
        let mut cursor = 0;
        while cursor < out.len() {
            let at = offset + cursor;
            let value = word(slot, at & !3)?.read().to_le_bytes();
            let start = at & 3;
            let length = (4 - start).min(out.len() - cursor);
            out[cursor..cursor + length].copy_from_slice(&value[start..start + length]);
            cursor += length;
        }
        cortex_m::asm::dsb();
        Ok(())
    }
    fn program_phrase(
        &mut self,
        slot: u8,
        offset: usize,
        data: &[u8; PHRASE_BYTES],
    ) -> Result<(), Error> {
        if slot > 1 || offset > SECTOR_BYTES - PHRASE_BYTES || !offset.is_multiple_of(PHRASE_BYTES)
        {
            return Err(Error::Input);
        }
        if self.read(Register::Control)? != 2 || self.read(Register::Status)? & 0x007e_000b != 0 {
            return Err(Error::Io);
        }
        let words = [
            word(slot, offset)?,
            word(slot, offset + 4)?,
            word(slot, offset + 8)?,
            word(slot, offset + 12)?,
        ];
        cortex_m::interrupt::free(|_| {
            compiler_fence(Ordering::SeqCst);
            cortex_m::asm::dsb();
            for (register, value) in words.iter().zip(data.as_chunks::<4>().0) {
                register.write_value(u32::from_le_bytes(*value));
            }
            cortex_m::asm::dsb();
            compiler_fence(Ordering::SeqCst);
        });
        Ok(())
    }
    fn emergency_reset(&mut self) -> ! {
        cortex_m::peripheral::SCB::sys_reset()
    }
}
