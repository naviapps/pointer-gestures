using System.Collections.ObjectModel;
using NaviApps.PointerGestures.Recognition;

namespace NaviApps.PointerGestures.Rendering;

/// <summary>Immutable Rust-validated trace compared by its ordered directions, points and optional tail.</summary>
public sealed class OverlayTrace : IEquatable<OverlayTrace>
{
    internal TraceHandle Handle { get; }
    /// <summary>Captured cardinal directions copied from caller or native storage.</summary>
    public ReadOnlyCollection<Direction> Directions { get; }
    /// <summary>Origin and one endpoint per direction, in the same display space as the tail and viewport.</summary>
    public ReadOnlyCollection<GesturePoint> Points { get; }
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
        Directions = SnapshotValues<Direction>.TakeOwned(ownedDirections);
        Points = SnapshotValues<GesturePoint>.TakeOwned(ownedPoints);
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
                ? new OverlayTrace(handle, previous.Directions, previous.Points, tail)
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
        Directions = SnapshotValues<Direction>.TakeOwned(new ReadOnlySpan<Direction>(view.Directions, checked((int)view.Count)).ToArray());
        Points = SnapshotValues<GesturePoint>.TakeOwned(new ReadOnlySpan<GesturePoint>(view.Points, checked((int)view.PointCount)).ToArray());
        Tail = view.TailPresent == 0 ? null : view.Tail;
    }
    /// <summary>Create an immutable tail replacement sharing native geometry and managed coordinates.</summary>
    public OverlayTrace WithTail(GesturePoint? tail) =>
        TryWithTail(tail) ?? throw new ArgumentException("Tail must be finite.", nameof(tail));

    private OverlayTrace? TryWithTail(GesturePoint? tail)
    {
        var pointer = Native.TraceWithTail(Handle, tail ?? default, tail.HasValue ? 1u : 0u);
        return pointer == 0 ? null : new OverlayTrace(new TraceHandle(pointer), Directions, Points, tail);
    }
    private OverlayTrace(TraceHandle handle, ReadOnlyCollection<Direction> directions, ReadOnlyCollection<GesturePoint> points, GesturePoint? tail)
    { Handle = handle; Directions = directions; Points = points; Tail = tail; }
    /// <summary>Test conservative stroke/tail visibility using immutable Rust-owned cached bounds.</summary>
    public bool Intersects(Bounds viewport, double lineWidth = 0) => Native.TraceIntersects(Handle, viewport, lineWidth) != 0;

    /// <inheritdoc/>
    public bool Equals(OverlayTrace? other)
        => ReferenceEquals(this, other)
        || (other is not null && Tail == other.Tail
            && (ReferenceEquals(Directions, other.Directions) || Directions.SequenceEqual(other.Directions))
            && (ReferenceEquals(Points, other.Points) || Points.SequenceEqual(other.Points)));

    /// <inheritdoc/>
    public override bool Equals(object? obj) => obj is OverlayTrace trace && Equals(trace);

    /// <inheritdoc/>
    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Tail);
        hash.Add(Directions.Count);
        foreach (var direction in Directions) hash.Add(direction);
        foreach (var point in Points) hash.Add(point);
        return hash.ToHashCode();
    }

    /// <summary>Compare immutable trace values, including null.</summary>
    public static bool operator ==(OverlayTrace? left, OverlayTrace? right)
        => left is null ? right is null : left.Equals(right);

    /// <summary>Check whether two trace values differ.</summary>
    public static bool operator !=(OverlayTrace? left, OverlayTrace? right) => !(left == right);
}

