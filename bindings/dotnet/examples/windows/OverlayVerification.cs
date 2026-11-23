using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using NaviApps.PointerGestures.Recognition;
using NaviApps.PointerGestures.Rendering;
using NaviApps.PointerGestures.Rendering.Windows;

namespace NaviApps.PointerGestures.WindowsExample;

// Runs only in the explicitly owned interactive Windows acceptance fixture.
internal static class OverlayVerification
{
    [DllImport("user32.dll")] private static extern nint GetForegroundWindow();
    [DllImport("user32.dll")] private static extern bool IsWindow(nuint window);
    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")] private static extern nint GetWindowLong(nuint window, int index);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(nuint window, out uint process);
    [DllImport("user32.dll")] private static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] private static extern uint SendInput(uint count, Input[] input, int size);
    [DllImport("user32.dll", EntryPoint = "SendMessageW")] private static extern nint SendMessage(nuint window, uint message, nuint flags, nint data);
    [DllImport("user32.dll")] private static extern nint GetDC(nint window);
    [DllImport("user32.dll")] private static extern int ReleaseDC(nint window, nint dc);
    [DllImport("gdi32.dll")] private static extern bool BitBlt(nint destination, int x, int y, int width, int height, nint source, int sourceX, int sourceY, uint operation);
    [DllImport("kernel32.dll")] private static extern nint GetCurrentProcess();
    [DllImport("kernel32.dll")] private static extern bool GetProcessHandleCount(nint process, out uint count);
    [DllImport("user32.dll")] private static extern uint GetGuiResources(nint process, uint flags);
    [StructLayout(LayoutKind.Sequential)] private struct MouseInput { internal int X, Y; internal uint Data, Flags, Time; internal nuint Extra; }
    [StructLayout(LayoutKind.Sequential)] private struct Input { internal uint Type; internal MouseInput Mouse; }
    private static void Check(bool value, string description) { if (!value) throw new InvalidOperationException(description); }

    private static Bitmap Capture(GestureCanvas canvas, string directory, string name)
    {
        var image = new Bitmap(canvas.Width, canvas.Height);
        try
        {
            using var graphics = Graphics.FromImage(image);
            var origin = canvas.PointToScreen(System.Drawing.Point.Empty);
            // CopyFromScreen rejects combined enum values. Native CAPTUREBLT
            // explicitly includes layered windows in this owned desktop capture.
            var desktop = GetDC(0);
            Check(desktop != 0, "Acquire owned desktop capture DC");
            try
            {
                var target = graphics.GetHdc();
                try { Check(BitBlt(target, 0, 0, image.Width, image.Height, desktop, origin.X, origin.Y, 0x40CC0020), "Capture native layered overlay"); }
                finally { graphics.ReleaseHdc(target); }
            }
            finally { Check(ReleaseDC(0, desktop) != 0, "Release owned desktop capture DC"); }
            image.Save(Path.Combine(directory, name + ".png"), ImageFormat.Png);
            return image;
        }
        catch { image.Dispose(); throw; }
    }

    internal static async Task<object> Run(ExampleWindow window, string directory)
    {
        var canvas = window.Canvas;
        canvas.ClearFeedback();
        var origin = canvas.PointToScreen(System.Drawing.Point.Empty);
        var from = new GesturePoint(origin.X + 60, origin.Y + 160);
        var to = new GesturePoint(from.X + 120, from.Y);
        var trace = new OverlayTrace([Direction.Right], [from, to]);
        var foreground = GetForegroundWindow();
        var ownerThread = GetWindowThreadProcessId((nuint)window.Handle, out var owner);
        Check(ownerThread != 0 && owner == Environment.ProcessId, "Overlay verification uses only the owned window");
        using var overlay = new WindowsOverlay(new(0xF01E32FF, 4));
        try { _ = overlay.Snapshot; throw new System.Diagnostics.UnreachableException("Inactive snapshot accepted"); }
        catch (InvalidOperationException) { }
        overlay.Configure(new(0xF01E32FF, 4)); // Inactive configuration creates no resources.
        overlay.Activate(); overlay.Activate();
        var hidden = overlay.Snapshot;
        Check(!hidden.Visible && hidden.Bounds is null && IsWindow(hidden.Window), "Activation creates one hidden native window");
        var style = (ulong)GetWindowLong(hidden.Window, -20);
        Check((style & 0x080800A8UL) == 0x080800A8UL, "Layered, transparent, topmost, tool and no-activate styles");
        var thread = GetWindowThreadProcessId(hidden.Window, out owner);
        Check(owner == Environment.ProcessId && thread != 0 && thread != ownerThread, "Dedicated native owner thread");
        overlay.Update(trace);
        Check(overlay.Snapshot.Visible && GetForegroundWindow() == foreground, "Drawing never takes foreground focus");
        await Task.Delay(80);
        using (var image = Capture(canvas, directory, "native-overlay-horizontal"))
        {
            int pixels = 0;
            for (var x = 60; x <= 180; x++)
            {
                var color = image.GetPixel(x, 160);
                if (color.R > 180 && color.G < 90 && color.B < 100) pixels++;
            }
            Check(pixels >= 110, "Native transparent overlay paints the shared horizontal stroke");
        }
        overlay.Configure(new(0x1450E6FF, 12));
        Check(overlay.Snapshot.Window == hidden.Window && overlay.Snapshot.Visible, "Appearance updates preserve the native window and trace");
        try { overlay.Configure(new(0xF01E32FF, double.NaN)); throw new System.Diagnostics.UnreachableException("Invalid appearance accepted"); }
        catch (ArgumentException) { }
        Check(overlay.Snapshot.Window == hidden.Window && overlay.Snapshot.Visible, "Rejected appearance preserves native state");
        await Task.Delay(80);
        using (var image = Capture(canvas, directory, "native-overlay-reconfigured"))
        {
            var center = image.GetPixel(120, 160);
            var edge = image.GetPixel(120, 164);
            Check(center.B > 180 && center.R < 90 && edge.B > 180 && edge.R < 90,
                "Live native repaint applies new color and physical stroke width");
        }
        overlay.Configure(new(0xF01E32FF, 4));
        int clicks = 0;
        SendMessage(hidden.Window, 0x7E, 0, 0); // WM_DISPLAYCHANGE, owned overlay only.
        await Task.Delay(40);
        Check(overlay.Snapshot.Visible && GetForegroundWindow() == foreground, "Native layout notification refreshes without focus changes");
        MouseEventHandler listener = (_, args) => { if (args.Button == MouseButtons.Left) clicks++; };
        canvas.MouseDown += listener;
        try
        {
            Check(SetCursorPos((int)from.X + 60, (int)from.Y), "Place pointer on owned overlay stroke");
            Check(SendInput(2, [new() { Mouse = new() { Flags = 2 } }, new() { Mouse = new() { Flags = 4 } }], Marshal.SizeOf<Input>()) == 2, "Owned click injection");
            await Task.Delay(80);
            Check(clicks == 1, "OS click passes through the native overlay into the owned canvas");
        }
        finally { canvas.MouseDown -= listener; }
        overlay.Clear();
        Check(!overlay.Snapshot.Visible, "Clear discards pending geometry immediately");
        overlay.Update(trace);
        overlay.Update(new OverlayTrace([Direction.Right, Direction.Down], [from, to, new(to.X, to.Y + 80)]));
        await Task.Delay(60);
        using (var curve = Capture(canvas, directory, "native-overlay-corner"))
        {
            var bounds = Verification.RedBounds(curve);
            Check(bounds.Width >= 120 && bounds.Height >= 80, "Native Rust cubic path reaches both cardinal runs");
            var vertical = curve.GetPixel(180, 220);
            var outside = curve.GetPixel(170, 220);
            Check(vertical.R > 180 && vertical.G < 90 && outside.G > 90, "Native curved stroke preserves its vertical axis");
        }
        overlay.Clear();
        overlay.Configure(new(0xF01E32FF, 4, Timing: new OverlayTiming(0.05, 24)));
        overlay.Update(trace);
        overlay.Update(new OverlayTrace([Direction.Right], [from, new(to.X + 1, to.Y)]));
        overlay.Clear();
        Check(!overlay.Snapshot.Visible && overlay.Snapshot.Window == hidden.Window,
            "Clear discards pending movement on the same native window");
        await Task.Delay(80);
        Check(!overlay.Snapshot.Visible, "Cancelled native cadence cannot resurrect movement");
        overlay.Update(trace);
        overlay.Update(null);
        overlay.Clear();
        Check(!overlay.Snapshot.Visible, "Clear discards a pending final hide immediately");
        overlay.Update(trace);
        overlay.Update(null);
        overlay.Configure(new(0xF01E32FF, 5, Timing: new OverlayTiming(0.05, 24)));
        var configuredFinal = overlay.Snapshot;
        Check(configuredFinal.Window == hidden.Window && configuredFinal.Visible,
            "In-place configuration preserves the pending visible final frame on the same window");
        await Task.Delay(160);
        Check(!overlay.Snapshot.Visible, "Rust owner schedules final-frame hiding without a host timer");
        overlay.Deactivate(); overlay.Deactivate();
        Check(!IsWindow(hidden.Window), "Deactivation destroys native window resources");
        var process = GetCurrentProcess();
        Check(GetProcessHandleCount(process, out var handlesBefore), "Read owned process handle baseline");
        var gdiBefore = GetGuiResources(process, 0);
        var userBefore = GetGuiResources(process, 1);
        for (var index = 0; index < 8; index++)
        {
            overlay.Activate();
            var identity = overlay.Snapshot.Window;
            overlay.Update(trace); overlay.Clear(); overlay.Deactivate();
            Check(!IsWindow(identity), "Repeated native activation tears down every window");
        }
        Check(GetProcessHandleCount(process, out var handlesAfter), "Read owned process handle result");
        var gdiAfter = GetGuiResources(process, 0);
        var userAfter = GetGuiResources(process, 1);
        Check(handlesAfter <= handlesBefore + 2 && gdiAfter <= gdiBefore + 1 && userAfter <= userBefore + 1,
            "Repeated overlay lifetimes do not accumulate native resource handles");
        overlay.Dispose(); overlay.Dispose();
        try { overlay.Activate(); throw new System.Diagnostics.UnreachableException("Disposed overlay accepted"); }
        catch (ObjectDisposedException) { }
        return new
        {
            sharedRaster = true,
            physicalPixels = true,
            clickThrough = true,
            nonactivating = true,
            layoutNotification = true,
            liveReconfiguration = true,
            autonomousFinalHide = true,
            explicitTeardown = true,
            reactivationCount = 8,
            handlesBefore,
            handlesAfter,
            gdiBefore,
            gdiAfter,
            userBefore,
            userAfter
        };
    }
}
