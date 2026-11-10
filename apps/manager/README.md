> Historical R04 egui prototype. The current R05 UI target is [GPUI](../manager-gpui/README.md); neither has a production device backend.

# AuthDesk development build

English Rust/egui GUI for macOS, Windows and Linux. This is **not a usable
OpenKeys device manager yet**: `HardwareBackend` still refuses unlock and write.
There is no host-only unlock, OS biometric substitute, hidden mock switch or
default key. Never use this build for real recovery codes or credentials.

## Import privacy implementation

1. A verified hardware lease is required even to start import selection.
2. Starting selection revokes that lease, cancels an earlier read and clears
   private rows and the filter. Two masked UI frames precede native dialog
   creation. This also masks the window before a native backend's possible
   synchronous fallback; compositor presentation still needs platform testing.
3. The asynchronous picker returns paths only. No file contents are read and
   no path is displayed by AuthDesk while the picker is open or after it closes.
4. A fresh device PIN verification is required before a background
   worker opens the selected files. A path, picker completion or UI click does
   not manufacture a lease. Every private operation uses the ten physical decimal keys on R05,
   an import digest bound to the device request, and fresh
   physical confirmation; this backend/protocol is **not implemented**.
5. The one worker checks cancellation and both clocks between bounded reads,
   after parsing and before handing over a result. The UI accepts results only
   for the same live authorization generation. Lock, focus loss, expiry,
   backward wall time or a frame gap greater than five seconds revokes the
   lease and drops the result receiver. Old results cannot populate a new lease.

Limits: 1–16 regular files, **1 MiB JSON across the whole batch**, at most 256
notes/recovery entries. Final symlinks are refused on Unix; Windows opens and
rejects reparse points. Devices/directories are refused. An individual read
buffer reserves its bounded capacity before holding plaintext. Error messages
never include a filename, title, code or parser detail. Cancellation retains an
unfinished OS thread until it exits, preventing repeated imports from spawning
unbounded blocked readers. The UI does not join unfinished workers.

## Validation and remaining gates

`cargo +1.99.1 test -p authdesk --locked` exercises cancellation, aggregate
limits, regular-file checks, a real background fixture read, generation replay,
clock/focus/suspend privacy guards and locked rendering. All data is public
synthetic test content; test-only lease construction is not compiled into the
development executable. Clippy and rustc warnings are errors in CI.

Native builds and headless model tests do not establish actual window masking,
dialog behavior or OS suspend handling on the three systems. See
[M03 in the security test plan](../../docs/SECURITY_TEST_PLAN.md). Device
disconnect/USB suspend revocation and authenticated, cancellable asynchronous
device I/O must be integrated before activating the backend.

Host privacy has limits: cancellation cannot interrupt every kernel/network
filesystem call; a single cancelled thread can therefore remain blocked.
Input/entry strings are zeroized on drop on a best-effort basis. OS paging,
core dumps, parser temporaries, GUI text/texture caches and compositor memory
are outside that guarantee. This application cannot protect secrets from a
compromised host or erase a screenshot. The device must independently enforce
local PIN/request binding and never trust this host privacy guard as UV.
