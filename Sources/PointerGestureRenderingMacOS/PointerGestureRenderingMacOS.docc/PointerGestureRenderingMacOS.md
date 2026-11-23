# `PointerGestureRenderingMacOS`

Optional macOS overlay resources for the shared pointer gesture renderer.

## Overview

`GestureOverlayCoordinator` owns one trace subscription, primary-first display
snapshots, per-display filtered delivery, transparent SwiftUI windows and coalesced
screen/configuration invalidations. Appearance and cadence update existing resources;
changed display frames rebuild them. Selecting this product does not add AppKit to the
portable rendering product or to recognition-only consumers.

Construction is inactive. Retain a coordinator and explicitly call
`GestureOverlayCoordinator/configureOverlays(gestureRecognizer:)` after the host
has selected its activation and permission policy. The configuration closure receives
`GestureOverlayDisplay` values; return the host's chosen styling and cadence.
The adapter reads no preferences and requests no permissions.

Call `GestureOverlayCoordinator/removeOverlays()` to invalidate queued rebuilds,
cancel trace delivery and close windows synchronously. Reconfiguration replaces the
old subscription. Release performs the same teardown. Resource callbacks may remove
or replace the active overlays without an older build reclaiming ownership.

## Display and presentation contracts

Display frames use AppKit desktop points. Preserve primary-first ordering, matching
`NSScreen.screens`. Recognition uses the top-left of that primary display, including
negative y values for displays above it. The adapter applies the same transform for
viewport filtering and SwiftUI drawing. Refresh capability is reported as a value;
the host chooses how to bound its rendering rate.

`PointerGestureRendering` owns validated traces, conservative bounds, geometry and
presentation cadence through Rust. `PointerGestureRenderingSwiftUI.GestureOverlayModel`
delivers those deadlines and preserves the final trace before hiding. AppKit neither
adds another geometry reducer nor hides windows on trace changes. Windows remain
ordered and click-through until their explicit display lifetime ends.

Per-display subscriptions suppress duplicate filtered payloads. A callback may cancel
itself, add a subscriber, or publish newer recognition state. Newer state invalidates
older fanout before another display receives it. Snapshot conversion happens once
per recognition update, independently of display count.

## Injection and native validation

Inject `screensProvider`, `notificationCenter`, `configurationChangeEvents` and
`makeWindow` to exercise the lifecycle without native windows. Factories receive
`GestureOverlayWindowTraceSource` and return `GestureOverlayWindowResource`
resources. Implement `configure(traceSource:configuration:)` to rebind filtered input
and update style/cadence without resetting presentation. Retain each observation
token and cancel it when rebinding or closing a custom resource.
Configuration streams are captured at observation start and cancelled at removal;
a delayed consumer never creates a replacement stream after teardown.

The default factory retains the existing borderless `NSWindow` and `NSHostingController`
bridge. Native window tests require `POINTER_GESTURES_NATIVE_WINDOWS=1` in an owned
isolated desktop. Headless lifecycle tests, compilation and actual OS behavior are
separate evidence.

Windows/.NET uses the same Rust geometry, trace filtering and presentation state.
Hosts may draw into their own controls or explicitly activate the optional native
Windows overlay. This macOS product is independent of that resource adapter.

## Topics

- ``GestureOverlayCoordinator``
- ``GestureOverlayDisplay``
- ``GestureOverlayWindowTraceSource``
- ``GestureOverlayWindowResource``

## Immediate Cancellation

Normal hiding retains pending final geometry briefly. Immediate cancellation
uses Rust's shared clear operation and discards both visible and queued geometry
and scheduled deadlines. Configuration and previously returned snapshots remain
valid. The macOS adapter forwards recognition's explicit
`discardsPendingPresentation` intent; custom adapters must preserve it too.
Honor `GestureOverlayWindowTraceSource/initiallyDiscardsPendingPresentation`
when binding a resource, and the Boolean delivered by its `observe` closure on
later updates. With a SwiftUI model, call `GestureOverlayModel.clear()` for a
discard and `GestureOverlayModel.update(_:)` for ordinary trace or hide updates.
Cadence inputs use seconds and coordinate lag only, with normalized nanoseconds
private to the shared reducer.
