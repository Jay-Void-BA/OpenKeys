> R05 adds `dfr0555`, `ten_keys`, `pin_ui` and `otp`; see [R05 target](../r05/README.md). The four-key, FPC and old LCD paths below belong to historical R04. The shared crate retains its R04 build contract; the actual R05 target independently binds R05 PCB/pinout. Production Root is not integrated.

# R04 protected-domain board drivers

This no_std Rust crate is written for the actual routed R04 PCB, not an arbitrary development board. The build checks SHA256 of that PCB and pinout and compares every GPIO/package-pin/alternate-function mapping. The policy, drivers and boot/security configuration must be reviewed again when any of these change.

- LCD: NHD-C0216CiZ-FSW-FBW-3V3 at7-bit0x3e,3.0V bus; PC3 reset is open-drain. Full16-byte rows, display-off while updating, backlight enabled only after successful writes. I/O/invalid-byte faults reset/blank and latch. Lock/drop holds reset low2ms before GPIO teardown.
- Four keys:25ms stable debounce,800ms long press, one event only on release. Chords, rolling from one held key to another, context changes and >100ms sampling gaps discard input. Clock rollback permanently faults the instance. Call `discard_pending` before a new authorization scene.
- Flash: W25Q64JV8MiB,256-byte pages,4KiB sectors. Validates exact JEDEC ID, ranges/alignment, bounded busy polling, WEL and write/erase readback. Only authenticated ciphertext may be stored here. The driver itself provides no AEAD, commit atomicity or rollback protection.
- FPC2534 paired-runtime SPI: explicit guarded reset release; 600µs CS-low wake delay and continuous frame transaction; 48-byte untrusted receive, IRQ nonblocking poll,32-frame budget, no ambiguous write retry. `send_identify` consumes a policy-minted encrypted request and checks its current request/deadline before and after write. See [review and missing production conditions](../../docs/FPC_RUNTIME_REVIEW.md). The post-call100ms check cannot preempt a stuck HAL; an independent watchdog is required.

All buses/GPIO and their DMA/memory attribution must be owned by the protected device domain. Do not expose these methods as host/vendor commands. A driver returning success never authorizes private access; that requires the real fingerprint policy, optional device PIN, complete on-device review and one-time grant consumption in the protected runtime.

Host tests inject I/O errors, malformed bytes, Flash boundary and verification faults, and key timing changes. The Cortex-M patch has separate tests of maintained critical-section nesting/thread exclusion/unwinding. Those host tests cannot validate IRQ masking on silicon, ADC thresholds, GPIO wiring, RF/EMC or physical attack resistance.

```sh
cargo +1.99.1 test -p openkeys-board --locked
cargo +1.99.1 check -p openkeys-board --target thumbv8m.main-none-eabihf --locked
```
