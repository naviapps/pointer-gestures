import CoreGraphics
import XCTest

@testable import PointerGestureRendering

func assertEqualRect(
  _ actual: CGRect,
  _ expected: CGRect,
  accuracy: CGFloat,
  _ message: @autoclosure () -> String = "",
  file: StaticString = #filePath,
  line: UInt = #line
) {
  let diagnostic = message()
  XCTAssertEqual(
    actual.minX,
    expected.minX,
    accuracy: accuracy,
    rectAssertionMessage(diagnostic, component: "minX"),
    file: file,
    line: line
  )
  XCTAssertEqual(
    actual.minY,
    expected.minY,
    accuracy: accuracy,
    rectAssertionMessage(diagnostic, component: "minY"),
    file: file,
    line: line
  )
  XCTAssertEqual(
    actual.maxX,
    expected.maxX,
    accuracy: accuracy,
    rectAssertionMessage(diagnostic, component: "maxX"),
    file: file,
    line: line
  )
  XCTAssertEqual(
    actual.maxY,
    expected.maxY,
    accuracy: accuracy,
    rectAssertionMessage(diagnostic, component: "maxY"),
    file: file,
    line: line
  )
}

private func rectAssertionMessage(_ message: String, component: String) -> String {
  if message.isEmpty {
    return "Unexpected rect.\(component)"
  }
  return "\(message).\(component)"
}
