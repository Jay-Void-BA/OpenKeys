# Protected FIDO core review

The Rust `openkeys-authenticator-core` integrates the pinned Apache-2.0 OpenSK
library, rather than implementing a new FIDO signature or hmac-secret scheme.
[Provenance and deliberate patches](../vendor/opensk/README.md) are separately
hash-checked. Upstream test/software-crypto providers are dev dependencies,
not a production hardware environment.

## Authorization boundary

Untrusted CTAP2 CBOR first passes an allocation-free resource scan: at most
7609 bytes, 512 nodes and depth16, with no indefinite encodings, tags, floats,
nonminimal lengths or trailing bytes. The canonical upstream reader then
rejects duplicate/out-of-order map keys and invalid UTF-8.

`Prepared` owns immutable zeroizing effective bytes. The request digest covers
the actual command after enforcing UV/UP, its operation and length. A host
cannot supply `uv=false`/`up=false` to lower device requirements. The complete
ASCII/Punycode RP ID is the trusted-screen identity; an RP display name is not
used as the authority. U2F exposes its application hash rather than inventing
a website name.

Only GetInfo, legacy VERSION and ClientPIN key agreement are public. Credential
usage counts are removed from public GetInfo. Host PIN enrollment, changes,
PIN-token requests and retries queries are rejected: the additional PIN is
entered only on the four physical buttons. GetInfo advertises alwaysUv and
does not advertise blocked host-PIN/config/large-blob APIs.

The processor consumes the opaque `device_policy::Authorization`, checks its
exact operation/digest/channel, activates a fixed request-bound `Execution`,
transfers that scope to the platform, rechecks generation/time/state around
dispatch, and revokes scope before returning. [Execution limits and required
root wiring](EXECUTION_SCOPE_REVIEW.md) are explicit. Late faults, rollback or
expiry erase the response. Every private provider must also make live checks
around its own blocking I/O; an outer check alone cannot undo an executed
signature or persistent write. Generation revocation and cancellation belong
to the protected platform and are not a host-supplied Boolean.

Each GetNextAssertion retains the original RP, channel, request seed, sequence
and original deadline, and needs a fresh fingerprint/confirmation authority.
A new command or public request clears that continuation. U2F user presence
is explicitly cleared before and after one protected legacy call, including
check-only requests. This prevents an upstream cached touch becoming a later
private-command grant.

## Implemented evidence

Core tests exercise actual OpenSK MakeCredential/GetAssertion, independently
verify P-256 signatures, test hmac-secret protocols1/2 with independently
constructed client ECDH/HMAC/AES-CBC messages, and reject altered salt MACs.
They cover UV/UP overrides, public metadata, malformed resource-bounded input,
two-account continuation/replay/channel changes, legacy U2F signatures,
wrong authorization and late platform/time faults. Their `TestEnv` and FPC
fixtures exist only under `cfg(test)` and prove software behavior only.

[SE key operations](../crates/se-state/src/keys.rs) implement in-chip P-256
generation with an explicit restrictive policy, public-key validation,
sign-prehash APDUs and independent signature verification. They implement
SE HKDF with `info=credRandom`, exact SHA256/salt/output parameters, without
reading the hmac-secret root. RFC6979 and fixed HKDF/APDU fixtures plus
bit-corruption tests are model evidence; exact E2 applet/ACL/KAT behavior is
still NOT_RUN. Public catalog metadata must be authenticated, slots durably
reserved and never automatically overwritten/reused. Slot capacity is bounded.

## Integration still required

Encrypted logical persistence is now supplied by `RecordPersist` over
`device-records`; it checks commissioned profile/AAGUID, rejects torn reset,
requires live scope checks, and uses a separate SE signature counter. It does
not create missing keys/profiles or perform a partial factory reset. Root
receives ownership of the opaque authorization while a private call executes.
The gateway intercepts CTAP reset and requires a root-owned reset transaction.

The production hardware `Env`, fallible hardware-key bridges for upstream
crypto traits, complete slot lifecycle, actual SE roots
and attestation provisioning, protected boot/runtime and USB/NFC transports
are not supplied by `TestEnv`. Manager hardware unlock remains disabled.
The diagnostic ELF is not an authenticator. Fingerprint management/reset also
needs a crash-safe lifecycle spanning sensor templates, SE roots and the vault;
an upstream FIDO reset must not orphan readable notes or reinitialize an
already commissioned device after a torn write.

This is an integration and source review, not FIDO certification, a hardware
side-channel result or an independent complete security audit.
