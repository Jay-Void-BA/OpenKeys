# R05 STM32H573 target

The active PCB uses a device PIN and ten decimal keys. U5 is the SE051C2
full-feature candidate; [selection and SYS backup corrections](../../docs/CLASSICAL_SE_POWER_R05.md)
require Applet 7.2/native-PIN qualification before any private service is enabled. No fingerprint sensor,
default PIN, or host hardware-PIN entry exists in this profile. TOTP/HOTP
and OpenPGP/PIV/SSH are production integration targets, not diagnostic features.

**There is no production authenticator image yet.** This target is an
OPEN-lifecycle / TZEN=0 electrical diagnostic. It must not contain owner secrets
or be used for FIDO accounts, vaults, commissioning or production fuse writes.

## Build from the saved electrical PCB

`board-contract.sha256` binds the reviewed R05 PCB and schematic-derived pinout.
`build.rs` rejects changed bytes and independently checks all GPIO/AF assignments
used by this diagnostic. Do not copy the old R04 pin module or its binary.

```sh
rustup target add thumbv8m.main-none-eabihf --toolchain 1.99.1
cargo +1.99.1 build -p openkeys-h573-r05 --release --target thumbv8m.main-none-eabihf --features r05-diagnostics --locked
# Explicit alternative ONLY for a physically verified DFR0555 V1.0:
cargo +1.99.1 build -p openkeys-h573-r05 --release --target thumbv8m.main-none-eabihf --features r05-diagnostics,lcd-v1-0 --locked
```

The default diagnostic is for BOM V1.1 (`0x6B` backlight). The alternative V1.0
uses `0x60`. Both use `0x3E` character control. No automatic scan or fallback.
Export both with `python3 scripts/export-r05-diagnostic.py`; verify the manifest
with `python3 scripts/check-r05-diagnostic.py`. Export does not flash a board.

The diagnostic tests only public LCD frames, individual key releases and the
Flash part ID. It holds SE_ENABLE_REQUEST low, NFC deselected and charging
disabled, and has no USB/NFC protocol, private records or PIN verification.
Key display is a public wiring test: never enter a real PIN into diagnostics.
Case/guard faults cut the display and latch the diagnostic; watchdog/panic reset
does not intentionally restore a prior frame. Actual fault timing remains untested.
The frozen PCB now includes a TPS3808 hardware reset hold. LCD initialization
waits500ms after releasing MR before a new arm edge, then200ms before I2C;
see the [power/reset bounds and bench requirements](../../docs/CLASSICAL_LCD_POWER_R05.md).

## PIN candidate modules

- [ten_keys](../board/src/ten_keys.rs): ten independent contacts, release-only,
  debounce, chord/roll/held-context/poll-gap rejection.
- [pin_ui](../board/src/pin_ui.rs): host request cannot spend retries; a fresh
  local 5 release starts the protected 6-digit PIN entry; masked progress.
- [pin_only](../../crates/device-policy/src/pin_only.rs): request-bound receipt,
  exact review, timeout and one-use grant; the paired-session cache is in
  [session](../../crates/device-policy/src/session.rs), with independent hardware
  and protocol leases and the [30-second rules](../../docs/PIN_SESSIONS_R05.md).
- [Native SE PIN backend](../../crates/se-state/src/native_pin.rs): UserID
  verification, persistent precharge, PIN-owner-only credit and sealed private
  I/O. Exact C2 commissioning/ACLs, full wipe and Root wiring remain required.
  The historical HKDF backend cannot authorize this private interface.
- [DFR0555](../board/src/dfr0555.rs): explicit module revision, cold init,
  exact ASCII rows, fault-latched power removal and isolated bus.

The production Root must connect these modules to isolated GPIO, live protected
time, the exact canonical request, trusted display writes, SE state, FIDO Env
and authenticated management. Old R04 biometric Gate paths must not be exposed
as R05 authorization alternatives. SE native ACL/PIN enforcement beyond trust
in the MCU Root is still an open architectural issue. See the
[R05 security review](../../docs/SECURITY_REVIEW_R05.md) and
[manufacturing/test instructions](../../manufacturing/BUILD_R05_ZH.md).
