import CPointerGestureRecognition

/// The trie lives in Rust. Swift retains only generic command values and an immutable snapshot.
public struct GesturePatternMatcher<Match: Sendable>: Sendable {
  // Sharing is immutable. A mutating value first obtains its own handle and Swift
  // command array; Rust then copies its shared trie only if a snapshot still uses it.
  private final class Storage: @unchecked Sendable {
    let handle: OpaquePointer
    var values: [Match?]
    var freeIDs: [UInt64]

    init() {
      precondition(
        pointer_gesture_recognition_abi_version() == POINTER_GESTURE_RECOGNITION_ABI_VERSION,
        "Unsupported native recognition ABI")
      handle = pointer_gesture_recognition_matcher_create()!
      values = []
      freeIDs = []
    }

    init(copying other: Storage) {
      handle = pointer_gesture_recognition_matcher_clone(other.handle)!
      values = other.values
      freeIDs = other.freeIDs
    }

    deinit { pointer_gesture_recognition_matcher_destroy(handle) }
  }

  private var storage = Storage()

  /// Creates an empty exact-pattern matcher.
  public init() {}

  /// Whether no pattern is registered.
  public var isEmpty: Bool { pointer_gesture_recognition_matcher_is_empty(storage.handle) != 0 }

  /// Registers an exact pattern of 1...4096 direction runs, replacing its previous match.
  ///
  /// Empty, excessive or consecutive equal directions return a structured error without changing the matcher.
  /// A successful registration returns `nil`.
  /// Recognition cannot produce consecutive equal runs. Choose a session direction budget
  /// large enough for the registered patterns. Copies and active
  /// recognition sessions retain their captured values after later registrations.
  @discardableResult
  public mutating func register(pattern: [GestureDirection], match: Match) -> GesturePatternError? {
    if !isKnownUniquelyReferenced(&storage) { storage = Storage(copying: storage) }
    let codes = pattern.map(\.coreCode)
    return codes.withUnsafeBufferPointer { buffer in
      let old = pointer_gesture_recognition_matcher_match(
        storage.handle, buffer.baseAddress, buffer.count)
      let id = old.present == 0 ? (storage.freeIDs.last ?? UInt64(storage.values.count)) : old.value
      if let error = GesturePatternError.decode(
        pointer_gesture_recognition_matcher_register(
          storage.handle, buffer.baseAddress, buffer.count, id))
      {
        return error
      }
      if old.present == 0 && storage.freeIDs.isEmpty {
        storage.values.append(.some(match))
      } else {
        if old.present == 0 { storage.freeIDs.removeLast() }
        storage.values[Int(id)] = .some(match)
      }
      return nil
    }
  }

  /// Validates without changing registrations. `nil` means valid.
  public static func validationError(for pattern: [GestureDirection]) -> GesturePatternError? {
    precondition(
      pointer_gesture_recognition_abi_version() == POINTER_GESTURE_RECOGNITION_ABI_VERSION,
      "Unsupported native recognition ABI")
    return pattern.map(\.coreCode).withUnsafeBufferPointer {
      GesturePatternError.decode(
        pointer_gesture_recognition_pattern_validate($0.baseAddress, $0.count))
    }
  }

  /// Removes an exact registration and returns its command, or `nil` when absent.
  /// Copies and active recognition retain their previously captured registrations.
  @discardableResult
  public mutating func remove(pattern: [GestureDirection]) -> Match? {
    guard !pattern.isEmpty else { return nil }
    if !isKnownUniquelyReferenced(&storage) { storage = Storage(copying: storage) }
    return pattern.map(\.coreCode).withUnsafeBufferPointer { buffer in
      let removed = pointer_gesture_recognition_matcher_remove(
        storage.handle, buffer.baseAddress, buffer.count)
      guard removed.present != 0 else { return nil }
      let index = Int(removed.value)
      let value = storage.values[index]
      storage.values[index] = nil
      storage.freeIDs.append(removed.value)
      return value
    }
  }

  /// Removes all registrations and releases this value's retained commands/storage.
  /// Copies and active sessions remain valid.
  public mutating func removeAll() {
    if !isKnownUniquelyReferenced(&storage) {
      storage = Storage()
      return
    }
    pointer_gesture_recognition_matcher_clear(storage.handle)
    storage.values.removeAll()
    storage.freeIDs.removeAll()
  }

  /// Returns the match for an exact pattern, or `nil` for empty input and unmatched prefixes.
  public func match(pattern: [GestureDirection]) -> Match? {
    guard !pattern.isEmpty else { return nil }
    return pattern.map(\.coreCode).withUnsafeBufferPointer {
      let result = pointer_gesture_recognition_matcher_match(
        storage.handle, $0.baseAddress, $0.count)
      return result.present == 0 ? nil : value(for: result.value)
    }
  }

  func value(for id: UInt64) -> Match? {
    guard id < UInt64(storage.values.count) else { return nil }
    return storage.values[Int(id)]
  }

  // The receiving engine retains its own Arc. It never mutates or retains this handle.
  func withForeignHandle<Result>(_ body: (OpaquePointer) -> Result) -> Result {
    body(storage.handle)
  }

}
