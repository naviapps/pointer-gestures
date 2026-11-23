using NaviApps.PointerGestures.Recognition;

namespace NaviApps.PointerGestures.Rendering;

/// <summary>Converts recognition snapshots to Rust-validated rendering traces.
/// Use a converter serially for one stream of updates.</summary>
public sealed class OverlayTraceConverter
{
    private OverlayTrace? previous;

    /// <summary>Return visible geometry with a copied, independent snapshot.
    /// Tail-only updates share immutable geometry. Hidden or invalid snapshots return null
    /// and release cached geometry without invalidating previously returned traces.</summary>
    public OverlayTrace? Convert(GestureTrace trace)
    {
        ArgumentNullException.ThrowIfNull(trace);
        previous = OverlayTrace.FromSnapshot(trace, previous);
        return previous;
    }
}
