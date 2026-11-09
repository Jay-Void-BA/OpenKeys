# Encrypted logical records for R04

Own `no_std` Rust with bounded allocation. `Records` owns one `device-vault`
and an index of record offsets/lengths, not all plaintext values. Every index,
list, read and edit requires the root's live `Checkpoint`. The root must bind
it to the exact consumed fingerprint/PIN/request/channel and must not expose
the internal record API as raw USB/NFC commands.

## Format and limits

Encrypted snapshot plaintext begins with32 bytes: `OpenKeysStoreV1\0`, count
(u32BE), positive revision(u64BE), and4 zero reserved bytes. Each record has
namespace(u16BE), key(u16BE), length(u32BE), then its bytes. Records are strictly
sorted by(namespace,key), with no duplicates, unknown namespaces or trailing
bytes. At most4096 records fit the index; the entire snapshot remains≤1MiB.

FIDO keys use0..4095 and values0..1023 bytes, including present empty flags.
Notes/recovery use nonzero u16 IDs, at most256 items combined, and a maximum
16896-byte encoded value. Separate typed accessors prevent a private-note
command being used as an arbitrary FIDO store read/write.

`item` is the host/device canonical value codec. Its prefix is version1,
kind1(note)/2(recovery), title length(u16BE) and UTF8 title. A note then has
body length(u32BE)+body; recovery has count(u16BE) and repeated
code length(u16BE)+code. Titles≤256 bytes, note body≤16384,1..64 codes≤256 bytes
each. Empty fields, forbidden controls, every Unicode Bidi_Control, malformed
UTF8/lengths, wrong namespace/kind and trailers are rejected. Device parsing
borrows the zeroizing backing buffer without allocating copied strings.
`vault-import::Entry::canonical_record` uses the same codec; the device still
validates received bytes independently.

## Streaming, authorization and failure

Only one bounded plaintext sector is cached while scanning headers. Edits
stream a new header, old prefix, changed record and old suffix into the
inactive encrypted bank. They compute the exact source digest, recheck it
during replacement, verify readbacks and then commit the protected SE head.
No duplicate flash owner or1MiB plaintext allocation is needed. Values and
owned scratch buffers zeroize on drop; cached private index fields zeroize
on lock. Callers own output-buffer/UI/OS clearing responsibilities.

Any ambiguous read/write/format failure closes the affected owner. A private
read fault also closes cached metadata; locking cannot clear that failure.
Late cancel can suppress an acknowledgment after a durable commit, but cannot
undo a committed edit. Records are not a cryptographic secure-delete scheme:
inactive ciphertext can remain and needs separate key lifecycle design.

The FIDO adapter is `openkeys-authenticator-core::persist::RecordPersist`.
It requires an explicitly commissioned profile/AAGUID and separate actual SE
signature counter; it neither initializes missing profiles nor performs a
partial per-key factory reset. Ordinary signatures do not rewrite all notes.

Tests cover mixed operations against an independent map, encrypted remounts,
all modeled edit-I/O cuts/unknown commits, namespace and capacity bounds,
malformed but cryptographically authentic records, scope loss, late faults,
index closure and canonical note/recovery parsing. Hardware integration,
bounded long-operation authorization, production boot, staging/bulk import,
actual flash/SE power cuts and physical security qualification remain required.
