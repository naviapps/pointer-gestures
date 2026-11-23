namespace NaviApps.PointerGestures.Recognition;

/// <summary>Recognition thresholds, deadlines and immutable input policy.</summary>
/// <param name="StartDistance">Finite, nonnegative dominant-axis distance needed to begin a gesture.</param>
/// <param name="ChangeDistance">Finite, nonnegative dominant-axis distance needed for a new direction.</param>
/// <param name="TotalDurationNs">Total input lifetime in nanoseconds; zero disables this deadline.</param>
/// <param name="IdleDurationNs">Inactivity limit in nanoseconds; zero disables this deadline.</param>
/// <param name="MaximumPoints">Positive raw trace point limit. Recovery retains the origin and at least one recent point.</param>
/// <param name="Button">Host button identifier: zero primary, one secondary, two middle, then auxiliary buttons.</param>
/// <param name="PassesThroughEmptyMatcher">Whether unconfigured non-recording input stays in the original event stream. Defaults to true.</param>
/// <param name="MaximumDirections">Direction budget in 1..4096, captured at the start of each session.</param>
public readonly record struct GestureRecognitionConfiguration(
    double StartDistance,
    double ChangeDistance,
    ulong TotalDurationNs,
    ulong IdleDurationNs,
    int MaximumPoints = 256,
    uint Button = 1,
    bool PassesThroughEmptyMatcher = true,
    int MaximumDirections = 64)
{
    /// <summary>Validate portable tuning without creating a recognition engine or starting input capture.</summary>
    /// <exception cref="ArgumentOutOfRangeException">A distance or count is invalid. ParamName identifies the first invalid field.</exception>
    public void Validate()
    {
        Native.EnsureAbi();
        // Unsigned nanoseconds are already valid, including zero to disable a deadline.
        switch (Native.ValidateTuning(StartDistance, ChangeDistance, 0, 0, 0, MaximumPoints))
        {
            case 0: break;
            case 1: throw new ArgumentOutOfRangeException(nameof(StartDistance), StartDistance, "Distance must be finite and nonnegative.");
            case 2: throw new ArgumentOutOfRangeException(nameof(ChangeDistance), ChangeDistance, "Distance must be finite and nonnegative.");
            case 5: throw new ArgumentOutOfRangeException(nameof(MaximumPoints), MaximumPoints, "Point limit must be positive.");
            default: throw new InvalidDataException("Unknown native tuning validation result.");
        }
        if (Native.ValidateDirectionLimit(MaximumDirections) != 1)
            throw new ArgumentOutOfRangeException(nameof(MaximumDirections), MaximumDirections, "Direction limit must be in 1..4096.");
    }

    internal NativeConfiguration ToNative()
    {
        Validate();
        return new() { StartDistance = StartDistance, ChangeDistance = ChangeDistance, TotalNs = TotalDurationNs, IdleNs = IdleDurationNs, MaximumPoints = (nuint)MaximumPoints, Button = Button, PassEmpty = PassesThroughEmptyMatcher ? 1u : 0u, MaximumDirections = (nuint)MaximumDirections };
    }
}
