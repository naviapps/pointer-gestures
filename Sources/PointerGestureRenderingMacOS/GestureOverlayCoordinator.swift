import AppKit
import PointerGestureRecognition
import PointerGestureRenderingSwiftUI

/// An overlay resource updated in place and closed when its display lifetime ends.
@MainActor
public protocol GestureOverlayWindowResource: AnyObject {
  /// Rebinds filtered input and updates appearance without resetting presentation.
  func configure(
    traceSource: GestureOverlayWindowTraceSource, configuration: GestureOverlayConfiguration)
  /// Cancels subscriptions and releases native presentation resources.
  func close()
}

/// Owns display resources, one recognition subscription, and coalesced invalidations.
///
/// Construction creates no windows. The host explicitly activates and removes
/// overlays, supplies appearance and cadence, and owns all permission policy.
@MainActor
public final class GestureOverlayCoordinator {
  private let configurationProvider:
    @MainActor (GestureOverlayDisplay) -> GestureOverlayConfiguration
  private let screensProvider: @MainActor () -> [GestureOverlayDisplay]
  private let invalidationObserver: GestureOverlayInvalidationObserver
  private let makeWindow:
    @MainActor (GestureOverlayWindowTraceSource, GestureOverlayDisplay, GestureOverlayConfiguration)
      -> any GestureOverlayWindowResource
  private var currentGestureRecognizer: (any GestureRecognizerObserving)?
  private var currentTraceFanout: GestureOverlayTraceFanout?
  private var overlayWindows: [any GestureOverlayWindowResource] = []
  private var currentScreenFrames: [CGRect] = []
  private var generation: UInt64 = 0

  /// Creates an inactive adapter with explicit host styling and injectable resources.
  ///
  /// `screensProvider` must return primary-first AppKit display snapshots. The
  /// default window factory creates transparent, nonactivating, click-through
  /// SwiftUI overlays. Inject a factory for tests that must create no windows.
  /// `configurationChangeEvents` is captured synchronously when observation starts;
  /// its producer is cancelled on removal. Rebuild invalidations coalesce for 20 ms.
  public init(
    configurationProvider:
      @escaping @MainActor (GestureOverlayDisplay) -> GestureOverlayConfiguration,
    screensProvider: @escaping @MainActor () -> [GestureOverlayDisplay] = {
      GestureOverlayDisplay.current
    },
    notificationCenter: NotificationCenter = .default,
    configurationChangeEvents: (@MainActor () -> AsyncStream<Void>)? = nil,
    makeWindow:
      (
        @MainActor (
          GestureOverlayWindowTraceSource, GestureOverlayDisplay, GestureOverlayConfiguration
        ) -> any GestureOverlayWindowResource
      )? = nil
  ) {
    self.configurationProvider = configurationProvider
    self.screensProvider = screensProvider
    invalidationObserver = GestureOverlayInvalidationObserver(
      notificationCenter: notificationCenter,
      configurationChangeEvents: configurationChangeEvents,
      coalescingDelayNanoseconds: 20_000_000
    )
    self.makeWindow =
      makeWindow ?? {
        GestureOverlayWindow(traceSource: $0, display: $1, configuration: $2)
      }
  }

  isolated deinit { removeOverlays() }

  /// Activates overlays for the current layout, replacing any previous source.
  ///
  /// Displays share one trace observation. Reentrant removal or reconfiguration
  /// from injected resource callbacks invalidates the older build immediately.
  public func configureOverlays(gestureRecognizer: any GestureRecognizerObserving) {
    generation &+= 1
    let revision = generation
    currentGestureRecognizer = gestureRecognizer
    invalidationObserver.cancelPendingRebuild()
    invalidationObserver.start { [weak self] in
      guard let self, let recognizer = currentGestureRecognizer else { return }
      refreshOverlays(gestureRecognizer: recognizer)
    }
    guard generation == revision else { return }
    rebuildOverlays(gestureRecognizer: gestureRecognizer)
  }

  /// Stops invalidations and trace delivery before closing all owned resources.
  /// Safe to repeat or call synchronously from a trace or resource callback.
  public func removeOverlays() {
    generation &+= 1
    let revision = generation
    currentGestureRecognizer = nil
    invalidationObserver.stop()
    guard generation == revision else { return }
    removeOverlayWindows()
  }

  private func rebuildOverlays(gestureRecognizer: any GestureRecognizerObserving) {
    generation &+= 1
    let revision = generation
    removeOverlayWindows()
    guard generation == revision else { return }
    let screens = screensProvider()
    guard generation == revision, !screens.isEmpty else { return }
    let fanout = GestureOverlayTraceFanout(
      gestureRecognizer: gestureRecognizer, screenFrames: screens.map(\.frame))
    guard generation == revision else {
      fanout.stop()
      return
    }
    currentTraceFanout = fanout
    currentScreenFrames = screens.map(\.frame)
    for screen in screens {
      let configuration = configurationProvider(screen)
      guard generation == revision else { return }
      let source = fanout.source(
        visibleIn: screen.frame, lineWidth: configuration.strokeStyle.lineWidth)
      let window = makeWindow(source, screen, configuration)
      guard generation == revision else {
        window.close()
        return
      }
      overlayWindows.append(window)
    }
  }

  private func refreshOverlays(gestureRecognizer: any GestureRecognizerObserving) {
    generation &+= 1
    let revision = generation
    let screens = screensProvider()
    guard generation == revision else { return }
    guard let fanout = currentTraceFanout,
      screens.map(\.frame) == currentScreenFrames,
      screens.count == overlayWindows.count
    else {
      rebuildOverlays(gestureRecognizer: gestureRecognizer)
      return
    }
    let windows = overlayWindows
    for (screen, window) in zip(screens, windows) {
      let configuration = configurationProvider(screen)
      guard generation == revision else { return }
      let source = fanout.source(
        visibleIn: screen.frame, lineWidth: configuration.strokeStyle.lineWidth)
      window.configure(traceSource: source, configuration: configuration)
      guard generation == revision else { return }
    }
  }

  private func removeOverlayWindows() {
    // Detach ownership before invoking injected cancellation/close callbacks.
    let windows = overlayWindows
    overlayWindows.removeAll()
    currentScreenFrames.removeAll()
    let fanout = currentTraceFanout
    currentTraceFanout = nil
    fanout?.stop()
    for window in windows { window.close() }
  }
}
