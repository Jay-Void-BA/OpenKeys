# Fixed H573 secure journal flash

This allocation-free Rust controller implements `root-journal::Storage` for
**physical bank2 sectors126/127 only**, each8192bytes. Secure addresses are
`0x0C1FC000..0x0C1FFFFF`; corresponding nonsecure aliases are
`0x081FC000..0x081FFFFF`. Both aliases must be excluded from every production
firmware, download/data slot, boot repair and updater erase range.

The [actual H573 port](../../firmware/secure-mmio/src/root_flash.rs) consumes
the unique FLASH owner, uses secure SECKEYR/SECSR/SECCR/SECCCR and has a fixed
register-value whitelist. The small audited unsafe boundary constructs only
aligned words inside those two fixed sectors. No caller pointer, arbitrary
flash range, nonsecure programmer, bank/mass erase, force-write, option/OBK key,
error clear or ECC clear is provided. Four ordered32-bit stores form one
16-byte phrase, with barriers and interrupts excluded during those stores.

Every operation requires fresh qualified closed hardware-key context, the
secure watermark and Secure+privileged attributes for both sectors, no bank
swap/EDATA remap and caches off/quiet. The captured context/layout cannot
change during the owner's lifetime. Caches are deliberately rejected rather
than assuming volatile reads bypass them; enabling a coherent cache strategy
would require separate implementation and review.

Programming checks virgin readback and the exact PG control, clears **old EOP
only**, writes once, requires a fresh completion with BSY/WBNE/DBNE all clear,
relocks and verifies every byte. All-ones/no-change phrases do not consume ECC.
Erase uses only the selected one of the two sectors and verifies the whole
sector. Any error/ECC flag, torn/false completion, readback mismatch or lost ACK
closes the owner. Errors are never cleared into success. Control is rechecked
before cleanup so an unexpected change cannot be silently repaired. An
unconfirmed lock or stuck write buffer causes a whole-device reset, without
forcing or retrying a partial phrase. Read failures erase tentative output.

The model cuts every port call before/after for read/program/erase and initial
construction, changes context at every call, and covers stale EOP, ECC, torn
phrases, incomplete last-byte erase, stuck buffers and invalid mapping/cache
state. Integration tests run the real journal over this controller, including
epochs, reopen, corrupt tails and a lost plan-seal ACK. Model HMAC/nonce/guard
fixtures are `cfg(test)` only. **No real flash, timing, cache, KDF or physical
fault resistance is qualified by these tests.**

## Not a deployed production Root

There is no production linker image/STiRoT slot configuration yet reserving
these addresses, or complete verified-boot/SAU/MPU/GTZC/DMA/watchdog/Root wiring.
`initialize_root_flash` states these prerequisites; reading option bytes does
not prove they were fulfilled. The OPEN/TZEN=0 public diagnostics never call
this port and cannot satisfy its hardware gate. Do not program these sectors
on a real key until the complete production boot/update layout is implemented
and independently reviewed. Whole-device destruction still needs SE, NOR,
capsules, pairing, protocol state and crash recovery; this store is metadata.

Physical qualification steps are in the
[security test instructions](../../manufacturing/ROOT_STORAGE_TESTS_R05_ZH.md).
