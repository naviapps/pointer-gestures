namespace NaviApps.PointerGestures.Recognition;

/// <summary>Terminal recognition decisions; ordinary clicks are not OS errors.</summary>
public enum SessionOutcome : uint
{
    /// <summary>An exact registered pattern produced its host-owned match effect.</summary>
    Matched = 1,
    /// <summary>Recording completed a nonempty direction pattern.</summary>
    Recorded = 2,
    /// <summary>Movement did not start recognition; ordinary input recovery was requested.</summary>
    BelowThreshold = 3,
    /// <summary>The completed pattern was a registered prefix without an exact match.</summary>
    Unmatched = 4,
    /// <summary>A new direction could not extend any registered pattern.</summary>
    UnmatchedPrefix = 5,
    /// <summary>The host explicitly cancelled retained input.</summary>
    Cancelled = 6,
    /// <summary>Recognition stopped or closed while retaining input.</summary>
    Stopped = 7,
    /// <summary>A replacement button-down superseded retained input.</summary>
    Superseded = 8,
    /// <summary>The total session deadline was the earliest expired deadline.</summary>
    TotalDeadline = 9,
    /// <summary>The inactivity deadline was the earliest expired deadline.</summary>
    IdleDeadline = 10,
    /// <summary>A matching release had a non-finite location and could not match.</summary>
    InvalidRelease = 11,
    /// <summary>Another direction would exceed the session's captured budget.</summary>
    DirectionLimit = 12,
}

/// <summary>Coordinate-free copied measurements compared by value, not proof of OS recovery or command success.</summary>
/// <param name="Generation">Engine-local nonzero wrapping session identity.</param>
/// <param name="Outcome">Why retained input ended.</param>
/// <param name="ElapsedNs">Time since the original press, unaffected by tuning resets.</param>
/// <param name="IdleNs">Time since the last changed finite pointer location.</param>
/// <param name="PointCount">Admitted finite pointer samples, including stationary events.</param>
/// <param name="MaximumAxisDistance">Largest axis displacement in caller coordinate units, saturated finite.</param>
/// <param name="Captured">Whether recognition started.</param>
/// <param name="Recording">Recording mode captured at the press.</param>
/// <param name="Directions">Accepted runs, bounded by the captured budget and copied into read-only storage.</param>
/// <param name="RejectedDirection">Direction rejected by prefix matching or the budget.</param>
public sealed record SessionReport(ulong Generation, SessionOutcome Outcome, ulong ElapsedNs,
    ulong IdleNs, ulong PointCount, double MaximumAxisDistance, bool Captured, bool Recording,
    IReadOnlyList<Direction> Directions, Direction? RejectedDirection)
{
    private readonly SnapshotValues<Direction> directions = SnapshotValues<Direction>.Copy(Directions);

    /// <summary>Accepted direction runs in read-only storage.</summary>
    public IReadOnlyList<Direction> Directions
    {
        get => directions;
        init => directions = SnapshotValues<Direction>.Copy(value);
    }
}
