namespace NaviApps.PointerGestures.Rendering;

/// <summary>Serial portable presentation state. Host owns clocks, scheduling and drawing.
/// Call Tick at the absolute monotonic deadline. Dispose releases memory only.</summary>
public sealed class OverlayState : IDisposable
{
    private readonly object gate = new();
    private readonly OverlayHandle handle;
    private OverlayTrace? trace;
    /// <summary>Create presentation state. Null/default timing selects the normal Rust-normalized defaults.</summary>
    public OverlayState(ulong nowNs, OverlayTrace? trace = null, OverlayTiming? timing = null)
    {
        Native.EnsureAbi();
        this.trace = trace;
        var config = timing ?? new OverlayTiming();
        handle = new(Native.CreateOverlay(config.NativeValue, trace?.Handle.DangerousGetHandle() ?? 0, nowNs));
        GC.KeepAlive(trace);
    }
    /// <summary>Submit a visible trace or null to hide. Ordinary movement coalesces; final geometry presents before hiding.</summary>
    public OverlayUpdate Update(OverlayTrace? trace, ulong nowNs)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(handle.IsClosed, this);
            var frame = Native.UpdateOverlay(handle, trace?.Handle.DangerousGetHandle() ?? 0, nowNs);
            GC.KeepAlive(trace);
            return Read(frame);
        }
    }
    /// <summary>Immediately discard visible/pending geometry and deadlines while retaining timing.</summary>
    public OverlayUpdate Clear(ulong nowNs)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(handle.IsClosed, this);
            return Read(Native.ClearOverlay(handle, nowNs));
        }
    }
    /// <summary>Deliver absolute monotonic nanoseconds; clock regressions are clamped and deadlines saturate.</summary>
    public OverlayUpdate Tick(ulong nowNs)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(handle.IsClosed, this);
            return Read(Native.TickOverlay(handle, nowNs));
        }
    }
    /// <summary>Replace cadence without resetting visible/pending geometry or final hiding.
    /// Deadlines remain anchored to the last presentation; schedule the returned deadline.</summary>
    public OverlayUpdate Configure(OverlayTiming timing, ulong nowNs)
    {
        ArgumentNullException.ThrowIfNull(timing);
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(handle.IsClosed, this);
            return Read(Native.ConfigureOverlay(handle, timing.NativeValue, nowNs));
        }
    }
    private OverlayUpdate Read(nint frame)
    {
        if (frame == 0) throw new InvalidDataException("Missing native overlay frame.");
        try
        {
            var view = Native.OverlayFrameView(frame);
            if (view.Changed != 0) trace = view.Trace == 0 ? null : new OverlayTrace(view.Trace);
            return new(view.Changed != 0, trace, view.DeadlinePresent == 0 ? null : view.DeadlineNs);
        }
        finally { Native.DestroyOverlayFrame(frame); }
    }
    /// <summary>Release reducer memory and cached trace ownership without invalidating previously returned snapshots.</summary>
    public void Dispose()
    {
        lock (gate)
        {
            handle.Dispose();
            trace = null;
        }
    }
}
