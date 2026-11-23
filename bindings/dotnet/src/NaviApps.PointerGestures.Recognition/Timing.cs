namespace NaviApps.PointerGestures.Recognition;

/// <summary>Shared saturating conversion for host clocks and scheduling.</summary>
public static class Timing
{
    /// <summary>Convert positive finite seconds upward to nanoseconds, saturating at <see cref="ulong.MaxValue"/>.</summary>
    /// <param name="seconds">Duration; zero, negative and non-finite values produce zero.</param>
    /// <returns>A host delay; positive sub-nanosecond durations produce one.</returns>
    public static ulong DurationNanoseconds(double seconds)
    {
        Native.EnsureAbi();
        return Native.DurationNanoseconds(seconds);
    }
}
