using NaviApps.PointerGestures.Rendering;

internal static class NativeLayoutTests
{
    internal static void Verify(string path)
    {
        var layout = new AbiLayout(path, "PointerGestureRendering");
        layout.Check<PathCommand>("Command");
        layout.Check<NativeOverlayConfiguration>("OverlayConfiguration",
            ("maximum_point_lag", nameof(NativeOverlayConfiguration.MaximumPointLag)));
        layout.Check<NativeTraceView>("TraceView",
            ("points", nameof(NativeTraceView.Points)), ("tail", nameof(NativeTraceView.Tail)),
            ("tail_present", nameof(NativeTraceView.TailPresent)));
        layout.Check<NativeTraceConversion>("TraceConversion",
            ("reused_geometry", nameof(NativeTraceConversion.ReusedGeometry)));
        layout.Check<NativeOverlayView>("OverlayView", ("deadline_ns", nameof(NativeOverlayView.DeadlineNs)));

        if (OperatingSystem.IsWindows())
        {
            layout.Check<NativeWindowsOverlayConfiguration>("WindowsOverlayConfiguration",
                ("line_width", nameof(NativeWindowsOverlayConfiguration.LineWidth)),
                ("timing", nameof(NativeWindowsOverlayConfiguration.Timing)));
            layout.Check<NativeWindowsOverlayError>("WindowsOverlayError",
                ("system", nameof(NativeWindowsOverlayError.System)));
            layout.Check<NativeWindowsOverlayView>("WindowsOverlayView",
                ("bounds", nameof(NativeWindowsOverlayView.Bounds)));
        }
        layout.CheckComplete();
    }
}
