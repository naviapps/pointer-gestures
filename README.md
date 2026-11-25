# Pointer Gestures

Pointer gesture recognition and rendering libraries. One
repository coordinates their shared contracts; the libraries remain independently
consumable. Hosts own application commands, permission onboarding, product policy,
persistence and style. Optional macOS and Windows adapters own overlay resources when selected.

## Documentation

- Installation and consumer examples: [Swift](#swift-installation),
  [Rust](#rust-installation), [C on macOS](#c-installation-on-macos),
  [browser](https://github.com/naviapps/pointer-gestures/blob/main/docs/WEB.md) and [.NET](#net-usage).
- Swift API guides: [recognition](https://github.com/naviapps/pointer-gestures/blob/main/docs/swift-recognition.md),
  [macOS input](https://github.com/naviapps/pointer-gestures/blob/main/docs/swift-macos-input.md),
  [portable rendering](https://github.com/naviapps/pointer-gestures/blob/main/docs/swift-rendering.md),
  [SwiftUI](https://github.com/naviapps/pointer-gestures/blob/main/docs/swiftui.md)
  and [macOS overlays](https://github.com/naviapps/pointer-gestures/blob/main/docs/swift-macos-overlays.md).
- Shared contracts: [session measurements and budgets](#session-measurements-and-budgets)
  and [input and geometry](#input-and-geometry-contracts).
- SDK work: [development and acceptance](#development-and-acceptance),
  [browser builds](https://github.com/naviapps/pointer-gestures/blob/main/docs/WEB.md#build-and-verify) and [Windows development](https://github.com/naviapps/pointer-gestures/blob/main/docs/WINDOWS.md).

## Interfaces

| Interface                                   | Responsibility                                                                     |
| ------------------------------------------- | ---------------------------------------------------------------------------------- |
| Rust `pointer-gesture-recognition`          | Safe matcher, movement, serial state machine, startup policy and replay sampling   |
| Rust `pointer-gesture-recognition-windows`  | Windows hooks, bounded delivery and native input recovery                          |
| Rust `pointer-gesture-rendering-windows`    | Optional transparent desktop overlay and dedicated native resource ownership       |
| Rust `pointer-gesture-rendering`            | Pattern/live geometry, bounds, presentation cadence and optional CPU rasterization |
| Swift `PointerGestureRecognition`           | Main-actor recognition, callbacks and observation                                  |
| Swift `PointerGestureRecognitionMacOS`      | macOS event tap and consumed-input replay                                          |
| Swift `PointerGestureRendering`             | Portable rendering values and Rust planning                                        |
| Swift `PointerGestureRenderingSwiftUI`      | Native paths, styling and presentation delivery                                    |
| Swift `PointerGestureRenderingMacOS`        | Optional macOS overlay windows, display lifecycle and trace fanout                 |
| .NET `NaviApps.PointerGestures.Recognition` | Copied recognition outputs and Windows input adapter                               |
| .NET `NaviApps.PointerGestures.Rendering`   | Copied geometry, presentation state and optional native Windows overlay            |
| Web `@naviapps/pointer-gestures`            | Shared recognition/geometry through an independent Wasm adapter                    |

The workspace pins Rust 1.100.0 for builds. Swift requires 6.3 or later on macOS 26 or later; .NET
requires version 10. Windows SDK packages target x64. The Windows 11 ARM64 worker
uses the approved x64 .NET SDK and records emulated execution. Universal
macOS artifacts include arm64 and x86_64; compilation is separate from native
runtime coverage. The two C ABI crates own their handles independently and export
only their declared functions. Safe Rust consumers do not emit C exports.

## Repository layout

`Sources/<Target>` and `Tests/<TargetTests>` own the Swift interfaces and tests.
The recognition/rendering packages under `crates` separately own portable logic,
C ABI callers in `tests/c`, and Windows resources. Named .NET projects and tests
live under `bindings/dotnet`; its `examples` directory is a runnable consumer.
`bindings/web` owns the independent browser adapter and nested Rust workspace.
External package consumers live in `fixtures/consumers`. The .NET tests reuse the
same public API fixtures when running against source projects. Distribution
drivers and their tests live in `distribution`, and cross-language guides live
in `docs`. `Artifacts` and `ThirdParty` are retained consumer inputs.
`.native-build` and `.web-build` are ignored developer output.

## Swift installation

Add the public package at a published commit:

```swift
.package(
  url: "https://github.com/naviapps/pointer-gestures.git",
  revision: "<FULL_PUBLISHED_COMMIT_SHA>"
)
```

Select only the products used by the target:

```swift
.product(name: "PointerGestureRecognition", package: "pointer-gestures"),
.product(name: "PointerGestureRecognitionMacOS", package: "pointer-gestures"),
.product(name: "PointerGestureRendering", package: "pointer-gestures"),
.product(name: "PointerGestureRenderingSwiftUI", package: "pointer-gestures"),
.product(name: "PointerGestureRenderingMacOS", package: "pointer-gestures")
```

Replace `<FULL_PUBLISHED_COMMIT_SHA>` with the complete 40-character published
revision accepted by your host. Local development uses
`.package(path: "../pointer-gestures")`. Recognition does not require rendering or
SwiftUI. Both native frameworks are checked in, so source-only Swift consumers
need neither Rust, Python nor a separate binary-download credential.

```swift
import PointerGestureRecognition
import PointerGestureRecognitionMacOS

@MainActor
func makeRecognizer() -> GestureRecognizer<String> {
  let source = GestureEventTap()
  var matcher = GesturePatternMatcher<String>()
  precondition(matcher.register(pattern: [.down, .right], match: "show-inspector") == nil)
  return GestureRecognizer(
    eventSource: source,
    configuration: GestureRecognizerConfiguration(
      makeMatcher: { [matcher] _ in matcher },
      onReplayRequested: { source.replay($0) },
      onMatch: { command in print(command) }
    )
  )
}
```

Retain the recognizer and observation tokens. Start/stop capture explicitly. Stop
disconnects the source before delivering consumed-input recovery and retains press
metadata until recovery returns. Custom sources implement the same ordering.
Direct Core Graphics replay can replace a physical release if it already posts
the release for that captured press. A host that queues replay can opt in with
`enqueueReplay(_:accepting:)` when its queue delivers accepted operations once,
in order, including after capture shutdown or host teardown. If the queue refuses
an operation, the host can still replay it directly in the same callback.
Observation cancellation is synchronous on the main actor. No further callback for
that subscription starts after `cancel()` returns, including within the current
notification; a running callback may finish. Destruction elsewhere schedules
actor-owned cleanup. Hosts own permission requests and command execution.

A replacement press recovers the old input before Swift asks for the new press's
host context. The session report identifies that replacement as superseded, the
same outcome as a replacement press in the portable Rust and .NET engines.
If recovery handles a newer press or restarts capture, the earlier press passes
through without asking for its host context.

Pattern registration returns a structured error (`nil`/`null` on success in
Swift/.NET, `Result` in Rust). `validationError(for:)`, `Validate` and Web
`validatePattern` validate without mutation. Empty and excessive patterns have no
index; repeated-run errors identify the zero-based second run of the first pair.
Foreign invalid direction values identify their first input index. Rejected
registrations preserve commands and captured sessions. Web registration throws
`GesturePatternError` with `reason` and `index`.

## Optional macOS overlay resources

```swift
import PointerGestureRenderingMacOS
import PointerGestureRenderingSwiftUI

@MainActor
func makeOverlays() -> GestureOverlayCoordinator {
  GestureOverlayCoordinator(configurationProvider: { display in
    GestureOverlayConfiguration(
      minimumAnimationInterval: 1.0 / Double(max(60, display.maximumFramesPerSecond)),
      strokeStyle: GestureStrokeStyle(lineWidth: 4)
    )
  })
}
```

Retain the coordinator. Call `configureOverlays(gestureRecognizer:)` only when the
host's activation and permission policy allows it; call `removeOverlays()` at the
end of that lifetime. Construction is inactive. The adapter shares one recognition
subscription across primary-first display snapshots, filters through Rust, and
owns transparent, nonactivating, click-through windows. Removal synchronously
cancels subscriptions before closing resources; release also tears down the adapter.
See the [macOS overlay guide](https://github.com/naviapps/pointer-gestures/blob/main/docs/swift-macos-overlays.md)
for configuration changes, injected resources and native validation.

## Rust installation

Select either or both workspace packages from the same Git source:

```toml
[dependencies]
pointer-gesture-recognition = { git = "https://github.com/naviapps/pointer-gestures", rev = "<FULL_PUBLISHED_COMMIT_SHA>" }
pointer-gesture-rendering = { git = "https://github.com/naviapps/pointer-gestures", rev = "<FULL_PUBLISHED_COMMIT_SHA>" }
```

Replace both placeholders with the same complete 40-character published revision
accepted by your host. The recognition engine accepts monotonic
nanoseconds and returns ordered effects; it has no OS input, wall-clock, callback,
UI or persistence dependency. Rendering depends on that safe crate for shared values.
Rendering's default feature set is geometry only. Enable `features = ["raster"]`
on the rendering dependency to draw shared paths into bounded premultiplied
RGBA8 images with antialiasing and round joins and caps. It uses tiny-skia
without PNG, file or network dependencies.

From a clean, complete SDK checkout with the pinned compiler,
`make check-rust-consumer` validates recognition-only, default geometry and explicit
raster consumers in an isolated workspace. Ordinary Cargo users need no SDK developer tools.

## C installation on macOS

Copy the desired `.framework` from the `macos-arm64_x86_64` slice of its
`Artifacts/*.xcframework` into a client-owned framework directory. Preserve its
symlinks and resources. Each framework includes its own public header and runtime
notices and supports arm64 and x86_64 on macOS 26 or later.

For recognition, include `<CPointerGestureRecognition/pointer_gesture_recognition.h>`.
Set `FRAMEWORKS` to the absolute directory containing the copied framework and
compile your own `client.c`:

```sh
xcrun clang -std=c11 -mmacosx-version-min=26.0 -F "$FRAMEWORKS" client.c \
  -framework CPointerGestureRecognition -Xlinker -rpath -Xlinker "$FRAMEWORKS" -o client
```

For rendering, include `<CPointerGestureRendering/pointer_gesture_rendering.h>`
and link `-framework CPointerGestureRendering`. Link both only if the client uses
both APIs. Keep the framework and header from the same SDK revision. This
installation uses the prebuilt framework without Rust, Python or SDK build tools;
native C runtime acceptance is a separate check.

For recognition, deliver the recovery output from the Stop event
(`PointerGestureRecognitionEvent.kind = 5`) before tearing down host input.
`pointer_gesture_recognition_engine_destroy` releases the engine without
requesting recovery. Owned output frames remain valid after engine destruction.

From a clean committed SDK checkout on owned macOS, `make check-c-consumer`
validates separate recognition and rendering clients against copies of the frameworks
with Clang from the macOS developer tools.

## Browser usage

The separate Web binding reuses the safe Rust recognition and rendering libraries.
Its source-pinned package includes JS, TypeScript declarations, Wasm, checksums and
runtime notices. Consumers can vendor that artifact without building Rust or accessing
private Git during ordinary website builds. DOM events, drawing and product examples
remain host-owned. See [Browser binding](https://github.com/naviapps/pointer-gestures/blob/main/docs/WEB.md) for the API and build checks.

## .NET usage

See the [.NET package guide](bindings/dotnet/README.md) for installation, examples
and Windows integration. Use the [local package checks](#local-net-package-checks)
to build and verify a candidate for selected runtimes.

Earlier type names `Output`, `Status`, `Trace` and `Point` are now
`GestureRecognitionOutput`, `GestureRecognitionStatus`, `GestureTrace` and
`GesturePoint`. Update annotations, aliases and any stored CLR type names.
Ordinary JSON property names and enum values keep their representation.

## Session measurements and budgets

Recognition retains a coordinate-free report of the latest completed retained
session: `Output.last_session` in Rust, `pointer_gesture_recognition_frame_session_report` in C,
`GestureRecognizerState.lastSessionReport` in Swift, `GestureRecognitionOutput.LastSessionReport`
in .NET and `GestureUpdate.lastSession` on the Web. It distinguishes matches,
recording, ordinary clicks, unmatched prefixes, cancellation, stop, replacement,
total/idle expiration, malformed releases and direction-budget recovery. Elapsed
time starts at the original press and idle time at the last changed finite point;
tuning resets do not rewrite measurements. Sample count includes admitted finite
press, move and release events. Maximum axis displacement uses the caller's
coordinate units. The report includes the accepted direction runs, any rejected
direction, whether the start threshold was crossed and the recording mode at the press.

Reports own their data (C views borrow the frame). They persist through later
snapshots and input until another retained session ends. Compare the nonzero,
wrapping engine-local generation; this is a latest value, not an event history or
proof of successful OS input recovery or host command execution. No measurements
are persisted or transmitted by the SDK.

The direction budget defaults to 64 and must be in 1..4096. It is captured at
button-down, independently of the raw point budget, so unlimited-duration recording
cannot grow accepted directions or endpoints indefinitely. Attempting another
turn requests ordinary input recovery, hides the trace and never matches. Tuning
changes apply the new direction budget to subsequent sessions. Registration accepts
1..4096 direction runs and rejects consecutive equal directions that recognition
cannot emit. Choose a session budget large enough for registered patterns.
Reducing the raw point limit trims the current trace immediately and bounds
recovery. Recovery retains the press and at least one recent point.

## Input and geometry contracts

Patterns match exactly; duplicate registration replaces the previous command.
Matchers support exact removal and clearing. Copies and active sessions keep their
captured registrations; removed slots are reused during repeated edits. Swift
retains only its generic commands while Rust owns matching and mutation.
Recognition context identifiers are opaque, exact strings, including empty strings.
Hosts own validation/normalization; use an absent context when no identity exists.
Swift and .NET pass through normal pointer input by default when the active matcher
has no patterns. Set `passesThroughEmptyMatcher` or `PassesThroughEmptyMatcher`
to false to retain and recover empty-matcher input. Recording still admits input
when recognition policy allows it.
Active input retains its matcher, context, recording mode and press metadata.
Changing future policy must not silently replace an in-progress session. In
recording mode, a finite release may reach the start distance after an earlier
changed drag sample. A release alone does not start recording, and releases never
start normal matching. Non-finite presses and motion pass through. A non-finite release for retained input consumes
and interrupts it, using only finite history for recovery and never emitting a match.
Cancellation ignores pointer coordinates. Cancellation/overflow must preserve consumed-input recovery.
Keep native replay outside hook callbacks and report partial injection explicitly.

Coordinates increase downward. Finite endpoints keep their dominant direction
when the axis distance exceeds the representable range. Pattern previews and
live traces project straight segments onto the X/Y axes; platform/view transforms
belong at adapter boundaries.
Visible traces require at least one direction, finite points and matching
point/direction counts. Deliver recognition publications in order. Rust's
presentation reducer coalesces ordinary motion and publishes direction changes
and rate-limited large movements immediately.

A normal hide publishes any queued final trace first. When a trace is visible,
its hide deadline is two cadence intervals after the last visible publication.
A discard clears visible and queued geometry and cancels pending deadlines
immediately. Interrupted sessions do not emit matches. Configuration and
previously returned snapshots remain valid after a discard.

Hosts supply cadence in seconds and pointer lag in their coordinate units.
Reconfiguring cadence preserves visible and queued geometry and recomputes
deadlines from the last presentation. It returns any newly due update immediately.
Rust owns the geometry, visibility bounds and nanosecond deadlines. Native
adapters own clocks and graphics/window resources.

See the Swift API guides linked [above](#documentation), C fixtures under `crates/*-ffi/tests/c`, and
[Windows integration](https://github.com/naviapps/pointer-gestures/blob/main/docs/WINDOWS.md) for ownership and desktop-input requirements.
Private SDK diagnostics must not contain host secrets or unrelated user input.
Do not change global DPI, sharing, permissions or security settings for validation.

## Development and acceptance

Run developer commands from this checkout. Review `mise.toml` and
`rust-toolchain.toml`, then install the declared tools and fetch the locked Cargo
dependencies before running offline checks. Ordinary SwiftPM and NuGet
consumers do not need the developer setup.

```sh
mise trust mise.toml
mise install
mise exec -- cargo fetch --locked
mise exec -- make check
```

Run `mise run lint` before builds or `mise run format` to format owned source.
These tasks use the public SDK Tooling revision pinned in
[`mise.toml`](mise.toml). The first run downloads it without GitHub credentials.
The quality checks exclude vendored code, native artifacts and original notices.

`make check` runs source lint, Rust tests and Clippy, strict Swift tests and
standalone Swift consumers against the tracked frameworks. It also runs the C
ABI fixtures and builds the .NET test projects. `make lint` covers Rust formatting
for both the native and Web workspaces.

### Distribution verification

The [distribution plan](sdk-distribution.toml) defines the hosted acceptance
commands. For local native or Web distribution checks, run the relevant Make
targets through mise. The targets use the pinned [SDK Tooling](https://github.com/naviapps/sdk-tooling)
CLI and fixtures in this checkout.

```sh
mise exec -- make check-distribution
```

Run the hosted macOS and Windows distribution checks from the SDK checkout at
the intended commit on `main`:

```sh
gh workflow run check-sdk-distribution.yml -R naviapps/sdk-tooling \
  -f sdk=pointer-gestures -f "revision=$(git rev-parse HEAD)"
```

Run `mise exec -- make generate-header` to generate the portable and Windows C
headers and Swift module maps locally. The checkout ignores these copies.
`make check-header` compares regenerated portable bytes and module maps with
the bundled XCFrameworks. `make test-ffi` compiles the generated portable headers,
and the Windows build checks compile the generated Windows headers.
When native sources change, run `mise exec -- make native` to regenerate the
files and rebuild the two macOS frameworks declared in [`native.toml`](native.toml).
`make check-distribution`
checks the existing native receipts, the distribution fixtures, a Swift release
build and Rust API documentation.
`make check-dotnet` builds host-native Rust libraries and runs the managed
fixtures and README examples.
Run `mise run sdk-tooling -- xcframework --product recognition` to select one framework.
`mise run sdk-tooling -- python -m distribution.windows_sdk --help`
lists the Windows build, inspection, C and package consumer checks.
Windows input and drawing probes require explicit opt-in in an owned isolated desktop.
Local builds, actual native fixtures and exact-source CI are distinct evidence.
For documentation-only edits, `mise exec -- make check-docs` checks Rust API
documentation.

Each macOS framework contains the original notice bodies verbatim in
`Resources/ThirdPartyNotices.txt`. Original notices and Rust runtime provenance
remain under `ThirdParty/`; native packages retain the required notices for their runtimes.

Use Git on main. Keep related edits together, validate them, then
publish one coherent main unit. Preserve published commits and consumer pins.
Retain upstream runtime notices with redistributed artifacts. Keep product source,
QA data and credentials outside this repository.

### Local .NET package checks

Pack both libraries at the same local prerelease version. Include `--rid` once
per target. macOS assets come from the verified universal frameworks. Windows
packages use the verified `win-x64` native libraries. See
[Windows development](docs/WINDOWS.md#pack-local-candidates) for the x64 preparation steps.
The pack command stages both packages before adding them to the feed, so a build
failure leaves the selected version available for a retry. Existing versions
remain immutable. Pass `--sdk recognition` to build only recognition.

```sh
mise run sdk-tooling -- python -m distribution.nuget pack --rid osx-arm64 --rid osx-x64 \
  --version 0.1.0-local.1 --output /path/to/pointer-feed
mise run sdk-tooling -- python -m distribution.nuget test-package --rid osx-arm64 \
  --version 0.1.0-local.1 --feed /path/to/pointer-feed
```

`test-package` restores from the local feed into a fresh cache and executes
the portable recognition and rendering tests. Select the matching .NET SDK
on macOS or the x64 .NET SDK on Windows. `check-package` verifies restore,
compilation and native asset selection on a development host. The Windows
host gate records the x64 execution mode. Its desktop input and drawing
fixtures require explicit opt-in.

### Test responsibilities

See [AGENTS.md](https://github.com/naviapps/pointer-gestures/blob/main/AGENTS.md#test-ownership)
for test ownership. GitHub Actions runs the same strict Swift tests and standalone
Swift consumers on macOS 26 and 27. The Windows x64 job runs Rust and .NET binding
tests. These workflows run on pushes and pull requests targeting `main`.
The maintainer distribution workflow also checks C, installed NuGet consumers and
the Web package in Chromium. It combines the accepted x64 Windows builds and
the macOS libraries, then executes the same pair of packages on both macOS .NET
architectures and on Windows x64. Dispatch with `windows-on-arm=true` to also
verify x64 emulation on Windows 11 ARM64. Run `make check` for Swift and shared code.
Use the distribution setup for `make check-web`. Use an owned desktop for
native input and rendering probes.

## Shared tail updates and Windows observations

`OverlayTrace::with_tail`, Swift `replacingTailPoint(with:)` and .NET `WithTail`
return immutable snapshots sharing geometry and cached bounds. Use them when only
the latest pointer position changes; non-finite tails are rejected. The Swift
and .NET trace converters select this path automatically. Hidden or invalid
snapshots release the converter's cache. Previous snapshots stay valid.
Run `cargo bench -p pointer-gesture-rendering --bench trace_updates` to compare
allocations and CPU time; it does not measure complete UI/input latency.

Windows `ReadAsync` waits for native readiness without timer polling. `TryRead`
remains useful for draining batches. Consume serially and deliver effects in order.
Stop wakes readers, which drain final frames before completion; disposal wakes
pending readers with `ObjectDisposedException`. Cancellation before queue admission
leaves observations available. Once the read dequeues a frame, it returns the
owned frame. Hosts still stop capture explicitly. Native Rust/C consumers can duplicate a
wait-only notification handle and integrate it with their own event loop. The
adapter owns resetting readiness; duplicates are closed by their consumers.

The Windows guide owns [press context](https://github.com/naviapps/pointer-gestures/blob/main/docs/WINDOWS.md#native-rust-press-context),
[bounded delivery and recovery](https://github.com/naviapps/pointer-gestures/blob/main/docs/WINDOWS.md#native-input-recovery)
and [diagnostic evidence](https://github.com/naviapps/pointer-gestures/blob/main/docs/WINDOWS.md#native-rust-diagnostic-evidence).

## License

This SDK is licensed under the [MIT License](LICENSE). Keep the
[third-party notices](ThirdParty/) with redistributed artifacts.
