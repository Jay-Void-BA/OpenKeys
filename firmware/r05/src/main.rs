#![no_std]
#![no_main]
#![forbid(unsafe_code)]
//! R05 OPEN/TZEN=0 electrical bring-up, not an authenticator. No enrollment,
//! key generation, private storage, USB protocol or charging is enabled.
//! Its only display data are public diagnostics; SE stays disabled; there is no fingerprint hardware.

use cortex_m::{delay::Delay, peripheral::DWT};
use cortex_m_rt::entry;
use embassy_stm32::{
    Config,
    gpio::{Input, Level, Output, OutputOpenDrain, Pull, Speed},
    i2c::{Config as I2cConfig, I2c},
    spi::{Config as SpiConfig, Spi},
    time::Hertz,
    wdg::IndependentWatchdog,
};
use embedded_hal_bus::spi::ExclusiveDevice;
use openkeys_board::{
    dfr0555::{Lcd, Revision},
    flash::Flash,
    ten_keys::Keypad,
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    // NRST -> U15 LCD reset and U12 SE-disable are independent of successful
    // firmware reinitialization. No panic text, keys or memory dump is sent.
    cortex_m::peripheral::SCB::sys_reset()
}

#[entry]
fn main() -> ! {
    let mut config = Config::default();
    config.rcc.hsi48 = None; // No USB in this diagnostic image.
    let p = embassy_stm32::init(config);
    let mut core = cortex_m::Peripherals::take().unwrap();
    core.DCB.enable_trace();
    core.DWT.enable_cycle_counter();

    // Safest initial states, matching the actual R05 circuit and polarity.
    let _se_disabled = Output::new(p.PC8, Level::Low, Speed::Low);
    let _nfc_deselected = Output::new(p.PA4, Level::High, Speed::Low);
    // No USB suspend/configuration service yet; never enable CE in this image.
    let _charging_disabled = Output::new(p.PB4, Level::Low, Speed::Low);
    let lcd_reset = OutputOpenDrain::new(p.PC3, Level::Low, Speed::Low);
    let light = Output::new(p.PC4, Level::Low, Speed::Low);
    // PC13 is a hardware active-TAMP return, not a static "case closed"
    // level. Never poll its random waveform as a GPIO lid switch or drive
    // PA1 in this diagnostic. SW12 and mismatch logic assert real NRST.
    let _tamper_return = p.PC13;
    let _tamper_drive = p.PA1;
    let _battery_analog = p.PA0;
    let guard = Input::new(p.PC0, Pull::None);
    let buttons = [
        Input::new(p.PB0, Pull::None),
        Input::new(p.PB1, Pull::None),
        Input::new(p.PB2, Pull::None),
        Input::new(p.PB5, Pull::None),
        Input::new(p.PC6, Pull::None),
        Input::new(p.PC7, Pull::None),
        Input::new(p.PA15, Pull::None),
        Input::new(p.PC10, Pull::None),
        Input::new(p.PC11, Pull::None),
        Input::new(p.PC12, Pull::None),
    ];

    let mut watchdog = IndependentWatchdog::new(p.IWDG, 2_000_000);
    watchdog.unleash();
    let bus = I2c::new_blocking(p.I2C3, p.PA8, p.PC9, I2cConfig::default());
    let revision = if cfg!(feature = "lcd-v1-0") {
        Revision::V1_0
    } else {
        Revision::V1_1
    };
    let mut lcd = Lcd::new(
        bus,
        lcd_reset,
        light,
        Delay::new(core.SYST, 64_000_000),
        revision,
    )
    .unwrap();
    lcd.initialize().unwrap();
    if guard.is_low() {
        lcd.lock().unwrap();
        loop {
            watchdog.pet();
            cortex_m::asm::delay(64_000);
        }
    }

    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(1_000_000);
    let spi = Spi::new_blocking(p.SPI2, p.PB13, p.PB15, p.PC2, spi_config);
    let cs = Output::new(p.PB12, Level::High, Speed::Low);
    let device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
    let mut flash = Flash::new(device);
    // Independent SysTick delay owner is held by LCD. Delay cycles below are
    // only for this nonsecret diagnostic; production needs a secure live clock.
    struct SpinDelay;
    impl embedded_hal::delay::DelayNs for SpinDelay {
        fn delay_ns(&mut self, ns: u32) {
            let cycles = (u64::from(ns) * 64_000_000).div_ceil(1_000_000_000);
            cortex_m::asm::delay(u32::try_from(cycles).unwrap_or(u32::MAX));
        }
    }
    flash.identify(&mut SpinDelay).unwrap();
    lcd.show(&[*b"OpenKeys R05    ", *b"DIAGNOSTIC ONLY "])
        .unwrap();
    watchdog.pet();

    let mut last = DWT::cycle_count();
    let mut total_cycles = 0_u64;
    let mut input = Keypad::new(0);
    // Keep the exact hardware identity visible to the compiler/artifact.
    assert_eq!(openkeys_h573_r05::BOARD_REVISION, "0.5");
    loop {
        if guard.is_low() {
            lcd.lock().unwrap();
            loop {
                watchdog.pet();
                cortex_m::asm::delay(64_000);
            }
        }
        let current = DWT::cycle_count();
        total_cycles = total_cycles
            .checked_add(u64::from(current.wrapping_sub(last)))
            .unwrap();
        last = current;
        let now = total_cycles / 64_000;
        let pressed = buttons
            .iter()
            .enumerate()
            .fold(0, |mask, (i, pin)| mask | (u16::from(pin.is_low()) << i));
        if let Some((key, kind)) = input.sample(now, pressed).unwrap() {
            use device_policy::pin::Press;
            let mut name = *b"TEST KEY 0      ";
            name[9] = b'0' + key.value();
            let state = match kind {
                Press::Short => *b"Short release   ",
                Press::Long => *b"Long release    ",
            };
            lcd.show(&[name, state]).unwrap();
        }
        watchdog.pet();
        cortex_m::asm::delay(64_000);
    }
}
