# Classical R05: bounded extended APDUs

[Status](DEVELOPMENT_STATUS.md) · [C2 selection](CLASSICAL_SE_POWER_R05.md) · [Native PIN](NATIVE_PIN_RUNTIME_R05.md)

The Rust SCP03 and native-PIN session encoders now support extended APDUs.
This removes a transport blocker for RSA-4096 public keys and signatures.
It does not implement RSA, OpenPGP, PIV, commissioning or a production Root.
No private-key operation is moved to MCU software, and no host raw-APDU API
is added. Actual C2 Applet 7.2 interoperability remains NOT_RUN.

## Explicit bounds and encodings

| Layer | Hard limit / behavior |
|---|---|
| T=1 APDU | 2048 total wire bytes, chained through normal INF frames of at most 254 bytes |
| SCP03 plaintext command / response | 2015 bytes; mandatory ISO7816 padding consumes at least one byte |
| SCP03 maximum command | 2033 bytes: 7-byte header + 2016 padded bytes + S8 MAC + 2-byte Le |
| SCP03 maximum response | 2026 bytes: 2016 padded bytes + S8 R-MAC + 2-byte status |
| Native PIN PROCESS payload | Up to 1994 inner data bytes, including SID, canonical BER and inner extended header inside the outer limit |
| Resource / time limits | No heap; fixed arrays; 5 s total T=1 deadline, at most 32 received frames / 4 WTX; 4096 commands per SCP session |

`Command::le` distinguishes `Le::Short` and `Le::Extended`. Short zero means
256; extended zero means 65536 at the ISO7816 layer, still capped locally at
2015. If a long command forces extended Lc, short zero is encoded **0100**,
not 0000. An explicit extended Le forces extended Lc even for a short command.
The complete encoded Lc participates in C-MAC. Le does not participate in
C-MAC under SCP03; the client independently rejects an authenticated response
larger than the requested bound.

The native-PIN wrapper follows NXP `se05x_Transform`: SID TLV, TAG_1 with
canonical short / 81 / 82 BER length, then the embedded APDU with Lc and no Le.
An extended response hint selects the SDK's extended inner-Lc form. Overflow
is rejected before copying the SID/payload or sending I/O. The same native
PIN verification, durable debit/credit and typed private-key boundary remain
required for long and short commands.

R-MAC is checked in constant time before CBC decryption. Bare errors,
malformed lengths, bad tags, invalid padding, oversized authenticated output,
uncertain I/O and expired guards close the session. Partial private operations
are never replayed automatically. Returned plaintext and temporary buffers
are zeroizing; the Root still must protect stack, DMA, faults and peripheral
ownership. Fixed allocations reduce variability, but are not an ARM stack
high-water measurement or a side-channel qualification.

## Reproducible checks

```sh
cargo +1.99.1 test -p openkeys-scp03 -p openkeys-se-transport -p openkeys-se-state --locked
cargo +1.99.1 clippy -p openkeys-scp03 -p openkeys-se-transport -p openkeys-se-state --all-targets --locked -- -D warnings
cargo +1.99.1 check -p openkeys-scp03 -p openkeys-se-transport -p openkeys-se-state --target thumbv8m.main-none-eabihf --locked
```

Independent fixed transcripts are generated with the development-only
[OpenSSL script](../crates/scp03/tests/generate-fixtures.py), then checked in.
Normal Rust tests do not invoke OpenSSL or compile C/C++. AES-128 random /
pseudo and AES-256 transcripts cover consecutive short and extended commands,
512-byte replies, maximum payloads, both Le sizes and omitted Le. Additional
tests mutate every byte of the long response, enforce response limits,
reject overflow and exercise full 2048-byte T=1 command/response chaining
without resetting either sequence bit. Literal native envelope fixtures
cover BER and inner-Lc boundaries through the maximum payload.

SCP03 remains **S8, with 64-bit authentication tags**. Enlarging buffers,
AES-256 or a passing test suite does not increase that tag size. Device-wide
forgery budgets, session lifecycle, actual SE limits, watchdog timing,
power-loss behavior and side-channel/fault testing remain qualification tasks.

References: [GlobalPlatform SCP03 Amendment D](https://globalplatform.org/specs-library/secure-channel-protocol-03-amendment-d-v1-2/),
[NXP nano-package 26d55b4 SCP03](https://github.com/NXPPlugNTrust/nano-package/tree/26d55b43225bc84de364057660cba676fa69f6e4/lib/apdu/scp03),
[NXP plug-and-trust 7102413](https://github.com/NXP/plug-and-trust/tree/7102413e0457d3dfa4fc4be686ca8ec50bd9f5e2).
