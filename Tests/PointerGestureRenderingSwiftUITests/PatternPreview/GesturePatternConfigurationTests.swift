import CoreGraphics
import PointerGestureRenderingSwiftUI
import XCTest

@testable import PointerGestureRendering

final class GesturePatternConfigurationTests: XCTestCase {
  func testPatternPreviewValuesAreExpected() {
    let configuration = GesturePatternConfiguration.patternPreview

    XCTAssertEqual(configuration.frameSize, CGSize(width: 96, height: 64))
    XCTAssertEqual(configuration.strokeStyle, .patternPreview)
    XCTAssertTrue(configuration.showsArrowhead)
    XCTAssertEqual(configuration.padding, 0, accuracy: 0.0001)
  }

  func testInitializerUsesPatternPreviewDefaults() {
    XCTAssertEqual(GesturePatternConfiguration(), .patternPreview)
  }

  func testInitializerPreservesCustomValues() {
    let strokeStyle = GestureStrokeStyle(
      lineWidth: 8
    )
    let configuration = GesturePatternConfiguration(
      frameSize: CGSize(width: 120, height: 80),
      strokeStyle: strokeStyle,
      showsArrowhead: false,
      padding: 12
    )

    XCTAssertEqual(configuration.frameSize, CGSize(width: 120, height: 80))
    XCTAssertEqual(configuration.strokeStyle, strokeStyle)
    XCTAssertFalse(configuration.showsArrowhead)
    XCTAssertEqual(configuration.padding, 12, accuracy: 0.0001)
  }

  func testInitializerNormalizesNonFiniteAndNegativeDimensionsAndPadding() {
    let configuration = GesturePatternConfiguration(
      frameSize: CGSize(width: -.infinity, height: .infinity),
      padding: .nan
    )
    let negativeFrameSizeConfiguration = GesturePatternConfiguration(
      frameSize: CGSize(width: -12, height: -8)
    )
    let negativePaddingConfiguration = GesturePatternConfiguration(padding: -12)
    let infinitePaddingConfiguration = GesturePatternConfiguration(padding: .infinity)

    XCTAssertEqual(configuration.frameSize, .zero)
    XCTAssertEqual(configuration.padding, 0, accuracy: 0.0001)
    XCTAssertEqual(negativeFrameSizeConfiguration.frameSize, .zero)
    XCTAssertEqual(negativePaddingConfiguration.padding, 0, accuracy: 0.0001)
    XCTAssertEqual(infinitePaddingConfiguration.padding, 0, accuracy: 0.0001)
  }

  func testPatternConfigurationEqualityIncludesStoredValues() {
    let configuration = GesturePatternConfiguration(
      frameSize: CGSize(width: 120, height: 80),
      showsArrowhead: false,
      padding: 12
    )
    let differentFrameSize = GesturePatternConfiguration(
      frameSize: CGSize(width: 80, height: 120),
      showsArrowhead: false,
      padding: 12
    )
    let differentStrokeStyle = GesturePatternConfiguration(
      frameSize: CGSize(width: 120, height: 80),
      strokeStyle: .liveOverlay,
      showsArrowhead: false,
      padding: 12
    )
    let differentArrowheadVisibility = GesturePatternConfiguration(
      frameSize: CGSize(width: 120, height: 80),
      showsArrowhead: true,
      padding: 12
    )
    let differentPadding = GesturePatternConfiguration(
      frameSize: CGSize(width: 120, height: 80),
      showsArrowhead: false,
      padding: 6
    )

    XCTAssertEqual(configuration, configuration)
    XCTAssertNotEqual(configuration, differentFrameSize)
    XCTAssertNotEqual(configuration, differentStrokeStyle)
    XCTAssertNotEqual(configuration, differentArrowheadVisibility)
    XCTAssertNotEqual(configuration, differentPadding)
  }

}
