# Mobile manager integration targets

This is a specification, **not a working APK/IPA**. English UI and portable own
logic use Rust; native OS bindings, app signing and hardware tests remain separate
work. GPUI CE mobile source availability does not establish a working transport.

| Platform | Requested transport | Required API / qualification |
|---|---|---|
| iOS | NFC and USB-C | CoreNFC ISO7816 foreground sessions; USB-C CCID / CryptoTokenKit with smartcard entitlement, supported device and OS |
| iPadOS | USB-C | CCID / CryptoTokenKit and entitlement; no NFC claim |
| Android | NFC and USB-C | IsoDep reader mode and USB Host permission / CCID support; OTG and device tests |
| macOS / Windows / Linux | USB | Standard FIDO HID plus CCID / management AID design; verify composite interfaces and driver access separately |

iOS does not offer a general-purpose desktop USB HID API. The
[YubiKit iOS reference](https://github.com/Yubico/yubikit-ios) documents NFC and
USB-C CCID / CryptoTokenKit on iOS 16+, and the
[registration guidance](https://developers.yubico.com/Mobile/iOS/App_Registration_Process/)
distinguishes Lightning MFi requirements from NFC / USB-C. This is API precedent,
not confirmation that Apple's entitlement or arbitrary OpenKeys AID is already
approved. Xcode, the owner's signing team and actual devices are required.

A dedicated ISO7816 management AID must authenticate both device and paired host,
encrypt and sequence every message, bind version / transcript / peer / transport
epoch, reject replay/truncation and limit input sizes. UID, AID and USB descriptors
are discovery metadata, never authentication. SE-side SCP03 does not by itself
provide a phone-to-key secure channel.

Both local hardware PIN and separate protocol PINs use the
[30-second paired-session rules](../../docs/PIN_SESSIONS_R05.md). USB attachment,
NFC polling and keepalive do not extend them. Field loss, reconnect, suspend,
manual lock or a fault revokes immediately. Sensitive requests require a new
on-key confirmation. Standard third-party FIDO / CCID clients must not inherit a
manager lease. Phone biometrics or an OS PIN cannot replace the key's local PIN.

OTP import is bounded and encrypted; no seed export, raw MAC, calculate-all or
keyboard output. Six-digit codes appear only on the key LCD. Notes / recovery
lists and titles remain inaccessible before hardware authorization. Implementing
all this depends on the missing production device handlers and authenticated
channel, not just a phone UI or an NFC transceive call.
