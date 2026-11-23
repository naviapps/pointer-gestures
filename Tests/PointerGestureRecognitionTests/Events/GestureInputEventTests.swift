import PointerGestureRecognition
import XCTest

final class GestureInputEventTests: XCTestCase {
  func testInitializerStoresKindLocationAndModifiers() {
    let event = GestureInputEvent(
      kind: .buttonMoved(.secondary),
      location: GesturePoint(x: 10, y: 20),
      modifiers: [.command, .shift]
    )

    XCTAssertEqual(event.kind, .buttonMoved(.secondary))
    XCTAssertEqual(event.location, GesturePoint(x: 10, y: 20))
    XCTAssertEqual(event.modifiers, [.command, .shift])
  }

  func testInitializerDefaultsToNoModifiers() {
    let event = GestureInputEvent(
      kind: .buttonDown(.secondary),
      location: .zero
    )

    XCTAssertEqual(event.modifiers, [])
  }

  func testCancelEventKeepsSourceLocationAndModifiers() {
    let event = GestureInputEvent(
      kind: .cancel,
      location: GesturePoint(x: 10, y: 20),
      modifiers: [.option]
    )

    XCTAssertEqual(event.kind, .cancel)
    XCTAssertEqual(event.location, GesturePoint(x: 10, y: 20))
    XCTAssertEqual(event.modifiers, [.option])
  }
}
