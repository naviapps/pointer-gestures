import CPointerGestureRecognition
import XCTest

final class NativeLayoutTests: XCTestCase {
  func testImportedFrameAndEffectLayout() {
    typealias Frame = MemoryLayout<PointerGestureRecognitionFrameView>
    XCTAssertEqual(Frame.stride, 160)
    XCTAssertEqual(Frame.alignment, 8)
    XCTAssertEqual(Frame.offset(of: \.input_generation), 32)
    XCTAssertEqual(Frame.offset(of: \.directions), 48)
    XCTAssertEqual(Frame.offset(of: \.tail), 120)
    XCTAssertEqual(Frame.offset(of: \.deadline_ns), 152)

    typealias Effect = MemoryLayout<PointerGestureRecognitionEffectView>
    XCTAssertEqual(Effect.stride, 144)
    XCTAssertEqual(Effect.alignment, 8)
    XCTAssertEqual(Effect.offset(of: \.trace), 32)
    XCTAssertEqual(Effect.offset(of: \.ready), 112)
    XCTAssertEqual(Effect.offset(of: \.recorded), 128)
  }
}
