import XCTest

@testable import PointerGestureRecognition

@MainActor
final class GestureObservationTokenTests: XCTestCase {
  func testCancelRunsHandlerSynchronouslyOnce() {
    var cancelCount = 0
    let token = GestureObservationToken(onCancel: {
      cancelCount += 1
    })

    token.cancel()
    token.cancel()

    XCTAssertEqual(cancelCount, 1)
  }

  func testDeinitCancelsActiveTokenOnce() async {
    var cancelCount = 0
    var token: GestureObservationToken? = GestureObservationToken(onCancel: {
      cancelCount += 1
    })

    XCTAssertNotNil(token)
    token = nil

    await assertEventually { cancelCount == 1 }
    XCTAssertEqual(cancelCount, 1)
  }

  func testReleaseFromAnotherActorCancelsOnMainActor() async {
    var cancelCount = 0
    var token: GestureObservationToken? = GestureObservationToken { cancelCount += 1 }
    let holder = TokenHolder(token!)
    token = nil
    await holder.release()
    await assertEventually { cancelCount == 1 }
  }

}

private actor TokenHolder {
  private var token: GestureObservationToken?
  init(_ token: GestureObservationToken) { self.token = token }
  func release() { token = nil }
}
