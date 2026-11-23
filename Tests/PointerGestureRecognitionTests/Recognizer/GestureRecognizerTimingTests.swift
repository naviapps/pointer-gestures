import Foundation
import XCTest

@testable import PointerGestureRecognition

final class GestureRecognizerTimingTests: XCTestCase {
  func testTaskSleepNanosecondsForwardsFractionalSeconds() {
    XCTAssertEqual(
      GestureRecognizerTiming.taskSleepNanoseconds(for: 1.000_000_000_1),
      1_000_000_001
    )
  }
}
