import AppKit

/// A captured AppKit display frame and its reported refresh capability.
///
/// Supply primary-first snapshots, matching `NSScreen.screens`. Frames are in
/// AppKit points; recognition traces use the primary display's top-left origin.
public struct GestureOverlayDisplay: Equatable, Sendable {
  /// The display's frame in AppKit desktop coordinates.
  public let frame: CGRect
  /// The display's reported maximum refresh rate. The host selects a rendering cadence.
  public let maximumFramesPerSecond: Int

  /// Creates a snapshot without accessing a native screen or creating a window.
  public init(frame: CGRect, maximumFramesPerSecond: Int = 0) {
    self.frame = frame
    self.maximumFramesPerSecond = maximumFramesPerSecond
  }

  /// Captures the current primary-first display layout without creating windows.
  @MainActor
  public static var current: [GestureOverlayDisplay] {
    NSScreen.screens.map {
      GestureOverlayDisplay(frame: $0.frame, maximumFramesPerSecond: $0.maximumFramesPerSecond)
    }
  }
}
