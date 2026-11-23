/// Host-defined context used to select gesture policy and gesture matches.
public struct GestureRecognitionContext: Equatable, Sendable {
  /// Host-owned identifier for the context.
  public let identifier: String

  /// Creates a context with a host-owned identifier.
  ///
  /// Preserves the identifier exactly, including empty strings and whitespace.
  /// Hosts own identity validation and normalization. Use a `nil` context when
  /// no host context is active.
  public init(identifier: String) {
    self.identifier = identifier
  }
}
