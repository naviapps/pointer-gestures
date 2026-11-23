using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using NaviApps.PointerGestures.Recognition;

namespace NaviApps.PointerGestures.WindowsExample;

internal static class Verification
{
    [DllImport("user32.dll", EntryPoint = "SendMessageW")]
    private static extern nint SendMessage(nint window, uint message, nint flags, nint point);
    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(nint window, out uint process);

    private static void Check(bool value, string description) { if (!value) throw new InvalidOperationException(description); }
    private static void Send(GestureCanvas canvas, uint message, int x, int y)
    {
        var thread = GetWindowThreadProcessId(canvas.Handle, out var process);
        Check(thread != 0 && process == Environment.ProcessId, "Only the owned canvas may receive test input");
        SendMessage(canvas.Handle, message, message == 0x205 ? 0 : 2, unchecked((nint)((uint)(ushort)x | ((uint)(ushort)y << 16))));
    }
    private static Bitmap Capture(GestureCanvas canvas, string directory, string name)
    {
        canvas.Update();
        var image = new Bitmap(canvas.Width, canvas.Height);
        canvas.DrawToBitmap(image, new Rectangle(System.Drawing.Point.Empty, image.Size));
        image.Save(Path.Combine(directory, name + ".png"), ImageFormat.Png);
        return image;
    }
    internal static Rectangle RedBounds(Bitmap image)
    {
        int minX = image.Width, minY = image.Height, maxX = -1, maxY = -1;
        for (var y = 0; y < image.Height; y++)
            for (var x = 0; x < image.Width; x++)
            {
                var color = image.GetPixel(x, y);
                if (color.R > 140 && color.R > color.G * 2 && color.R > color.B * 2)
                { minX = Math.Min(minX, x); maxX = Math.Max(maxX, x); minY = Math.Min(minY, y); maxY = Math.Max(maxY, y); }
            }
        return maxX < 0 ? Rectangle.Empty : Rectangle.FromLTRB(minX, minY, maxX + 1, maxY + 1);
    }
    internal static async Task<object> Run(ExampleWindow window, string directory)
    {
        var canvas = window.Canvas;
        await Task.Delay(60);
        Send(canvas, 0x204, 40, 160); Send(canvas, 0x200, 140, 160);
        Check(canvas.Capture, "Native mouse capture starts");
        using var horizontal = Capture(canvas, directory, "horizontal");
        var horizontalBounds = RedBounds(horizontal);
        Check(horizontalBounds.Width >= 95 && horizontalBounds.Height <= 5, "Horizontal path stays on its cardinal axis");
        Send(canvas, 0x200, 140, 240);
        using var corner = Capture(canvas, directory, "corner");
        Check(RedBounds(corner).Height >= 75, "Native cubic corner draws the vertical run");
        Send(canvas, 0x205, 140, 240);
        await Task.Delay(130);
        Check(!canvas.Capture && canvas.Matches.SequenceEqual([1UL]), "Native window input reaches Rust exact recognition");
        using var hidden = Capture(canvas, directory, "hidden");
        Check(RedBounds(hidden).IsEmpty, "Native timer hides the completed trace");

        window.Recording.Checked = true;
        Send(canvas, 0x204, 260, 140); Send(canvas, 0x200, 260, 240);
        using var vertical = Capture(canvas, directory, "vertical");
        var verticalBounds = RedBounds(vertical);
        Check(verticalBounds.Height >= 95 && verticalBounds.Width <= 5, "Vertical path stays on its cardinal axis");
        Send(canvas, 0x205, 260, 240);
        Check(canvas.Recorded.SequenceEqual([Direction.Down]) && canvas.Matches.Count == 1, "Recording returns directions without a match");
        var recorded = canvas.Recorded.ToArray();
        canvas.ClearFeedback();

        Send(canvas, 0x204, 40, 160); Send(canvas, 0x200, 140, 160);
        await Task.Delay(1200);
        using var expired = Capture(canvas, directory, "expired");
        Check(!canvas.Capture && RedBounds(expired).IsEmpty && canvas.RecoveryKinds.Last() == "release", "Expiration releases native capture and visible feedback");

        Send(canvas, 0x204, 40, 160); Send(canvas, 0x200, 140, 160);
        canvas.Capture = false;
        await Task.Delay(130);
        using var cancelled = Capture(canvas, directory, "cancelled");
        Check(RedBounds(cancelled).IsEmpty && canvas.RecoveryKinds.Last() == "release", "Capture loss cancels input and visible feedback");
        Send(canvas, 0x204, 40, 160);
        SendMessage(canvas.Handle, 0x100, (nint)(int)Keys.Escape, 0); // WM_KEYDOWN
        Check(!canvas.Capture && canvas.RecoveryKinds.Last() == "click", "Escape cancels pending input and native capture");
        var nativeOverlay = await OverlayVerification.Run(window, directory);
        Send(canvas, 0x204, 40, 160);
        window.Close();
        Check(canvas.IsStopped && canvas.RecoveryKinds.Last() == "click", "Close delivers pending recovery before releasing the input owner");
        return new
        {
            state = "passed",
            architecture = RuntimeInformation.ProcessArchitecture.ToString(),
            matches = canvas.Matches,
            recorded,
            recovery = canvas.RecoveryKinds,
            horizontalBounds,
            verticalBounds,
            nativeOverlay,
            scope = "Actual WinForms window messages and capture, Rust recognition/presentation, native GDI+ raster. Owned-window synthetic recognition messages plus native transparent overlay and OS click-through injection. No global hook, physical-device or user-data check."
        };
    }
}
