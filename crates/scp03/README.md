# Strict SCP03 channel

Own allocation-free Rust, AES128/256 SCP03 **S8** only. Every session requests
security level0x33: C-MAC/C-ENC/R-MAC/R-ENC. No downgrade, SCP02, software
production cipher, default key, fallback key version or automatic retry.
S8 uses64-bit wire MACs; this is **not**128-bit authentication. A session is
limited to4096 commands. Whole-device lifetime/active-forgery budgets and
rekey behavior need qualification; AES256 does not increase the S8 tag size.

Per-device ENC/MAC keys must come from controlled authenticated provisioning.
New host challenges require the checked hardware entropy source. INITIALIZE
UPDATE enforces key version, SCP03/i60 or70, exact lengths, card cryptogram,
and, for pseudo challenges, the complete selected AID/sequence binding.
EXTERNAL AUTHENTICATE has a status-only acknowledgement; it does not itself
authenticate application data or prove a fingerprint. The first private
result still needs a valid R-MAC. KDD/ATR is not trusted identity.

Every command advances the counter and full16-byte MAC chain. The response
MAC is compared in constant time **before** CBC decryption/unpadding. Only
authenticated9000 responses produce a zeroizing private response. Bare
errors, malformed replies, warnings62/63, invalid MACs and ambiguous I/O
close the client; they never become a trusted "missing object/empty vault".
Private command data is bounded to239bytes; larger objects require reviewed
application chunking, not an unbounded raw-APDU escape.

`client::Client` composes T=1 and SCP03 with required live checkpoints before
and after blocking crypto/I/O, including after decryption. A closed instance
cannot be reused. It borrows crypto for each transaction so the single SAES
owner can also serve the vault. Guards must be actual protected-runtime
policy/fault checks, not host Booleans. Internal fingerprint attempt/crypto
verification precedes a match; that phase never authorizes arbitrary private
operations. A channel alone is not a user grant.

The selected application, full AID, provisioned key version and applet profile
must be qualified beforehand. This SE050 client requires applet>=4.3.0 for
the all-APDU counter behavior. It does not infer that fact from untrusted
version bytes or claim a chip was qualified. Actual E2 commissioning traces,
SCP-only mandate/ACLs, key rotation and recovery remain required.

Tests use public synthetic keys and fixed transcripts generated independently
by `tests/generate-fixtures.py` with OpenSSL3.5.7; normal tests do not execute
OpenSSL. They cover AES128/256/random/pseudo handshake, three consecutive
commands including empty/full-boundary payloads, NIST CMAC, downgrade,
replay/reorder/bit mutation, bad padding, primitive faults, authorization
expiry and length/error ambiguity. Software AES/CMAC is `cfg(test)` only.
Hardware SAES/SCA/fault/interoperability testing remains **NOT_RUN**.

References: GlobalPlatform AmendmentD SCP03; NXP nano-package
[`26d55b4`](https://github.com/NXPPlugNTrust/nano-package/tree/26d55b43225bc84de364057660cba676fa69f6e4/lib/apdu/scp03).
No C/C++ source is compiled or included as a runtime implementation.
