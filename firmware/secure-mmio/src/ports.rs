use embassy_stm32::{Peri, pac, peripherals};
use openkeys_h573_crypto::{
    Error, MaskedAes, Register, Registers,
    device_key::{DeviceKeyContext, Snapshot},
    entropy::{CheckedRng, Source},
};

const RNG_CONFIG: u32 = 0x00f0_1e00; // AN4230 Rev15 table3, H563/573/562,48MHz.
const RNG_HTCR: u32 = 0x6a91;
const RNG_NSCR: u32 = 0x3af66;
const AUTO_RESET_DISABLE: u32 = 1 << 7;
const CONDITION_RESET: u32 = 1 << 30;
const CONFIG_LOCK: u32 = 1 << 31;
const ENABLE: u32 = 1 << 2;
const ERROR_BITS: u32 = 0b110_0110; // CECS,SECS,CEIS,SEIS; never cleared as recovery.

struct Aliases {
    rng: pac::rng::Rng,
    saes: pac::saes::Saes,
    rcc: pac::rcc::Rcc,
    gtzc: pac::gtzc::Gtzc1Tzsc,
    nscr: pac::common::Reg<u32, pac::common::RW>,
    i2c1: pac::i2c::I2c,
    flash: pac::flash::Flash,
    sbs: pac::syscfg::Syscfg,
    tamp: pac::tamp::Tamp,
}
#[allow(unsafe_code)]
fn aliases() -> Aliases {
    // SAFETY: RM0481 Rev5 section2.3 secure peripheral memory map. Each fixed
    // address is a documented,4-byte-aligned volatile peripheral register
    // region, not RAM or a Rust reference. RNG/SAES ownership is exclusively
    // moved into their ports before use. RCC/GTZC are bootstrap-only and are
    // not returned to clients. The boot caller must be privileged Secure and
    // serialise clock/attribution configuration; NS code cannot call this.
    unsafe {
        Aliases {
            rng: pac::rng::Rng::from_ptr(0x520c_0800 as *mut ()),
            saes: pac::saes::Saes::from_ptr(0x520c_0c00 as *mut ()),
            rcc: pac::rcc::Rcc::from_ptr(0x5402_0c00 as *mut ()),
            gtzc: pac::gtzc::Gtzc1Tzsc::from_ptr(0x5003_2400 as *mut ()),
            nscr: pac::common::Reg::from_ptr(0x520c_080c as *mut u32),
            i2c1: pac::i2c::I2c::from_ptr(0x5000_5400 as *mut ()),
            // Only readbacks through these three handles. Option bytes,
            // HDPL/epoch selection and tamper flags are NEVER written here.
            flash: pac::flash::Flash::from_ptr(0x5002_2000 as *mut ()),
            sbs: pac::syscfg::Syscfg::from_ptr(0x5400_0400 as *mut ()),
            tamp: pac::tamp::Tamp::from_ptr(0x5400_7c00 as *mut ()),
        }
    }
}

/// Holds the unique HAL owner to preserve pin/peripheral lifetime. Runtime
/// transactions use the bounded, progress-aware Secure-alias implementation;
/// the HAL's ambiguous-NACK read/write methods are never called.
pub struct I2c1Registers<'d> {
    _owned: embassy_stm32::i2c::I2c<
        'd,
        embassy_stm32::mode::Blocking,
        embassy_stm32::i2c::mode::Master,
    >,
    registers: pac::i2c::I2c,
    rcc: pac::rcc::Rcc,
    gtzc: pac::gtzc::Gtzc1Tzsc,
    clock_source: u8,
}
impl I2c1Registers<'_> {
    fn attributed(&self) -> bool {
        self.gtzc.seccfgr1().read().i2c1sec()
            && self.gtzc.privcfgr1().read().i2c1priv()
            && self.rcc.apb1lenr().read().i2c1en()
            && self.rcc.ccipr4().read().i2c1sel().to_bits() == self.clock_source
    }
}
pub type SecureSeI2c<'d, C> = openkeys_board::se_i2c::Master<I2c1Registers<'d>, C>;

/// Privileged Secure bootstrap, serialized with all clock/pin attribution.
/// The root must already own PB6/PB7 and establish their SAU/GPIO attributes.
/// No clock reset, bus-unwedge pulses, DMA, interrupt, address or key API.
pub fn initialize_se_i2c<'d, C: openkeys_se_transport::transport::Clock>(
    bus: embassy_stm32::i2c::I2c<
        'd,
        embassy_stm32::mode::Blocking,
        embassy_stm32::i2c::mode::Master,
    >,
    clock: C,
) -> Result<SecureSeI2c<'d, C>, openkeys_board::se_i2c::Error> {
    let a = aliases();
    let registers = I2c1Registers {
        _owned: bus,
        registers: a.i2c1,
        rcc: a.rcc,
        gtzc: a.gtzc,
        clock_source: a.rcc.ccipr4().read().i2c1sel().to_bits(),
    };
    if !registers.attributed() {
        return Err(openkeys_board::se_i2c::Error::Hardware);
    }
    // HAL master configuration must not enable any target address.
    if registers.registers.oar1().read().0 & (1 << 15) != 0
        || registers.registers.oar2().read().0 & (1 << 15) != 0
    {
        return Err(openkeys_board::se_i2c::Error::Hardware);
    }
    openkeys_board::se_i2c::Master::new(registers, clock)
}
impl openkeys_board::se_i2c::Registers for I2c1Registers<'_> {
    fn read(
        &mut self,
        reg: openkeys_board::se_i2c::Register,
    ) -> Result<u32, openkeys_board::se_i2c::Error> {
        use openkeys_board::se_i2c::{Error, Register as R};
        if !self.attributed() {
            return Err(Error::Hardware);
        }
        let p = self.registers;
        Ok(match reg {
            R::Cr1 => p.cr1().read().0,
            R::Cr2 => p.cr2().read().0,
            R::Isr => p.isr().read().0,
            R::Timing => p.timingr().read().0,
            R::Rxdr => p.rxdr().read().0,
            _ => return Err(Error::Hardware),
        })
    }
    fn write(
        &mut self,
        reg: openkeys_board::se_i2c::Register,
        value: u32,
    ) -> Result<(), openkeys_board::se_i2c::Error> {
        use openkeys_board::se_i2c::{Error, PE, Register as R};
        // A terminal PE-disable is attempted even after attribution changed.
        if !(reg == R::Cr1 && value & PE == 0) && !self.attributed() {
            return Err(Error::Hardware);
        }
        let p = self.registers;
        match reg {
            R::Cr1 => p.cr1().write_value(pac::i2c::regs::Cr1(value)),
            R::Cr2 => p.cr2().write_value(pac::i2c::regs::Cr2(value)),
            R::Icr => p.icr().write_value(pac::i2c::regs::Icr(value)),
            R::Txdr => p.txdr().write_value(pac::i2c::regs::Txdr(value)),
            _ => return Err(Error::Hardware),
        }
        Ok(())
    }
}

pub struct SaesPort<'d> {
    _owned: Peri<'d, peripherals::SAES>,
    registers: pac::saes::Saes,
    readbacks: Aliases,
}
pub struct RngPort<'d> {
    _owned: Peri<'d, peripherals::RNG>,
    registers: pac::rng::Rng,
    nscr: pac::common::Reg<u32, pac::common::RW>,
}
pub type Initialized<'d> = (MaskedAes<SaesPort<'d>>, CheckedRng<RngPort<'d>>);

/// Privileged Secure bootstrap only, before interrupts/tasks/peripheral use.
/// STiRoT/TZEN, GTZC attribution, TAMP continuity and the48MHz HSI48 source
/// must already be established by verified boot. This never configures fuses,
/// bypasses a lock, clears health errors, retries a seed or supplies test keys.
/// Missing attributes/clock or an incompatible locked RNG profile fail shut.
pub fn initialize<'d>(
    saes: Peri<'d, peripherals::SAES>,
    rng: Peri<'d, peripherals::RNG>,
) -> Result<Initialized<'d>, Error> {
    let a = aliases();
    // metapac's common H5 GTZC type omits SAES fields. Their raw bits are
    // explicit in RM0481: RNG18,SAES19. Both Secure AND privileged required.
    let required = (1 << 18) | (1 << 19);
    if a.gtzc.seccfgr3().read().0 & required != required
        || a.gtzc.privcfgr3().read().0 & required != required
        || !a.rcc.cr().read().hsi48rdy()
        || a.rcc.ccipr5().read().rngsel() != pac::rcc::vals::Rngsel::HSI48
    {
        return Err(Error::Hardware);
    }
    // Do not use RCC reset to erase a previous RNG/SAES error. No CPU DMA or
    // cipher IRQ is enabled. The caller serialises all RCC modification.
    a.rcc.ahb2enr().modify(|w| w.set_rngen(true));
    let _ = a.rcc.ahb2enr().read();
    if a.rng.sr().read().0 & ERROR_BITS != 0 {
        return Err(Error::Hardware);
    }
    let expected = RNG_CONFIG | AUTO_RESET_DISABLE | ENABLE | CONFIG_LOCK;
    if !a.rng.cr().read().configlock() {
        a.rng.cr().modify(|w| w.set_rngen(false));
        a.rng.cr().write_value(pac::rng::regs::Cr(
            RNG_CONFIG | AUTO_RESET_DISABLE | CONDITION_RESET,
        ));
        // The PAC's RECOMMENDED enum is for other devices. Use H573 values
        // directly and write the HTCR unlock key before the configuration.
        a.rng.htcr().write_value(pac::rng::regs::Htcr(0x1759_0abc));
        a.rng.htcr().write_value(pac::rng::regs::Htcr(RNG_HTCR));
        a.nscr.write_value(RNG_NSCR);
        if a.rng.htcr().read().0 != RNG_HTCR || a.nscr.read() != RNG_NSCR {
            return Err(Error::Hardware);
        }
        a.rng.cr().write_value(pac::rng::regs::Cr(expected));
    }
    let mut ready = false;
    for _ in 0..500_000 {
        if a.rng.sr().read().0 & ERROR_BITS != 0 {
            return Err(Error::Hardware);
        }
        if !a.rng.cr().read().condrst() {
            ready = true;
            break;
        }
    }
    if !ready {
        return Err(Error::Timeout);
    }
    // No blind clear of SEIS/CEIS after startup. A detected error rejects
    // this boot; provisioning state must remain provisioned across resets.
    if a.rng.cr().read().0 != expected
        || a.rng.htcr().read().0 != RNG_HTCR
        || a.nscr.read() != RNG_NSCR
    {
        return Err(Error::Hardware);
    }
    let source = RngPort {
        _owned: rng,
        registers: a.rng,
        nscr: a.nscr,
    };
    let rng = CheckedRng::new(source)?;
    // SAES autonomously fetches masks from the running healthy RNG. Keep its
    // clock/source alive for the whole engine lifetime; do not stop it when
    // application random-byte demand is idle.
    a.rcc.ahb2enr().modify(|w| w.set_saesen(true));
    let _ = a.rcc.ahb2enr().read();
    let aes = MaskedAes::new(SaesPort {
        _owned: saes,
        registers: a.saes,
        readbacks: a,
    })?;
    Ok((aes, rng))
}

impl Source for RngPort<'_> {
    fn poll(&mut self) -> Result<Option<u32>, Error> {
        let r = self.registers;
        if r.cr().read().0 != (RNG_CONFIG | AUTO_RESET_DISABLE | ENABLE | CONFIG_LOCK)
            || r.htcr().read().0 != RNG_HTCR
            || self.nscr.read() != RNG_NSCR
        {
            return Err(Error::Hardware);
        }
        let status = r.sr().read();
        if status.0 & ERROR_BITS != 0 {
            return Err(Error::Hardware);
        }
        if !status.drdy() {
            return Ok(None);
        }
        let word = r.dr().read();
        if r.sr().read().0 & ERROR_BITS != 0 {
            return Err(Error::Hardware);
        }
        Ok(Some(word))
    }
}

impl Registers for SaesPort<'_> {
    fn read(&mut self, r: Register) -> u32 {
        let p = self.registers;
        match r {
            Register::Cr => p.cr().read().0,
            Register::Sr => p.sr().read().0,
            Register::Isr => p.isr().read().0,
            Register::Dout => p.doutr().read(),
            // Invalid internal register access is a coding/security fault,
            // never a host-selectable address or a fabricated register value.
            _ => self.emergency_reset(),
        }
    }
    fn write(&mut self, r: Register, value: u32) {
        let p = self.registers;
        match r {
            Register::Cr => p.cr().write_value(pac::saes::regs::Cr(value)),
            Register::Icr => p.icr().write_value(pac::saes::regs::Icr(value)),
            Register::Ier => p.ier().write_value(pac::saes::regs::Ier(value)),
            Register::Din => p.dinr().write_value(value),
            Register::Key(i) if i < 8 => p.keyr(usize::from(i)).write_value(value),
            Register::Iv(i) if i < 4 => p.ivr(usize::from(i)).write_value(value),
            _ => self.emergency_reset(),
        }
    }
    fn device_key_context(&mut self) -> Result<DeviceKeyContext, Error> {
        // Two fresh reads catch transitions while assembling the snapshot;
        // the crypto core also compares before/after every block and cleanup.
        let before = self.device_snapshot()?.qualify()?;
        let after = self.device_snapshot()?.qualify()?;
        if before != after {
            return Err(Error::Hardware);
        }
        Ok(after)
    }
    fn emergency_reset(&mut self) -> ! {
        cortex_m::peripheral::SCB::sys_reset()
    }
}

impl SaesPort<'_> {
    fn device_snapshot(&self) -> Result<Snapshot, Error> {
        let a = &self.readbacks;
        if !a.rcc.ahb2enr().read().rngen()
            || !a.rcc.ahb2enr().read().saesen()
            || !a.rcc.cr().read().hsi48rdy()
            || a.rcc.ccipr5().read().rngsel() != pac::rcc::vals::Rngsel::HSI48
            || a.rng.cr().read().0 != (RNG_CONFIG | AUTO_RESET_DISABLE | ENABLE | CONFIG_LOCK)
            || a.rng.htcr().read().0 != RNG_HTCR
            || a.nscr.read() != RNG_NSCR
            || a.rng.sr().read().0 & ERROR_BITS != 0
        {
            return Err(Error::Hardware);
        }
        Ok(Snapshot {
            flash_options: a.flash.optsr_cur().read().0,
            flash_options2: a.flash.optsr2_cur().read().0,
            flash_secure_boot: a.flash.secbootr_cur().read().0,
            flash_secure_epoch: a.flash.secepochr_cur().read().0,
            flash_privilege: a.flash.privcfgr().read().0,
            hdpl: a.sbs.hdplsr().read().0,
            next_hdpl: a.sbs.nexthdplcr().read().0,
            epoch_select: a.sbs.epochselcr().read().0,
            sbs_security: a.sbs.seccfgr().read().0,
            memory_erase_status: a.sbs.mesr().read().0,
            tamper_control: [
                a.tamp.cr1().read().0,
                a.tamp.cr2().read().0,
                a.tamp.cr3().read().0,
            ],
            tamper_security: a.tamp.seccfgr().read().0,
            tamper_privilege: a.tamp.privcfgr().read().0,
            tamper_filter: a.tamp.fltcr().read().0,
            tamper_active: a.tamp.atcr1().read().0,
            tamper_status: a.tamp.sr().read().0,
            crypto_security: a.gtzc.seccfgr3().read().0,
            crypto_privilege: a.gtzc.privcfgr3().read().0,
            unprivileged: cortex_m::register::control::read().npriv()
                == cortex_m::register::control::Npriv::Unprivileged,
            debugger_attached: cortex_m::peripheral::DCB::is_debugger_attached(),
        })
    }
}
