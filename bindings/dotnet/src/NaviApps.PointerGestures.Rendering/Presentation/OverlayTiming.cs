namespace NaviApps.PointerGestures.Rendering;

/// <summary>Cadence normalized by Rust, shared with Swift and safe Rust consumers.</summary>
public sealed record OverlayTiming
{
    internal NativeOverlayConfiguration NativeValue { get; }
    /// <summary>Normalized animation interval in seconds; invalid input becomes 1/60 second.</summary>
    public double MinimumAnimationInterval => NativeValue.IntervalSeconds;
    /// <summary>Normalized point lag allowance; invalid input becomes 24 coordinate units.</summary>
    public double MaximumPointLagDistance => NativeValue.MaximumPointLag;
    /// <summary>Create timing normalized by the shared Rust reducer, defaulting to 60 Hz and 24 units of lag.</summary>
    public OverlayTiming(double minimumAnimationInterval = 1.0 / 60.0, double maximumPointLagDistance = 24)
    {
        Native.EnsureAbi();
        NativeValue = Native.OverlayTiming(minimumAnimationInterval, maximumPointLagDistance);
    }
}
