# OpenSK source provenance

This Apache-2.0 subset is pinned to Google OpenSK commit
`2382394d4d61e170ef9ed1a2f7d39bf62e94b7d5`. `UPSTREAM.json` records the
**original** hashes, including unmodified licenses. Only the Rust `opensk` and
`cbor` libraries are integrated; upstream board firmware is not built.

OpenKeys modifies three files, marked in their headers and recorded separately
in `PATCHES.json` and `OPENKEYS.patch`:

* `api/key_store.rs`: a mandatory derived-credential-secret operation permits a
  hardware key store to keep its hmac-secret master inside the secure element.
  The upstream software helper retains its equivalent HKDF implementation for
  tests; it is not a production secure-element adapter.
* `api/persist.rs`: signature-counter overflow fails instead of wrapping.
* `ctap/mod.rs`: use the derived-secret operation and provide a legacy U2F
  entry point whose user-presence state is cleared before and after one call.
  Only the OpenKeys protected processor may invoke this after consuming a
  request-bound fingerprint/confirmation authorization.

Run `python3 scripts/check-opensk-vendor.py` from the repository root. Original
Google copyrights and Apache licenses apply to these files. The repository's
MIT license does not replace them. No Google endorsement or certification is
claimed. This hash check verifies source provenance, not security correctness.
