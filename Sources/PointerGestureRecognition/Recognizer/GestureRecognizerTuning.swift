import CPointerGestureRecognition
import Foundation

/// Validated tuning values for gesture recognition and event-source start retry.
public struct GestureRecognizerTuning: Equatable, Sendable {
  /// A reason custom tuning values could not be created.
  public enum ValidationError: Error, Equatable, Sendable {
    /// The gesture-start distance was negative or non-finite.
    case invalidMinimumGestureStartAxisDistance
    /// The direction-change distance was negative or non-finite.
    case invalidMinimumDirectionChangeAxisDistance
    /// The non-`nil` session duration was non-positive or non-finite.
    case invalidMaximumGestureSessionDuration
    /// The non-`nil` inactivity duration was non-positive or non-finite.
    case invalidMaximumGestureIdleDuration
    /// The raw point count was not positive.
    case invalidMaximumRawPointCount
    /// The direction budget is outside 1...4096.
    case invalidMaximumDirectionCount
    /// A retry delay at the zero-based index was negative or non-finite.
    case invalidEventSourceStartRetryDelay(index: Int)
  }

  /// Minimum horizontal or vertical movement needed before a gesture starts.
  public let minimumGestureStartAxisDistance: Double
  /// Minimum horizontal or vertical movement needed before accepting a new direction.
  public let minimumDirectionChangeAxisDistance: Double
  /// Maximum duration for pending or active gesture input, or `nil` to disable the total limit.
  public let maximumGestureSessionDuration: TimeInterval?
  /// Maximum time without a changed pointer location, or `nil` to disable idle expiration.
  public let maximumGestureIdleDuration: TimeInterval?
  /// Maximum number of raw pointer points retained for an active gesture.
  public let maximumRawPointCount: Int
  /// Maximum accepted direction runs per session, captured at button-down.
  public let maximumDirectionCount: Int
  /// Delays used in order when retrying event-source startup.
  public let eventSourceStartRetryDelays: [TimeInterval]

  /// Recommended production tuning.
  public static let standard = Self(
    minimumGestureStartAxisDistance: 10,
    minimumDirectionChangeAxisDistance: 25,
    maximumGestureSessionDuration: 1,
    maximumGestureIdleDuration: nil,
    maximumRawPointCount: 256,
    maximumDirectionCount: 64,
    eventSourceStartRetryDelays: [0.25, 0.5, 1, 2, 4]
  )

  /// Creates custom tuning after validating every value.
  ///
  /// Distances and retry delays must be finite and nonnegative. A non-`nil` session duration must
  /// be finite and positive. The same rules apply to the optional inactivity duration. The earliest
  /// enabled deadline expires input; stationary events do not extend the idle deadline. The raw
  /// point count must be positive. Retry order and duplicates are preserved, and an explicit zero
  /// delay means retry on the next task yield.
  public static func validated(
    minimumGestureStartAxisDistance: Double = 10,
    minimumDirectionChangeAxisDistance: Double = 25,
    maximumGestureSessionDuration: TimeInterval? = 1,
    maximumGestureIdleDuration: TimeInterval? = nil,
    maximumRawPointCount: Int = 256,
    maximumDirectionCount: Int = 64,
    eventSourceStartRetryDelays: [TimeInterval] = [0.25, 0.5, 1, 2, 4]
  ) throws -> Self {
    precondition(
      pointer_gesture_recognition_abi_version() == POINTER_GESTURE_RECOGNITION_ABI_VERSION,
      "Unsupported native recognition ABI")
    let result = pointer_gesture_recognition_validate_tuning(
      minimumGestureStartAxisDistance, minimumDirectionChangeAxisDistance,
      maximumGestureSessionDuration ?? 0, maximumGestureIdleDuration ?? 0,
      (maximumGestureSessionDuration == nil ? 0 : 1) | (maximumGestureIdleDuration == nil ? 0 : 2),
      Int64(maximumRawPointCount))
    switch result {
    case 0: break
    case 1: throw ValidationError.invalidMinimumGestureStartAxisDistance
    case 2: throw ValidationError.invalidMinimumDirectionChangeAxisDistance
    case 3: throw ValidationError.invalidMaximumGestureSessionDuration
    case 4: throw ValidationError.invalidMaximumGestureIdleDuration
    case 5: throw ValidationError.invalidMaximumRawPointCount
    default: preconditionFailure("Unsupported tuning result")
    }
    guard pointer_gesture_recognition_validate_direction_limit(Int64(maximumDirectionCount)) == 1
    else {
      throw ValidationError.invalidMaximumDirectionCount
    }
    let invalidRetry = eventSourceStartRetryDelays.withUnsafeBufferPointer {
      pointer_gesture_recognition_validate_retry_delays($0.baseAddress, $0.count)
    }
    if invalidRetry != Int(bitPattern: UInt.max) {
      throw ValidationError.invalidEventSourceStartRetryDelay(index: invalidRetry)
    }

    return Self(
      minimumGestureStartAxisDistance: minimumGestureStartAxisDistance,
      minimumDirectionChangeAxisDistance: minimumDirectionChangeAxisDistance,
      maximumGestureSessionDuration: maximumGestureSessionDuration,
      maximumGestureIdleDuration: maximumGestureIdleDuration,
      maximumRawPointCount: maximumRawPointCount,
      maximumDirectionCount: maximumDirectionCount,
      eventSourceStartRetryDelays: eventSourceStartRetryDelays
    )
  }

  private init(
    minimumGestureStartAxisDistance: Double,
    minimumDirectionChangeAxisDistance: Double,
    maximumGestureSessionDuration: TimeInterval?,
    maximumGestureIdleDuration: TimeInterval?,
    maximumRawPointCount: Int,
    maximumDirectionCount: Int,
    eventSourceStartRetryDelays: [TimeInterval]
  ) {
    self.minimumGestureStartAxisDistance = minimumGestureStartAxisDistance
    self.minimumDirectionChangeAxisDistance = minimumDirectionChangeAxisDistance
    self.maximumGestureSessionDuration = maximumGestureSessionDuration
    self.maximumGestureIdleDuration = maximumGestureIdleDuration
    self.maximumRawPointCount = maximumRawPointCount
    self.maximumDirectionCount = maximumDirectionCount
    self.eventSourceStartRetryDelays = eventSourceStartRetryDelays
  }
}
