# `PointerGestureRecognitionMacOS`

Capture macOS pointer-button drag input with a Core Graphics event tap.

## Overview

PointerGestureRecognitionMacOS provides the live macOS adapter for `PointerGestureRecognition`. Use
`GestureEventTap` when a macOS app needs to capture pointer-button drag gestures with a Core
Graphics session event tap. It defaults to the secondary button and can be configured for primary,
middle, or auxiliary pointer buttons. Passing an empty captured-button set captures no input and
`GestureEventTap/start(handler:)` returns `false`.

When a recognizer uses a non-default recognition button, create `GestureEventTap` with a
`capturedButtons` set that includes that same button.

The adapter maps configured mouse-button down, drag-move, up, and initial Escape presses to
platform-neutral `PointerGestureRecognition.GestureInputEvent` values. Primary buttons use left-mouse
events, secondary buttons use right-mouse events, and middle or auxiliary buttons use other-mouse
events with the Core Graphics button number.
When the recognizer consumes an Escape press, the adapter also consumes its repeated downs
and release for the same event source process and keyboard type. An Escape press passed to
the foreground app retains its repeats and release. Stopping or losing the tap discards
pending Escape press state.

## Usage

Use the same event tap as the recognizer's source and replay handler. For a
non-default recognition button, configure both with that button:

```swift
import PointerGestureRecognition
import PointerGestureRecognitionMacOS

@MainActor
func makeRecognizer() -> GestureRecognizer<String> {
  let eventTap = GestureEventTap(capturedButtons: [.primary])
  var matcher = GesturePatternMatcher<String>()
  precondition(matcher.register(pattern: [.down, .right], match: "show-inspector") == nil)
  return GestureRecognizer(
    eventSource: eventTap,
    configuration: GestureRecognizerConfiguration(
      makeMatcher: { [matcher] _ in matcher },
      onReplayRequested: { eventTap.replay($0) },
      onMatch: { command in print(command) },
      recognitionButton: .primary
    )
  )
}
```

Retain the recognizer and call `start()` only after the host's permission and
activation policy allows capture. Call `stop()` when that lifetime ends; replay
requests are delivered through the configured handler during recovery.

Convert Core Graphics points at the adapter boundary when a host app needs to
move between live `CGPoint` values and neutral gesture coordinates:

```swift
import CoreGraphics
import PointerGestureRecognition
import PointerGestureRecognitionMacOS

let point = GesturePoint(CGPoint(x: 10, y: 20))
let coreGraphicsPoint = point.cgPoint
```

## Responsibility Boundary

PointerGestureRecognitionMacOS owns the live Core Graphics event-tap adapter for pointer-button
drag input, Escape-key cancellation, modifier mapping, and pointer-button replay adaptation. Use
`GestureEventTap/replay(_:)` to handle click, drag, and release replay
requests emitted by the recognizer. `GestureReplayRequest.dragStart(button:points:)`
restores a consumed drag start without posting a release; a later release request finishes it.
A click replay posts button-down and button-up events before returning. A drag replay posts down, move, and up events; empty drag requests are ignored and
single-point drag requests replay as a click at that point. Replayed events are marked so the tap can
ignore them if Core Graphics reports them back through the event stream.
Recovery retains the original press's click count, so a captured double or triple click does not
become a single click. Replayed down retains the original press's native modifier flags;
movement and up retain the latest captured pointer event's flags. This preserves modified clicks
and drags when a key changes during capture. Deliver replay requests directly from the replay callback,
or call `GestureEventTap/prepareReplay(_:)` in that callback and enqueue the returned operation
for ordered, deferred delivery. Preparing posts no input and retains its native events independently
of later input or tap teardown. Deliver each operation once; canceling unrelated host work must not
discard consumed-input recovery. Completed input and stopped taps discard their capture metadata.

When a session expires during the callback for the same captured button's physical release,
the replay can already contain the release. A complete terminal replay delivered by
`GestureEventTap/replay(_:)` can replace that original event. To preserve the same behavior
with a host queue, use `GestureEventTap/enqueueReplay(_:accepting:)`. Return `true` only after
the queue owns the entire operation for delivery exactly once, in request order, even after
tap shutdown or host teardown. Return `false` without keeping or executing a refused operation.
Refusal preserves the original press metadata, so a host can immediately fall back to
`GestureEventTap/replay(_:)` in the same callback.
Ordinary preparation does not replace the physical release, and unrelated or reentrant input
cannot claim the release on behalf of another press.

For a queue that defers delivery of a restored drag start, use
``GestureEventTap/enqueueReplayFollowingThrough(_:inOrder:)``. When called from the exact live
movement callback of a consumed press, it puts that drag start and its subsequent same-button
events in the same retained FIFO. One continued movement can be pending at a time. The adapter
always queues the release. A new same-button press arriving before that release posts is forwarded
in queue order as raw input. The queue
must retain each operation immediately and run it later exactly once, in order on the main actor,
even across teardown. Stopping or losing the tap drops an incomplete start that never posted.
If its Down already posted, the queue receives a fallback Up. A pair whose physical Up was
already accepted remains deliverable. Physical input cannot be intercepted while a tap is disabled.

`GestureEventTap/stop(recoveringInput:)` disconnects the native tap before invoking recovery,
keeping the stopped capture metadata available only until the closure returns. This prevents
a queued replayed press from being lost when its own capture port is invalidated.
Reentrant input from a restarted tap uses its new capture metadata while stopped-input recovery
continues using the stopped capture, preserving each press's click count and modifier flags.
Replay without captured input uses a single click.

The native event source is installed only on the main run loop. Input handlers and
tap-disable recovery run synchronously there; no background callback is queued into
a later capture session. Stopping invalidates the Mach port before releasing its
run loop source, following the [Core Foundation teardown contract](https://developer.apple.com/documentation/corefoundation/cfmachport).

Keep command execution, trace UI, Accessibility permission presentation, privacy policy,
persistence, telemetry, analytics, and product behavior in the host app. This target translates
supported `CGEvent` values into `PointerGestureRecognition.GestureInputEvent` values, applies the
recognizer's event-disposition decision, and replays concrete Core Graphics events requested by
`GestureReplayRequest` values. It reports event-tap creation failure through
`GestureEventTap/start(handler:)`. Host applications are responsible for Accessibility
permission onboarding and recovery UI.
The `GesturePoint.init(_:)` and `GesturePoint.cgPoint`
bridges preserve coordinates and expose the core `GesturePoint.isFinite` check
for non-finite input. They do not flip coordinate axes; platform-specific
screen or view coordinate transforms belong in the host app.

## Topics

### Event Tap

- ``GestureEventTap``
- ``GestureEventTap/init(capturedButtons:)``
- ``GestureEventTap/start(handler:)``
- ``GestureEventTap/stop(recoveringInput:)``
- ``GestureEventTap/replay(_:)``
- ``GestureEventTap/enqueueReplay(_:accepting:)``
- ``GestureEventTap/enqueueReplayFollowingThrough(_:inOrder:)``
- ``GestureEventTap/prepareReplay(_:)``
