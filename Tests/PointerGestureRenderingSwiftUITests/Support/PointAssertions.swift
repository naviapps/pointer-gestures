import CoreGraphics
import XCTest

@testable import PointerGestureRendering

func assertEqualPoint(
  _ actual: CGPoint,
  _ expected: CGPoint,
  accuracy: CGFloat,
  _ message: @autoclosure () -> String = "",
  file: StaticString = #filePath,
  line: UInt = #line
) {
  let diagnostic = message()
  XCTAssertEqual(
    actual.x,
    expected.x,
    accuracy: accuracy,
    pointAssertionMessage(diagnostic, component: "x"),
    file: file,
    line: line
  )
  XCTAssertEqual(
    actual.y,
    expected.y,
    accuracy: accuracy,
    pointAssertionMessage(diagnostic, component: "y"),
    file: file,
    line: line
  )
}

private func pointAssertionMessage(_ message: String, component: String) -> String {
  if message.isEmpty {
    return "Unexpected point.\(component)"
  }
  return "\(message).\(component)"
}
