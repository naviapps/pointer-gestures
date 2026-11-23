namespace NaviApps.PointerGestures.Rendering;

/// <summary>Owned preview geometry compared by its ordered path and optional arrowhead commands.</summary>
public sealed record GesturePatternGeometry
{
    private readonly SnapshotValues<PathCommand> path;
    private readonly SnapshotValues<PathCommand>? arrowhead;

    /// <summary>The shaft or live trace command stream.</summary>
    public IReadOnlyList<PathCommand> Path
    {
        get => path;
        init
        {
            ArgumentNullException.ThrowIfNull(value);
            path = SnapshotValues<PathCommand>.Copy(value);
        }
    }

    /// <summary>The separate closed arrowhead, or null when omitted or unable to fit.</summary>
    public IReadOnlyList<PathCommand>? Arrowhead
    {
        get => arrowhead;
        init => arrowhead = value is null ? null : SnapshotValues<PathCommand>.Copy(value);
    }

    /// <summary>Copy the supplied command streams.</summary>
    public GesturePatternGeometry(IReadOnlyList<PathCommand> Path, IReadOnlyList<PathCommand>? Arrowhead)
        : this(SnapshotValues<PathCommand>.Copy(Path ?? throw new ArgumentNullException(nameof(Path))),
            Arrowhead is null ? null : SnapshotValues<PathCommand>.Copy(Arrowhead))
    { }

    private GesturePatternGeometry(SnapshotValues<PathCommand> path, SnapshotValues<PathCommand>? arrowhead)
    {
        this.path = path;
        this.arrowhead = arrowhead;
    }

    internal static GesturePatternGeometry FromOwnedCommands(PathCommand[] path, PathCommand[]? arrowhead)
        => new(SnapshotValues<PathCommand>.TakeOwned(path),
            arrowhead is null ? null : SnapshotValues<PathCommand>.TakeOwned(arrowhead));

    /// <summary>Return the owned, read-only command streams.</summary>
    public void Deconstruct(out IReadOnlyList<PathCommand> Path, out IReadOnlyList<PathCommand>? Arrowhead)
    {
        Path = path;
        Arrowhead = arrowhead;
    }
}
