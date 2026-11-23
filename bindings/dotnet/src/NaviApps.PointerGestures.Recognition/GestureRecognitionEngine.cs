namespace NaviApps.PointerGestures.Recognition;

/// <summary>Serial native recognition with copied outputs. Deliver effects in order.
/// Call Close and deliver its recovery effects before disposing the host's input adapter.
/// Dispose/finalization free memory; they cannot inject input into an OS.</summary>
public sealed class GestureRecognitionEngine : IDisposable
{
    private readonly object gate = new();
    private readonly EngineHandle handle;

    /// <summary>Create a stopped engine after validating portable tuning.</summary>
    /// <exception cref="ArgumentOutOfRangeException">A distance or count in the configuration is invalid.</exception>
    public GestureRecognitionEngine(GestureRecognitionConfiguration configuration)
    {
        handle = new(Native.CreateEngine(configuration.ToNative()));
        if (handle.IsInvalid) throw new ArgumentException("Native recognition rejected the configuration.", nameof(configuration));
    }

    /// <summary>Capture a matcher for future input; existing input keeps its earlier snapshot.</summary>
    public void UseMatcher(GesturePatternMatcher matcher)
    {
        ArgumentNullException.ThrowIfNull(matcher);
        lock (gate)
        {
            EnsureOpen();
            matcher.WithHandle(h => Native.UseMatcher(handle, h));
        }
    }

    /// <summary>Apply valid tuning and restart active deadlines; false means unchanged. Button and empty-matcher policy are immutable.</summary>
    /// <remarks>A smaller raw point limit trims the current trace immediately and bounds replay.</remarks>
    /// <exception cref="ArgumentOutOfRangeException">A distance or count in the configuration is invalid.</exception>
    public bool UpdateTuning(GestureRecognitionConfiguration configuration, ulong nowNs, bool resetUnchanged = false)
    {
        lock (gate)
        {
            EnsureOpen();
            return Native.UpdateTuning(handle, configuration.ToNative(), nowNs, resetUnchanged ? 1u : 0u) switch
            {
                0 => false,
                1 => true,
                2 => throw new ArgumentException("Invalid distances, point limit, or immutable engine policy.", nameof(configuration)),
                _ => throw new InvalidDataException("Unknown native tuning result.")
            };
        }
    }

    /// <summary>Process input at monotonic nanoseconds. Non-finite down/move points pass through.
    /// A non-finite release for retained input consumes and interrupts it using finite recovery, without matching.</summary>
    public GestureRecognitionOutput Step(PointerEvent input, ulong nowNs)
    {
        if (input.Kind is not (PointerEventKind.Down or PointerEventKind.Moved or PointerEventKind.Up))
            throw new ArgumentException("Unknown pointer event kind.", nameof(input));
        return Send(new() { Kind = (uint)input.Kind, Button = input.Button, X = input.Point.X, Y = input.Point.Y, NowNs = nowNs });
    }

    /// <summary>Recover retained input before choosing a matcher and policy for an incoming press.
    /// An elapsed deadline takes precedence over supersession.</summary>
    /// <remarks>Call for a finite Down of the configured button. Deliver the returned effects,
    /// resolve the host context, then submit the press with Step if it is still current.</remarks>
    public GestureRecognitionOutput PrepareReplacementPress(ulong nowNs) => Send(new() { Kind = 11, NowNs = nowNs });

    /// <summary>Enable recognition. Repeated starts preserve active input.</summary>
    public GestureRecognitionOutput Start(ulong nowNs) => Send(new() { Kind = 6, NowNs = nowNs });
    /// <summary>Stop and recover consumed input; the engine remains reusable.</summary>
    public GestureRecognitionOutput Stop(ulong nowNs) => Send(new() { Kind = 5, NowNs = nowNs });
    /// <summary>Cancel consumed input without stopping recognition; no active input preserves the failure state.</summary>
    public GestureRecognitionOutput Cancel(ulong nowNs) => Send(new() { Kind = 4, NowNs = nowNs });
    /// <summary>Copy current state without advancing expiration; use Tick to deliver a deadline.</summary>
    public GestureRecognitionOutput Snapshot(ulong nowNs) => Send(new() { Kind = 9, NowNs = nowNs });
    /// <summary>Deliver absolute monotonic nanoseconds to expire consumed input without emitting a match.</summary>
    public GestureRecognitionOutput Tick(ulong nowNs) => Send(new() { Kind = 10, NowNs = nowNs });
    /// <summary>Set recording for future sessions; active input retains its captured mode.</summary>
    public GestureRecognitionOutput SetRecording(bool enabled, ulong nowNs) => Send(new() { Kind = 7, Value = enabled ? 1u : 0u, NowNs = nowNs });
    /// <summary>Set host enablement and modifier approval for future input; active input is not cancelled.</summary>
    public GestureRecognitionOutput SetPolicy(bool enabled, bool modifiersSatisfied, ulong nowNs) => Send(new() { Kind = 8, Value = (enabled ? 1u : 0u) | (modifiersSatisfied ? 2u : 0u), NowNs = nowNs });

    /// <summary>Consumes the native engine exactly once, returning final ordered recovery effects.</summary>
    public GestureRecognitionOutput Close(ulong nowNs)
    {
        lock (gate)
        {
            EnsureOpen();
            var frame = Native.Step(handle, new NativeEvent { Kind = 5, NowNs = nowNs });
            handle.Dispose();
            return FrameReader.Read(frame);
        }
    }

    private GestureRecognitionOutput Send(NativeEvent input)
    {
        lock (gate)
        {
            EnsureOpen();
            return FrameReader.Read(Native.Step(handle, input));
        }
    }
    private void EnsureOpen() => ObjectDisposedException.ThrowIf(handle.IsClosed, this);
    /// <summary>Release memory. Call Close and deliver recovery before disposing an active host input adapter.</summary>
    public void Dispose() { lock (gate) handle.Dispose(); }

}
