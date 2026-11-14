# H573 secure MMIO boundary

This is an explicit exception to `forbid(unsafe_code)` in the other own-code
crates. Its `aliases()` function contains twelve fixed-address PAC handle
constructors, with no caller-selected pointer, memory slice, allocator, data
conversion or foreign code. No C/C++ is compiled. The constructors map the
documented Secure aliases of RNG, SAES, RCC, GTZC, RNG_NSCR, I2C1, FLASH,
SBS, TAMP, PWR, ICACHE and DCACHE. Hardware-key qualification uses readbacks only.
The exclusive journal port also writes FLASH secure control/key/EOP registers,
but never option bytes, HDPL/epoch selection or tamper/error/ECC flags. The selected
metapac21.0.0 has Non-secure constants and omits RNG_NSCR and SAES GTZC fields.
Moving a `Peri<RNG>` / `Peri<SAES>` into the ports preserves single-owner
runtime use; the whole privileged Secure program remains the trusted caller.

`root_flash::word()` is a second narrowly bounded exception: it constructs
only aligned32-bit words inside the two fixed8KiB secure journal sectors. The
unique FLASH owner is retained; no arbitrary pointer/reference or address is
accepted. Its four ordered stores are interrupt-excluded and bracketed by
barriers. Register writes have a literal whitelist. See the
[controller and limitations](../../crates/root-flash/README.md); verified
boot/OTA reservation, MPU/bus isolation and physical qualification are still
mandatory and are not established by these handle constructors.

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

DHUK CMAC additionally performs two fresh snapshots before/after each block
and after erasure, requiring CLOSED/LOCKED, locked STiRoT selection, TZEN plus
effective Secure SBS access, privileged Root HDPL2/SEC_EPOCH, completed
memory erasure, SRAM reset erasure/ECC, Secure+privileged crypto/TAMP and the
R05 external/internal tamper enables with immediate erasure. Debug attachment,
any pending tamper flag, potential/no-erase mode, forced-zero epoch, missing
RNG profile/clock or changing context rejects the result. This code never
clears a flag to regain a key. These readbacks cannot prove the running image
was authenticated, a sensor is functional or SAU/MPU/DMA attribution is correct;
the protected Root and real STiRoT/provisioning integration remain mandatory.

The independent-backup R05 profile additionally reads the actual PWR/RCC
security/privilege and backup-clock registers, active OUT4/IN1 mapping,
initialization/seed status, resource erasure and BKP8..25 continuity words.
PWR MONEN must be on, BREN and primary-cell charging off. All32 backup
registers must be Secure and privileged. Only live PRNG bits and LSIRDY are
normalised before lifetime comparisons; their configuration and fault bits
remain checked. Missing/corrupt continuity words and the old passive-IN1/IN3
profile deny hardware-key use. No initialization, reseeding, flag clearing or
backup write is implemented by this read boundary.

UM3125's STiRoT path authenticates the secure image then hands off at HDPL2
(encoded0x8A). This candidate Root profile remains at that level, without a
HDPL3/nonsecure runtime handoff. A future split profile needs a separate key
context/boot design. This module does not increment HDPL or substitute an
option-byte readback for STiRoT's authenticated jump.

I2C1 uses the fixed Secure alias0x50005400, consumes the unique initialized HAL/pin owner, and performs transactions through the bounded board state machine. Every register operation checks current GTZC Secure+privileged attribution, RCC enable and the captured kernel-clock selector. No HAL ambiguous-NACK transfer method is used. PB6/PB7 SAU/GPIO attribution, verified boot and a live clock remain root obligations; this port never enables SE_EN or authorizes private operations. See [SE I2C review](../../docs/SE_I2C_REVIEW.md).
