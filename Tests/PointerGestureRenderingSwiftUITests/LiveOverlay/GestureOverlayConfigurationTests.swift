import CoreGraphics
import PointerGestureRenderingSwiftUI
import XCTest

@testable import PointerGestureRendering

final class GestureOverlayConfigurationTests: XCTestCase {
  func testOverlayConfigurationInitializerUsesLiveOverlayDefaults() {
    XCTAssertEqual(GestureOverlayConfiguration(), .liveOverlay)
  }

  func testDefaultValuesAreExpected() {
    let configuration = GestureOverlayConfiguration.liveOverlay

    XCTAssertEqual(
      configuration.minimumAnimationInterval,
      1.0 / 60.0,
      accuracy: 0.0001
    )
    XCTAssertEqual(configuration.maximumPointLagDistance, 24, accuracy: 0.0001)
    XCTAssertEqual(configuration.strokeStyle.lineWidth, GestureStrokeStyle.liveOverlay.lineWidth)
    XCTAssertEqual(configuration.turnStyle, .rounded)
  }

  func testCustomStrokeStyleIsUsed() {
    let configuration = GestureOverlayConfiguration(
      strokeStyle: GestureStrokeStyle(
        color: .red,
        lineWidth: 9.0
      )
    )

    XCTAssertEqual(configuration.strokeStyle.lineWidth, 9.0, accuracy: 0.0001)
  }

  func testCustomTurnStyleIsUsed() {
    let configuration = GestureOverlayConfiguration(
      turnStyle: .rightAngle
    )

    XCTAssertEqual(configuration.turnStyle, .rightAngle)
  }

  func testCustomMinimumAnimationIntervalIsUsed() {
    let configuration = GestureOverlayConfiguration(minimumAnimationInterval: 0.25)

    XCTAssertEqual(configuration.minimumAnimationInterval, 0.25, accuracy: 0.0001)
  }

  func testCustomMaximumPointLagDistanceIsUsed() {
    let configuration = GestureOverlayConfiguration(maximumPointLagDistance: 32)

    XCTAssertEqual(configuration.maximumPointLagDistance, 32, accuracy: 0.0001)
  }

  func testOverlayConfigurationNormalizesInvalidMinimumAnimationInterval() {
    assertInvalidMinimumAnimationIntervalsUseDefault([
      0, -1, .nan, .infinity, -.infinity, .greatestFiniteMagnitude, Double(Int64.max),
    ])
  }

  func testLargeRepresentableIntervalKeepsSafeHideDuration() {
    let interval = Double(Int64.max / 4) / 1_000_000_000
    let configuration = GestureOverlayConfiguration(minimumAnimationInterval: interval)
    XCTAssertEqual(configuration.minimumAnimationInterval, interval)
    XCTAssertLessThan(configuration.minimumAnimationInterval * 2, Double(Int64.max) / 1_000_000_000)
  }

  func testOverlayConfigurationNormalizesInvalidMaximumPointLagDistance() {
    assertInvalidMaximumPointLagDistancesUseDefault([-1, .nan, .infinity, -.infinity])
  }

  func testOverlayConfigurationEqualityIncludesStoredValues() {
    let configuration = GestureOverlayConfiguration(
      minimumAnimationInterval: 0.25,
      maximumPointLagDistance: 32,
      strokeStyle: GestureStrokeStyle(
        color: .red,
        lineWidth: 9.0
      ),
      turnStyle: .rightAngle
    )
    let differentInterval = GestureOverlayConfiguration(
      minimumAnimationInterval: 0.5,
      maximumPointLagDistance: 32,
      strokeStyle: GestureStrokeStyle(
        color: .red,
        lineWidth: 9.0
      ),
      turnStyle: .rightAngle
    )
    let differentPointLagDistance = GestureOverlayConfiguration(
      minimumAnimationInterval: 0.25,
      maximumPointLagDistance: 48,
      strokeStyle: GestureStrokeStyle(
        color: .red,
        lineWidth: 9.0
      ),
      turnStyle: .rightAngle
    )
    let differentStrokeStyle = GestureOverlayConfiguration(
      minimumAnimationInterval: 0.25,
      maximumPointLagDistance: 32,
      strokeStyle: GestureStrokeStyle(
        color: .blue,
        lineWidth: 9.0
      ),
      turnStyle: .rightAngle
    )
    let differentTurnStyle = GestureOverlayConfiguration(
      minimumAnimationInterval: 0.25,
      maximumPointLagDistance: 32,
      strokeStyle: GestureStrokeStyle(
        color: .red,
        lineWidth: 9.0
      ),
      turnStyle: .rounded
    )

    XCTAssertEqual(configuration, configuration)
    XCTAssertNotEqual(configuration, differentInterval)
    XCTAssertNotEqual(configuration, differentPointLagDistance)
    XCTAssertNotEqual(configuration, differentStrokeStyle)
    XCTAssertNotEqual(configuration, differentTurnStyle)
  }

  private func assertInvalidMinimumAnimationIntervalsUseDefault(
    _ intervals: [TimeInterval],
    file: StaticString = #filePath,
    line: UInt = #line
  ) {
    for interval in intervals {
      let configuration = GestureOverlayConfiguration(minimumAnimationInterval: interval)

      XCTAssertEqual(
        configuration.minimumAnimationInterval,
        GestureOverlayConfiguration.liveOverlay.minimumAnimationInterval,
        accuracy: 0.0001,
        file: file,
        line: line
      )
    }
  }

  private func assertInvalidMaximumPointLagDistancesUseDefault(
    _ distances: [CGFloat],
    file: StaticString = #filePath,
    line: UInt = #line
  ) {
    for distance in distances {
      let configuration = GestureOverlayConfiguration(maximumPointLagDistance: distance)

      XCTAssertEqual(
        configuration.maximumPointLagDistance,
        GestureOverlayConfiguration.liveOverlay.maximumPointLagDistance,
        accuracy: 0.0001,
        file: file,
        line: line
      )
    }
  }

}
