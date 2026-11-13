# OpenSK source provenance

This Apache-2.0 subset is pinned to Google OpenSK commit
`2382394d4d61e170ef9ed1a2f7d39bf62e94b7d5`. `UPSTREAM.json` records the
**original** hashes, including unmodified licenses. Only the Rust `opensk` and
`cbor` libraries are integrated; upstream board firmware is not built.

OpenKeys modifies nine files, marked in their headers and recorded separately
in `PATCHES.json` and `OPENKEYS.patch`:

* `api/key_store.rs`: a mandatory derived-credential-secret operation permits a
  hardware key store to keep its hmac-secret master inside the secure element.
  The upstream software helper retains its equivalent HKDF implementation for
  tests; it is not a production secure-element adapter.
* `api/persist.rs`: signature-counter overflow fails instead of wrapping.
* `ctap/mod.rs`: use the derived-secret operation and provide a legacy U2F
  entry point whose user-presence state is cleared before and after one call.
  Only the OpenKeys protected processor may invoke this after consuming a
  request-bound local-PIN/confirmation authorization. The classical feature
  uses the Root's mandatory `LocalPin` interface and exposes no biometric
  enrollment. Public GetInfo skips private catalog/PIN reads.
* `Cargo.toml`, `env/mod.rs`, `env/test/mod.rs`, `ctap/storage.rs`: separate
  `openkeys-local-pin` from `fingerprint`; mixed profiles and raw CTAP debug
  logging are compile errors. The upstream test environment defaults to an
  unconfigured, refusing local-PIN provider.
* `ctap/client_pin.rs`, `ctap/pin_protocol.rs`: independently configured host
  PINs cannot be bypassed with local UV. Setting/changing a host PIN or failed
  verification invalidates earlier tokens. Revocation zeroes tokens and
  regenerates ECDH keys while preserving persistent/consecutive retries.

The active OpenKeys core enables `ctap1` and `openkeys-local-pin`, not
`fingerprint` or production software crypto. `std`/software crypto is used
only by synthetic tests. The real H573 Env, PIN sessions and native transports
are still integration requirements; these traits are not hardware proof.

Run `python3 scripts/check-opensk-vendor.py` from the repository root. Original
Google copyrights and Apache licenses apply to these files. The repository's
MIT license does not replace them. No Google endorsement or certification is
claimed. This hash check verifies source provenance, not security correctness.
The zero-context distributable patch is applied with `git apply --unidiff-zero`
from the pinned upstream tree; reconstruction was checked against every
modified source hash without changing the original hash inventory.
