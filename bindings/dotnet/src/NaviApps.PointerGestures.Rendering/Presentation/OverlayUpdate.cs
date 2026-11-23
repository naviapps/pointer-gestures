namespace NaviApps.PointerGestures.Rendering;

/// <summary>An independently owned presentation snapshot. Hosts schedule the returned absolute deadline.</summary>
/// <param name="Changed">Whether the visible trace changed during this call.</param>
/// <param name="Trace">Current trace, or null while hidden; survives later updates and reducer disposal.</param>
/// <param name="DeadlineNanoseconds">Absolute monotonic time for OverlayState.Tick, or null when no tick is pending.</param>
public sealed record OverlayUpdate(bool Changed, OverlayTrace? Trace, ulong? DeadlineNanoseconds);

