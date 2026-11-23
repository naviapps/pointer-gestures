using NaviApps.PointerGestures.Recognition.Windows;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using Microsoft.Win32.SafeHandles;

namespace NaviApps.PointerGestures.Recognition.Interop;

internal sealed class WindowsRecognizerHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    [ThreadStatic]
    private static bool resolving;

    private sealed class ResolverSlot(Func<WindowsPressContext, WindowsSessionContext> resolver)
    {
        internal Func<WindowsPressContext, WindowsSessionContext> Resolver = resolver;
    }

    private ResolverSlot? resolver;
    private GCHandle callbackContext;

    internal WindowsRecognizerHandle(nint value) : base(true) => SetHandle(value);

    [SupportedOSPlatform("windows")]
    internal static unsafe WindowsRecognizerHandle StartWithContext(NativeConfiguration configuration, Func<WindowsPressContext, WindowsSessionContext> resolver)
    {
        var owner = new WindowsRecognizerHandle(0) { resolver = new ResolverSlot(resolver) };
        // The SafeHandle owns the slot. A weak callback root permits finalization
        // even when a resolver closure indirectly captures the recognizer.
        owner.callbackContext = GCHandle.Alloc(owner.resolver, GCHandleType.Weak);
        try
        {
            owner.SetHandle(NativeWindows.StartWithContext(configuration, &Resolve, GCHandle.ToIntPtr(owner.callbackContext), out var error));
            error.ThrowIfFailed();
            if (owner.IsInvalid) throw new InvalidOperationException("Windows input returned no adapter.");
            return owner;
        }
        catch (Exception error)
        {
            owner.Dispose();
            owner.ReleaseResolver();
            if (error is EntryPointNotFoundException)
                throw new NotSupportedException("This Windows input library does not expose press-context resolution.", error);
            throw;
        }
    }

    internal void ReplaceResolver(Func<WindowsPressContext, WindowsSessionContext> value)
    {
        if (resolver is null) throw new InvalidOperationException("An active context-enabled recognizer is required.");
        Volatile.Write(ref resolver.Resolver, value);
    }

    internal static void ThrowIfResolving()
    {
        if (resolving) throw new InvalidOperationException("Recognizer calls are unavailable inside a press resolver.");
    }

    [SupportedOSPlatform("windows")]
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void Resolve(nint context, NativePressContext press, nint selection)
    {
        resolving = true;
        try
        {
            if (GCHandle.FromIntPtr(context).Target is not ResolverSlot slot) return;
            var resolve = Volatile.Read(ref slot.Resolver);
            var session = resolve(new WindowsPressContext(
                press.Point, press.RecipientProcessId, press.ForegroundProcessId == 0 ? null : press.ForegroundProcessId));
            if (session is not null)
                NativeWindows.SelectContext(selection, session.Matcher, session.Enabled ? 1u : 0u,
                    (uint)session.RequiredModifiers, session.Epoch);
        }
        catch (Exception)
        {
            // An empty selection terminates capture on the native owner thread.
            // A managed exception must never cross the unmanaged hook boundary.
        }
        finally { resolving = false; }
    }

    protected override bool ReleaseHandle()
    {
        NativeWindows.Destroy(handle);
        ReleaseResolver();
        return true;
    }

    // Only after a joined stop/destroy, or a start which returned no worker.
    internal void ReleaseResolver()
    {
        if (callbackContext.IsAllocated) callbackContext.Free();
        resolver = null;
    }
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativePressContext
{
    internal GesturePoint Point;
    internal uint RecipientProcessId, ForegroundProcessId;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeEpoch
{
    internal ulong Value;
    internal uint Present;
}

[StructLayout(LayoutKind.Sequential)]
internal struct WindowsError
{
    internal uint Kind, System, Sent, Expected;
    internal readonly void ThrowIfFailed()
    {
        if (Kind == 0) return;
        throw Kind switch
        {
            1 => new ArgumentException("Invalid Windows input configuration, modifier flags or immutable tuning."),
            2 => new WindowsRecognitionException(WindowsRecognitionFailure.AlreadyRunning),
            3 => new WindowsRecognitionException(WindowsRecognitionFailure.System, System),
            4 => new WindowsRecognitionException(WindowsRecognitionFailure.IncompleteRecovery, System, Sent, Expected),
            5 => new WindowsRecognitionException(WindowsRecognitionFailure.ConsumerLagged),
            6 => new WindowsRecognitionException(WindowsRecognitionFailure.WorkerStopped),
            7 => new WindowsRecognitionException(WindowsRecognitionFailure.ReplayTargetChanged),
            _ => new InvalidDataException("Unknown Windows input error."),
        };
    }
}

internal static partial class NativeWindows
{
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_abi_version")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint AbiVersion();
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_start")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint Start(NativeConfiguration configuration, MatcherHandle matcher, out WindowsError error);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_start_with_context")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial nint StartWithContext(NativeConfiguration configuration,
        delegate* unmanaged[Cdecl]<nint, NativePressContext, nint, void> callback, nint context, out WindowsError error);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_select_context")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint SelectContext(nint selection, MatcherHandle matcher, uint enabled, uint modifiers, ulong epoch);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_read_with_context")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint ReadWithContext(WindowsRecognizerHandle adapter, out nint frame, out NativeEpoch epoch, out WindowsError error);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_duplicate_notification_handle")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint DuplicateNotificationHandle(WindowsRecognizerHandle adapter, out WindowsError error);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_now_ns")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial ulong Now(WindowsRecognizerHandle adapter);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_use_matcher")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint UseMatcher(WindowsRecognizerHandle adapter, MatcherHandle matcher, out WindowsError error);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_set_policy")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint Policy(WindowsRecognizerHandle adapter, uint enabled, uint modifiers, out WindowsError error);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_set_recording")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint Recording(WindowsRecognizerHandle adapter, uint enabled, out WindowsError error);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_update_tuning")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint Tuning(WindowsRecognizerHandle adapter, NativeConfiguration config, out WindowsError error);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_cancel")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint Cancel(WindowsRecognizerHandle adapter, out WindowsError error);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_stop")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint Stop(WindowsRecognizerHandle adapter, out WindowsError error);
    [LibraryImport(Native.Library, EntryPoint = "pointer_gesture_recognition_windows_destroy")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void Destroy(nint adapter);
}
