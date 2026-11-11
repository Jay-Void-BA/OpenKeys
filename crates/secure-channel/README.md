# OpenKeys management channel core

Rust `no_std` + `alloc`; no native OS or C/C++ dependency. This is a protocol
implementation, **not a completed H573 runtime or a connected GPUI backend**.

The only suite is `Noise_XX_25519_ChaChaPoly_SHA256`. XX has three empty-payload
messages of 32, 96 and 64 bytes. There is no suite negotiation, password-based
handshake, plaintext fallback, raw APDU forwarding or exported transport key.
The low-level Noise states, clone, rekey and split APIs are private.

The root supplies checked fresh hardware entropy, its independently protected
identity, trusted monotonic milliseconds and a native USB/RF attachment epoch.
Managers supply OS CSPRNG/keystore and a native connection epoch. Epochs are
local, not packet fields. Ephemerals additionally mix role, identity, transport
and epoch into fresh entropy; this is a hedge, not permission to reuse entropy
or reset epochs in a live context. X25519 rejects non-contributory peers.
Chaining/hash/cipher keys, hash core/buffer and HMAC pads use zeroization.
Zeroization does not establish resistance to power/EM/fault attacks.

Completed XX returns a `PairingCandidate`, with no record or PIN-lease API.
Known peers require authenticated storage lookup. New key-side pairing consumes
an exact `device-policy::Authorization` and requires persistent readback.
Its request binds both identities, transcript and transport. Compare all 20 hex
characters of the 80-bit pairing comparison on manager and key; the key must
collect a fresh local hardware PIN and release confirmation. Host consent can
only pin a device on the host; it cannot authorize a host on the key. Neither
the comparison nor a paired connection verifies a PIN or grants a private action.

Records authenticate the full 16-byte header plus the 32-byte XX transcript:

| Offset | Bytes | Meaning |
|---|---|---|
| 0 | 2 | `OK` |
| 2 | 1 | Version 1 |
| 3 | 1 | Sender: manager 1, key 2 |
| 4 | 8 | Big-endian ordered sequence, starts at 0 in each direction |
| 12 | 2 | Big-endian plaintext length, 1–17,408 |
| 14 | 2 | Reserved, must be zero |
| 16 | length + 16 | Noise ChaChaPoly ciphertext and tag |

Bad tag/header/order/length, backward time, changed native epoch/transport or
expiry destroys both cipher states. Independent directional sequence numbers
prevent reflection/reordering/replay. Channels expire after 10 minutes, 65,536
records per direction or 16 MiB per direction. Rehandshake must revoke old
hardware/protocol PIN leases. Receiving an authenticated record does **not**
renew the separate 30-second PIN leases; only checked successful operations do.

`apdu` supplies one bounded transport codec for USB CCID and NFC ISO-DEP, with a
proprietary-use AID, short APDUs, 240-byte fragments, strict offsets/sequences,
one pending message, 5-second fragment deadlines and close-on-error. SELECT,
including reselect of the same AID, revokes handler state. Native removal/reset/
suspend/field-loss paths must also reset the endpoint and lock Root immediately.
Transport `9000` is not authenticated approval: application results remain AEAD.

`wire` defines bounded canonical requests. It has no hardware-PIN submission,
OTP calculation/export, seed export or SE command. Private parsing and policy
checks still belong to Root, including every list/title and notes/recovery read.
`Status` and `Lock` are not evidence of activity and must not renew PIN leases.

Tests use independent Snow interoperability for the XX transcript **and both
AEAD directions**, byte mutations, malformed APDUs, maximum-size encrypted
fragmentation, replay/reflection, native epoch changes, expiry, failed keystore
readback and wrong/stale local pairing authority. Synthetic loopback exists only
under `cfg(test)` and is never a device/manager production backend.

Still required: native transports/keystores, paired-identity commissioning,
H573 Root dispatch and protected time/link/lease wiring, management responses,
GPUI/mobile integration, hardware traces and an independent security review.
