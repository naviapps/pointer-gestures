import AppKit
import PointerGestureRecognition
import PointerGestureRendering
import PointerGestureRenderingSwiftUI
import SwiftUI
import XCTest

@testable import PointerGestureRenderingMacOS

@MainActor
final class GestureOverlayWindowTests: XCTestCase {
  func testDisplayFrameKeepsPrimaryOriginWhenAnotherScreenIsAboveIt() {
    let primary = CGRect(x: 0, y: 0, width: 200, height: 100)
    let above = CGRect(x: 0, y: 100, width: 200, height: 100)

    XCTAssertEqual(
      GestureOverlayWindow.displayCoordinateFrame(for: primary, screenFrames: [primary, above]),
      CGRect(x: 0, y: 0, width: 200, height: 100)
    )
    XCTAssertEqual(
      GestureOverlayWindow.displayCoordinateFrame(for: above, screenFrames: [primary, above]),
      CGRect(x: 0, y: -100, width: 200, height: 100)
    )
  }

  func testPrimaryTraceRemainsVisibleWhenAnotherScreenIsAboveIt() throws {
    let primary = CGRect(x: 0, y: 0, width: 200, height: 100)
    let above = CGRect(x: 0, y: 100, width: 200, height: 100)
    let trace = try XCTUnwrap(
      GestureOverlayTrace(
        directions: [.right],
        points: [CGPoint(x: 20, y: 30), CGPoint(x: 60, y: 30)]
      )
    )

    XCTAssertEqual(
      GestureOverlayWindow.trace(
        trace,
        visibleIn: primary,
        screenFrames: [primary, above],
        lineWidth: 4
      ),
      trace
    )
    XCTAssertNil(
      GestureOverlayWindow.trace(
        trace,
        visibleIn: above,
        screenFrames: [primary, above],
        lineWidth: 4
      )
    )
  }

  func testPrimaryScreenTopYUsesFirstFrameRatherThanTallestScreen() {
    let bottom = GestureOverlayWindow.primaryScreenTopY(
      screenFrames: [
        CGRect(x: 0, y: 0, width: 100, height: 400),
        CGRect(x: 0, y: 200, width: 100, height: 500),
      ],
      currentScreenFrame: CGRect(x: 0, y: 50, width: 100, height: 100)
    )

    XCTAssertEqual(bottom, 400, accuracy: 0.0001)
  }

  func testPrimaryScreenTopYUsesCurrentFrameWhenNoScreensAreProvided() {
    let bottom = GestureOverlayWindow.primaryScreenTopY(
      screenFrames: [],
      currentScreenFrame: CGRect(x: 10, y: 20, width: 100, height: 300)
    )

    XCTAssertEqual(bottom, 320, accuracy: 0.0001)
  }

  func testNativeWindowClearsCancelledPendingTraceWithoutReplacingResources() async throws {
    try XCTSkipUnless(
      ProcessInfo.processInfo.environment["POINTER_GESTURES_NATIVE_WINDOWS"] == "1",
      "Native windows require an owned isolated desktop.")
    let screen = try XCTUnwrap(NSScreen.screens.first)
    _ = NSApplication.shared
    let source = FixedResultGestureEventSource(startResult: true)
    let recognizer = GestureRecognizer<String>(
      eventSource: source,
      configuration: .init(
        makeMatcher: { _ in .init() }, onReplayRequested: { _ in }, onMatch: { _ in }))
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    let configuration = GestureOverlayConfiguration(
      minimumAnimationInterval: 0.5,
      strokeStyle: .init(color: .red, lineWidth: 8))
    let window = GestureOverlayWindow(
      traceSource: makeWindowTraceSource(
        gestureRecognizer: recognizer, screen: screen, configuration: configuration),
      display: .init(frame: screen.frame), configuration: configuration)
    defer {
      window.close()
      recognizer.stop()
    }
    let controller = window.contentViewController
    let number = window.windowNumber
    _ = source.send(.init(kind: .buttonDown(.secondary), location: .init(x: 80, y: 120)))
    _ = source.send(.init(kind: .buttonMoved(.secondary), location: .init(x: 200, y: 120)))
    await waitUntil { nativeRedPixels(window) > 5 }
    print(
      "Native overlay initial red pixels:",
      nativeRedPixels(window, saveAs: "macos-overlay-before-clear.png"))
    _ = source.send(.init(kind: .buttonMoved(.secondary), location: .init(x: 201, y: 120)))
    recognizer.cancelActiveGesture()
    await waitUntil { nativeRedPixels(window) == 0 }
    try await Task.sleep(for: .milliseconds(550))
    XCTAssertEqual(nativeRedPixels(window, saveAs: "macos-overlay-after-clear.png"), 0)
    XCTAssertTrue(window.contentViewController === controller)
    XCTAssertEqual(window.windowNumber, number)
    XCTAssertTrue(window.isVisible)
  }

  func testInitConfiguresWindowProperties() throws {
    try XCTSkipUnless(
      ProcessInfo.processInfo.environment["POINTER_GESTURES_NATIVE_WINDOWS"] == "1",
      "Native windows require explicit opt-in in an owned isolated desktop.")
    let screens = NSScreen.screens
    try XCTSkipIf(screens.isEmpty, "No screens available in this environment.")

    _ = NSApplication.shared

    let window = GestureOverlayWindow(
      traceSource: makeWindowTraceSource(
        gestureRecognizer: GestureTestRecognizer(),
        screen: screens[0],
        configuration: .liveOverlay
      ),
      display: GestureOverlayDisplay(frame: screens[0].frame),
      configuration: .liveOverlay
    )

    XCTAssertEqual(window.level, .screenSaver)
    XCTAssertEqual(window.ignoresMouseEvents, true)
    XCTAssertEqual(window.isOpaque, false)
    XCTAssertEqual(window.hasShadow, false)
    XCTAssertNotNil(window.contentViewController)
    XCTAssertNotNil(window.contentView)
    XCTAssertTrue(window.isVisible)
    let controller = window.contentViewController
    let number = window.windowNumber
    window.configure(
      traceSource: makeWindowTraceSource(
        gestureRecognizer: GestureTestRecognizer(), screen: screens[0], configuration: .liveOverlay),
      configuration: .init(strokeStyle: .init(lineWidth: 12), turnStyle: .rightAngle))
    XCTAssertTrue(window.contentViewController === controller)
    XCTAssertEqual(window.windowNumber, number)
    XCTAssertTrue(window.isVisible)
    window.close()
  }

  func testWindowShowsImmediatelyWhenRecognizerSnapshotAlreadyHasRenderableTrace() throws {
    try XCTSkipUnless(
      ProcessInfo.processInfo.environment["POINTER_GESTURES_NATIVE_WINDOWS"] == "1",
      "Native windows require explicit opt-in in an owned isolated desktop.")
    let screens = NSScreen.screens
    try XCTSkipIf(screens.isEmpty, "No screens available in this environment.")

    _ = NSApplication.shared

    let recognizer = GestureTestRecognizer()
    let visiblePoints = displayCoordinatePoints(
      in: screens[0].frame,
      screenFrames: screens.map(\.frame)
    )
    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = visiblePoints.map {
      GesturePoint(x: Double($0.x), y: Double($0.y))
    }

    let window = GestureOverlayWindow(
      traceSource: makeWindowTraceSource(
        gestureRecognizer: recognizer,
        screen: screens[0],
        configuration: .liveOverlay
      ),
      display: GestureOverlayDisplay(frame: screens[0].frame),
      configuration: .liveOverlay
    )
    defer { window.close() }

    XCTAssertTrue(window.isVisible)
  }

  func testWindowCloseCancelsTraceObservation() throws {
    try XCTSkipUnless(
      ProcessInfo.processInfo.environment["POINTER_GESTURES_NATIVE_WINDOWS"] == "1",
      "Native windows require explicit opt-in in an owned isolated desktop.")
    let screens = NSScreen.screens
    try XCTSkipIf(screens.isEmpty, "No screens available in this environment.")

    _ = NSApplication.shared

    let recognizer = ObservationCountingGestureRecognizer()
    let window = GestureOverlayWindow(
      traceSource: makeWindowTraceSource(
        gestureRecognizer: recognizer,
        screen: screens[0],
        configuration: .liveOverlay
      ),
      display: GestureOverlayDisplay(frame: screens[0].frame),
      configuration: .liveOverlay
    )
    XCTAssertEqual(recognizer.observerCount, 1)

    window.close()

    XCTAssertEqual(recognizer.observerCount, 0)
  }

  func testWindowCloseCancelsTraceObservationAndIgnoresLaterTraceUpdates() async throws {
    try XCTSkipUnless(
      ProcessInfo.processInfo.environment["POINTER_GESTURES_NATIVE_WINDOWS"] == "1",
      "Native windows require explicit opt-in in an owned isolated desktop.")
    let screens = NSScreen.screens
    try XCTSkipIf(screens.isEmpty, "No screens available in this environment.")

    _ = NSApplication.shared

    let recognizer = GestureTestRecognizer()
    let window = GestureOverlayWindow(
      traceSource: makeWindowTraceSource(
        gestureRecognizer: recognizer,
        screen: screens[0],
        configuration: .liveOverlay
      ),
      display: GestureOverlayDisplay(frame: screens[0].frame),
      configuration: .liveOverlay
    )

    let visiblePoints = displayCoordinatePoints(
      in: screens[0].frame,
      screenFrames: screens.map(\.frame)
    )
    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = visiblePoints.map {
      GesturePoint(x: Double($0.x), y: Double($0.y))
    }
    recognizer.notifyObservers()

    XCTAssertTrue(window.isVisible)

    recognizer.isTraceVisible = false
    recognizer.gestureDirections = []
    recognizer.normalizedDirectionPoints = []
    recognizer.notifyObservers()

    XCTAssertTrue(window.isVisible)

    window.close()

    XCTAssertFalse(window.isVisible)

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = visiblePoints.map {
      GesturePoint(x: Double($0.x), y: Double($0.y))
    }
    recognizer.notifyObservers()

    await assertNotEventually(timeout: 0.18) {
      window.isVisible
    }

    XCTAssertFalse(window.isVisible)
  }

  func testWindowStaysOrderedAfterTraceStopsRendering() async throws {
    try XCTSkipUnless(
      ProcessInfo.processInfo.environment["POINTER_GESTURES_NATIVE_WINDOWS"] == "1",
      "Native windows require explicit opt-in in an owned isolated desktop.")
    let screens = NSScreen.screens
    try XCTSkipIf(screens.isEmpty, "No screens available in this environment.")

    _ = NSApplication.shared

    let recognizer = GestureTestRecognizer()
    let window = GestureOverlayWindow(
      traceSource: makeWindowTraceSource(
        gestureRecognizer: recognizer,
        screen: screens[0],
        configuration: .liveOverlay
      ),
      display: GestureOverlayDisplay(frame: screens[0].frame),
      configuration: .liveOverlay
    )
    defer { window.close() }

    let visiblePoints = displayCoordinatePoints(
      in: screens[0].frame,
      screenFrames: screens.map(\.frame)
    )
    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = visiblePoints.map {
      GesturePoint(x: Double($0.x), y: Double($0.y))
    }
    recognizer.notifyObservers()

    XCTAssertTrue(window.isVisible)

    recognizer.isTraceVisible = false
    recognizer.gestureDirections = []
    recognizer.normalizedDirectionPoints = []
    recognizer.notifyObservers()

    await assertNotEventually(timeout: 0.18) {
      !window.isVisible
    }
    XCTAssertTrue(window.isVisible)
  }

  func testWindowStaysOrderedWhenHiddenTraceKeepsStalePath() async throws {
    try XCTSkipUnless(
      ProcessInfo.processInfo.environment["POINTER_GESTURES_NATIVE_WINDOWS"] == "1",
      "Native windows require explicit opt-in in an owned isolated desktop.")
    let screens = NSScreen.screens
    try XCTSkipIf(screens.isEmpty, "No screens available in this environment.")

    _ = NSApplication.shared

    let recognizer = GestureTestRecognizer()
    let window = GestureOverlayWindow(
      traceSource: makeWindowTraceSource(
        gestureRecognizer: recognizer,
        screen: screens[0],
        configuration: .liveOverlay
      ),
      display: GestureOverlayDisplay(frame: screens[0].frame),
      configuration: .liveOverlay
    )
    defer { window.close() }

    let visiblePoints = displayCoordinatePoints(
      in: screens[0].frame,
      screenFrames: screens.map(\.frame)
    )
    let tracePoints = visiblePoints.map {
      GesturePoint(x: Double($0.x), y: Double($0.y))
    }

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = tracePoints
    recognizer.notifyObservers()

    XCTAssertTrue(window.isVisible)

    recognizer.isTraceVisible = false
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = tracePoints
    recognizer.notifyObservers()

    XCTAssertTrue(window.isVisible)
    await assertNotEventually(timeout: 0.18) {
      !window.isVisible
    }
    XCTAssertTrue(window.isVisible)
  }

  func testWindowKeepsOverlayOrderedAcrossBriefHiddenGap() async throws {
    try XCTSkipUnless(
      ProcessInfo.processInfo.environment["POINTER_GESTURES_NATIVE_WINDOWS"] == "1",
      "Native windows require explicit opt-in in an owned isolated desktop.")
    let screens = NSScreen.screens
    try XCTSkipIf(screens.isEmpty, "No screens available in this environment.")

    _ = NSApplication.shared

    let recognizer = GestureTestRecognizer()
    let window = GestureOverlayWindow(
      traceSource: makeWindowTraceSource(
        gestureRecognizer: recognizer,
        screen: screens[0],
        configuration: .liveOverlay
      ),
      display: GestureOverlayDisplay(frame: screens[0].frame),
      configuration: .liveOverlay
    )
    defer { window.close() }

    let visiblePoints = displayCoordinatePoints(
      in: screens[0].frame,
      screenFrames: screens.map(\.frame)
    )

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = visiblePoints.map {
      GesturePoint(x: Double($0.x), y: Double($0.y))
    }
    recognizer.notifyObservers()

    XCTAssertTrue(window.isVisible)

    recognizer.isTraceVisible = false
    recognizer.gestureDirections = []
    recognizer.normalizedDirectionPoints = []
    recognizer.notifyObservers()

    XCTAssertTrue(window.isVisible)
    await assertNotEventually(timeout: 0.06) {
      !window.isVisible
    }

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = visiblePoints.map {
      GesturePoint(x: Double($0.x), y: Double($0.y))
    }
    recognizer.notifyObservers()

    await assertNotEventually(timeout: 0.18) {
      !window.isVisible
    }
    XCTAssertTrue(window.isVisible)
  }

  func testWindowStaysOrderedAcrossRepeatedHiddenCycles() async throws {
    try XCTSkipUnless(
      ProcessInfo.processInfo.environment["POINTER_GESTURES_NATIVE_WINDOWS"] == "1",
      "Native windows require explicit opt-in in an owned isolated desktop.")
    let screens = NSScreen.screens
    try XCTSkipIf(screens.isEmpty, "No screens available in this environment.")

    _ = NSApplication.shared

    let recognizer = GestureTestRecognizer()
    let window = GestureOverlayWindow(
      traceSource: makeWindowTraceSource(
        gestureRecognizer: recognizer,
        screen: screens[0],
        configuration: .liveOverlay
      ),
      display: GestureOverlayDisplay(frame: screens[0].frame),
      configuration: .liveOverlay
    )
    defer { window.close() }

    let visiblePoints = displayCoordinatePoints(
      in: screens[0].frame,
      screenFrames: screens.map(\.frame)
    )
    let tracePoints = visiblePoints.map {
      GesturePoint(x: Double($0.x), y: Double($0.y))
    }

    for _ in 0..<4 {
      recognizer.isTraceVisible = true
      recognizer.gestureDirections = [.right]
      recognizer.normalizedDirectionPoints = tracePoints
      recognizer.notifyObservers()

      XCTAssertTrue(window.isVisible)

      recognizer.isTraceVisible = false
      recognizer.gestureDirections = []
      recognizer.normalizedDirectionPoints = []
      recognizer.notifyObservers()

      await assertNotEventually(timeout: 0.18) {
        !window.isVisible
      }
      XCTAssertTrue(window.isVisible)
    }
  }

  func testTraceForScreenHidesRenderableTraceOutsideScreenDisplayFrame() {
    let trace = GestureOverlayTrace(
      directions: [.right],
      points: [CGPoint(x: 250, y: 10), CGPoint(x: 300, y: 10)]
    )

    let filteredTrace = GestureOverlayWindow.trace(
      trace,
      visibleIn: CGRect(x: 0, y: 0, width: 100, height: 100),
      screenFrames: [CGRect(x: 0, y: 0, width: 100, height: 100)],
      lineWidth: 4
    )

    XCTAssertNil(filteredTrace)
  }

  func testOverlayTraceRejectsInvalidPointCount() {
    let trace = GestureOverlayTrace(
      directions: [.right],
      points: [CGPoint(x: 10, y: 10)]
    )

    XCTAssertNil(trace)
  }

  func testTraceForScreenKeepsRenderableTraceIntersectingScreenDisplayFrame() {
    let trace = GestureOverlayTrace(
      directions: [.right],
      points: [CGPoint(x: 96, y: 10), CGPoint(x: 140, y: 10)]
    )

    let filteredTrace = GestureOverlayWindow.trace(
      trace,
      visibleIn: CGRect(x: 0, y: 0, width: 100, height: 100),
      screenFrames: [CGRect(x: 0, y: 0, width: 100, height: 100)],
      lineWidth: 4
    )

    XCTAssertEqual(filteredTrace, trace)
  }

  func testTraceForScreenUsesStrokeRadiusForVisibilityPadding() {
    let trace = GestureOverlayTrace(
      directions: [.right],
      points: [CGPoint(x: 106, y: 10), CGPoint(x: 140, y: 10)]
    )

    let filteredTrace = GestureOverlayWindow.trace(
      trace,
      visibleIn: CGRect(x: 0, y: 0, width: 100, height: 100),
      screenFrames: [CGRect(x: 0, y: 0, width: 100, height: 100)],
      lineWidth: 10
    )

    XCTAssertNil(filteredTrace)
  }

  func testTraceForScreenKeepsRenderableTraceWhenTailPointIntersectsScreenDisplayFrame() {
    let trace = GestureOverlayTrace(
      directions: [.right],
      points: [CGPoint(x: 150, y: 10), CGPoint(x: 180, y: 10)],
      tailPoint: CGPoint(x: 96, y: 10)
    )

    let filteredTrace = GestureOverlayWindow.trace(
      trace,
      visibleIn: CGRect(x: 0, y: 0, width: 100, height: 100),
      screenFrames: [CGRect(x: 0, y: 0, width: 100, height: 100)],
      lineWidth: 4
    )

    XCTAssertEqual(filteredTrace, trace)
  }

  func testTraceForScreenUsesDisplayCoordinateFrameForVerticalScreenOffsets() {
    let bottomScreen = CGRect(x: 0, y: 0, width: 100, height: 100)
    let topScreen = CGRect(x: 0, y: 100, width: 100, height: 100)
    let trace = GestureOverlayTrace(
      directions: [.right],
      points: [CGPoint(x: 10, y: 50), CGPoint(x: 40, y: 50)]
    )

    let filteredTrace = GestureOverlayWindow.trace(
      trace,
      visibleIn: bottomScreen,
      screenFrames: [bottomScreen, topScreen],
      lineWidth: 4
    )

    XCTAssertEqual(filteredTrace, trace)
  }

  func testTraceForScreenHidesTraceInDifferentVerticalScreenDisplayFrame() {
    let bottomScreen = CGRect(x: 0, y: 0, width: 100, height: 100)
    let topScreen = CGRect(x: 0, y: 100, width: 100, height: 100)
    let trace = GestureOverlayTrace(
      directions: [.right],
      points: [CGPoint(x: 10, y: 50), CGPoint(x: 40, y: 50)]
    )

    let filteredTrace = GestureOverlayWindow.trace(
      trace,
      visibleIn: topScreen,
      screenFrames: [bottomScreen, topScreen],
      lineWidth: 4
    )

    XCTAssertNil(filteredTrace)
  }

}

@MainActor
private func makeWindowTraceSource(
  gestureRecognizer: any GestureRecognizerObserving,
  screen: NSScreen,
  configuration: GestureOverlayConfiguration
) -> GestureOverlayWindowTraceSource {
  let screenFrames = NSScreen.screens.map(\.frame)
  let lineWidth = configuration.strokeStyle.lineWidth
  return GestureOverlayWindowTraceSource(
    initialTrace: GestureOverlayWindow.trace(
      GestureOverlayTraceConverter().convert(gestureRecognizer.snapshot.trace),
      visibleIn: screen.frame,
      screenFrames: screenFrames,
      lineWidth: lineWidth
    ),
    initiallyDiscardsPendingPresentation: gestureRecognizer.snapshot.trace
      .discardsPendingPresentation,
    screenFrames: screenFrames,
    observe: { handler in
      gestureRecognizer.observeTrace { trace in
        handler(
          GestureOverlayWindow.trace(
            GestureOverlayTraceConverter().convert(trace),
            visibleIn: screen.frame,
            screenFrames: screenFrames,
            lineWidth: lineWidth
          ), trace.discardsPendingPresentation
        )
      }
    }
  )
}

private func displayCoordinatePoints(
  in screenFrame: CGRect,
  screenFrames: [CGRect]
) -> [CGPoint] {
  let standardizedFrame = screenFrame.standardized
  let primaryTopY = (screenFrames.first ?? standardizedFrame).standardized.maxY
  let displayFrame = CGRect(
    x: standardizedFrame.origin.x,
    y: primaryTopY - standardizedFrame.maxY,
    width: standardizedFrame.width,
    height: standardizedFrame.height
  )
  let midY = displayFrame.midY
  return [
    CGPoint(x: displayFrame.midX - 10, y: midY),
    CGPoint(x: displayFrame.midX + 10, y: midY),
  ]
}

@MainActor
private final class ObservationCountingGestureRecognizer: GestureRecognizerObserving {
  private var observers: [UUID: @MainActor @Sendable (GestureRecognizerState.Trace) -> Void] = [:]

  var observerCount: Int {
    observers.count
  }

  var snapshot: GestureRecognizerState {
    GestureRecognizerState(
      status: .init(),
      trace: .init()
    )
  }

  func observe(
    _: @escaping @MainActor @Sendable (GestureRecognizerState) -> Void
  ) -> GestureObservationToken {
    GestureObservationToken {}
  }

  func observeTrace(
    _ handler: @escaping @MainActor @Sendable (GestureRecognizerState.Trace) -> Void
  ) -> GestureObservationToken {
    let id = UUID()
    observers[id] = handler
    handler(snapshot.trace)
    return GestureObservationToken { [weak self] in
      self?.observers.removeValue(forKey: id)
    }
  }
}

@MainActor
private func nativeRedPixels(_ window: NSWindow, saveAs: String? = nil) -> Int {
  // Render the actual native window's SwiftUI root, including Canvas content.
  guard let hosting = window.contentViewController as? NSHostingController<GestureOverlayView>,
    let image = ImageRenderer(
      content: hosting.rootView.frame(
        width: window.frame.width, height: window.frame.height)
    ).cgImage
  else { return -1 }
  let bitmap = NSBitmapImageRep(cgImage: image)
  if let saveAs, let directory = ProcessInfo.processInfo.environment["POINTER_GESTURE_UI_RESULTS"],
    let png = bitmap.representation(using: .png, properties: [:])
  {
    try? png.write(to: URL(fileURLWithPath: directory).appendingPathComponent(saveAs))
  }
  var count = 0
  for y in stride(from: 0, to: bitmap.pixelsHigh, by: 4) {
    for x in stride(from: 0, to: bitmap.pixelsWide, by: 4) {
      if let color = bitmap.colorAt(x: x, y: y)?.usingColorSpace(.sRGB),
        color.alphaComponent > 0.5, color.redComponent > 0.7,
        color.redComponent > color.greenComponent * 1.5,
        color.redComponent > color.blueComponent * 1.5
      {
        count += 1
      }
    }
  }
  return count
}
