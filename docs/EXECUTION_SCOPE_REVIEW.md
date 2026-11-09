# Request-bound execution scope

The protected root consumes an opaque `Authorization` only before its 15s
use-by deadline. `Authorization::activate` produces a non-clonable `Execution`
for the **same operation, complete request digest and device-derived channel**.
An expired approval cannot be activated. No host packet can construct either
type, choose its duration or renew it.

Private reads/signatures/derivation have a fixed 15s execution budget. An
already approved NOR-changing operation, enrollment, pairing, reset or update
has a fixed 300s budget. This is a bound on one operation, not a period during
which other requests are unlocked. Registration needs the longer budget
because durable resident-credential storage can erase/program the NOR.
The budgets are engineering limits, not measured performance claims.

`Execution::check` compares the complete request, the root's current generation,
live secure time and latched security state before and after a possibly
blocking state check. A rollback, fault, cancellation, wrong request, generation
change or expiry permanently closes that execution. A continuation may only
shorten the original deadline. Generation `u64::MAX` is terminal: the policy
gate stops issuing authority instead of minting an unrevokable final epoch.

The FIDO processor transfers ownership of `Execution` to its protected platform,
checks generation and live time before crypto and after dispatch, drops late
responses, and calls `end_authorized` before returning. Its pre-crypto time
comparison uses the latest activation check, not an older outer timestamp.
Tests cover rollback during activation, revocation during dispatch and
registration not having occurred when the pre-crypto check fails.

## Required production wiring

The root must retain the execution only for this call and apply live checks
around every actual private read, SE/SCP exchange, crypto operation, NOR write,
commit and output. The vault `Checkpoint`, SCP `Guard`, key providers and
dispatcher must all refer to this same scope. The `Authorization` accessor is
for provider request classification; it is not a substitute for the live
execution guard and its later fixed deadline.

Cancel, disconnect, suspend, lock, tamper and relevant hardware faults must
revoke the root generation and blank private display state. End-of-call must
drop all scope state. A host-supplied clock, generation, PIN, Boolean fingerprint
result or permissive checkpoint is invalid. Blocking HAL operations also need
an independent secure watchdog; checks cannot preempt a driver that never
returns. CTAP keepalive/cancel and batched-import staging remain to be wired.

The current target image is diagnostic only. This policy module and processor
integration do **not** deploy production isolation, authentic sensor pairing,
USB/NFC management or the complete hardware `Env`. The human fingerprint/PIN/
full LCD review deadline remains 15s and still needs a usability review; this
change does not silently extend cached biometric approval. Physical fault and
side-channel resistance remain NOT_RUN.
