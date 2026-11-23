# `PointerGestureRendering`

Plan pointer gesture geometry with the shared Rust implementation.

## Overview

Use `GesturePath/liveTrace(directions:points:cornerRadiusScale:tailPoint:)`
for a validated live path and `GesturePath/pattern(_:canvasSize:padding:lineWidth:showsArrowhead:cornerRadiusScale:)`
for a compact preview. Coordinates use the caller's drawing space. Paths own command
values copied from owned native geometry. Commands use moves, lines, cubic Bézier curves
and closing a subpath, with identical planning for native renderers. Captured runs
are projected onto cardinal axes from the stroke origin. Segment lengths are
checked after projection, including when the live tail replaces the final
endpoint. Default corners use a 24-point radius limit, bounded by half of
neighboring segments. The planner includes optional tail updates along the
captured axes and conservative visibility checks. Paths do not create windows,
views, colors or animation tasks.

Use `GesturePath.planLiveTrace` and `GesturePath.planPattern` for a typed
`Result` when input is editable or a live trace might be incomplete. The
shared Rust planner reports missing directions, a mismatched point count,
or coordinates that cannot be represented as finite geometry. Unknown native
statuses or results missing their geometry owner report an invalid native
result. The optional methods return `nil` on failure.

`PointerGestureRenderingSwiftUI` converts these values using `swiftUIPath` and
provides live overlay and preview views. The optional `PointerGestureRenderingMacOS`
adapter owns macOS overlay resources and display lifecycle. Hosts own recognition,
activation, permissions and appearance policy.

## Topics

- ``GesturePath``
- ``GesturePath/Command``
- ``GesturePath/commands``
- ``GestureGeometryError``
- ``GesturePatternGeometry``
- ``GesturePatternGeometry/path``
- ``GesturePatternGeometry/arrowhead``

## Portable presentation

`GestureOverlayTrace` owns Rust-validated immutable trace data and cached bounds.
`GestureOverlayTraceConverter` converts recognition snapshots without retaining
their raw history or a recognizer. Use one serial converter to reuse endpoint
storage while the tail changes. Hidden, empty or invalid inputs release the
converter's cached geometry and return `nil`. Previously returned traces remain valid.
The Rust renderer determines whether the directions and endpoints can share the
previous snapshot's geometry.
`GestureOverlayTrace/replacingTailPoint(with:)` shares native coordinates and cached
bounds with a prior immutable trace, avoiding their repeated copies on tail-only motion.
`GestureOverlayState` coalesces ordinary movement, immediately publishes direction
changes and rate-limited large movement, and presents the final pending trace before
hiding. Pass monotonic nanoseconds to `update` and `tick`; schedule the next tick at
`GestureOverlayUpdate/deadlineNanoseconds`. Snapshots outlive later calls and the
state object. Access one state serially. Rust clamps clock regressions and saturates
deadlines; it never creates timers or invokes callbacks.
Presentation updates compare their change flag, trace value and deadline.
Returning to the currently presented geometry cancels an older queued movement or hide;
an unchanged trace can therefore clear the next deadline.

`GestureOverlayTiming` normalizes non-positive, non-finite or unrepresentable
intervals to 1/60 second and invalid lag distances to 24 points. SwiftUI uses this
same state and only delivers deadlines and observation. Use
`GestureOverlayState/configure(timing:nowNanoseconds:)` to change cadence without
resetting visible/pending geometry. New deadlines remain anchored to the last actual
presentation, including final hiding; already due work is returned immediately.

- ``GestureOverlayTrace``
- ``GestureOverlayTrace/replacingTailPoint(with:)``
- ``GestureOverlayTraceConverter``
- ``GestureOverlayTraceConverter/init()``
- ``GestureOverlayTraceConverter/convert(_:)``
- ``GestureOverlayTiming``
- ``GestureOverlayState``
- ``GestureOverlayUpdate``

Viewport filtering uses immutable, Sendable `GestureOverlayTraceFilter` values
over each trace's Rust-owned bounds. Filters select payloads without UI or window state.

- ``GestureOverlayTraceFilter``

## Immediate Cancellation

Normal hiding retains pending final geometry briefly. Use
``GestureOverlayState/clear(nowNanoseconds:)`` for immediate cancellation. It
uses Rust's shared clear operation and discards both visible and queued geometry
and scheduled deadlines. Configuration and previously returned snapshots remain
valid. The AppKit adapter forwards recognition's explicit
`discardsPendingPresentation` intent; custom adapters must preserve it too.
Cadence inputs use seconds and coordinate lag only, with normalized nanoseconds
private to the shared reducer.
