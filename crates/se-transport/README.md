# Bounded SE050 T=1-over-I2C

Own Rust `no_std`, no heap/unsafe/dependencies. Normal frames only: NAD5A/A5,
INF<=254, canonical I/R/S controls and CRC16/X25 (reflected8408, init/xorFFFF,
little-endian wire). CRC is error detection, **never authentication**. The
exact vendor/profile ATR is parsed without fallback defaults. An ATR never
provides trusted chip identity or an applet-version/security assertion.

One fixed5s operation budget includes I/O, polling, guard delays and WTX;
maximum32 received frames/4 WTX. A trusted clock must never move backwards.
Wait-extension factors never extend the total budget. This deliberately may
fail slow/unqualified devices. A watchdog bounds HAL calls that themselves
hang. The actual clock/watchdog/service adapter is still a runtime task.

R-ACK/error bits and both I-sequence directions are exact. An uncertain
partial read, data/unknown NACK, bad CRC/sequence, malformed ATR, timeout or
unexpected supervisory frame latches the owner; no private APDU is retried.
Only an **address** NACK before acceptance is retryable. Output is erased on
error and success is typed `UntrustedApdu` until SCP03 verifies it. The R04
port fixes I2C0x48 and PB6/PB7/I2C1; it does not enable SE_EN or authorize use.

Primary protocol reference: NXP nano-package
[`26d55b4`](https://github.com/NXPPlugNTrust/nano-package/tree/26d55b43225bc84de364057660cba676fa69f6e4/lib/t1oi2c),
Apache2. C source was read as a reference; none is compiled or linked.
Unit/integration negative tests are model evidence, not an actual chip trace.

The R04 target now uses a fixed Secure-alias I2C1 register implementation,
not Embassy0.6.0's ambiguous-NACK transfer API. Address busy is returned only
with zero data-register progress, empty FIFO state and confirmed STOP/idle;
all other errors latch. The generic embedded-hal adapter still rejects
Unknown NACKs. See [source review and chip tests](../../docs/SE_I2C_REVIEW.md).
This removes the missing driver implementation; physical SE timing and fault
qualification still require a real board and are not declared passed.
