import CPointerGestureRecognition

/// Rust-owned validation failure for exact direction-run registration.
public enum GesturePatternError: Error, Equatable, Sendable {
  /// A pattern needs at least one direction.
  case empty
  /// A pattern supports at most 4096 directions.
  case tooLong
  /// Zero-based second member of the first consecutive equal pair.
  case repeatedDirection(index: Int)

  static func decode(_ result: PointerGestureRecognitionPatternValidation) -> Self? {
    switch result.code {
    case 0: nil
    case 1: .empty
    case 2: .tooLong
    case 3: .repeatedDirection(index: Int(result.index))
    default:
      preconditionFailure("Invalid typed direction buffer or unsupported native validation code")
    }
  }
}
