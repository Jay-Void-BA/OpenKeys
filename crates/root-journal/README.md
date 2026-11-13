# Protected Root lifecycle journal

Allocation-free `no_std` Rust, with no production software MAC or ordinary
external-flash adapter. This is a tested state/persistence component, **not a
deployed production Root, boot implementation or whole-device wipe service**.

Two exclusively owned internal secure8KiB sectors each hold a256-byte head and
a256-byte write-ahead plan. A hardware MAC covers schema, record kind, physical
slot, device/board/SE-profile/policy context, sequence, user generation, random
owner nonce and previous-head digest. The16-byte completion seal is programmed
last, with readback. The unused sector area must remain erased.

Before erasing/replacing the other sector, the current sector receives a
complete authenticated plan. A complete pending plan can finish only its exact
journal transition. A torn/corrupt record or unused tail rejects the entire
journal: there is no fallback to an older `Ready`. Repair of that condition
needs a separate independently authorized full destructive recovery service;
losing the record is never permission to initialize a new PIN.

The state chain is `FactoryBlank -> PinInstalling -> PinInstalled ->
ApplicationsInstalling -> Ready`. A transition to `ErasePending` increments the
user generation and requires a fresh healthy hardware-RNG owner nonce. After
full verified destruction it may reach `FactoryBlank` in that same new epoch.
No `Ready -> FactoryBlank` shortcut, counter wrap, zero nonce or reuse of the
previous owner nonce is allowed. Every transition needs trusted Root authority;
no manager command is permitted to select a phase or assert erasure completion.

[`pin::PinRoot`](src/pin.rs) connects the actual SE PIN installer to those
committed phases. The installer gets its nonce only from `PinInstalling`; its
success commits `PinInstalled`, not whole-device `Ready`. It cannot call back
into a previous blank state or replace an initialized PIN namespace.

## Independent SE epoch binding

A MAC alone cannot detect replay of **both** old internal sectors. Production
must protect MCU flash/boot/debug and additionally bind all private use to
[native PIN v4](../../docs/NATIVE_PIN_RUNTIME_R05.md): an immutable SE owner
object must equal the authenticated journal's random owner nonce. The nonce
changes on a full wipe. `NativePin::open` rejects mismatches before a PIN guess;
its private port rechecks the owner with Root on every APDU. Do not learn the
expected nonce from the observed SE or accept it from the host.

User-data roots/capsules must also depend on the new, non-exportable SE secret
for that epoch. Persisting an old vault key wrapped solely by a stable MCU HUK
would undermine destructive-reset guarantees when an entire old MCU/NOR image
is replayed. The production vault/root/capsule adapters are not yet implemented.
Sequence numbers and MACs are not a substitute for that independent binding.

## Required hardware adapters

- Internal secure MCU flash only; reserve both sectors in boot/OTA/linker
  configuration and qualify16-byte programming, erase, ECC and readback. Normal
  code, DMA, debug and unsigned/rolled-back firmware may not modify the store.
- Real device MAC with healthy hardware key loading and stable qualified
  security/HDPL/epoch/key-mode context. Pending TAMP/dummy DHUK, debug, untrusted
  lifecycle and hardware faults must fail closed. The
  [H573 target adapter](../../firmware/target/src/root_mac.rs) now supplies
  domain-separated non-exportable DHUK CMAC through the bounded masked SAES
  service. Hardware readback gating and independent register-model tests are
  present; its required closed boot/profile and on-chip KDF/clear behavior
  are not yet integrated/qualified. It is not a deployed production Root.
- A Root guard tied to verified boot, exact device/SE identity, exclusively
  owned maintenance state, real power/time/fault facts and local authorization.
  HUK MAC generation and `state()` metadata do not authorize private operations.
- A complete reset coordinator for SE, external NOR, Root secrets/caches,
  pairing and all protocol state before it authorizes `ErasePending ->
  FactoryBlank`. `recover()` repairs journal metadata only.

Synthetic tests cut every read/program/erase before and after, simulate torn
phrases/erase, corrupt either head/plan/tail, change identity/key context, reject
invalid state changes and cover the native installer bridge. The HMAC test
oracle and deterministic nonce provider are compiled only under `cfg(test)`.
They do not qualify MCU flash, the HUK, SE ACLs or physical attacks.
