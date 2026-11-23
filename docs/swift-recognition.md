# `PointerGestureRecognition`

Recognize pointer gestures from platform-neutral input events.

## Overview

The deterministic matcher and input state machine are implemented once in Rust.
Rust also validates second-based tuning and converts positive finite delays to
saturating nanoseconds, rounding upward. Rust owns startup revisions, retry policy
and replay sampling too. Swift owns main-actor callbacks, source/task resources,
observation and timer delivery.
The package includes its universal macOS dynamic framework; consumers do not build Rust.

PointerGestureRecognition contains the recognizer core, direction model, matcher,
recognition policy inputs, observable state, and event-source protocol used to
turn pointer-button drag paths into app-owned matches. Recognition defaults to
the secondary button and can be configured for primary, middle, or auxiliary
pointer buttons.
Gesture movement is recorded as one of four cardinal directions: up, down, left,
and right. Movement without a dominant axis does not add a direction.
Finite endpoints retain their dominant direction even when the axis distance
overflows.
`GesturePoint` values use gesture coordinates where positive vertical movement resolves as down.
The recognizer passes through presses and motion with non-finite coordinates.
A non-finite release for retained input consumes and interrupts it, recovers from
finite history, and hides the trace without matching. Cancellation ignores its
location. Other-button releases and releases without retained input pass through.

The core target does not depend on AppKit, Core Graphics, or live input capture. Host apps provide
event sources to `GestureRecognizer` and provide recognition-context policy plus callbacks through
`GestureRecognizerConfiguration`. Trace rendering, Accessibility permission presentation,
persistence, telemetry, and command execution stay outside this target.
Use `GestureInputEvent.Kind` to distinguish pointer-button down, move, and up input events.

Use `GesturePatternMatcher` to map direction patterns to app-owned match values, then create a
`GestureRecognizer` with an event source and configuration.

## Usage

Register app-owned matches with `GesturePatternMatcher` and pass the matcher to a
`GestureRecognizer` configuration:

```swift
import PointerGestureRecognition

enum AppCommand: Sendable {
  case showInspector
  case focusSearch
}

@MainActor
func makeGestureMatcher() -> GesturePatternMatcher<AppCommand> {
  var matcher = GesturePatternMatcher<AppCommand>()
  matcher.register(pattern: [.down, .right], match: .showInspector)
  matcher.register(pattern: [.up, .left], match: .focusSearch)
  return matcher
}
```

Pattern matching is exact. Node storage and lookup are iterative so deep host-provided
patterns do not grow the call stack during registration, matching or teardown. Matcher
copies retain value semantics: later host registrations cannot change a captured session.
During normal recognition, trace visibility begins when the current
direction sequence is a valid matcher prefix; completed matches still require an exact registered
pattern. Host apps validate settings-level conflicts before building a matcher.
Use `GestureRecognizerConfiguration` to provide policy callbacks, replay handling, match handling,
and tuning. Keep the returned `GestureObservationToken` while status or trace observation should
remain active.
Use `GestureRecognizer.observeTrace(_:)` for live overlay feedback; after a trace becomes visible,
tail-point and hidden-state updates are delivered during input handling so hosts can render fast
gestures without an extra main-actor turn. A hidden trace's
`GestureRecognizerState.Trace/discardsPendingPresentation` requests immediate
cancellation of visible and queued rendering; normal completion preserves final geometry.
Use `GesturePatternMatcher.validationError(for:)` for nonmutating validation.
The repeated-direction error indexes the zero-based second run; length errors have no index.

### Editing Registrations and Host Identity

`GesturePatternMatcher.remove(pattern:)` removes one exact registration;
`GesturePatternMatcher.removeAll()` releases this matcher's registrations and
commands. Copies and active sessions retain their captured values. Registration
slots are reused during repeated edits. An absent removal returns `nil`.

`GestureRecognitionContext` preserves its identifier exactly, including whitespace
and empty strings. Hosts own normalization/validation of bundle IDs or other
identity formats. An absent context is represented by `nil`, not an implicit string
rewrite in the SDK.

## Session and Input Contracts

Matching is exact; registering an invalid pattern returns a `GesturePatternError` (`nil` means success), and registering an existing pattern
replaces its match. Context, matcher, and recording mode are captured for a gesture session.
An empty matcher passes through normal pointer input by default. Pass
`passesThroughEmptyMatcher: false` to the configuration initializer to retain
and recover input when the matcher is empty.
Changes to recording mode affect subsequent sessions. Recording bypasses matcher/modifier
approval while still respecting the recognition-enable policy. After a changed
drag sample, a finite release can finish a recording that reaches the start
distance in a cardinal direction only at that release. Without prior motion, a
release does not start recording; normal matching still starts on motion.

`GestureModifierFlags.init(rawValue:)` preserves supplied package bits, including unknown
bits; it does not accept platform modifier masks as equivalent values. Convert platform flags
in the event-source adapter and normalize host settings at the host boundary.

Keep observation tokens alive while observation is needed. Status and trace observation emit
an initial snapshot. Cancellation or token release removes the observer. Once
`GestureObservationToken.cancel()` returns on the main actor, no further callback
for that subscription starts, even within the current notification. A callback
already running may finish. A replacement subscription receives its own initial
snapshot and no notifications queued for the removed registration.
Stopping recognition cancels start retries and active capture; it can still emit
replay requests for consumed input.

## Input Deadlines

The default tuning retains a one-second total session limit and no inactivity limit.
Hosts can opt into `maximumGestureIdleDuration` to allow deliberate movement while
canceling a stalled gesture. A changed pointer location refreshes only the inactivity
deadline; duplicate stationary events do not. When both limits are enabled, the earlier
deadline wins. Setting both to `nil` disables expiration. Expiration never emits a match
and preserves the usual interrupted-input replay behavior.

Timing uses a continuous clock and one expiration task per pending input or active
session, rather than a new task per point. Updating tuning restarts enabled deadlines
for current input. Promotion from pending input to a gesture preserves the button-input
start time, so it cannot extend the total deadline. Deadlines are also checked during
input delivery: a delayed timer cannot permit an expired release to emit a match.
A retained non-finite release stays consumed at the deadline and requests recovery
once, while preserving the expiration failure.
Cancellation, movement and release of the configured recognition button check
expiration in one core transition. Input started by a recovery callback belongs
to a new session. The event that caused recovery cannot advance or end it.
Host apps own the choice of limits, settings, and recovery UI.

## Session Reports and Direction Budget

Read `GestureRecognizerState.lastSessionReport` for a copied, coordinate-free
`GestureSessionReport`. `GestureSessionOutcome` distinguishes a match, recording,
ordinary click, failed prefix, cancellation, stop, replacement, each deadline,
malformed release and budget recovery. Reports measure from the original press and
last actual changed location even after tuning restarts a deadline. They describe
recognition decisions and requested effects, not successful OS replay or command
execution. No data is persisted or sent.

The latest report persists across snapshots and later input. Compare its wrapping,
recognizer-local generation; observation can coalesce and is not an event history.
Status, trace and report snapshots compare their scalar fields and ordered
collection contents by value.
When a new button-down replaces retained input, the recognizer requests its recovery
before checking the new press's host policy. The old session reports `superseded`,
or its deadline outcome if it already expired.
If recovery handles a newer press or restarts the source, the earlier press
passes through before the recognizer asks for its host context.
`GestureRecognizerTuning.maximumDirectionCount` defaults to 64 and is validated in
1...4096. Each press captures the limit. Attempting another direction recovers input
and hides the trace without matching. Lowering tuning affects subsequent sessions,
not the budget already captured by current input. The raw point limit remains
independent; choose a direction budget large enough for registered patterns.
Reducing the raw point limit trims the current trace immediately. Replay keeps
the press and at least one recent point, and earlier snapshots retain their copies.

## Recognition Pipeline

The recognizer turns platform-neutral input into a match through a small fixed pipeline:

1. Capture raw gesture points from the configured pointer-button input.
2. Resolve dominant-axis movement into cardinal directions.
3. Build a completed direction sequence for the gesture session.
4. Match the completed sequence exactly against registered patterns.
5. Emit either an app-owned match or a pointer-button replay request.

## Responsibility Boundary

PointerGestureRecognition owns direction recognition, matching, recognizer state, trace observation,
event-disposition modeling, event-source start retry behavior, and platform-neutral event-source
contracts. Event-source implementations expose `GestureEventSource.start(handler:)` and
`GestureEventSource.stop(recoveringInput:)` as the live-input lifecycle hooks consumed by the recognizer.
Stop disconnects capture before invoking recovery exactly once, retaining the metadata that
recovery needs until it returns. A restart from recovery begins a new capture that the old
stop leaves running. The no-argument `stop()` convenience supplies empty recovery.
They apply the returned `GestureEventDisposition` to the original platform event:
`GestureEventDisposition.consume` suppresses it, while
`GestureEventDisposition.passThrough` leaves it in the platform event stream.

PointerGestureRecognition does not own command catalogs, command execution, overlay UI, Accessibility
permission presentation, privacy disclosures, persistence, telemetry, analytics, non-macOS
event-source adapters, or diagonal gesture directions.
