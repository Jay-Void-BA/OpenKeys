# R04 electrical diagnostic image

The target library also contains candidate protected-runtime crypto and FPC SPI3 adapters. They are not called by the diagnostic. Their [security review](../../docs/FPC_RUNTIME_REVIEW.md) lists the remaining pairing, TrustZone attribution, runtime and chip-qualification gates.

This is a real Cortex-M33 executable for the frozen STM32H573RIT6 PCB. It is **not a FIDO authenticator**, a vault, or production secure firmware. It contains no successful enrollment/authentication path and must never be used to provision the three real keys.

Actual bindings: I2C3 PA8/PC9 to the 3.0V LCD, PC3 **open-drain** reset, PC4 backlight; SPI2 PB13/PB15/PC2 and PB12 CS to W25Q64; PB0/PB1/PB2/PB5 buttons; PC13 case and PC0 input-guard fault. PC8 SE request and PC7 sensor reset stay LOW; PA15 and PA4 deselect FP/NFC. PB4 charging enable stays LOW because the USB suspend/current service is not implemented in this image. No USB enumeration or NFC operation is advertised.

It initializes the LCD with the vendor command sequence, identifies the exact Flash JEDEC ID, shows `DIAGNOSTIC ONLY`, and reports debounced physical key releases. It never writes/erases external Flash. Case or input-guard faults blank/reset the display and latch the diagnostic loop. A 2s independent watchdog resets stalls. A panic resets the MCU, independently asserting the R04 LCD/SE interlocks.

The build verifies SHA256 of the actual PCB and pinout and compares every GPIO/AF entry before compiling. Changed hardware requires a reviewed new contract. These checks cannot prove real bus timing, physical fit, sensor authentication or resistance to fault injection.

```sh
cargo +1.99.1 build -p openkeys-h573 --release \
  --target thumbv8m.main-none-eabihf --features r04-diagnostics --locked
cargo +1.99.1 clippy -p openkeys-h573 \
  --target thumbv8m.main-none-eabihf --features r04-diagnostics --locked -- -D warnings
```

The memory map is deliberately **OPEN lifecycle / TZEN=0**: FLASH0x08000000/2MiB, first256KiB SRAM0x20000000. No option-byte, erase, key provisioning or irreversible lifecycle action is automated. **Do not try this image on a provisioned/closed device or weaken its protection to run it.** Production requires a separate STiRoT verified-image/TrustZone/rollback profile, authenticated sensor/SE adapters and the full protocol runtime.

Ask the assembler to load the ELF through an internal SWD fixture on a blank test board, using their supported programmer and verified device selection. Validate rails/current first, then LCD reset/white backlight and four buttons; follow H01/H02/E01/E07 in [the safety test plan](../../docs/SECURITY_TEST_PLAN.md). No instrument or physical target was connected in this workspace, so runtime/board tests remain NOT_RUN.
