# SE050 persistent state

Own `no_std`, allocation-free Rust. Implements actual SE050 object APDUs,
`device_vault::Ledger` and biometric `ProtectedAttempts`. Production exchange
is sealed to `ScpSession`, which uses the strict authenticated SCP03 client.
There is no ordinary Flash/RAM counter backend in a production build. Models
exist only under `cfg(test)`. The root must install real hardware crypto and
live guards; a Rust type or the seal is not a trust boundary against malicious
privileged firmware or a fake crypto provider installed by that firmware.

## Namespace and commissioning

[Public layout](../../manufacturing/se-state-layout.json) fixes five persistent
8-byte counters and three BinaryFiles. Open validates exact ReadType,
PERSISTENT=01, size and a policy/layout marker. Counter values use big-endian
encoding. Malformed/duplicate/trailing TLV data and unprotected/missing-object
errors never become zero or an empty vault. There are no runtime create,
delete, arbitrary-ID or factory-key APIs. Persistent IDs have explicit values.

`scripts/export-se-state-layout.py` exports public schema data only; it does
not communicate with hardware or handle keys/fuses. Its independent Python
marker is checked by a fixed Rust fixture. The marker binds the retry limit,
tag width, snapshot bound and IDs; it is not peer identity or an ACL proof.
Actual per-device ENC/MAC/DEK, SCP mandate, object policies, exact E2
personality, lifecycle and durable write behavior need controlled qualification.
Existing namespaces must not be overwritten by a commissioning script.

## Vault transaction

Reserve/increment the SE nonce counter before AEAD; never refund. Write and
read back the inactive SE head, then advance a separate commit counter as the
linearization point. That anchor selects exactly one head slot. The selected
record must have its exact sequence, valid domain/hash/fields and an epoch
not exceeding the reserved nonce. A bad selected head never causes fallback
to an older slot. A lost reply closes the owner; a trusted reboot reads the
actual anchor. A head checksum only detects torn data, not a compromised SE.

The code implements a fixed104-byte head and checks bank/epoch/sequence/size
invariants. Actual SE increment atomicity, persistence ordering, NVM wear and
readback semantics are assumptions to verify on the chip, not properties
established by the model. The transaction does not provide secure deletion of
old ciphertext, key rotation or an initialized-state recovery protocol.

## Fingerprint retry policy

Debit `BioSpent`, then reserve the never-reused `BioTag`, confirming both
before sending Identify. Every reservation binds the exact request/channel
digest. Only `AuthenticatedMatch` produced by device-policy's trusted frame
verifier can reach the credit operation; it must match the pending tag,
request and durable debit. Pending matches are single-use. Credit never
resets lifetime tags. A canceled/uncertain attempt still burns its budget.

The current engineering policy is **eight persistent consecutive failures**,
including canceled/uncertain debits. A successful authenticated match credits
its own debit. At the failure limit or65535 lifetime tags, new attempts stop;
neither power cycling nor a correct PIN clears that state. This can make a
device unavailable; destructive/maintenance recovery is **not implemented or
qualified**. Assess that availability policy before commissioning the3keys.
Changing the constant requires an explicit reviewed schema/migration, not
resetting counters. All counters are checked for impossible/observed rollback.

## Sharing and failure

`Service` owns one state and issues borrowed `StateRef` adapters for the vault
and fingerprint policy. Access is exclusive per call. Reentry poisons every
handle and cannot turn an outer success into a usable proof. The crypto
service similarly owns one SAES/RNG and uses short per-operation borrows,
so holding an SE session does not monopolize hardware needed by the vault/FP.
These services are internal to the protected root, never raw host RPCs.

## Evidence

Tests execute the actual APDU builders/parsers and state machine against a
separate SE model. They cut each APDU before/after execution, tear head writes,
lose commit/credit acknowledgements, corrupt/replay selected heads, reset
between attempts, exhaust budgets, substitute request digests, test schema
and counter faults, and compose the real vault core with AES-GCM and the
new ledger while replaying old NOR images. Fixed marker generation is
independent. Model cuts are not real chip power-cut or physical attack tests.

Still required: commissioned E2 hardware traces and cut tests, target I2C
NACK-progress driver, real sensor SPI/pairing, protected dispatcher/guard and
boot attribution, PIN adapter, full CTAP/management integration, plus SCA,
fault and EMC qualification. The diagnostic image enables none of them.
