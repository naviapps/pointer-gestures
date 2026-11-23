/// A cardinal direction in a pointer gesture pattern or trace.
public enum GestureDirection: Hashable, Sendable {
  /// Movement toward a decreasing y coordinate.
  case up
  /// Movement toward an increasing y coordinate.
  case down
  /// Movement toward a decreasing x coordinate.
  case left
  /// Movement toward an increasing x coordinate.
  case right

  // Keep the shared ABI encoding within this package's language boundary.
  package var coreCode: UInt32 {
    switch self {
    case .up: 1
    case .down: 2
    case .left: 3
    case .right: 4
    }
  }

  package init(coreCode: UInt32) {
    switch coreCode {
    case 1: self = .up
    case 2: self = .down
    case 3: self = .left
    case 4: self = .right
    default: preconditionFailure("Unsupported core direction")
    }
  }
}
