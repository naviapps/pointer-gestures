/// A geometry planning failure reported across the Rust boundary.
public enum GestureGeometryError: Error, Equatable, Sendable {
  /// No directions were supplied.
  case empty
  /// The points did not contain exactly one more value than the directions.
  case pointCount
  /// Input or projected geometry could not be represented with finite coordinates.
  case nonFinite
  /// The native planner rejected a direction code.
  case invalidDirection
  /// The native planner returned an invalid buffer status or result.
  case invalidNativeResult

  init(nativeCode: UInt32) {
    switch nativeCode {
    case 1: self = .empty
    case 2: self = .pointCount
    case 3: self = .nonFinite
    case 5: self = .invalidDirection
    default: self = .invalidNativeResult
    }
  }
}
