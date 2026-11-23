import PointerGestureRecognition
import XCTest

final class GesturePatternMatcherTests: XCTestCase {
  func testRegisterRejectsEmptyPattern() {
    var matcher = GesturePatternMatcher<String>()
    let result = matcher.register(pattern: [], match: "A")

    XCTAssertEqual(result, .empty)
    XCTAssertNil(matcher.match(pattern: []))
  }

  func testRegisterAcceptsNonEquatableSendableMatchValues() {
    var matcher = GesturePatternMatcher<NonEquatableMatch>()

    let result = matcher.register(pattern: [.down], match: NonEquatableMatch(id: 1))

    XCTAssertNil(result)
  }

  func testMatcherCopiesPreserveExistingSessionSnapshotsWhenHostUpdatesPatterns() {
    var matcher = GesturePatternMatcher<String>()
    matcher.register(pattern: [.down], match: "original")
    let captured = matcher
    matcher.register(pattern: [.down], match: "replacement")
    matcher.register(pattern: [.down, .right], match: "new")
    XCTAssertEqual(captured.match(pattern: [.down]), "original")
    XCTAssertNil(captured.match(pattern: [.down, .right]))
    XCTAssertEqual(matcher.match(pattern: [.down]), "replacement")
    XCTAssertEqual(matcher.match(pattern: [.down, .right]), "new")
  }

  func testRemoveAndClearPreserveCopiesAndReuseVacatedCommands() {
    var matcher = GesturePatternMatcher<String?>()
    XCTAssertTrue(matcher.isEmpty)
    matcher.register(pattern: [.up], match: nil)
    matcher.register(pattern: [.up, .right], match: "long")
    let captured = matcher
    let removed = matcher.remove(pattern: [.up])
    XCTAssertNotNil(removed as Any?)
    XCTAssertNil(removed!)
    XCTAssertNil(matcher.remove(pattern: [.left]) as Any?)
    XCTAssertNil(matcher.remove(pattern: []) as Any?)
    XCTAssertEqual(matcher.match(pattern: [.up, .right]), "long")
    XCTAssertNotNil(captured.match(pattern: [.up]) as Any?)
    for _ in 0..<1000 {
      XCTAssertNil(matcher.register(pattern: [.down], match: "reused"))
      XCTAssertEqual(matcher.remove(pattern: [.down]), "reused")
    }
    matcher.removeAll()
    matcher.removeAll()
    XCTAssertTrue(matcher.isEmpty)
    XCTAssertEqual(captured.match(pattern: [.up, .right]), "long")
    matcher.register(pattern: [.left], match: "new")
    XCTAssertEqual(matcher.match(pattern: [.left]), "new")
  }

  func testStructuredValidationAndRejectionPreserveCommands() {
    var matcher = GesturePatternMatcher<String>()
    XCTAssertNil(matcher.register(pattern: [.up], match: "retained"))
    let error = GesturePatternMatcher<String>.validationError(for: [.up, .right, .right])
    XCTAssertEqual(error, .repeatedDirection(index: 2))
    XCTAssertEqual(matcher.register(pattern: [.up, .right, .right], match: "rejected"), error)
    XCTAssertEqual(matcher.match(pattern: [.up]), "retained")
  }

  private struct NonEquatableMatch: Sendable {
    let id: Int
  }
}
