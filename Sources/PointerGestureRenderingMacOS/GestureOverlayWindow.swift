import AppKit
import PointerGestureRecognition
import PointerGestureRendering
import PointerGestureRenderingSwiftUI
import SwiftUI

final class GestureOverlayWindow: NSWindow, GestureOverlayWindowResource {
  private var overlayContent: GestureOverlayWindowContent?

  init(
    traceSource: GestureOverlayWindowTraceSource,
    display: GestureOverlayDisplay,
    configuration: GestureOverlayConfiguration = .liveOverlay
  ) {
    super.init(
      contentRect: display.frame,
      styleMask: [.borderless],
      backing: .buffered,
      defer: false
    )

    level = .screenSaver
    backgroundColor = .clear
    isOpaque = false
    hasShadow = false
    ignoresMouseEvents = true
    collectionBehavior = [.canJoinAllSpaces, .stationary, .fullScreenAuxiliary]
    isReleasedWhenClosed = false

    let content = GestureOverlayWindowContent(
      traceSource: traceSource, configuration: configuration)
    overlayContent = content

    let view = GestureOverlayView(
      model: content.model,
      coordinateMapping: GestureOverlayCoordinateMapping(
        windowOrigin: display.frame.origin,
        displayCoordinateMaxY: Self.primaryScreenTopY(
          screenFrames: traceSource.screenFrames,
          currentScreenFrame: display.frame
        )
      )
    )
    let hostingController = NSHostingController(rootView: view)
    hostingController.view.frame = CGRect(origin: .zero, size: display.frame.size)
    hostingController.view.autoresizingMask = [.width, .height]
    contentViewController = hostingController
    contentView = hostingController.view

    setFrame(display.frame, display: false)
    orderFrontRegardless()
  }

  func configure(
    traceSource: GestureOverlayWindowTraceSource, configuration: GestureOverlayConfiguration
  ) {
    overlayContent?.configure(traceSource: traceSource, configuration: configuration)
  }

  override func close() {
    let content = overlayContent
    overlayContent = nil
    content?.close()
    contentViewController = nil
    contentView = nil
    super.close()
  }

  static func primaryScreenTopY(
    screenFrames: [CGRect],
    currentScreenFrame: CGRect
  ) -> CGFloat {
    // NSScreen.screens keeps the primary screen first. CGEvent.location uses
    // that display's top-left as (0, 0), even when another screen sits above it.
    (screenFrames.first ?? currentScreenFrame).standardized.maxY
  }

  static func trace(
    _ trace: GestureOverlayTrace?,
    visibleIn screenFrame: CGRect,
    screenFrames: [CGRect],
    lineWidth: CGFloat
  ) -> GestureOverlayTrace? {
    Self.trace(
      trace,
      visibleInDisplayCoordinateFrame: displayCoordinateFrame(
        for: screenFrame,
        screenFrames: screenFrames
      ),
      lineWidth: lineWidth
    )
  }

  static func trace(
    _ trace: GestureOverlayTrace?,
    visibleInDisplayCoordinateFrame displayCoordinateFrame: CGRect,
    lineWidth: CGFloat
  ) -> GestureOverlayTrace? {
    GestureOverlayTraceFilter(
      displayCoordinateFrame: displayCoordinateFrame,
      lineWidth: lineWidth
    )
    .filteredTrace(trace)
  }

  static func displayCoordinateFrame(
    for screenFrame: CGRect,
    screenFrames: [CGRect]
  ) -> CGRect {
    let standardizedFrame = screenFrame.standardized
    let primaryTopY = primaryScreenTopY(
      screenFrames: screenFrames,
      currentScreenFrame: standardizedFrame
    )
    return CGRect(
      x: standardizedFrame.origin.x,
      y: primaryTopY - standardizedFrame.maxY,
      width: standardizedFrame.width,
      height: standardizedFrame.height
    )
  }

}
