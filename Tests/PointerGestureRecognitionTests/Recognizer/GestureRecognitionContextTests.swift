import PointerGestureRecognition
import XCTest

final class GestureRecognitionContextTests: XCTestCase {
  func testHostIdentifiersRemainOpaque() {
    for identifier in ["", " \n\t ", "  host.context\n", "host.context", "host context", "🧭"] {
      XCTAssertEqual(GestureRecognitionContext(identifier: identifier).identifier, identifier)
    }
  }

  func testEqualityPreservesHostIdentityDistinctions() {
    let context = GestureRecognitionContext(identifier: " host.context ")
    XCTAssertEqual(context, GestureRecognitionContext(identifier: " host.context "))
    XCTAssertNotEqual(context, GestureRecognitionContext(identifier: "host.context"))
    XCTAssertNotEqual(
      GestureRecognitionContext(identifier: ""), GestureRecognitionContext(identifier: " "))
  }

}
