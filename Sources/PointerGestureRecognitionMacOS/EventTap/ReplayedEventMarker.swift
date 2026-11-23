import class CoreGraphics.CGEvent
import enum CoreGraphics.CGEventField

/// Replay marking is sufficient even if the tap misses, reorders or redelivers an event.
enum ReplayedEventMarker {
  private static let value: Int64 = 0x5047_4B54

  static func mark(_ event: CGEvent) {
    event.setIntegerValueField(.eventSourceUserData, value: value)
  }

  static func isPresent(on event: CGEvent) -> Bool {
    event.getIntegerValueField(.eventSourceUserData) == value
  }
}
