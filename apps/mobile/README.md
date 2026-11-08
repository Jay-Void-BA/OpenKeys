# Android / iOS NFC manager feasibility

This is an integration specification, **not a working mobile app**. No APK/IPA or mobile device security test has been delivered. Product UI must be English; own portable logic should use Rust. Native OS bindings remain platform code that must be reviewed separately.

Android can exchange ISO-DEP data using [`IsoDep.transceive`](https://developer.android.com/reference/android/nfc/tech/IsoDep), with reader mode, bounded APDU size/timeouts and cancellation when the tag is removed. iOS offers [`NFCISO7816Tag`](https://developer.apple.com/documentation/corenfc/nfciso7816tag) and APDU commands; the app needs the NFC capability, usage description and declared application identifiers. These APIs establish a possible transport, not device authentication or guaranteed access to every FIDO/security-key AID. Validate the actual selected management AID, OS versions, session duration and NFC controller behavior.

A dedicated management application/protocol is required alongside standard FIDO. It must authenticate the device and host, encrypt and sequence every record, bind each operation to the current connection, and reject replay/truncation. Do not invent an unaudited crypto handshake or treat UID/AID/VID/PID as device identity. No plaintext NDEF notes, recovery codes, PINs or secrets.

Flow: start foreground scan → authenticate device channel → device fingerprint → optional **device-button** PIN → on-device request review/confirmation → one operation → revoke on timeout/removal/suspend. Phone biometrics do not satisfy the hardware fingerprint condition. Lists/titles/import previews require authorization; no automatic private refresh while locked. Large imports need resumable encrypted transactions, bounded storage and new confirmation for commit.

A usable implementation depends on the not-yet-implemented device APDU handler, authenticated channel and vault backend. iOS builds/signing also require Xcode and the user's signing team; neither a simulator nor this Linux environment can validate NFC interaction with an iPhone. Three-platform desktop GUI code does not imply mobile NFC support.
