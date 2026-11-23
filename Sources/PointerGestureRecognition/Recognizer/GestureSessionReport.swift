import CPointerGestureRecognition
import Foundation

/// Why retained input ended. Ordinary clicks and requested recovery are not OS failures.
public enum GestureSessionOutcome: UInt32, Sendable {
  /// An exact registered pattern produced its host-owned match effect.
  case matched = 1
  /// Recording completed a nonempty direction pattern.
  case recorded = 2
  /// Movement did not start recognition; ordinary input recovery was requested.
  case belowThreshold = 3
  /// The completed pattern was a registered prefix without an exact match.
  case unmatched = 4
  /// A new direction could not extend any registered pattern.
  case unmatchedPrefix = 5
  /// The host explicitly cancelled retained input.
  case cancelled = 6
  /// Recognition stopped or closed while retaining input.
  case stopped = 7
  /// A replacement button-down superseded retained input.
  case superseded = 8
  /// The total session deadline was the earliest expired deadline.
  case totalDeadline = 9
  /// The inactivity deadline was the earliest expired deadline.
  case idleDeadline = 10
  /// A matching release had a non-finite location and could not match.
  case invalidRelease = 11
  /// Another direction would exceed the session's captured budget.
  case directionLimit = 12
}

/// Coordinate-free measurements of the last completed retained session.
///
/// The report describes recognition and requested input effects, not successful OS
/// replay or command execution. Its identity is local to one recognizer and wraps.
/// Full snapshots can coalesce; this is a latest value, not an event history.
public struct GestureSessionReport: Equatable, Sendable {
  /// Nonzero wrapping identity local to this recognizer.
  public let generation: UInt64
  /// The terminal recognition decision.
  public let outcome: GestureSessionOutcome
  /// Seconds since the original press, unaffected by deadline resets.
  public let elapsedDuration: TimeInterval
  /// Seconds since the last changed finite pointer location.
  public let idleDuration: TimeInterval
  /// Admitted finite samples, including press, release and stationary movement.
  public let pointCount: UInt64
  /// Largest axis displacement from the press in host units, saturated finite.
  public let maximumAxisDistance: Double
  /// Whether the session crossed the start threshold.
  public let wasCaptured: Bool
  /// Recording mode captured at the press.
  public let wasRecording: Bool
  /// Accepted direction runs, bounded by the captured session budget.
  public let directions: [GestureDirection]
  /// A direction rejected by prefix validation or the direction budget.
  public let rejectedDirection: GestureDirection?

  init?(_ value: PointerGestureRecognitionSessionReportView) {
    guard value.present != 0 else { return nil }
    generation = value.generation
    outcome = GestureSessionOutcome(rawValue: value.outcome)!
    elapsedDuration = Double(value.elapsed_ns) / 1_000_000_000
    idleDuration = Double(value.idle_ns) / 1_000_000_000
    pointCount = value.point_count
    maximumAxisDistance = value.maximum_axis_distance
    wasCaptured = value.captured != 0
    wasRecording = value.recording != 0
    directions = UnsafeBufferPointer(start: value.directions.data, count: value.directions.count)
      .map(GestureDirection.init(coreCode:))
    rejectedDirection =
      value.rejected_direction == 0 ? nil : GestureDirection(coreCode: value.rejected_direction)
  }

}
