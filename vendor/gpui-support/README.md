# GPUI CE 0.3 support forks

Published crate sources, with explicit local patches; original copyright and
licenses remain in every package. These are host GUI dependencies, not firmware.
The application remains an engineering prototype without a device backend.

[UPSTREAM.json](UPSTREAM.json) records original crates.io versions, archive
SHA-256 and download URLs. [patches](patches) are exact changes against those
archives; [manifest.json](manifest.json) binds the delivered sources and patches.
The advertised package VCS commit was not assumed to be a verified reachable
upstream Git commit. The published archive is the reproducible baseline.

Changes remove actual archived dependencies rather than suppressing RustSec:

- GPUI: remove unused `ttf-parser` (the implementation uses the existing modern
  font stack). Default image formats are restricted to PNG/JPEG/GIF/WebP; the
  unused AVIF encoder and its dependency tree are not needed for this manager.
- `metal` and `wgsl-rs`: use the maintained `pastey` macro implementation, aliased
  to the source's `paste` import. No copied or renamed archived `paste` source.
- Apple `block`: replace the uninhabited foreign static with the actual
  32-pointer Blocks ABI symbol, take its raw address, and spell the C ABI
  explicitly in Rust FFI declarations. Replace the unavailable published
  Objective-C test harness with pure Rust native invocation and retain/drop
  tests against the OS runtime. This removes actual future-incompatible code;
  the warning is not disabled. The package remains edition 2015 explicitly.
- Linux GPUI: upgrade ashpd 0.13 to 0.14 and oo7 0.6 to 0.7; adapt inhibit flags.
- AccessKit / AT-SPI / lockstep: migrate dependencies to zbus 6.2.1 and zbus_xml 6,
  using the maintained wire / names / derive implementations merged into zbus.
  Update Variant types, infallible builders, XML iterators and connection runtime
  ownership. Preserve intended signature-ignoring body decoding with its new
  `deserialize_ignore_signature` API. Object-server registration maps `None` to
  a newly installed interface. Unused legacy executor spawning is removed.

This increases local maintenance responsibility. It is not an audit of GPUI or
zbus, and it does not prove native accessibility / portal / keyring behavior on
every desktop. Canonical public D-Bus bytes, event roundtrip and wrong-body
rejection have integration fixtures in the manager; native builds and strict
Clippy are separate checks. These forks are scoped to the features used by this
application; unrelated upstream connection/test/mobile configurations are not
claimed to be qualified.

The excluded upstream build metadata is `.cargo-ok`, `.cargo-checksum.json`,
crate-local Cargo.lock and `.github`; original source and licenses are retained.
No dependency warning allowlist is introduced. Refreshing the source inventory
is a maintainer action after reviewing changes, not a way to bypass auditing.

```sh
python3 scripts/check-gpui-vendor.py
# Network: verify original archive checksums and apply every exact patch in temp.
python3 scripts/check-gpui-vendor.py --verify-upstream
cargo +1.99.1 audit --deny warnings
```
