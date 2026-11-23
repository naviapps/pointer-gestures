using System.Diagnostics.CodeAnalysis;
using Microsoft.Win32.SafeHandles;
using System.Runtime.Versioning;
using NaviApps.PointerGestures.Recognition.Interop;

namespace NaviApps.PointerGestures.Recognition.Windows;

/// <summary>Modifier groups required at a new press. Either left or right key satisfies a group.</summary>
[Flags]
public enum WindowsModifierKeys : uint
{
    /// <summary>No modifier requirement.</summary>
    None = 0,
    /// <summary>Either Shift key.</summary>
    Shift = 1,
    /// <summary>Either Control key.</summary>
    Control = 2,
    /// <summary>Either Alt key.</summary>
    Alt = 4,
    /// <summary>Either Windows key.</summary>
    Windows = 8,
}

/// <summary>Native Windows desktop pointer recognition on a dedicated message-loop thread.
/// Consume observations promptly through ReadAsync or TryRead. Only Match and Publish effects remain: input recovery
/// has already been applied, so hosts must not inject it again. Hosts own commands,
/// UI, policy and permission/error presentation. One instance may run per process.</summary>
[SupportedOSPlatform("windows")]
public sealed class WindowsRecognizer : IDisposable
{
    private readonly object gate = new();
    private readonly WindowsRecognizerHandle handle;
    private readonly ObservationWaitHandle notification;
    private readonly CancellationTokenSource disposal = new();
    private readonly WindowsObservationReader reader;

    private sealed class ObservationWaitHandle : WaitHandle
    {
        internal ObservationWaitHandle(nint handle) => SafeWaitHandle = new SafeWaitHandle(handle, ownsHandle: true);
    }

    /// <summary>Install global mouse/Escape hooks for the configured button (0 through 4).
    /// Distances use physical pixels and MaximumPoints must not exceed 4096.
    /// Requires an interactive desktop; never requests elevation or changes permissions.</summary>
    public WindowsRecognizer(GestureRecognitionConfiguration configuration, GesturePatternMatcher matcher)
        : this(configuration, matcher ?? throw new ArgumentNullException(nameof(matcher)), null) { }

    /// <summary>Install native input hooks and resolve each eligible press with a prebuilt context.
    /// The synchronous hook callback must use bounded immutable lookups. Do not call the adapter,
    /// acquire blocking locks, perform I/O or execute commands. A failed callback stops capture
    /// before suppressing the new press. It can run before this method returns.</summary>
    public static WindowsRecognizer StartWithContext(GestureRecognitionConfiguration configuration, Func<WindowsPressContext, WindowsSessionContext> resolveContext)
        => new(configuration, null, resolveContext ?? throw new ArgumentNullException(nameof(resolveContext)));

    private WindowsRecognizer(GestureRecognitionConfiguration configuration, GesturePatternMatcher? matcher,
        Func<WindowsPressContext, WindowsSessionContext>? resolveContext)
    {
        WindowsRecognizerHandle.ThrowIfResolving();
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Windows desktop input requires Windows.");
        Native.EnsureAbi();
        if (NativeWindows.AbiVersion() != 3) throw new NotSupportedException("Unsupported Windows input adapter ABI.");
        var config = configuration.ToNative();
        reader = new(() => GuardedGate, ReadObservation, WaitForNotificationAsync);
        if (resolveContext is not null)
        {
            handle = WindowsRecognizerHandle.StartWithContext(config, resolveContext);
        }
        else
        {
            var created = matcher!.WithHandle(m =>
            {
                var pointer = NativeWindows.Start(config, m, out var error);
                return (pointer, error);
            });
            created.error.ThrowIfFailed();
            handle = new(created.pointer);
            if (handle.IsInvalid) throw new InvalidOperationException("Windows input returned no adapter.");
        }
        try
        {
            var ready = NativeWindows.DuplicateNotificationHandle(handle, out var error);
            error.ThrowIfFailed();
            notification = new(ready);
        }
        catch { handle.Dispose(); disposal.Dispose(); throw; }
    }

    /// <summary>Monotonic nanoseconds in the adapter's clock domain. Use this clock
    /// for presentation updates that consume its observation deadlines.</summary>
    public ulong NowNs { get { lock (GuardedGate) { EnsureOpen(); return NativeWindows.Now(handle); } } }

    /// <summary>Read one owned observation without waiting. Drain until false.
    /// A full 256-frame ordinary queue stops capture and restores input. Two reserved
    /// slots retain the triggering and shutdown observations before the lag error.
    /// Native hooks never wait for the consumer. Host command execution belongs after this call.</summary>
    public bool TryRead([NotNullWhen(true)] out GestureRecognitionOutput? output)
    {
        TryReadWithContext(out var observation);
        output = observation?.Output;
        return output is not null;
    }

    /// <summary>Drain one frame from the same queue as TryRead, with its captured host epoch.
    /// Retain the epoch's host command table until all associated observations have drained.</summary>
    public bool TryReadWithContext([NotNullWhen(true)] out WindowsGestureObservation? observation)
    {
        lock (GuardedGate)
        {
            observation = ReadObservation().Observation;
            return observation.HasValue;
        }
    }

    private (WindowsGestureObservation? Observation, bool Completed) ReadObservation()
    {
        EnsureOpen();
        var result = NativeWindows.ReadWithContext(handle, out var frame, out var epoch, out var error);
        error.ThrowIfFailed();
        return result switch
        {
            0 => (null, false),
            1 => (new WindowsGestureObservation(FrameReader.Read(frame), epoch.Present == 0 ? null : epoch.Value), false),
            3 => (null, true),
            _ => throw new InvalidDataException("Unknown Windows observation result."),
        };
    }
    /// <summary>Read the next observation asynchronously without timer polling.
    /// Returns null after Stop and final observations have drained. Consume serially and
    /// deliver effects in order. Cancellation before queue admission leaves the frame available.
    /// Once admitted, a dequeued frame is returned. Stop owns input recovery;
    /// Dispose wakes readers with ObjectDisposedException.</summary>
    public async ValueTask<GestureRecognitionOutput?> ReadAsync(CancellationToken cancellationToken = default)
        => (await ReadWithContextAsync(cancellationToken).ConfigureAwait(false))?.Output;

    /// <summary>Wait for the next frame with its host epoch without polling. Cancellation only
    /// ends the wait before queue admission; a dequeued frame is returned. Dispose wakes readers;
    /// Stop allows final frames to drain before completion.</summary>
    public ValueTask<WindowsGestureObservation?> ReadWithContextAsync(CancellationToken cancellationToken = default)
        => reader.ReadAsync(cancellationToken);

    // Registrations are installed under gate before Dispose can close the wait handle.
    // No host callback or continuation runs inside native capture or this lock.
    private async Task WaitForNotificationAsync(CancellationToken cancellationToken)
    {
        var completion = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        using var cancelled = cancellationToken.UnsafeRegister(static (state, token) => ((TaskCompletionSource)state!).TrySetCanceled(token), completion);
        using var disposed = disposal.Token.UnsafeRegister(static state => ((TaskCompletionSource)state!).TrySetException(new ObjectDisposedException(nameof(WindowsRecognizer))), completion);
        var wait = ThreadPool.RegisterWaitForSingleObject(notification,
            static (state, _) => ((TaskCompletionSource)state!).TrySetResult(), completion,
            Timeout.Infinite, executeOnlyOnce: true);
        try { await completion.Task.ConfigureAwait(false); }
        finally { wait.Unregister(null); }
    }

    /// <summary>Replace the future matcher for basic recognition. When using press context,
    /// the resolver selects the matcher at the next eligible press.</summary>
    public void UseMatcher(GesturePatternMatcher matcher)
    {
        ArgumentNullException.ThrowIfNull(matcher);
        lock (GuardedGate)
        {
            EnsureOpen();
            matcher.WithHandle(m => { NativeWindows.UseMatcher(handle, m, out var error); error.ThrowIfFailed(); return 0; });
        }
    }
    /// <summary>For a context-enabled recognizer, publish a new resolver for later presses.
    /// Held input and queued observations retain their captured policy, matcher and epoch.</summary>
    public void SetContextResolver(Func<WindowsPressContext, WindowsSessionContext> resolveContext)
    {
        ArgumentNullException.ThrowIfNull(resolveContext);
        lock (GuardedGate) { EnsureOpen(); handle.ReplaceResolver(resolveContext); }
    }
    /// <summary>Set future enablement and inclusive modifier requirements for basic recognition.
    /// When using press context, publish enablement changes through SetContextResolver.</summary>
    public void SetPolicy(bool enabled, WindowsModifierKeys requiredModifiers = WindowsModifierKeys.None)
    {
        lock (GuardedGate) { EnsureOpen(); NativeWindows.Policy(handle, enabled ? 1u : 0u, (uint)requiredModifiers, out var error); error.ThrowIfFailed(); }
    }
    /// <summary>Set recording mode for future sessions; existing input retains its mode.</summary>
    public void SetRecording(bool enabled)
    {
        lock (GuardedGate) { EnsureOpen(); NativeWindows.Recording(handle, enabled ? 1u : 0u, out var error); error.ThrowIfFailed(); }
    }
    /// <summary>Apply mutable tuning while preserving the configured button and empty policy.</summary>
    public void UpdateTuning(GestureRecognitionConfiguration configuration)
    {
        var config = configuration.ToNative();
        lock (GuardedGate) { EnsureOpen(); NativeWindows.Tuning(handle, config, out var error); error.ThrowIfFailed(); }
    }
    /// <summary>Cancel current recognition. Pending held input returns to its native
    /// pressed state so its later physical release stays balanced. Recovery fails
    /// if the original receiving window changed before a retained press is restored.</summary>
    public void Cancel()
    {
        lock (GuardedGate) { EnsureOpen(); NativeWindows.Cancel(handle, out var error); error.ThrowIfFailed(); }
    }
    /// <summary>Unhook, restore consumed held input and join the native thread.
    /// Idempotent; final observations may still be read. Recovery errors are reported,
    /// including a changed original recipient. Target checks before injection are best effort.</summary>
    public void Stop()
    {
        lock (GuardedGate)
        {
            EnsureOpen();
            NativeWindows.Stop(handle, out var error);
            handle.ReleaseResolver();
            error.ThrowIfFailed();
        }
    }
    private object GuardedGate { get { WindowsRecognizerHandle.ThrowIfResolving(); return gate; } }
    private void EnsureOpen() => ObjectDisposedException.ThrowIf(handle.IsClosed, this);
    /// <summary>Stop/join the adapter and free memory. Call Stop first when native
    /// recovery errors must be reported; finalization cannot report them.</summary>
    public void Dispose()
    {
        lock (GuardedGate)
        {
            if (handle.IsClosed) return;
            disposal.Cancel();
            handle.Dispose();
            notification.Dispose();
            disposal.Dispose();
        }
    }
}
