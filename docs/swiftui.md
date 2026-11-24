# `PointerGestureRenderingSwiftUI`

Render live pointer gesture traces and compact direction-pattern previews in SwiftUI.

## Overview

Geometry planning belongs to `PointerGestureRendering`. This adapter converts its
command values to native SwiftUI paths and owns display styling, observation and
timer delivery. Trace validation, visibility and update cadence belong to Rust
through `PointerGestureRendering.GestureOverlayState`.

PointerGestureRenderingSwiftUI is the rendering companion to PointerGestureRecognition. It
uses PointerGestureRecognition's `GestureDirection` as the shared recognition and
rendering vocabulary.

For a live overlay, the host owns a `GestureOverlayModel` and submits validated
`PointerGestureRendering.GestureOverlayTrace` values as recognition changes. `GestureOverlayView`
observes the model and renders it using a `GestureOverlayCoordinateMapping`.
A host can own recognition observation and window lifecycle directly, or select
`PointerGestureRenderingMacOS` for shared trace observation and macOS overlay
resources. Host activation and permission policy remain outside both adapters.

Use `GesturePatternView` for static previews in menus, lists, and settings.

## Live Overlay

```swift
import CoreGraphics
import PointerGestureRecognition
import PointerGestureRendering
import PointerGestureRenderingSwiftUI

let model = GestureOverlayModel(
  configuration: GestureOverlayConfiguration(
    strokeStyle: GestureStrokeStyle(lineWidth: 4),
    turnStyle: .rounded
  )
)

let view = GestureOverlayView(
  model: model,
  coordinateMapping: GestureOverlayCoordinateMapping(
    windowOrigin: CGPoint(x: 0, y: 0),
    displayCoordinateMaxY: 1_080
  )
)

if let trace = GestureOverlayTrace(
  directions: [.right],
  points: [CGPoint(x: 0, y: 0), CGPoint(x: 48, y: 0)],
  tailPoint: CGPoint(x: 52, y: 6)
) {
  model.update(trace)
}
```

The model delivers the shared reducer's deadlines on the main actor. A delayed
timer uses the original deadline.
Use `GestureOverlayModel.configure(_:)` to update host style and cadence in place.
Configuration changes and updates from a trace observation callback are applied
in order after the current publication, including a callback that hides the trace
or starts another gesture.

## Coordinate Mapping

`GestureOverlayCoordinateMapping` accepts only the values used by rendering:
the overlay window origin and the maximum y value of the trace's global display
coordinate system. Trace points and the mapping must use the same display-space
convention.

## Pattern Previews

```swift
import PointerGestureRecognition
import PointerGestureRendering
import PointerGestureRenderingSwiftUI

GesturePatternView(
  pattern: [.right, .down, .left],
  configuration: GesturePatternConfiguration(
    showsArrowhead: true,
    padding: 8
  )
)
```

`turnStyle` controls preview rounding relative to the canvas; `.rightAngle`
keeps corners sharp. Scale the canvas, stroke and padding together for matching
previews at different sizes; the arrowhead scales with them. A host can also draw
one logical canvas and scale the complete view to preserve its proportions across
different aspect ratios. The arrowhead must fit the final run and remain visible
beyond the round stroke cap; it is omitted when space is insufficient.

The final run extends past preceding reversal lanes so short back-and-forth patterns
retain a legible terminal arrow, including at thick stroke widths.
Successive same-axis reversals use parallel return lanes rather than retracing an
earlier run. This is an illustrative layout, not captured movement distances.
Very long patterns remain bounded but may be too dense to distinguish every run
at thumbnail sizes; hosts should retain a direction summary or count alongside them.

Empty patterns render no path. Path scaling and arrowhead geometry stay internal
to keep the public API focused on reusable rendering choices.

## Responsibility Boundary

The package owns validated trace presentation, update cadence, coordinate
mapping, path planning, styling, and SwiftUI rendering. Host apps own pointer
events, recognition policy, permissions, persistence and product-specific behavior.
The optional `PointerGestureRenderingMacOS` product owns macOS overlay windows,
trace fanout and display rebuilds; SwiftUI-only consumers can supply their own resources.

## Immediate Cancellation

Call `GestureOverlayModel.update(nil)` for a normal hide. Call
`GestureOverlayModel.clear()` when recognition reports
`discardsPendingPresentation`. See
[shared presentation behavior](../README.md#input-and-geometry-contracts).
