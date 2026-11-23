using System.Diagnostics;
using System.Drawing.Drawing2D;
using NaviApps.PointerGestures.Recognition;
using NaviApps.PointerGestures.Rendering;

namespace NaviApps.PointerGestures.WindowsExample;

internal sealed class GestureCanvas : Control
{
    private readonly GestureRecognitionEngine engine = new(new GestureRecognitionConfiguration(10, 25, 1_000_000_000, 0));
    private readonly GesturePatternMatcher matcher = new();
    private readonly System.Windows.Forms.Timer timer = new() { Interval = 8 };
    private readonly OverlayTraceConverter traceConverter = new();
    private readonly OverlayTiming timing;
    private OverlayState overlay;
    private OverlayTrace? visible;
    private ulong? engineDeadline, overlayDeadline;
    private bool activeInput;
    internal bool IsStopped { get; private set; }
    internal List<ulong> Matches { get; } = [];
    internal List<string> RecoveryKinds { get; } = [];
    internal IReadOnlyList<Direction> Recorded { get; private set; } = [];
    internal Action<string>? StatusChanged;
    private static ulong Now => (ulong)(Stopwatch.GetTimestamp() * (1_000_000_000.0 / Stopwatch.Frequency));

    internal GestureCanvas(double interval)
    {
        DoubleBuffered = true; ResizeRedraw = true; TabStop = true;
        BackColor = SystemColors.Window;
        AccessibleName = "Gesture input and preview";
        timing = new(interval);
        overlay = new(Now, timing: timing);
        matcher.Register([Direction.Right, Direction.Down], 1);
        engine.UseMatcher(matcher);
        Apply(engine.Start(Now));
        timer.Tick += (_, _) =>
        {
            var now = Now;
            if (engineDeadline is ulong deadline && now >= deadline) Apply(engine.Tick(now));
            if (overlayDeadline is ulong presentation && now >= presentation) Present(overlay.Tick(now));
            UpdateTimer();
        };
    }

    internal void SetRecording(bool enabled) { if (!IsStopped) Apply(engine.SetRecording(enabled, Now)); }
    internal void CancelInput()
    {
        if (IsStopped || !activeInput) return;
        Apply(engine.Cancel(Now));
    }
    internal void ClearFeedback()
    {
        CancelInput(); ResetOverlay(); Recorded = [];
        StatusChanged?.Invoke("Ready");
    }

    // Input is confined to this control. This example installs no global hook and
    // reports recovery to its own host rather than injecting into other windows.
    protected override void WndProc(ref Message message)
    {
        if (!IsStopped)
        {
            var point = new GesturePoint((short)(long)message.LParam, (short)((long)message.LParam >> 16));
            if (message.Msg == 0x204) // WM_RBUTTONDOWN
            {
                Focus(); Capture = true;
                Apply(engine.Step(new(PointerEventKind.Down, 1, point), Now));
                message.Result = 0; return;
            }
            if (message.Msg == 0x200 && Capture && activeInput) // WM_MOUSEMOVE
            {
                if (((long)message.WParam & 2) != 0) Apply(engine.Step(new(PointerEventKind.Moved, 1, point), Now));
                else CancelInput();
                message.Result = 0; return;
            }
            if (message.Msg == 0x205 && activeInput) // WM_RBUTTONUP
            {
                var output = engine.Step(new(PointerEventKind.Up, 1, point), Now);
                Recorded = output.Status.RecordedDirections;
                Apply(output);
                message.Result = 0; return;
            }
            if (message.Msg is 0x215 or 0x1F) CancelInput(); // WM_CAPTURECHANGED / WM_CANCELMODE
        }
        base.WndProc(ref message);
    }

    protected override void OnKeyDown(KeyEventArgs args)
    {
        if (args.KeyCode == Keys.Escape) { CancelInput(); args.Handled = true; }
        base.OnKeyDown(args);
    }

    private void Apply(GestureRecognitionOutput output)
    {
        activeInput = output.InputGeneration != 0;
        engineDeadline = output.DeadlineNs;
        if (output.DiscardPendingVisibleTrace) ResetOverlay();
        string? status = null;
        foreach (var effect in output.Effects)
        {
            switch (effect)
            {
                case Effect.Publish publication:
                    Present(overlay.Update(traceConverter.Convert(publication.Trace), Now));
                    break;
                case Effect.Match match:
                    Matches.Add(match.Identifier); status = "Matched Right → Down"; break;
                case Effect.Click:
                    RecoveryKinds.Add("click"); status = "Unmatched click returned to this canvas"; break;
                case Effect.Drag:
                    RecoveryKinds.Add("drag"); status = "Unmatched drag returned to this canvas"; break;
                case Effect.DragStart:
                    RecoveryKinds.Add("drag-start"); status = "Unmatched drag continues in this canvas"; break;
                case Effect.Release:
                    RecoveryKinds.Add("release"); status = "Captured button released"; break;
            }
        }
        if (!activeInput && IsHandleCreated && Capture) Capture = false;
        UpdateTimer();
        if (status != null) StatusChanged?.Invoke(status);
    }
    private void Present(OverlayUpdate update)
    {
        overlayDeadline = update.DeadlineNanoseconds;
        if (update.Changed) { visible = update.Trace; if (!IsStopped) Invalidate(); }
    }
    private void UpdateTimer() => timer.Enabled = !IsStopped && (engineDeadline.HasValue || overlayDeadline.HasValue);
    private void ResetOverlay() => Present(overlay.Clear(Now));

    protected override void OnPaint(PaintEventArgs args)
    {
        base.OnPaint(args);
        var graphics = args.Graphics;
        graphics.SmoothingMode = SmoothingMode.AntiAlias;
        var geometry = GestureGeometryPlanner.PlanPattern([Direction.Right, Direction.Down], Math.Max(0, Width), Math.Min(100, Math.Max(0, Height)), lineWidth: 3);
        using var pen = new Pen(SystemColors.Highlight, 3) { StartCap = LineCap.Round, EndCap = LineCap.Round, LineJoin = LineJoin.Round };
        using var preview = DrawingPath.Create(geometry.Path);
        graphics.DrawPath(pen, preview);
        if (geometry.Arrowhead is { } commands)
        {
            using var head = DrawingPath.Create(commands);
            using var brush = new SolidBrush(pen.Color);
            graphics.FillPath(brush, head);
        }
        if (visible is { } trace)
        {
            using var path = DrawingPath.Create(GestureGeometryPlanner.PlanLiveTrace(trace.Directions.ToArray(), trace.Points.ToArray(), tail: trace.Tail));
            pen.Color = Color.Crimson;
            graphics.DrawPath(pen, path);
        }
    }

    internal void Stop()
    {
        if (IsStopped) return;
        IsStopped = true; timer.Stop();
        Apply(engine.Close(Now));
    }
    protected override void Dispose(bool disposing)
    {
        if (disposing) { Stop(); timer.Dispose(); overlay.Dispose(); engine.Dispose(); matcher.Dispose(); }
        base.Dispose(disposing);
    }
}
