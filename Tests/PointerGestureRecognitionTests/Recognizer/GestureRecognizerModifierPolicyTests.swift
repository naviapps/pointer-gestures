import PointerGestureRecognition
import XCTest

@MainActor
final class GestureRecognizerModifierPolicyTests: XCTestCase {
  func testRepeatedButtonDownDuringActiveSessionRechecksModifiersForNewInput() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var evaluatedModifiers: [GestureModifierFlags] = []
    var releaseRequests: [GesturePoint] = []

    let matchedID = UUID()
    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in
          var matcher = GesturePatternMatcher<UUID>()
          matcher.register(pattern: [.right], match: matchedID)
          return matcher
        },
        onReplayRequested: { request in
          if case .release(_, let point) = request {
            releaseRequests.append(point)
          }
        },
        areModifiersSatisfied: { modifiers, _ in
          evaluatedModifiers.append(modifiers)
          if modifiers.isEmpty {
            XCTAssertEqual(releaseRequests, [.init(x: 100, y: 10)])
          }
          return modifiers.contains(.command)
        },
        tuning: .testing(
          minimumGestureStartAxisDistance: 0,
          minimumDirectionChangeAxisDistance: 0
        )
      )

    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: configuration
    )

    recognizer.start()

    assertDisposition(
      eventSource.send(
        makeGestureInputEvent(
          kind: .buttonDown(.secondary),
          location: .init(x: 10, y: 10),
          modifiers: [.command]
        )),
      .consume
    )
    XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)
    assertDisposition(
      eventSource.send(
        makeGestureInputEvent(
          kind: .buttonMoved(.secondary),
          location: .init(x: 100, y: 10),
          modifiers: [.command]
        )),
      .consume
    )
    XCTAssertTrue(recognizer.snapshot.status.isCapturingGesture)

    assertDisposition(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 120, y: 10))
      ),
      .passThrough
    )
    XCTAssertEqual(evaluatedModifiers, [[.command], []])
    XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)
    XCTAssertEqual(recognizer.snapshot.status.lastFailure, .modifiersNotSatisfied)

    assertDisposition(
      eventSource.send(
        makeGestureInputEvent(
          kind: .buttonUp(.secondary),
          location: .init(x: 140, y: 10),
          modifiers: [.command]
        )),
      .passThrough
    )
    XCTAssertEqual(releaseRequests, [.init(x: 100, y: 10)])
  }

  func testRecordingModeGestureStartClearsStaleModifierFailure() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var modifierEvaluations = 0

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        areModifiersSatisfied: { modifiers, _ in
          modifierEvaluations += 1
          return modifiers.contains(.command)
        }
      )

    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: configuration
    )

    recognizer.start()

    assertDisposition(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 10, y: 10))
      ),
      .passThrough
    )
    XCTAssertEqual(recognizer.snapshot.status.lastFailure, .modifiersNotSatisfied)

    recognizer.isRecordingModeEnabled = true
    assertDisposition(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 20, y: 20))
      ),
      .consume
    )

    XCTAssertNil(recognizer.snapshot.status.lastFailure)
    XCTAssertEqual(modifierEvaluations, 1)
  }
}
