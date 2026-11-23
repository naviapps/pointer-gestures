namespace NaviApps.PointerGestures.Recognition;

/// <summary>Pointer events normalized by the host input adapter.</summary>
public enum PointerEventKind : uint
{
    /// <summary>A button press.</summary>
    Down = 1,
    /// <summary>Movement while the button is held.</summary>
    Moved = 2,
    /// <summary>A button release.</summary>
    Up = 3
}
/// <summary>One host-normalized pointer event; apply the returned disposition to its original event.</summary>
/// <param name="Kind">Press, held movement or release.</param>
/// <param name="Button">The same host button identifier used by GestureRecognitionConfiguration.</param>
/// <param name="Point">Position increasing rightward and downward.</param>
public readonly record struct PointerEvent(
    PointerEventKind Kind,
    uint Button,
    GesturePoint Point);
/// <summary>How a host handles the original pointer event.</summary>
public enum Disposition : uint
{
    /// <summary>Leave the original event in the platform stream.</summary>
    PassThrough = 0,
    /// <summary>Suppress the original event and deliver recovery effects in order.</summary>
    Consume = 1,
    /// <summary>A control call with no original pointer event.</summary>
    Control = 2
}
/// <summary>Current consumed-input stage, distinct from recognizer readiness.</summary>
public enum InputStage : uint
{
    /// <summary>No consumed input remains active.</summary>
    Idle = 0,
    /// <summary>A held press below the gesture start threshold.</summary>
    Pending = 1,
    /// <summary>An established visible direction sequence.</summary>
    Active = 2
}
/// <summary>A rejection or expiration reason; platform-source errors remain host-owned.</summary>
public enum Failure : uint
{
    /// <summary>New input was disabled by host policy.</summary>
    Disabled = 1,
    /// <summary>New input did not satisfy modifier policy.</summary>
    ModifiersNotSatisfied = 2,
    /// <summary>A total or inactivity deadline ended input without a match.</summary>
    Expired = 3
}
/// <summary>A copied value compared by its status fields and ordered directions.</summary>
/// <param name="Ready">Whether recognition has been started.</param>
/// <param name="Capturing">Whether input has reached the active gesture stage.</param>
/// <param name="Recording">Recording mode for future input; active input keeps its captured mode.</param>
/// <param name="Failure">Current rejection or expiration reason, if any.</param>
/// <param name="RecordedDirections">The last completed sequence, copied into read-only storage.</param>
public sealed record GestureRecognitionStatus(
    bool Ready,
    bool Capturing,
    bool Recording,
    Failure? Failure,
    IReadOnlyList<Direction> RecordedDirections)
{
    private readonly SnapshotValues<Direction> recordedDirections = SnapshotValues<Direction>.Copy(RecordedDirections);

    /// <summary>The last completed sequence in read-only storage.</summary>
    public IReadOnlyList<Direction> RecordedDirections
    {
        get => recordedDirections;
        init => recordedDirections = SnapshotValues<Direction>.Copy(value);
    }
}
/// <summary>Copied live geometry compared by its ordered values, independent of the engine lifetime.</summary>
/// <param name="Visible">Whether the trace is visible.</param>
/// <param name="Directions">Captured cardinal directions.</param>
/// <param name="Endpoints">Origin followed by one endpoint per direction.</param>
/// <param name="RawPoints">The bounded, periodically published raw coordinate buffer.</param>
/// <param name="Tail">The newest visible position, including movement since raw buffer publication.</param>
public sealed record GestureTrace(
    bool Visible,
    IReadOnlyList<Direction> Directions,
    IReadOnlyList<GesturePoint> Endpoints,
    IReadOnlyList<GesturePoint> RawPoints,
    GesturePoint? Tail)
{
    private readonly SnapshotValues<Direction> directions = SnapshotValues<Direction>.Copy(Directions);
    private readonly SnapshotValues<GesturePoint> endpoints = SnapshotValues<GesturePoint>.Copy(Endpoints);
    private readonly SnapshotValues<GesturePoint> rawPoints = SnapshotValues<GesturePoint>.Copy(RawPoints);

    /// <summary>Captured cardinal directions in read-only storage.</summary>
    public IReadOnlyList<Direction> Directions
    {
        get => directions;
        init => directions = SnapshotValues<Direction>.Copy(value);
    }

    /// <summary>Origin followed by one endpoint per direction, in read-only storage.</summary>
    public IReadOnlyList<GesturePoint> Endpoints
    {
        get => endpoints;
        init => endpoints = SnapshotValues<GesturePoint>.Copy(value);
    }

    /// <summary>The bounded raw coordinate buffer in read-only storage.</summary>
    public IReadOnlyList<GesturePoint> RawPoints
    {
        get => rawPoints;
        init => rawPoints = SnapshotValues<GesturePoint>.Copy(value);
    }
}
/// <summary>An ordered host delivery. The SDK never injects OS input, draws UI or executes commands.</summary>
public abstract record Effect
{
    /// <summary>Recover a consumed click by posting a press and release.</summary>
    /// <param name="Button">Host button identifier.</param>
    /// <param name="Point">Original press position.</param>
    public sealed record Click(uint Button, GesturePoint Point) : Effect;
    /// <summary>Recover a complete consumed drag, including its release.</summary>
    /// <param name="Button">Host button identifier.</param>
    /// <param name="Points">Origin and retained recent positions, in delivery order.</param>
    public sealed record Drag(uint Button, IReadOnlyList<GesturePoint> Points) : Effect
    {
        private readonly SnapshotValues<GesturePoint> points = SnapshotValues<GesturePoint>.Copy(Points);

        /// <summary>Owned replay points in delivery order.</summary>
        public IReadOnlyList<GesturePoint> Points
        {
            get => points;
            init => points = SnapshotValues<GesturePoint>.Copy(value);
        }
    }
    /// <summary>Restore a consumed press and movement, leaving release to the real event stream.</summary>
    /// <param name="Button">Host button identifier.</param>
    /// <param name="Points">Origin and retained recent positions, in delivery order.</param>
    public sealed record DragStart(uint Button, IReadOnlyList<GesturePoint> Points) : Effect
    {
        private readonly SnapshotValues<GesturePoint> points = SnapshotValues<GesturePoint>.Copy(Points);

        /// <summary>Owned replay points in delivery order.</summary>
        public IReadOnlyList<GesturePoint> Points
        {
            get => points;
            init => points = SnapshotValues<GesturePoint>.Copy(value);
        }
    }
    /// <summary>Post only a release to leave button state consistent after capture.</summary>
    /// <param name="Button">Host button identifier.</param>
    /// <param name="Point">Release position.</param>
    public sealed record Release(uint Button, GesturePoint Point) : Effect;
    /// <summary>Deliver an exact match after preceding recovery effects.</summary>
    /// <param name="Identifier">Host-owned identifier; zero and ulong.MaxValue are valid.</param>
    public sealed record Match(ulong Identifier) : Effect;
    /// <summary>Publish intermediate state, including a completed gesture before it hides.</summary>
    /// <param name="Status">Status at this publication.</param>
    /// <param name="Trace">Geometry at this publication.</param>
    public sealed record Publish(GestureRecognitionStatus Status, GestureTrace Trace) : Effect;
}
/// <summary>A copied reducer result. Deliver every effect in order before processing later host input.</summary>
/// <param name="Disposition">How to handle the original event.</param>
/// <param name="Stage">Final input stage.</param>
/// <param name="Status">Final recognition status.</param>
/// <param name="Trace">Final trace; effects can include earlier intermediate publications.</param>
/// <param name="InputGeneration">Nonzero active-input identity; zero means no active input.</param>
/// <param name="DiscardPendingVisibleTrace">Whether the host should discard queued visible publications from interrupted input.</param>
/// <param name="DeadlineNs">Next absolute monotonic deadline; schedule GestureRecognitionEngine.Tick at this time.</param>
/// <param name="Effects">Read-only, independently owned ordered deliveries.</param>
/// <param name="LastSessionReport">Last completed retained session. Persists across later input; compare generations. Not an event history.</param>
public sealed record GestureRecognitionOutput(
    Disposition Disposition,
    InputStage Stage,
    GestureRecognitionStatus Status,
    GestureTrace Trace,
    ulong InputGeneration,
    bool DiscardPendingVisibleTrace,
    ulong? DeadlineNs,
    IReadOnlyList<Effect> Effects,
    SessionReport? LastSessionReport)
{
    private readonly SnapshotValues<Effect> effects = SnapshotValues<Effect>.Copy(Effects);

    /// <summary>Owned effects in delivery order.</summary>
    public IReadOnlyList<Effect> Effects
    {
        get => effects;
        init => effects = SnapshotValues<Effect>.Copy(value);
    }
}
