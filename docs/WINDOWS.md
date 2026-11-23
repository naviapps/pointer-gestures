# Windows SDK development

Recognition and rendering share one Cargo workspace and one build/package driver.
Run these commands from the repository root. Both libraries remain independently
consumable; rendering uses recognition's shared values. Fetch the locked registry
dependencies (`cargo fetch --locked`) before offline builds. Tools are preinstalled;
the driver starts no VM, uses no hosted CI and publishes no package. It sets
`RUSTUP_AUTO_INSTALL=0` for child commands to prevent implicit installation.
The optional cargo-xwin backend manages its Microsoft CRT/SDK cache separately.

See the [SDK overview](../README.md) for consumer installation and portable API
examples and the [development setup](../README.md#development-and-acceptance)
for pinned tools and offline use. Run source commands with the
prepared tools on `PATH`, or prefix them with `mise exec --`.

## Requirements and validation boundaries

On Windows, clone the repository with `git clone -c core.longpaths=true <repository-url>`.
The included macOS frameworks have long paths; this repository-local Git setting
avoids changing Windows policy. Package-consumer checks use the system temporary
directory so a deeply nested checkout does not lengthen NuGet's extracted paths.

- The Windows SDK runtime is `win-x64`. Install native Python 3.11+, the pinned
  host Rust toolchain and its `x86_64-pc-windows-msvc` standard library.
- Select the pinned x64 .NET 10 SDK through an absolute `DOTNET_BIN` path on
  ARM64 Windows. Consumers require a [supported .NET operating system](https://github.com/dotnet/core/blob/main/release-notes/10.0/supported-os.md).
- Select host-native MSVC tools for the x64 target. The Windows ARM64 acceptance
  host uses ARM64 MSVC and executes x64 test programs under emulation.

Examples use `python3`. In an MSVC developer shell where that name is unavailable,
use the prepared `python` executable or `py -3` for the same module commands.

On a Mac, `python3 -m tools.windows check` checks the x64 Windows Rust target, C layouts
when Clang is present, and the portable managed build. It does not link or run
Windows binaries. The PE inspector's synthetic fixtures test malformed files,
architecture, exports and dependencies; they are never shipped as DLLs.

```sh
python3 -m tools.windows check
python3 -m unittest discover -s tools/tests
```

`make check-dotnet` separately
executes the host-native bridge, including geometry/recognition recovery and
presentation state, and does not prove Windows runtime behavior.

## Native desktop rendering

`pointer-gesture-rendering-windows`, the C header
`windows/pointer_gesture_rendering_windows.h` and .NET
`NaviApps.PointerGestures.Rendering.Windows.WindowsOverlay` expose the optional
resource adapter. Creation validates appearance but starts no window or thread.
`Activate` creates a hidden, topmost, layered, click-through, nonactivating window
on a dedicated per-monitor-DPI-aware thread. Only that thread's DPI context changes;
no host/global DPI policy, input hook or permission is installed.

Coordinates and width use physical virtual-desktop pixels, including negative
origins. Convert host view coordinates at its boundary. Rust plans the same live
geometry, clips its padded raster rectangle to the current virtual desktop and
rasterizes with premultiplied alpha. Native display/DPI notifications reclip and
repaint the presented geometry without rescaling physical coordinates. It supports at most 4096 directions, 16384
pixels per raster axis and 16 million raster pixels. Excessive/unrepresentable
geometry returns an explicit error; native presentation failure hides stale
geometry. A failed worker preserves its terminal error for the next caller;
deactivate to join it before reactivation.

The bounded, synchronous command boundary must be called outside input hooks.
The worker owns `OverlayState` deadlines and waits on a kernel event/native messages,
without idle polling. Deliver recognition `Publish` effects in order and clear
before delivery when `DiscardPendingVisibleTrace` is true. Null trace preserves
the shared final-frame cadence; `Clear` discards pending geometry immediately.
`Deactivate` hides, destroys the window/class and joins, and is idempotent.
Disposal also joins but cannot report failures; deactivate explicitly when needed.

Reactivation starts fresh resources. `Snapshot`/C view returns submitted visibility
and bounds plus a borrowed HWND valid only while the owner is active; worker failure also invalidates it. It is not a
compositor completion fence; never mutate/destroy the window from the host.

The owned WinForms acceptance fixture captures the native overlay on screen,
checks styles/foreground ownership and injects a click into its own canvas through
the visible stroke. It also checks autonomous hiding, cancellation and repeated
window teardown. These are VM/native checks, separate from physical mice, mixed-DPI
multiple displays, HDR and secure-desktop behavior. Portable geometry and raster
unit tests do not prove those native conditions.

### Reconfiguring overlays

`WindowsOverlayConfiguration` contains host-selected color, physical stroke width,
rounding and cadence. Construct `WindowsOverlay` with that value and call
`Configure` to update it. Configuration before activation allocates no window;
configuration while active repaints the same HWND, preserves pending geometry and
recomputes deadlines from the last presentation. Invalid appearance leaves the
previous configuration and resources intact. `OverlayState.Configure` exposes the
same Rust timing update for hosts that own their own UI. The portable rendering
version is declared by `POINTER_GESTURE_RENDERING_ABI_VERSION` in the portable header;
the optional Windows overlay declares `POINTER_GESTURE_RENDERING_WINDOWS_ABI_VERSION`
in its Windows header. The managed adapter checks both versions. Use matching
managed and native assets from the same verified package.

## Native input recovery

Native input hooks stay on their message-loop owner. A separate ordered worker
submits replay batches while the owner continues dispatching hook messages.
This also handles a compound move/button packet whose later button event arrives
while an earlier replay is still in flight. Delayed button-down targets are
revalidated immediately before native submission. Normal stop drains required
recovery before removing hooks and joining both workers.

Managed recognition reports native runtime failures as `WindowsRecognitionException`.
Its `Failure` distinguishes duplicate capture, OS failure, incomplete recovery,
consumer lag, worker termination and a changed replay recipient. `NativeErrorCode`
retains the system code; `AcceptedEventCount` and `RequestedEventCount` retain partial
recovery progress. Configuration, disposal and wait cancellation keep their standard
.NET exception types. Hosts select recovery/restart policy from these fields, never
from message text. A lagged reader still drains final observations before the
terminal failure; no recovery is retried into a replacement recipient.

Observation buffering is bounded: 256 ordinary frames plus two reserved closing
slots. Overflow stops capture immediately, recovers input, and retains the
triggering observation and final shutdown frame in order before reporting
consumer lag. Hosts must drain promptly; no native hook waits on their processing.

## Native Rust diagnostic evidence

`Recognizer::set_diagnostics_enabled(true)` enables a preallocated 64-record
atomic history; it is disabled by default. `diagnostic_reader()` returns a cloned,
readonly `DiagnosticReader` whose `snapshot()` does not schedule worker commands
or wait for a callback. The reader remains usable after recognizer teardown.
`Recognizer::diagnostics()` copies the same snapshot directly.

Records retain button-down/up and move callback entry, final forwarding/suppression
result, packet timestamp and age, callback duration, physical/captured/logical input
state, keyboard forwarding, observation publication, replay submission and accepted
counts, command entry/completion, and hook/worker teardown. Replay records include
their retained `delivery_epoch`
separately from the current input state's epoch. No coordinates, paths, input text
or host context contents are retained. Only the native owner thread writes records;
the hook adds no diagnostic allocation, lock, I/O or retry loop. Copying a snapshot
allocates only on the calling thread.

`records_written` counts enabled reservations, `overwritten` counts history older
than the retention window, and `unavailable` counts retained reservations omitted
because publication or overwrite raced the snapshot. Snapshots are bounded copies,
not an input-delivery fence. A record's elapsed clock and the snapshot's current
clock can identify an unfinished callback or cleanup phase; absence of a callback
alone does not establish why Windows passed an event. Synthetic input can supply
timestamps, so event age is evidence rather than a timeout verdict. Successful
native submission records report `SendInput` acceptance, not recipient delivery;
owned receiver hooks must establish actual event provenance and delivery.
Submission start/completion records bracket the ordered worker request and the
owner's message dispatch as well as the native call; they are not isolated API
latency measurements.

## Native Rust press context

`Recognizer::start_with_context` accepts a `FnMut(PressContext) -> SessionContext`
resolver. Before each eligible configured button-down, the adapter supplies the
physical press position, the native input recipient's process ID (including native
capture), and the foreground window's process ID when available. These IDs are
lookup hints; hosts validate process identity and choose application/profile policy.
The SDK does not interpret executable paths or host configuration.

A repeated configured button-down while that physical press remains held cancels
its recognition and is consumed until the balancing button-up. The adapter keeps
the original recipient, host epoch and recovery metadata, and does not call the
context resolver or start another recognition generation for repeated downs.
Pending input restores its original native down at most once; active recognition
keeps the remaining release hidden. A fresh press after the up can recognize normally.

The resolver runs synchronously inside the low-level hook. It must finish promptly
using bounded native identity queries and immutable lookups. Do not perform UI,
IPC, blocking locks, filesystem/network work, actions, or recognizer/overlay calls.
Return a prebuilt O(1)-cloned `Matcher`, a `Policy`, and an opaque host `epoch`.
A resolver panic or invalid modifier mask stops capture and passes the new original
press through. Existing input recovery still completes through the adapter.

Use `set_context_resolver` to publish a closure owning a new immutable host snapshot
on the adapter thread. The engine captures matching and policy at button-down;
changing future registrations, enablement or the resolver cannot redirect a held
gesture. Resolver policy is authoritative at the next press, including enablement,
so a contextual host publishes pause changes through the resolver too.

`try_read_with_context` drains `ContextReadResult::Frame(ContextOutput)` from the
same queue as `try_read`. Each frame carries the epoch selected for its press through
release, cancellation and shutdown. Before the first resolved press, the epoch is
absent. Hosts retain immutable command tables until all associated observations are
drained, and execute effects outside the hook in their original order. Queued older
frames retain their epochs even after a new resolver or press. Use `start` and
`try_read` for basic recognition without an epoch.

### C and .NET press context

C hosts use `pointer_gesture_recognition_windows_start_with_context` and
`pointer_gesture_recognition_windows_set_context_resolver` to install resolvers.
In the callback, call `pointer_gesture_recognition_windows_select_context` once
with the selected matcher, policy and epoch. The SDK clones the matcher before the
call returns. The selection handle belongs to that callback only. Keep callback
data valid until successful replacement or until stop joins the worker.
`pointer_gesture_recognition_windows_read_with_context` delivers the owned frame
and an optional epoch. As with the original reader, destroy the frame after copying.

Managed hosts prebuild `WindowsSessionContext` values from their matchers and pass
a bounded resolver to `WindowsRecognizer.StartWithContext`. Each context owns a
matcher snapshot independently of later mutation or disposal. Use
`SetContextResolver` to publish matcher or enablement changes for later presses.
The resolver controls context-enabled recognition on every eligible press.
`ReadWithContextAsync` and `TryReadWithContext` deliver the copied output and
nullable host epoch. An epoch of zero is valid. Keep each host command table until
its observations drain. Consume either the basic or contextual reader serially.
The async reader checks cancellation after waiting for any concurrent managed
native command to release the gate. Cancellation observed there leaves the queue
untouched. Once a read dequeues a frame, it returns that frame and its epoch.
Exceptions from the callback stop capture and surface as `WorkerStopped`. Invalid
policy selections fail with the standard configuration error. Replay recovery
and observation ordering remain owned by the Windows adapter.

## Build and inspect Windows x64

The SDK's build checks cover its header layout, static-runtime import policy,
C/.NET consumers and native acceptance fixtures.

From native ARM64 Windows Python, select the x64 targets of the host-native
Rust and MSVC toolchains. For managed execution checks, point `DOTNET_BIN`
to the pinned x64 executable.

```powershell
python -m tools.windows build
python -m tools.windows verify
```

The build emits both SDK libraries for `win-x64`. Keep the full RID directories
with the same SDK revision when moving candidates between hosts.
Use the original immutable SDK checkout to verify a historical ARM64 build.

Outputs are under `.native-build/windows/<recognition|rendering>/win-x64/`: DLL, import library, public C
headers, license/runtime notices and `native-build.json`. Each build uses the pinned
compiler, locked/offline Cargo dependencies and static CRT. The tool checks PE
architecture, exactly the public C exports, and system-only imports before replacing
a previous candidate. Missing tools/targets fail without an installation attempt.
The receipt identifies sources, immutable dependency sources, compiler and output
hashes. `verify` detects source/output drift without executing the DLL.
Receipt paths use forward slashes on every host, so a copied RID directory can be
verified against the same clean SDK sources on Windows.
No Windows support claim follows from producing a receipt alone.

### Cross-link on macOS or Linux

With cargo-xwin and LLVM already installed and on `PATH`, the same build and PE
inspection work without a Windows VM:

```sh
python3 -m tools.windows build --cross --rid win-x64
```

The cross route uses `cargo xwin build` and records its version in each receipt.
LLVM is keg-only on Homebrew; add its `bin` directory to this command's environment,
not to global shell configuration. See [cargo-xwin](https://github.com/rust-cross/cargo-xwin)
for installation and Microsoft SDK license terms. Its first run can download the
CRT/SDK; `XWIN_CACHE_DIR` selects a reusable cache, defaulting to
`.native-build/windows/<recognition|rendering>/xwin`. Its `XWIN_SDK_VERSION` and `XWIN_CRT_VERSION` controls
can pin those inputs. No Windows executable is run by this route. Rust does not
officially support MSVC cross-compilation from a non-Windows host; native testing
below remains required for deployment.

On the owned Windows host, run from the checkout used to build the libraries:

```powershell
python -m tools.windows test-native
```

The driver checks the x64 PE machine of the C caller and staged DLL before
executing the caller and the x64 Rust tests. The receipt identifies the host
architecture, target architecture and execution mode.

For recognition input, use the additional opt-in command **only in an owned,
isolated, unlocked Windows desktop**:

```powershell
python -m tools.windows test-input
```

It builds/runs the maintained Rust `desktop_probe` and managed adapter fixture.
The Rust probe starts a separate, uniquely owned receiver process with private pipes.
Windows `SendInput` exercises all five configured buttons, matched/rejected gestures,
cancellation, Escape, modifier preservation, expiry, observation overflow and
64 start/stop cycles with a native-handle check. Replay ordering covers button-only
releases, compound move/releases at unchanged and changed positions, and a single
batch containing separate move/release inputs. Each mode
uses eight cancelled presses followed by fresh matching gestures. The example's
`--held-release-stress` mode runs only this matrix, with 32 cycles per mode.
The receiver also checks replay coordinates and native mouse capture outside its
window. Covered, hidden, moved or terminated recipients must not redirect a
retained press into another window;
`ReplayTargetChanged` is the terminal result. The target check is best effort before
`SendInput`, which does not atomically address a window.
The probe installs global hooks during the check; do not run it in a desktop
containing unsaved user work. It does not test physical hardware, secure desktops,
elevated windows or product commands.
The managed test flag `--windows-input` is similarly opt-in. Default core/package
checks never install input hooks.

### Machine-level acceptance

The built `desktop_probe.exe --acceptance <owned-control-directory>` adds a manual
controller protocol after the normal input regression. Use a disposable unlocked
guest, hold its exclusive execution lease and restore its baseline afterward.
The directory must be new, private to this run and empty of `resumed.txt`.

1. At `stage.txt` = `external-escape`, deliver an Escape down/up from the VM's
   virtual keyboard. A passive hook must observe both events without
   `LLKHF_INJECTED`; the receiver must receive neither, and the pending click
   must recover. Sending another `SendInput` key cannot satisfy this check.
2. At `stage.txt` = `suspend`, suspend and resume that same owned VM, then create
   `resumed.txt`. The pending click must finish balanced in the same receiver.
3. The fixture temporarily selects an enumerated alternate display resolution,
   verifies replay coordinates, restores the original mode, and runs 256
   alternating click/match sessions. Results include both resolutions, elapsed
   time and match count. No persistent display configuration is written.

Each wait is bounded. Reap the owned process and restore the guest even on failure.
Hypervisor suspend/resume is separate from Windows system sleep; a virtual
keyboard is separate from physical devices. These checks do not establish
multi-monitor or mixed-DPI hardware support.

## Native host gate

GitHub Actions uses a Windows x64 runner. For x64 emulation on an owned Windows
ARM64 machine, run:

```powershell
python -m tools.native_host --architecture arm64
```

The gate builds both x64 SDKs, runs C/Rust/managed tests and fresh-cache
NuGet consumers, and compiles the WinForms example. Pass `--desktop-input` in an owned Windows desktop
to exercise the native input receiver and WinForms drawing and click-through.
The Windows receipt distinguishes x64 emulation from native x64 execution.
The macOS entry point selects the matching macOS runtime.
The gate checks that the retained notice index matches the checked source.

## Pack local candidates

Build and verify x64 from one source revision, then pack both libraries with
the same explicit prerelease version and feed.

```powershell
python -m tools.windows pack --version 0.1.0-local.1 --output C:/pointer-feed
```

The tool validates source/output receipts and PE metadata before packing. Direct
NuGet packing is disabled by default. These are local development candidates;
existing candidate files are not replaced. Each package contains one managed API with XML documentation,
DLLs for the explicitly selected RID, and applicable third-party notices.
Rendering depends on the recognition package at the candidate version. No Rust,
Python, Visual Studio or repository access is needed by the installed consumer.
Use [`tools.nuget pack`](../README.md#local-net-package-checks) with repeated `--rid`
arguments when a candidate should include Windows x64 and macOS runtimes.

## Check installation without Windows execution

After packing both packages, run on a development host with .NET 10.

```sh
python3 -m tools.windows check-package --rid win-x64 --version 0.1.0-local.1 --feed /path/to/pointer-feed
```

This restores only from the local feed into an empty cache, compiles the maintained
consumer through `PackageReference`, and verifies exact package versions, XML API
documentation and selected native DLL bytes, including transitive packages. It
creates no Windows executable app host and runs no Windows code. The .NET SDK's
installed reference packs are sufficient; external feeds are not a fallback.

## Verify native package loading

On Windows with the approved tools and the x64 .NET SDK:

```powershell
python -m tools.windows test-package --version 0.1.0-local.1 --feed C:/pointer-feed
```

This creates a clean consumer with `PackageReference`, an isolated empty NuGet
cache and a local-feed-only restore. It builds and runs the maintained managed
interop fixture without `ProjectReference` or a source-native search path. The
result records the exact package and test-source hashes. It also checks that a missing
native library stays missing when a same-name DLL exists only in the working directory or `PATH`,
that DLLs beside their managed assembly load, and that an explicit host resolver can
select a trusted custom location. These probes use the installed managed assemblies
and inspected package DLLs. Keep both packages in the feed for rendering's dependency. The x64 .NET SDK/runtime must
already be installed; network package feeds are not a fallback.

The native C/Rust tests and package round-trip establish deployment acceptance only
for the tested RID. Desktop input adds the separate opt-in gate above; native drawing, physical-device
and product behavior remain separate checks. No automatic release or public NuGet publication is
part of this procedure.

References: [Rust MSVC targets](https://doc.rust-lang.org/rustc/platform-support/windows-msvc.html),
[NuGet native assets](https://learn.microsoft.com/en-us/nuget/create-packages/native-files-in-net-packages),
[Rust native linkage](https://doc.rust-lang.org/reference/linkage.html).

[Rustup environment controls](https://rust-lang.github.io/rustup/environment-variables.html) describe the toolchain installation opt-out.
