namespace NaviApps.PointerGestures.Recognition;

/// <summary>A mutable exact matcher. Clone captures an independent copy-on-write snapshot.</summary>
public sealed class GesturePatternMatcher : IDisposable
{
    private readonly object gate = new();
    private readonly MatcherHandle handle;

    /// <summary>Create an empty exact matcher.</summary>
    public GesturePatternMatcher()
    {
        Native.EnsureAbi();
        handle = new(Native.CreateMatcher());
        if (handle.IsInvalid) throw new InvalidOperationException("Native matcher creation failed.");
    }

    private GesturePatternMatcher(MatcherHandle handle) => this.handle = handle;

    /// <summary>Whether no nonempty pattern is registered.</summary>
    public bool IsEmpty => WithHandle(h => Native.MatcherIsEmpty(h) != 0);

    /// <summary>Create an independently mutable copy-on-write snapshot.</summary>
    public GesturePatternMatcher Clone() => new(CloneHandle());

    internal MatcherHandle CloneHandle() => WithHandle(h => new MatcherHandle(Native.CloneMatcher(h)));

    /// <summary>Register or replace 1..4096 exact direction runs. Invalid input returns a structured error without mutation; null means success; zero and ulong.MaxValue are valid identifiers.</summary>
    public unsafe PatternValidationError? Register(ReadOnlySpan<Direction> pattern, ulong identifier)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(handle.IsClosed, this);
            fixed (Direction* data = pattern)
                return PatternValidationError.Decode(Native.Register(handle, data, (nuint)pattern.Length, identifier));
        }
    }

    /// <summary>Validate without mutation; null means valid. Indexes refer to zero-based input runs.</summary>
    public static unsafe PatternValidationError? Validate(ReadOnlySpan<Direction> pattern)
    {
        Native.EnsureAbi();
        fixed (Direction* data = pattern)
            return PatternValidationError.Decode(Native.ValidatePattern(data, (nuint)pattern.Length));
    }

    /// <summary>Find an exact nonempty pattern; null means no match, empty input or invalid directions.</summary>
    public unsafe ulong? Match(ReadOnlySpan<Direction> pattern)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(handle.IsClosed, this);
            fixed (Direction* data = pattern)
            {
                var result = Native.Match(handle, data, (nuint)pattern.Length);
                return result.Present == 0 ? null : result.Value;
            }
        }
    }

    /// <summary>Remove one exact registration and return its identifier, or null when absent/invalid.
    /// Clones and active recognition retain their captured registrations.</summary>
    public unsafe ulong? Remove(ReadOnlySpan<Direction> pattern)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(handle.IsClosed, this);
            fixed (Direction* data = pattern)
            {
                var result = Native.Remove(handle, data, (nuint)pattern.Length);
                return result.Present == 0 ? null : result.Value;
            }
        }
    }

    /// <summary>Remove all registrations and release mutable trie storage without invalidating snapshots.</summary>
    public void Clear() => WithHandle(h => Native.ClearMatcher(h));

    internal T WithHandle<T>(Func<MatcherHandle, T> action)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(handle.IsClosed, this);
            return action(handle);
        }
    }

    internal void WithHandle(Action<MatcherHandle> action) =>
      WithHandle(h => { action(h); return true; });

    /// <summary>Release this handle without invalidating clones or captured engine snapshots.</summary>
    public void Dispose() { lock (gate) handle.Dispose(); }
}
