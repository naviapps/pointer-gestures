using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory | DllImportSearchPath.System32)]

namespace NaviApps.PointerGestures.Recognition;

internal static partial class Native
{
    internal const string Library = "pointer_gesture_recognition_ffi";
    internal static void EnsureAbi()
    {
        if (AbiVersion() != 8) throw new NotSupportedException("Unsupported native recognition ABI.");
    }

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_abi_version")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint AbiVersion();

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_validate_tuning")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint ValidateTuning(double start, double change, double total, double idle, uint durationsPresent, long maximumPoints);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_validate_direction_limit")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint ValidateDirectionLimit(long maximumDirections);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_frame_session_report")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial NativeSessionReport ViewSessionReport(FrameHandle frame);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_matcher_create")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint CreateMatcher();

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_matcher_clone")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint CloneMatcher(MatcherHandle matcher);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_matcher_destroy")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void DestroyMatcher(nint matcher);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_pattern_validate")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial NativePatternValidation ValidatePattern(Direction* pattern, nuint count);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_matcher_register")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial NativePatternValidation Register(MatcherHandle matcher, Direction* directions, nuint count, ulong identifier);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_matcher_match")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial Lookup Match(MatcherHandle matcher, Direction* directions, nuint count);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_matcher_is_empty")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint MatcherIsEmpty(MatcherHandle matcher);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_matcher_remove")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial Lookup Remove(MatcherHandle matcher, Direction* directions, nuint count);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_matcher_clear")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void ClearMatcher(MatcherHandle matcher);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_engine_create")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint CreateEngine(NativeConfiguration configuration);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_engine_use_matcher")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void UseMatcher(EngineHandle engine, MatcherHandle matcher);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_engine_update_tuning")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint UpdateTuning(EngineHandle engine, NativeConfiguration configuration, ulong nowNs, uint resetUnchanged);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_engine_step")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint Step(EngineHandle engine, NativeEvent input);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_engine_destroy")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void DestroyEngine(nint engine);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_frame_view")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial NativeFrame ViewFrame(FrameHandle frame);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_frame_effect")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial NativeEffect ViewEffect(FrameHandle frame, nuint index);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_frame_destroy")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void DestroyFrame(nint frame);

    private static unsafe ReadOnlySpan<T> ViewElements<T>(NativeBuffer buffer) where T : unmanaged =>
        new((void*)buffer.Data, checked((int)buffer.Count));

    internal static T ReadFirst<T>(NativeBuffer buffer) where T : unmanaged => ViewElements<T>(buffer)[0];

    internal static SnapshotValues<T> CopySnapshot<T>(NativeBuffer buffer) where T : unmanaged =>
        SnapshotValues<T>.TakeOwned(ViewElements<T>(buffer).ToArray());

    [LibraryImport(Library, EntryPoint = "pointer_gesture_recognition_duration_nanoseconds")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial ulong DurationNanoseconds(double seconds);
}
