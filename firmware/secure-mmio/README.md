# H573 secure MMIO boundary

This is an explicit exception to `forbid(unsafe_code)` in the other own-code
crates. Its one `aliases()` function contains five fixed-address PAC handle
constructors, with no caller-selected pointer, memory slice, allocator, data
conversion or foreign code. No C/C++ is compiled. The constructors map the
documented Secure aliases of RNG, SAES, RCC, GTZC and RNG_NSCR. The selected
metapac21.0.0 has Non-secure constants and omits RNG_NSCR and SAES GTZC fields.
Moving a `Peri<RNG>` / `Peri<SAES>` into the ports preserves single-owner
runtime use; the whole privileged Secure program remains the trusted caller.

Review sources: RM0481 Rev5 §2.3,36,38; AN4230 Rev15 table3 (H563/573/562,
48MHz: CR0x00F01E00, HTCR0x6A91, NSCR0x3AF66). ARDIS disables automatic
noise-error recovery, CED remains enabled and CONFIGLOCK is set. Every draw
checks profile readback and current/latched errors before/after DR. No
fallback entropy, test key, silent seed retry or error-clearing is provided.

`initialize` requires verified privileged Secure boot, configured TAMP and
Secure/privileged SAES/RNG attribution, HSI48 at48MHz and serialized RCC
setup before any task/interrupt uses the peripherals. It rejects missing
attributes or incompatible locked configuration. It does not implement boot,
fuse provisioning, memory/DMA attribution or tamper continuity on its own.
It is not invoked by the OPEN/TZEN=0 diagnostic. Invalid internal register
access or stuck key erasure resets the whole MCU; NRST independently blanks
LCD/disables SE. Verified boot must preserve the provisioned/fault state.

ARM compilation is checked. Register-model tests run in `openkeys-h573-crypto`;
real Secure-alias access, startup, clock/health-error injection, KAT, clear
timing and physical leakage are **NOT_RUN** and block production use. This
boundary needs independent review before the3keys hold any real secrets.
