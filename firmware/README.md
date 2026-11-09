# Device firmware status — R0.4

This directory defines the target integration. The implemented Rust firmware **library** is [`device-policy`](../crates/device-policy); there is no flashable authenticator ELF/BIN or signed update bundle yet. A host executable or an ARM `.rlib` is not device firmware delivery.

Target: STM32H573RIT6, Cortex-M33, STiRoT immutable boot, TrustZone/GTZC isolation, SE050E, authenticated FPC2534AM, I2C 16x2 white LCD, four physical buttons, one USB-C USB2 FS port, FM11NC08S NFC card interface, encrypted external Flash. Exact pin assignments: [R0.4 pinout](../hardware/r04/pinout.csv).

Implemented and tested: request/channel/digest-bound one-shot authorization; mandatory authenticated biometric receipt; device-derived additional PIN policy; durable-attempt adapter contracts; FPC secure-frame parsing, replay/context/tag checks; bounded 12-symbol button PIN entry; English LCD identity pagination and cancellation clearing. Default hardware adapters deny access.

Integration still required: actual sensor pairing/driver and SE SCP03/AEAD/counter/PIN adapters; authenticated device management transport; full FIDO2/U2F/hmac-secret/credential storage; AEAD transactional vault with anti-rollback; LCD/button drivers; USB/NFC framing and cancellation; power/charging/suspend service and secure PC0 input-eFuse fault handling; backup-tamper continuity; STiRoT configuration, signed updates, rollback and production debug lifecycle. No production fuses or keys are supplied in this repository.

The protected domain must own the FP/SE/LCD buses, button GPIO, timers, DMA attribution, key memory and request dispatcher. Untrusted transport must not have raw SE, encrypt/decrypt, display, button-injection or counter-reset APIs. The SE enable circuit is a fault interlock, not proof of fingerprint verification.

Before first enrollment, a blank device has no user secrets. Provisioning is a separate physically controlled ceremony: unique device/sensor/SE pairing, initial device PIN, fingerprint enrollment, lifecycle closure and attestation of that state. A provisioned device must never re-enter blank enrollment merely because backup power or a sensor was reset. A missing fingerprint never enables PIN-only access.

```sh
rustup target add thumbv8m.main-none-eabihf --toolchain 1.99.1
cargo check -p device-policy --target thumbv8m.main-none-eabihf --locked
```

This verifies portable policy code only, not boot, registers, timing, pinmux, interoperability or resistance to physical attacks. Manufacturing remains blocked until target firmware and actual hardware evidence exist.

## R04 board support and executable

[openkeys-board](board) now implements the actual LCD, release-only four-key debounce and bounded/verified W25Q64 driver. Its build contract checks the PCB/pinout SHA256 and all GPIO/AF mappings. Tests include LCD I/O failure/reset/teardown, malformed display bytes, chords/context changes/clock discontinuities, Flash overflow/page wrap/WEL/readback corruption and maintained critical-section nesting/thread exclusion.

[Target diagnostic firmware](target/README.md) builds a real ARM ELF/BIN at [dist](dist). This is an explicitly OPEN/TZEN=0 **electrical diagnostic**, not a flashable authenticator. It keeps the sensor, SE, charging and private operations disabled. Production readiness does not accept this diagnostic as `openkeys.elf`, a signed update or a boot-security profile. The preceding list of integration requirements still applies; LCD/button/Flash drivers now exist but require real board timing/fit qualification.

## Encrypted snapshot integration progress

[device-vault](../crates/device-vault) now provides a bounded two-bank AEAD snapshot core with mandatory protected-runtime checkpoints and power-loss/replay tests. The board crate maps its complete sectors to the actual8MiB W25Q64; the target library implements H573 SAES AES-256-GCM with tag-before-release and key-register clearing. These compile for Cortex-M33. A real authenticated/anti-rollback SE ledger, key provisioning, record validation and authorized dispatcher are still required; the diagnostic binary exposes none of them. No production adapter silently falls back to a test key, host Boolean or ordinary Flash counter.
