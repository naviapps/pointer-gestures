import CoreGraphics
import XCTest

@testable import PointerGestureRendering

private struct RenderedPixel {
  let red: UInt8
  let green: UInt8
  let blue: UInt8
  let alpha: UInt8
}

private struct RenderedImageStatistics {
  let width: Int
  let height: Int
  let visiblePixelCount: Int
  let redPixelCount: Int
}

func assertRenderedImageContainsVisiblePixel(
  _ image: CGImage?,
  _ message: @autoclosure () -> String = "",
  file: StaticString = #filePath,
  line: UInt = #line
) {
  let diagnostic = message()
  withRenderedImageStatistics(image, diagnostic, file: file, line: line) { statistics in
    XCTAssertGreaterThan(
      statistics.visiblePixelCount,
      0,
      renderedImageDiagnostic(
        renderedImageAssertionMessage(
          diagnostic,
          expectation: "Expected rendered image to contain visible pixels."
        ),
        statistics
      ),
      file: file,
      line: line
    )
  }
}

func assertRenderedImageContainsNoVisiblePixels(
  _ image: CGImage?,
  _ message: @autoclosure () -> String = "",
  file: StaticString = #filePath,
  line: UInt = #line
) {
  let diagnostic = message()
  withRenderedImageStatistics(image, diagnostic, file: file, line: line) { statistics in
    XCTAssertEqual(
      statistics.visiblePixelCount,
      0,
      renderedImageDiagnostic(
        renderedImageAssertionMessage(
          diagnostic,
          expectation: "Expected rendered image to contain no visible pixels."
        ),
        statistics
      ),
      file: file,
      line: line
    )
  }
}

func assertRenderedImageContainsRedPixel(
  _ image: CGImage?,
  _ message: @autoclosure () -> String = "",
  file: StaticString = #filePath,
  line: UInt = #line
) {
  let diagnostic = message()
  withRenderedImageStatistics(image, diagnostic, file: file, line: line) { statistics in
    XCTAssertGreaterThan(
      statistics.redPixelCount,
      0,
      renderedImageDiagnostic(
        renderedImageAssertionMessage(
          diagnostic,
          expectation: "Expected rendered image to contain red pixels."
        ),
        statistics
      ),
      file: file,
      line: line
    )
  }
}

private func withRenderedImageStatistics(
  _ image: CGImage?,
  _ message: String,
  file: StaticString,
  line: UInt,
  assertion: (RenderedImageStatistics) -> Void
) {
  guard let image else {
    XCTFail(
      renderedImageAssertionMessage(message, expectation: "Expected a rendered image."),
      file: file,
      line: line
    )
    return
  }

  guard let statistics = renderedImageStatistics(in: image) else {
    XCTFail(renderedImagePixelReadFailureMessage(message, image: image), file: file, line: line)
    return
  }

  assertion(statistics)
}

private func renderedImageStatistics(in image: CGImage) -> RenderedImageStatistics? {
  var visiblePixelCount = 0
  var redPixelCount = 0
  guard let pixels = renderedPixels(in: image) else { return nil }

  for pixel in pixels {
    if pixel.alpha > 0 {
      visiblePixelCount += 1
    }
    if pixel.red > 180 && pixel.green < 80 && pixel.blue < 80 && pixel.alpha > 120 {
      redPixelCount += 1
    }
  }

  return .init(
    width: image.width,
    height: image.height,
    visiblePixelCount: visiblePixelCount,
    redPixelCount: redPixelCount
  )
}

private func renderedImageAssertionMessage(_ message: String, expectation: String) -> String {
  if message.isEmpty {
    return expectation
  }
  return "\(message): \(expectation)"
}

private func renderedImagePixelReadFailureMessage(_ message: String, image: CGImage) -> String {
  renderedImageAssertionMessage(
    message,
    expectation: "Could not read rendered image pixels. imageSize=\(image.width)x\(image.height)"
  )
}

private func renderedImageDiagnostic(
  _ message: String,
  _ statistics: RenderedImageStatistics
) -> String {
  "\(message) imageSize=\(statistics.width)x\(statistics.height), "
    + "visiblePixels=\(statistics.visiblePixelCount), "
    + "redPixels=\(statistics.redPixelCount)"
}

private func renderedPixels(in image: CGImage) -> [RenderedPixel]? {
  let width = image.width
  let height = image.height
  let bytesPerPixel = 4
  let bytesPerRow = bytesPerPixel * width
  var pixels = [UInt8](repeating: 0, count: bytesPerRow * height)
  let colorSpace = CGColorSpaceCreateDeviceRGB()
  let bitmapInfo =
    CGImageAlphaInfo.premultipliedLast.rawValue
    | CGBitmapInfo.byteOrder32Big.rawValue
  guard
    let context = CGContext(
      data: &pixels,
      width: width,
      height: height,
      bitsPerComponent: 8,
      bytesPerRow: bytesPerRow,
      space: colorSpace,
      bitmapInfo: bitmapInfo
    )
  else {
    return nil
  }

  context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))

  return stride(from: 0, to: pixels.count, by: bytesPerPixel).map { index in
    RenderedPixel(
      red: pixels[index],
      green: pixels[index + 1],
      blue: pixels[index + 2],
      alpha: pixels[index + 3]
    )
  }
}
