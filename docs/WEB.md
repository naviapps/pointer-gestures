# Browser binding

The browser package adapts the existing Rust recognition engine and geometry planners.
It owns no DOM, pointer capture, permissions, operating-system input recovery, commands,
SVG serialization, colors or product examples. Native Swift/.NET consumers remain
independent: the `pointer-gestures-wasm` crate is a nested Cargo workspace with a separate lockfile and
an optional `web` feature for wasm-bindgen.

For the other consumer interfaces and shared contracts, see the
[SDK overview](https://github.com/naviapps/pointer-gestures#readme).

## Consume an artifact

Copy the complete verified package directory to a consumer-controlled vendor directory
and depend on it through a `file:` npm dependency. Retain `provenance.json`, `SHA256SUMS`,
`LICENSE` and `THIRD_PARTY_NOTICES`. The package contains generated JS, declarations
and Wasm, so consumer builds need neither Rust nor private Git credentials. Pin the
full source revision and package hashes; a version number alone does not identify bytes.

For example, install an already generated and verified vendored directory or tarball:

```sh
npm install --ignore-scripts --offline ./vendor/pointer-gestures
# Or pack that directory and install its local immutable archive:
npm pack --ignore-scripts --offline ./vendor/pointer-gestures
npm install --ignore-scripts --offline ./naviapps-pointer-gestures-0.1.0.tgz
```

These paths refer to local artifacts. No public npm registry package is required or
published by these commands. Retain the archive's SHA-256 alongside its source revision.

```javascript
import init, { GestureSession, tracePath, patternPath } from "@naviapps/pointer-gestures";

await init();
const session = new GestureSession({
  startDistance: 10, changeDistance: 25, maximumPoints: 256,
  totalDurationMs: 1000, idleDurationMs: 0,
});
session.register(["down", "right"], 42);
session.begin({ x: 0, y: 0 }, 0);
session.move({ x: 0, y: 80 }, 20);
session.move({ x: 100, y: 80 }, 40);
const result = session.end({ x: 120, y: 80 }, 60);
// result.matchId === 42. Recognition has not calculated a drawing path.
const commands = tracePath(result.trace, { cornerRadiusScale: 1 });
// commands contains portable move/line/cubic/close data.
session.dispose();
```

`init()` defaults to the colocated Wasm URL. Pass a URL for bundlers that emit the asset
separately; for example, a bundler with a `?url` asset loader can import
`@naviapps/pointer-gestures/pointer_gestures_wasm_bg.wasm?url` and pass that URL to `init`.
`initSync(bytesOrModule)` initializes verified bytes in build-time or Node
hosts without fetching. Serve Wasm as `application/wasm`; a production CSP needs
`'wasm-unsafe-eval'` in `script-src` as well as authorization for the loader. Do not
enable general JavaScript evaluation. Loading failures must leave ordinary site content
and static illustrations available.

## Recognition and lifecycle

Register arbitrary nonempty direction patterns with uint32 identifiers, including zero.
`remove(pattern)` returns the removed identifier or `null`; `clear()` removes all
future registrations. Active sessions preserve their captured matcher. Repeated
edits reuse vacant Rust trie storage. Invalid direction names are rejected.
Matching is exact and duplicate registration replaces future matches. An active session
retains its matcher snapshot. `begin`, `move`, `end`, `cancel` and `tick` return plain
copied values; no native handle escapes the adapter. Coordinates increase right/down;
distances use the host's chosen coordinate units. Timestamps are finite, nonnegative,
monotonic milliseconds from one host-owned clock. Total and idle durations use
milliseconds; zero disables each deadline. Call `tick` at `deadlineMs`, replacing the
previous host timer after every result. `active` includes pending input before the start
threshold as well as recognized capture. `expired` identifies native timeout policy.

`trace` is nullable, deeply immutable copied geometry: cardinal `directions`,
origin/run endpoint `points` and optional `tailPoint`. It remains valid after later
updates and session disposal. Recognition performs no path planning or command
encoding. Call `tracePath(trace, { cornerRadiusScale })` only when presenting a
snapshot; a null trace produces no commands. Curvature is presentation policy,
independent of recognition tuning. Feed every input synchronously, retain the
latest snapshot and plan/draw once in the host's animation frame. Apply match and
terminal/discard results immediately and in order; drawing coalescing must never
discard those decisions. DOM resources and animation-frame scheduling stay in hosts.

`dispose()` releases Wasm memory without processing input or producing a cancellation
update. It needs no timestamp and is safe to repeat. Call `cancel(nowMs)` first when
the host needs a terminal update; that operation leaves the session reusable.
After disposal, session operations are rejected and copied results remain valid.
Hosts release their pointer capture, event listeners and scheduled drawing separately.

### Live policy and recording

`setRecording(enabled, nowMs)` changes future strokes and returns a current update;
an active stroke retains its captured mode and matcher. `updateTuning(tuning,
nowMs, { resetUnchanged })` replaces all six `GestureTuning` fields without
reconstructing the session or its registrations. Changed tuning restarts active
deadlines at the supplied time; unchanged values do so only with `resetUnchanged`.
Use the returned `deadlineMs` to replace the host timer. Invalid tuning or time
leaves configuration, input and the accepted clock unchanged. Active strokes
retain their captured direction budget. Both calls use the same monotonic clock
as input; appearance options belong to the planner.
Rust numeric/geometry rejections surface as JavaScript `RangeError` objects;
invalid JavaScript argument shapes and structured `GesturePatternError` remain
distinct. Wasm loading/runtime errors retain their original exception identity.

Set `recording: true` in `SessionConfiguration` for unrestricted pattern input, such
as an editor or playground. It uses the native recording mode, accepts unregistered
prefixes and empty matchers, and returns the completed directions and geometry
without a `matchId`. Hosts may map those directions to their own example labels;
this does not execute an action. Use zero deadlines when exploration should not be
time-limited. Cancellation, malformed release and expiry still clear the trace.
The default remains matching mode with the native one-second deadline.

`begin` and `move` reject non-finite coordinates without changing the session.
`end` accepts a non-finite release location as an interruption: it clears retained
input and geometry without matching, even when both deadlines are disabled. This
prevents a malformed terminal event from leaving capture active or matching on a
later release. All calls still require a valid monotonic timestamp; rejected times
leave the session unchanged. `cancel` needs no location.

## Host presentation

Feed every input event synchronously to retain native recognition semantics; coalesce
only DOM drawing. Before applying a result with `discardPendingVisibleTrace`, discard
any queued visible drawing. A valid completed stroke retains the last visible publication
before the native engine hides it, including an unassigned recorded pattern. Cancellation, expiration and rejected prefixes never
resurrect that geometry or emit a match. Browser hosts do not replay native click/drag
effects: they own browser event handling and this binding performs no OS action.

`patternPath(directions, { width, height, padding, lineWidth, showsArrowhead,
cornerRadiusScale })` calls the shared planner and returns shaft `commands` plus an
optional `arrowhead` command array. Defaults are padding 8, line width 3, arrowhead on
and corner scale 1. Commands contain `type` and `to`; cubic commands also contain
`control1` and `control2`. A close command has no points. The host converts these to
SVG or its drawing API. Static fallback paths can use this same function at build time.

## Session Reports

Every update includes nullable `lastSession`, a copied coordinate-free report of
the latest completed retained session. `outcome` distinguishes matches, recordings,
ordinary clicks, invalid prefixes, cancellation, stop, replacement, total/idle
expiration, malformed releases and direction-limit recovery. `elapsedMs` measures
from the original press, `idleMs` from the last changed finite location, `pointCount`
counts admitted finite pointer samples and `maximumAxisDistance` uses host units.
`directions` contains accepted runs; `rejectedDirection` identifies a rejected turn.
`captured` and `recording` describe the completed session.

`generation` and `pointCount` are `bigint` to preserve the full native range;
convert them explicitly to strings before JSON serialization. Compare the wrapping,
engine-local generation to detect replacement. This latest value persists across
later input and is not an event history or proof of a browser/native action.

`maximumDirections` defaults to 64, is validated in 1..4096 by Rust and is captured
at `begin`. Exceeding it abandons recording/recognition, discards queued geometry
and never emits a match. It bounds accepted direction/endpoint storage separately
from `maximumPoints`, including when both deadlines are disabled.

## Build and verify

These are SDK development commands. Clone the
[source repository](https://github.com/naviapps/pointer-gestures) and run them from
its root; the generated consumer package does not contain build tools or manifests.
Prepare the pinned tools using the [development setup](../README.md#development-and-acceptance)
and use the pinned SDK Tooling CLI from mise.

On macOS ARM64, install the Wasm target and fetch the separate locked dependencies
once when network access is intended:

```sh
mise exec -- rustup target add wasm32-unknown-unknown
mise exec -- cargo fetch --manifest-path bindings/web/rust/Cargo.toml --locked
mise run check-web-distribution
```

The hosted macOS distribution job runs this task with mise 2026.11.12.
The task selects the official ARM64 `wasm-bindgen` 0.2.104 archive with a
pinned SHA-256 and installs it only when needed.
For individual commands, prepare the matching `wasm-bindgen-cli` and the pinned
Rust, Node.js and TypeScript tools yourself:

```sh
mise run sdk-tooling -- python -m distribution.npm_package
mise run sdk-tooling -- python -m distribution.npm_package --verify
mise exec -- cargo test --manifest-path bindings/web/rust/Cargo.toml --locked --offline
mise exec -- cargo clippy --manifest-path bindings/web/rust/Cargo.toml --all-targets --features web --locked --offline -- -D warnings
mise exec -- node --test bindings/web/tests/*.test.mjs
mise exec -- node distribution/verify_web_consumer.mjs
```

The package builder runs offline with the exact lockfile and matching CLI. It stages the
portable Rust crates under the browser workspace so Cargo derives the same package
identities in different checkout paths. It remaps build paths, copies runtime
notices and emits `.web-build/package` with full source snapshot
revision, source hashes, compiler/CLI versions, dependency provenance and artifact hashes.
The source revision identifies the Git HEAD at build start, or the controller's
frozen commit identity when Git metadata is omitted. For local uncommitted edits,
HEAD identifies the baseline; the input hashes identify the actual build bytes. Identical inputs and toolchain should reproduce the
same runtime bytes; verify the hashes after copying. Tests execute the generated Wasm
in Node using binding tests in `bindings/web/tests`. Browser integration, HTTP CSP,
pointer capture, loading and accessibility need
separate consumer browser checks. Run the root `make check` for native regressions;
this nested workspace does not alter native Cargo.lock or framework receipts.

Run `mise exec -- cargo fetch --locked --manifest-path bindings/web/rust/Cargo.toml` and
`mise exec -- make check-web-source` for the Web Rust tests and lint in public CI.

Run `mise exec -- make check-web-reproducibility` from a local Git checkout to
build its committed HEAD offline from two fresh source paths, including one with
spaces. The check compares every generated package file, including Wasm,
provenance and checksums. Install the pinned build tools and fetch the locked
dependencies before the offline check.

`make check-web` also packs the generated directory with npm and installs that tarball
offline into an unrelated temporary client with an empty npm cache and lifecycle
scripts disabled. The clients under `fixtures/consumers/web` import the package
by name. The Node client resolves its explicit Wasm
and provenance exports, and executes recognition and rendering without a compiler or
native-library search path. Every retained package output is checked after installation.
The same installed client runs the TypeScript compiler pinned in `mise.toml` with
`--noEmit` against the package name and its exported declarations. The compile-only
fixture checks initialization, configuration, recognition results, rendering unions and structured pattern failures;
seven `@ts-expect-error` cases require invalid inputs and unsafe result use to fail
type checking. The fixture never executes. `make check-web-types` runs only the
pack/install and static declaration checks against an existing generated package.
Supply a different generated directory as the last argument to
`node distribution/verify_web_consumer.mjs [--types-only]` when needed, or set `TSC_BIN` to the
compiler executable. The fixture records the compiler's reported version. These
acceptance tools are developer prerequisites. Ordinary consumers only need the
generated package and their JS host.

## Browser acceptance

On macOS ARM64, build the package with `mise exec -- make check-web`, then run
the browser fixture using the runtimes pinned in `mise.toml`.

```sh
evidence_dir="$(mktemp -d)"
mise exec -- make check-web-browser BROWSER_ARTIFACTS="$evidence_dir"
```

This checks `.web-build/package` and writes receipts, screenshots, traces and
logs under `web-browser` in the evidence directory. GitHub Actions can choose a
fresh path under `RUNNER_TEMP` for evidence. The browser runs headless.

Without `BROWSER_ARTIFACTS`, Make creates a temporary evidence directory and prints
its path in the command. A previously verified SDK package can be checked by
running `mise run sdk-tooling -- python -m distribution.verify_web_browser`
with `--artifacts`, `--package`, `--source-revision` and `--provenance-sha256`.
The receipt records the commits of the fixture and the SDK. The browser gate
verifies the installed package against its provenance and checksums and serves its
installed JS/Wasm and the independent static fixture on `127.0.0.1`.

The flow under test is: independent page loads the installed SDK and colocated
Wasm over HTTP, a trusted mouse drag moves down then right outside the canvas under
pointer capture, release matches command 42 and draws the returned trace, and the
dispose control releases the SDK and host listeners while preserving the copied
trace. The same input after disposal must leave state and pixels unchanged.
The gate checks page identity, meaningful visible content, absent framework error
overlays, console/page/network health, capture/release events, recognition/report
state and Canvas pixels at 1000×760 and 390×844. Screenshots and traces support
later visual inspection; pixel assertions do not replace that inspection.

The gate uses Playwright 1.65.0 and Chromium 158.0.8127.18,
revision 1254, pinned by `fixtures/acceptance/browser/package-lock.json` and the
package's checked browser descriptor. The public npm integrity pins came from
the [official registry](https://registry.npmjs.org/playwright/1.65.0); the browser
version is documented in the [official release](https://github.com/microsoft/playwright/releases/tag/v1.65.0)
and [browser manifest](https://raw.githubusercontent.com/microsoft/playwright/v1.65.0/packages/playwright-core/browsers.json).
Provisioning runs `npm ci --ignore-scripts` and the official
`playwright install --no-shell chromium` in fresh job-local directories. It installs
no global tools, OS dependencies, security policy or base-image changes. The
browser's new headless mode uses the full Chromium browser with `channel: "chromium"`.
No SDK compiler or native framework is used by the consumer. Each provisioning
command and the browser flow have explicit time bounds, with no retries of failed
acceptance. All logs, JSON receipts, six viewport/state screenshots and two traces
are retained under `web-browser` in the evidence directory. A failure
during the browser flow additionally retains its screenshot and partial browser
receipt; earlier provisioning failures retain their command log and summary.

## Migrating package names

The JavaScript distribution replaces `@naviapps/pointer-gestures-web`; update npm
dependencies and imports to `@naviapps/pointer-gestures`. The Rust adapter replaces
`pointer-gestures-web` with `pointer-gestures-wasm`. Its compiled module is
`pointer_gestures_wasm.wasm`; wasm-bindgen produces `pointer_gestures_wasm.js`,
`pointer_gestures_wasm.d.ts`, `pointer_gestures_wasm_bg.wasm` and
`pointer_gestures_wasm_bg.wasm.d.ts`. Replace direct asset imports as shown above.
Rebuild and verify the complete package to regenerate matching provenance and
checksums before vendoring it; renaming generated files does not migrate an artifact.
If the default output still contains the previous package, use a fresh directory:

```sh
mise run sdk-tooling -- python -m distribution.npm_package --output .web-build/pointer-gestures
mise run sdk-tooling -- python -m distribution.npm_package --verify --output .web-build/pointer-gestures
POINTER_GESTURES_WEB_PACKAGE=.web-build/pointer-gestures node --test bindings/web/tests/*.test.mjs
```

Source API
values and host-owned session data retain their existing representation.
