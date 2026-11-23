using System.Collections.ObjectModel;
using NaviApps.PointerGestures.Recognition;

namespace NaviApps.PointerGestures.Rendering;

/// <summary>Immutable Rust-validated trace compared by its ordered directions, points and optional tail.</summary>
public sealed class OverlayTrace : IEquatable<OverlayTrace>
{
    private readonly SnapshotValues<Direction> directions;
    private readonly SnapshotValues<GesturePoint> points;

    internal TraceHandle Handle { get; }
    /// <summary>Captured cardinal directions copied from caller or native storage.</summary>
    public ReadOnlyCollection<Direction> Directions => directions;
    /// <summary>Origin and one endpoint per direction, in the same display space as the tail and viewport.</summary>
    public ReadOnlyCollection<GesturePoint> Points => points;
    /// <summary>Newest finite pointer position when supplied.</summary>
    public GesturePoint? Tail { get; }

    /// <summary>Copy and validate a nonempty direction sequence with matching finite coordinates.</summary>
    /// <exception cref="ArgumentException">Directions, counts or coordinates are invalid.</exception>
    public OverlayTrace(ReadOnlySpan<Direction> directions, ReadOnlySpan<GesturePoint> points, GesturePoint? tail = null)
    {
        var ownedDirections = directions.ToArray();
        var ownedPoints = points.ToArray();
        Handle = CreateHandle(ownedDirections, ownedPoints, tail)
            ?? throw new ArgumentException("Empty, invalid, mismatched or non-finite trace.");
        this.directions = SnapshotValues<Direction>.TakeOwned(ownedDirections);
        this.points = SnapshotValues<GesturePoint>.TakeOwned(ownedPoints);
        Tail = tail;
    }

    internal static unsafe OverlayTrace? FromSnapshot(GestureTrace snapshot, OverlayTrace? previous)
    {
        Native.EnsureAbi();
        IReadOnlyList<Direction> directions = snapshot.Visible ? snapshot.Directions : [];
        IReadOnlyList<GesturePoint> points = snapshot.Visible ? snapshot.Endpoints : [];
        // Marshalling uses temporary storage. Only a rebuilt trace needs new managed geometry.
        const int StackCapacity = 64;
        Span<Direction> codes = directions.Count <= StackCapacity ? stackalloc Direction[directions.Count] : new Direction[directions.Count];
        Span<GesturePoint> endpoints = points.Count <= StackCapacity ? stackalloc GesturePoint[points.Count] : new GesturePoint[points.Count];
        for (int i = 0; i < codes.Length; ++i) codes[i] = directions[i];
        for (int i = 0; i < endpoints.Length; ++i) endpoints[i] = points[i];
        var tail = snapshot.Tail;
        fixed (Direction* directionData = codes)
        fixed (GesturePoint* pointData = endpoints)
        {
            var converted = Native.TraceFromSnapshot(
                previous?.Handle.DangerousGetHandle() ?? 0, snapshot.Visible ? 1u : 0u, directionData,
                (nuint)codes.Length, pointData, (nuint)endpoints.Length, tail ?? default, tail.HasValue ? 1u : 0u);
            GC.KeepAlive(previous);
            if (converted.Trace == 0) return null;
            var handle = new TraceHandle(converted.Trace);
            return converted.ReusedGeometry != 0 && previous is not null
                ? new OverlayTrace(handle, previous.directions, previous.points, tail)
                : new OverlayTrace(handle);
        }
    }

    private static unsafe TraceHandle? CreateHandle(ReadOnlySpan<Direction> directions, ReadOnlySpan<GesturePoint> points, GesturePoint? tail)
    {
        Native.EnsureAbi();
        fixed (Direction* data = directions)
        fixed (GesturePoint* pointData = points)
        {
            var pointer = Native.CreateTrace(data, (nuint)directions.Length, pointData, (nuint)points.Length, tail ?? default, tail.HasValue ? 1u : 0u);
            return pointer == 0 ? null : new TraceHandle(pointer);
        }
    }
    internal OverlayTrace(nint borrowed) : this(new TraceHandle(Native.CloneTrace(borrowed))) { }

    private unsafe OverlayTrace(TraceHandle owned)
    {
        Handle = owned;
        if (Handle.IsInvalid) throw new InvalidDataException("Invalid native trace.");
        var view = Native.TraceView(Handle);
        directions = SnapshotValues<Direction>.TakeOwned(new ReadOnlySpan<Direction>(view.Directions, checked((int)view.Count)).ToArray());
        points = SnapshotValues<GesturePoint>.TakeOwned(new ReadOnlySpan<GesturePoint>(view.Points, checked((int)view.PointCount)).ToArray());
        Tail = view.TailPresent == 0 ? null : view.Tail;
    }
    /// <summary>Create an immutable tail replacement sharing native geometry and managed coordinates.</summary>
    public OverlayTrace WithTail(GesturePoint? tail)
    {
        var pointer = Native.TraceWithTail(Handle, tail ?? default, tail.HasValue ? 1u : 0u);
        if (pointer == 0) throw new ArgumentException("Tail must be finite.", nameof(tail));
        return new OverlayTrace(new TraceHandle(pointer), directions, points, tail);
    }
    private OverlayTrace(TraceHandle handle, SnapshotValues<Direction> directions, SnapshotValues<GesturePoint> points, GesturePoint? tail)
    { Handle = handle; this.directions = directions; this.points = points; Tail = tail; }
    /// <summary>Test conservative stroke/tail visibility using immutable Rust-owned cached bounds.</summary>
    public bool Intersects(Bounds viewport, double lineWidth = 0) => Native.TraceIntersects(Handle, viewport, lineWidth) != 0;

    /// <inheritdoc/>
    public bool Equals(OverlayTrace? other)
        => ReferenceEquals(this, other)
        || (other is not null && Tail == other.Tail && directions.Equals(other.directions) && points.Equals(other.points));

    /// <inheritdoc/>
    public override bool Equals(object? obj) => obj is OverlayTrace trace && Equals(trace);

    /// <inheritdoc/>
    public override int GetHashCode() => HashCode.Combine(Tail, directions, points);

    /// <summary>Compare immutable trace values, including null.</summary>
    public static bool operator ==(OverlayTrace? left, OverlayTrace? right)
        => left is null ? right is null : left.Equals(right);

    /// <summary>Check whether two trace values differ.</summary>
    public static bool operator !=(OverlayTrace? left, OverlayTrace? right) => !(left == right);
}

