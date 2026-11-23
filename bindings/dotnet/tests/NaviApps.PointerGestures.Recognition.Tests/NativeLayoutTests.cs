using NaviApps.PointerGestures.Recognition;
using NaviApps.PointerGestures.Recognition.Interop;

internal static class NativeLayoutTests
{
    internal static void Verify(string path)
    {
        var layout = new AbiLayout(path, "PointerGestureRecognition");
        layout.Check<GesturePoint>("Point");
        layout.Check<NativeConfiguration>("Config",
            ("maximum_points", nameof(NativeConfiguration.MaximumPoints)),
            ("pass_empty", nameof(NativeConfiguration.PassEmpty)),
            ("maximum_directions", nameof(NativeConfiguration.MaximumDirections)));
        layout.Check<NativeFrame>("FrameView",
            ("input_generation", nameof(NativeFrame.InputGeneration)),
            ("directions", nameof(NativeFrame.Directions)),
            ("tail", nameof(NativeFrame.Tail)),
            ("deadline_ns", nameof(NativeFrame.DeadlineNs)));
        layout.Check<NativeEffect>("EffectView",
            ("trace", nameof(NativeEffect.Trace)), ("ready", nameof(NativeEffect.Ready)),
            ("recorded", nameof(NativeEffect.Recorded)));
        layout.Check<NativeSessionReport>("SessionReportView",
            ("generation", nameof(NativeSessionReport.Generation)),
            ("directions", nameof(NativeSessionReport.Directions)));

        if (OperatingSystem.IsWindows())
        {
            layout.Check<WindowsError>("WindowsError", ("expected", nameof(WindowsError.Expected)));
            layout.Check<NativePressContext>("WindowsPressContext",
                ("foreground_process_id", nameof(NativePressContext.ForegroundProcessId)));
            layout.Check<NativeEpoch>("WindowsEpoch", ("present", nameof(NativeEpoch.Present)));
        }
        layout.CheckComplete();
    }
}
