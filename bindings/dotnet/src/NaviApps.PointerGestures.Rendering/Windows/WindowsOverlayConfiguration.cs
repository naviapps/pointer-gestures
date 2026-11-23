namespace NaviApps.PointerGestures.Rendering.Windows;

/// <summary>Host appearance and cadence. Color is straight RGBA packed 0xRRGGBBAA;
/// width is in physical desktop pixels. Rust validates positive width at most 4096
/// and finite nonnegative corner scale, and normalizes timing.</summary>
public sealed record WindowsOverlayConfiguration(uint Rgba, double LineWidth,
    double CornerRadiusScale = 1, OverlayTiming? Timing = null)
{
    internal NativeWindowsOverlayConfiguration NativeValue => new()
    {
        Rgba = Rgba,
        LineWidth = LineWidth,
        RadiusScale = CornerRadiusScale,
        Timing = (Timing ?? new()).NativeValue
    };
}
