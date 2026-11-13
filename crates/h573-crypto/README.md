# H573 bounded masked AES and checked entropy

The algorithm/sequencing core forbids unsafe Rust and has no HAL dependency.
Its production port is [secure-mmio](../../firmware/secure-mmio). It owns one
SAES peripheral and uses CPU polling, KEYPROT and no DMA/cipher interrupts.
Every wait is bounded, errors are terminal, failed output is erased and each
operation/drop uses IPRST to erase key/context state. Stuck erasure requests
a whole-device reset. Verified boot must preserve provisioned/fault state.

RM0481 Rev5 chapter38 defines the register contract: IVR3 contains the first
nonce word, IVR0 the low counter2; normal key registers may be written in
either monotonic order. GCM preserves EN across the final phase change,
uses the exact AAD/payload bit lengths and handles encryption padding with
NPBLB. Normal completion clears **CCF only**, never KEIF/RNGEIF/RWEIF. The
previous Embassy SAES cipher path is no longer used by the vault adapter.

ECB/CBC/CMAC and bounded GCM support AES128/256, maximum4KiB messages. GCM
decrypt produces **tentative** bytes for the private adapter to verify in
constant time before release. It is not an authenticated host decryption
API. Vault/FP callers need a fresh live authorization checkpoint as applicable.
All production keys are supplied through protected provisioning; none are
generated from public UID/default/test constants.

`dhuk_cmac` uses the non-exportable per-device256-bit H573 key. It generates
CMAC L with one CBC/zero-IV block and reinitializes the **same CBC context**
for the body; the usual ECB->CBC path is invalid here because CHMOD is an
input to the DHUK KDF (RM0481 §38.4.17). No KEYRx read/write or software
fallback occurs. Every block and final cleanup require fresh qualified
hardware context, pinned across the peripheral lifetime. Missing/tampered
context, debug, pending TAMP/dummy DHUK, mode/key-valid changes and failed
loads erase output and poison the service. Cleanup clears KEYSEL before
IPRST so no hardware key is reloaded. The Root journal's target adapter adds
a fixed domain and accepts only its fixed-size internal record body; no
manager, APDU or host MAC endpoint is provided.

The snapshot gate requires a very specific closed STiRoT/TZ/HDPL/epoch/tamper
profile. It neither installs that profile nor proves signed-image execution,
sensor operation, private-memory layout or every fault condition. The test
model substitutes a synthetic AES key only under `cfg(test)` and checks
CMAC with an independent RustCrypto oracle. It does **not** emulate or qualify
the silicon KDF. On-chip qualification is still required.

Checked entropy discards a baseline, checks consecutive32-bit repetitions,
retains the comparison across requests and erases partial output on failure.
Its source checks silicon current/latched clock/seed errors before/after DR;
no retry/reset fallback turns faults into a successful draw. Repetition
screening is not an entropy proof and does not detect every injected fault.

The register model uses separate RustCrypto primitives and NIST vectors to
check actual word order, phase/EN semantics, partial and4080-byte payloads,
key protection, cleanup, fault flags and timeout behavior. It is **not an
emulator of H573 silicon**. On-chip KAT, every error/tamper path, clock setup,
assembly and optimized-binary leakage/fault testing are NOT_RUN.

Masked SAES does not make the whole program side-channel resistant. CMAC
subkey/XOR operations, protected RAM/stack copies, comparison code, buses,
sensor classification and the complete root service need independent physical
evaluation. No software AES is linked by this crate's production build.

The shared `Service` owns SAES/RNG once. SCP03, vault and fingerprint
adapters borrow it per crypto call; no long-lived SE session holds an
exclusive hardware borrow. RNG/cipher failures or reentry poison all purposes
and erase failed output, including an outer operation which otherwise finished
successfully. Creating a new reference/adapter cannot clear poison. The root
still must revoke grants, blank LCD and handle physical fault/lifecycle state;
this component and its tests are not the complete protected root.
