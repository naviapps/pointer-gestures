using System.Collections;
using System.Collections.ObjectModel;

namespace NaviApps.PointerGestures;

internal sealed class SnapshotValues<T> : ReadOnlyCollection<T>, IEquatable<SnapshotValues<T>>, ICollection
{
    private static readonly SnapshotValues<T> EmptyValues = new(Array.Empty<T>());

    private SnapshotValues(T[] values) : base(values) { }

    internal static SnapshotValues<T> Copy(IReadOnlyList<T> values)
    {
        ArgumentNullException.ThrowIfNull(values);
        return values as SnapshotValues<T> ?? TakeOwned(values.ToArray());
    }

    internal static SnapshotValues<T> TakeOwned(T[] values)
        => values.Length == 0 ? EmptyValues : new(values);

    // ReadOnlyCollection forwards the wrapped collection's SyncRoot, which for an array is the array itself.
    object ICollection.SyncRoot => this;

    public bool Equals(SnapshotValues<T>? other)
        => ReferenceEquals(this, other) || (other is not null && this.SequenceEqual(other));

    public override bool Equals(object? obj) => obj is SnapshotValues<T> other && Equals(other);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Count);
        foreach (var value in this) hash.Add(value);
        return hash.ToHashCode();
    }
}
