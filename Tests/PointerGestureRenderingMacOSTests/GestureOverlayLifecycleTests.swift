import AppKit
import Foundation
import Observation
import PointerGestureRecognition
import PointerGestureRendering
import PointerGestureRenderingSwiftUI
import XCTest

@testable import PointerGestureRenderingMacOS

@MainActor
final class GestureOverlayLifecycleTests: XCTestCase {
  func testRemovalDuringWindowCreationClosesUnclaimedResourceAndStopsBuild() {
    let recognizer = GestureTestRecognizer()
    var windows: [LifecycleWindow] = []
    let state = LifecycleState()
    state.coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay }, screensProvider: { overlayTestDisplays },
      notificationCenter: NotificationCenter(),
      makeWindow: { _, _, _ in
        let window = LifecycleWindow()
        windows.append(window)
        state.coordinator.removeOverlays()
        return window
      })
    state.coordinator.configureOverlays(gestureRecognizer: recognizer)
    XCTAssertEqual(windows.count, 1)
    XCTAssertEqual(windows[0].closeCount, 1)
    XCTAssertEqual(recognizer.observerCount, 0)
    state.coordinator = nil
  }

  func testRemovalDuringConfigurationDoesNotCreateAnyResource() {
    let recognizer = GestureTestRecognizer()
    var created = 0
    let state = LifecycleState()
    state.coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in
        state.coordinator.removeOverlays()
        return .liveOverlay
      },
      screensProvider: { overlayTestDisplays }, notificationCenter: NotificationCenter(),
      makeWindow: { _, _, _ in
        created += 1
        return LifecycleWindow()
      })
    state.coordinator.configureOverlays(gestureRecognizer: recognizer)
    XCTAssertEqual(created, 0)
    XCTAssertEqual(recognizer.observerCount, 0)
    state.coordinator = nil
  }

  func testReconfigurationFromWindowFactoryCannotRestoreOlderSource() {
    let old = GestureTestRecognizer()
    let fresh = GestureTestRecognizer()
    var windows: [LifecycleWindow] = []
    let state = LifecycleState()
    state.coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay }, screensProvider: { overlayTestDisplays },
      notificationCenter: NotificationCenter(),
      makeWindow: { _, _, _ in
        let window = LifecycleWindow()
        windows.append(window)
        if windows.count == 1 { state.coordinator.configureOverlays(gestureRecognizer: fresh) }
        return window
      })
    state.coordinator.configureOverlays(gestureRecognizer: old)
    XCTAssertEqual(windows.count, 3)
    XCTAssertEqual(windows.map(\.closeCount), [1, 0, 0])
    XCTAssertEqual(old.observerCount, 0)
    XCTAssertEqual(fresh.observerCount, 1)
    state.coordinator.removeOverlays()
    XCTAssertEqual(windows.map(\.closeCount), [1, 1, 1])
    XCTAssertEqual(fresh.observerCount, 0)
    state.coordinator = nil
  }

  func testReconfigurationFromCloseCallbackKeepsNewResourcesOwned() {
    let old = GestureTestRecognizer()
    let fresh = GestureTestRecognizer()
    var windows: [LifecycleWindow] = []
    let state = LifecycleState()
    state.coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay }, screensProvider: { overlayTestDisplays },
      notificationCenter: NotificationCenter(),
      makeWindow: { _, _, _ in
        let window = LifecycleWindow()
        windows.append(window)
        if windows.count == 1 {
          window.onClose = { state.coordinator.configureOverlays(gestureRecognizer: fresh) }
        }
        return window
      })
    state.coordinator.configureOverlays(gestureRecognizer: old)
    state.coordinator.configureOverlays(gestureRecognizer: old)
    XCTAssertEqual(windows.count, 4)
    XCTAssertEqual(windows.map(\.closeCount), [1, 1, 0, 0])
    XCTAssertEqual(old.observerCount, 0)
    XCTAssertEqual(fresh.observerCount, 1)
    state.coordinator.removeOverlays()
    XCTAssertTrue(windows.allSatisfy { $0.closeCount == 1 })
    state.coordinator = nil
  }

  func testRemovalFromTraceDeliveryStopsLaterSubscribersEvenIfResourceRetained() {
    let recognizer = GestureTestRecognizer()
    var sources: [GestureOverlayWindowTraceSource] = []
    var windows: [LifecycleWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { [overlayTestDisplays[0], overlayTestDisplays[0]] },
      notificationCenter: NotificationCenter(),
      makeWindow: { source, _, _ in
        sources.append(source)
        let window = LifecycleWindow()
        windows.append(window)
        return window
      })
    coordinator.configureOverlays(gestureRecognizer: recognizer)
    let cancelling = sources[0].observe {
      _ = $1
      if $0 != nil { coordinator.removeOverlays() }
    }
    var later: [GestureOverlayTrace?] = []
    let observing = sources[1].observe {
      _ = $1
      later.append($0)
    }
    showTrace(recognizer, end: 40)
    XCTAssertTrue(later.isEmpty)
    XCTAssertEqual(recognizer.observerCount, 0)
    XCTAssertTrue(windows.allSatisfy { $0.closeCount == 1 })
    var staleDeliveries = 0
    let stale = sources[1].observe { _, _ in staleDeliveries += 1 }
    showTrace(recognizer, end: 60)
    XCTAssertEqual(staleDeliveries, 0)
    withExtendedLifetime((cancelling, observing, stale)) {}
  }

  func testSubscriberAddedDuringPublicationReceivesCurrentTraceOnlyOnce() {
    let recognizer = GestureTestRecognizer()
    let fanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer, screenFrames: [overlayTestDisplays[0].frame])
    let source = fanout.source(visibleIn: overlayTestDisplays[0].frame, lineWidth: 4)
    var added: GestureObservationToken?
    var traces: [GestureOverlayTrace?] = []
    let first = source.observe { trace, _ in
      if trace != nil, added == nil {
        added = source.observe {
          _ = $1
          traces.append($0)
        }
      }
    }
    showTrace(recognizer, end: 40)
    XCTAssertEqual(traces.count, 1)
    showTrace(recognizer, end: 60)
    XCTAssertEqual(traces.count, 2)
    withExtendedLifetime((first, added)) {}
  }

  func testNewerTraceFromSubscriberCannotBeOverwrittenOnLaterDisplay() {
    let recognizer = GestureTestRecognizer()
    let fanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer, screenFrames: [overlayTestDisplays[0].frame])
    let source = fanout.source(visibleIn: overlayTestDisplays[0].frame, lineWidth: 4)
    let first = source.observe { trace, _ in
      if trace?.points.last?.x == 40 { self.showTrace(recognizer, end: 60) }
    }
    var later: [CGFloat] = []
    let second = source.observe {
      _ = $1
      if let end = $0?.points.last?.x { later.append(end) }
    }
    showTrace(recognizer, end: 40)
    XCTAssertEqual(later, [60])
    withExtendedLifetime((first, second)) {}
  }

  func testRebuildUsesNewLayoutAndFreshAppearanceWithOneSubscription() async {
    let recognizer = GestureTestRecognizer()
    let center = NotificationCenter()
    let state = LifecycleState()
    var widths: [CGFloat] = []
    var sources: [GestureOverlayWindowTraceSource] = []
    var windows: [LifecycleWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in
        GestureOverlayConfiguration(strokeStyle: .init(lineWidth: state.lineWidth))
      },
      screensProvider: { state.screens }, notificationCenter: center,
      makeWindow: { source, _, configuration in
        sources.append(source)
        widths.append(configuration.strokeStyle.lineWidth)
        let window = LifecycleWindow()
        windows.append(window)
        return window
      })
    showTrace(recognizer, end: 40)
    recognizer.normalizedDirectionPoints = [.init(x: 20, y: 30), .init(x: 60, y: 30)]
    coordinator.configureOverlays(gestureRecognizer: recognizer)
    XCTAssertNotNil(sources[0].initialTrace)
    state.lineWidth = 8
    state.screens = [overlayTestDisplays[0], overlayTestDisplays[1]]
    center.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
    await waitUntil { windows.count == 3 }
    XCTAssertEqual(widths, [4, 8, 8])
    XCTAssertEqual(sources[1].screenFrames, state.screens.map(\.frame))
    XCTAssertNotNil(sources[1].initialTrace)
    XCTAssertNil(sources[2].initialTrace)
    XCTAssertEqual(windows[0].closeCount, 1)
    XCTAssertEqual(recognizer.observerCount, 1)
    coordinator.removeOverlays()
  }

  func testReentrantStopDuringStreamFactoryTerminatesCapturedStream() async {
    let (stream, continuation) = AsyncStream.makeStream(of: Void.self)
    let terminated = expectation(description: "Captured stream terminates")
    continuation.onTermination = { _ in terminated.fulfill() }
    var created = 0
    let state = LifecycleState()
    state.coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay }, screensProvider: { overlayTestDisplays },
      notificationCenter: NotificationCenter(),
      configurationChangeEvents: {
        state.coordinator.removeOverlays()
        return stream
      },
      makeWindow: { _, _, _ in
        created += 1
        return LifecycleWindow()
      })
    state.coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    await fulfillment(of: [terminated], timeout: 1)
    XCTAssertEqual(created, 0)
    state.coordinator = nil
  }

  func testWindowContentPresentsFinalPendingTraceBeforeHiddenDeadline() async throws {
    let recognizer = GestureTestRecognizer()
    let fanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer, screenFrames: [overlayTestDisplays[0].frame])
    let source = fanout.source(visibleIn: overlayTestDisplays[0].frame, lineWidth: 4)
    let content = GestureOverlayWindowContent(
      traceSource: source, configuration: .init(minimumAnimationInterval: 0.08))
    showTrace(recognizer, end: 40)
    showTrace(recognizer, end: 41)
    XCTAssertEqual(content.model.trace?.points.last?.x, 40)
    recognizer.cancelActiveGesture()
    XCTAssertEqual(content.model.trace?.points.last?.x, 41)
    await waitUntil { content.model.trace == nil }
    content.close()
    showTrace(recognizer, end: 60)
    XCTAssertNil(content.model.trace)
  }

  func testRecognitionCancellationClearsPendingPresentationSynchronously() async throws {
    let source = FixedResultGestureEventSource(startResult: true)
    let recognizer = GestureRecognizer<String>(
      eventSource: source,
      configuration: .init(
        makeMatcher: { _ in .init() }, onReplayRequested: { _ in }, onMatch: { _ in }))
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    defer { recognizer.stop() }
    let fanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer,
      screenFrames: [overlayTestDisplays[0].frame])
    let content = GestureOverlayWindowContent(
      traceSource: fanout.source(visibleIn: overlayTestDisplays[0].frame, lineWidth: 4),
      configuration: .init(minimumAnimationInterval: 0.04))
    _ = source.send(.init(kind: .buttonDown(.secondary), location: .zero))
    _ = source.send(.init(kind: .buttonMoved(.secondary), location: .init(x: 40, y: 0)))
    _ = source.send(.init(kind: .buttonMoved(.secondary), location: .init(x: 41, y: 0)))
    XCTAssertNotNil(content.model.trace)
    recognizer.cancelActiveGesture()
    XCTAssertTrue(recognizer.snapshot.trace.discardsPendingPresentation)
    XCTAssertNil(content.model.trace)
    try await Task.sleep(for: .milliseconds(100))
    XCTAssertNil(content.model.trace)
    content.close()
  }

  func testWindowContentCloseCancelsSubscriptionExactlyOnceAndIgnoresLateCallback() {
    var callback: (@MainActor @Sendable (GestureOverlayTrace?, Bool) -> Void)?
    var cancelCount = 0
    let source = GestureOverlayWindowTraceSource(
      initialTrace: nil, initiallyDiscardsPendingPresentation: false,
      screenFrames: [overlayTestDisplays[0].frame],
      observe: { handler in
        callback = handler
        return GestureObservationToken { cancelCount += 1 }
      })
    var content: GestureOverlayWindowContent? = GestureOverlayWindowContent(
      traceSource: source, configuration: .liveOverlay)
    weak let model = content?.model
    content?.close()
    content?.close()
    callback?(
      GestureOverlayTrace(directions: [.right], points: [.zero, CGPoint(x: 40, y: 0)]), true)
    XCTAssertNil(model?.trace)
    XCTAssertEqual(cancelCount, 1)
    content = nil
    XCTAssertNil(model)
    XCTAssertEqual(cancelCount, 1)
  }

  func testAppearanceRefreshPreservesContentAndRebindsVisibilityForNewStrokeWidth() async throws {
    let recognizer = GestureTestRecognizer()
    let center = NotificationCenter()
    let state = LifecycleState()
    state.lineWidth = 2
    var contents: [GestureOverlayWindowContent] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .init(strokeStyle: .init(lineWidth: state.lineWidth)) },
      screensProvider: { [overlayTestDisplays[0]] }, notificationCenter: center,
      makeWindow: { source, _, config in
        let content = GestureOverlayWindowContent(traceSource: source, configuration: config)
        contents.append(content)
        return ContentResource(content: content)
      })
    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = [.init(x: 203, y: 30), .init(x: 220, y: 30)]
    coordinator.configureOverlays(gestureRecognizer: recognizer)
    let model = try XCTUnwrap(contents.first?.model)
    XCTAssertNil(model.trace)
    state.lineWidth = 10
    center.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
    await waitUntil { model.configuration.strokeStyle.lineWidth == 10 }
    XCTAssertEqual(contents.count, 1)
    XCTAssertNotNil(model.trace, "New stroke padding must re-evaluate the unchanged trace")
    XCTAssertEqual(recognizer.observerCount, 1)
    coordinator.removeOverlays()
    XCTAssertEqual(recognizer.observerCount, 0)
  }

  func testRemovalDuringInPlaceConfigurationStopsLaterResourceCallbacks() async {
    let center = NotificationCenter()
    var windows: [LifecycleWindow] = []
    let state = LifecycleState()
    state.coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay }, screensProvider: { overlayTestDisplays },
      notificationCenter: center,
      makeWindow: { _, _, _ in
        let window = LifecycleWindow()
        windows.append(window)
        return window
      })
    state.coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    var laterCalled = false
    windows[0].onConfigure = { state.coordinator.removeOverlays() }
    windows[1].onConfigure = { laterCalled = true }
    center.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
    await waitUntil { windows[0].closeCount == 1 }
    XCTAssertFalse(laterCalled)
    XCTAssertTrue(windows.allSatisfy { $0.closeCount == 1 })
  }

  private func showTrace(_ recognizer: GestureTestRecognizer, end: Double) {
    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = [.zero, GesturePoint(x: end, y: 0)]
    recognizer.notifyObservers()
  }
}

@MainActor
private final class LifecycleWindow: GestureOverlayWindowResource {
  var closeCount = 0
  var onClose: (@MainActor () -> Void)?
  var onConfigure: (@MainActor () -> Void)?
  func configure(
    traceSource _: GestureOverlayWindowTraceSource, configuration _: GestureOverlayConfiguration
  ) { onConfigure?() }
  func close() {
    closeCount += 1
    let action = onClose
    onClose = nil
    action?()
  }
}

@MainActor
private final class LifecycleState {
  var coordinator: GestureOverlayCoordinator!
  var screens = [overlayTestDisplays[0]]
  var lineWidth: CGFloat = 4
}

@MainActor
private final class ContentResource: GestureOverlayWindowResource {
  let content: GestureOverlayWindowContent
  init(content: GestureOverlayWindowContent) { self.content = content }
  func configure(
    traceSource: GestureOverlayWindowTraceSource, configuration: GestureOverlayConfiguration
  ) {
    content.configure(traceSource: traceSource, configuration: configuration)
  }
  func close() { content.close() }
}
