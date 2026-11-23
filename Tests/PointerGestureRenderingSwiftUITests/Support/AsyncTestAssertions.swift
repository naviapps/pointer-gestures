import Foundation
import XCTest

@testable import PointerGestureRendering

private let asyncAssertionPollingIntervalNanoseconds: UInt64 = 20_000_000

@MainActor
func assertEventually(
  timeout: TimeInterval = 1,
  file: StaticString = #filePath,
  line: UInt = #line,
  _ predicate: @escaping @MainActor () -> Bool
) async {
  guard timeout.isFinite && timeout >= 0 else {
    XCTFail("Expected a finite non-negative timeout, got \(timeout)s", file: file, line: line)
    return
  }

  let deadline = ProcessInfo.processInfo.systemUptime + timeout
  if predicate() {
    return
  }
  while ProcessInfo.processInfo.systemUptime < deadline {
    do {
      try await Task.sleep(nanoseconds: asyncAssertionPollingIntervalNanoseconds)
    } catch {
      XCTFail("Condition wait cancelled before \(timeout)s elapsed", file: file, line: line)
      return
    }
    guard ProcessInfo.processInfo.systemUptime < deadline else { break }
    if predicate() {
      return
    }
  }
  XCTFail("Condition not met within \(timeout)s", file: file, line: line)
}
