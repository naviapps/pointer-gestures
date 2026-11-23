import CoreGraphics
import PointerGestureRecognition
import PointerGestureRendering
import XCTest

final class GestureOverlayTraceConverterTests: XCTestCase {
  func testOverlayTraceUsesPointerTraceTailPointAsLiveTail() throws {
    let trace = try XCTUnwrap(
      GestureOverlayTraceConverter().convert(
        GestureRecognizerState.Trace(
          isVisible: true,
          rawPoints: [
            .zero,
            GesturePoint(x: .nan, y: 0),
          ],
          directions: [.right],
          directionEndpoints: [.zero, GesturePoint(x: 40, y: 0)],
          tailPoint: GesturePoint(x: 45, y: 10)
        )
      )
    )

    XCTAssertEqual(trace.points, [CGPoint(x: 0, y: 0), CGPoint(x: 40, y: 0)])
    XCTAssertEqual(trace.tailPoint, CGPoint(x: 45, y: 10))
  }

  func testOverlayTraceConverterReusesStaticPathStorageForTailOnlyUpdates() throws {
    let converter = GestureOverlayTraceConverter()
    let first = try XCTUnwrap(
      converter.convert(
        GestureRecognizerState.Trace(
          isVisible: true,
          directions: [.right],
          directionEndpoints: [.zero, GesturePoint(x: 40, y: 0)],
          tailPoint: GesturePoint(x: 45, y: 0)
        )))
    let second = try XCTUnwrap(
      converter.convert(
        GestureRecognizerState.Trace(
          isVisible: true,
          directions: [.right],
          directionEndpoints: [.zero, GesturePoint(x: 40, y: 0)],
          tailPoint: GesturePoint(x: 70, y: 10)
        )))

    XCTAssertEqual(first.points, second.points)
    XCTAssertNotEqual(first.tailPoint, second.tailPoint)
    XCTAssertTrue(Self.shareStorage(first.points, second.points))
    XCTAssertTrue(Self.shareStorage(first.directions, second.directions))
  }

  func testHiddenSnapshotClearsTheBindingCacheWithoutInvalidatingOwnedTraces() throws {
    let endpoints = [GesturePoint.zero, GesturePoint(x: 40, y: 0)]
    let original = GestureRecognizerState.Trace(
      isVisible: true, directions: [.right], directionEndpoints: endpoints,
      tailPoint: GesturePoint(x: 45, y: 10))
    let next = GestureRecognizerState.Trace(
      isVisible: true, directions: [.right], directionEndpoints: endpoints,
      tailPoint: GesturePoint(x: 70, y: 10))
    let converter = GestureOverlayTraceConverter()
    let first = try XCTUnwrap(converter.convert(original))
    XCTAssertNil(converter.convert(.init()))
    let restored = try XCTUnwrap(converter.convert(next))

    XCTAssertEqual(first.points, restored.points)
    XCTAssertFalse(Self.shareStorage(first.points, restored.points))
    XCTAssertEqual(first.tailPoint, CGPoint(x: 45, y: 10))
    XCTAssertEqual(restored.tailPoint, CGPoint(x: 70, y: 10))
    XCTAssertTrue(first.intersects(viewport: CGRect(x: 44, y: 9, width: 2, height: 2)))
  }

  private static func shareStorage<Element>(_ first: [Element], _ second: [Element]) -> Bool {
    first.withUnsafeBufferPointer { firstBuffer in
      second.withUnsafeBufferPointer { secondBuffer in
        firstBuffer.baseAddress == secondBuffer.baseAddress
      }
    }
  }
}
