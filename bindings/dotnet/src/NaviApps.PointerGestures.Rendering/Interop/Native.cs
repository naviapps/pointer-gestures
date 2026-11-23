using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using NaviApps.PointerGestures.Recognition;

[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory | DllImportSearchPath.System32)]

namespace NaviApps.PointerGestures.Rendering;

internal static partial class Native
{
    private const string Library = "pointer_gesture_rendering_ffi";
    internal static void EnsureAbi()
    {
        if (AbiVersion() != 8) throw new NotSupportedException("Unsupported native rendering ABI.");
    }

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_abi_version")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    private static partial uint AbiVersion();

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_pattern")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial NativeResult Pattern(Direction* directions, nuint count, double width, double height, double padding, double lineWidth, uint arrowhead, double cornerRadiusScale);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_live_trace")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial NativeResult LiveTrace(Direction* directions, nuint count, GesturePoint* points, nuint pointCount, double radiusScale, GesturePoint tail, uint tailPresent);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_geometry_view")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial NativeView View(GeometryHandle geometry);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_geometry_destroy")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void Destroy(nint geometry);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_bounds")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial NativeBoundsResult Bounds(GesturePoint* points, nuint count);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_intersects")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint Intersects(Bounds bounds, GesturePoint tail, uint tailPresent, Bounds viewport, double lineWidth);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_overlay_configuration")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial NativeOverlayConfiguration OverlayTiming(double interval, double lag);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_trace_create")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial nint CreateTrace(Direction* directions, nuint count, GesturePoint* points, nuint pointCount, GesturePoint tail, uint tailPresent);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_trace_from_snapshot")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static unsafe partial NativeTraceConversion TraceFromSnapshot(nint previous, uint visible, Direction* directions, nuint count, GesturePoint* points, nuint pointCount, GesturePoint tail, uint tailPresent);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_trace_with_tail")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint TraceWithTail(TraceHandle trace, GesturePoint tail, uint present);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_trace_clone")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint CloneTrace(nint trace);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_trace_view")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial NativeTraceView TraceView(TraceHandle trace);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_trace_destroy")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void DestroyTrace(nint trace);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_trace_intersects")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint TraceIntersects(TraceHandle trace, Bounds viewport, double lineWidth);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_overlay_create")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint CreateOverlay(NativeOverlayConfiguration configuration, nint trace, ulong nowNs);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_overlay_update")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint UpdateOverlay(OverlayHandle overlay, nint trace, ulong nowNs);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_overlay_clear")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint ClearOverlay(OverlayHandle overlay, ulong nowNs);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_overlay_tick")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint TickOverlay(OverlayHandle overlay, ulong nowNs);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_overlay_configure")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint ConfigureOverlay(OverlayHandle overlay, NativeOverlayConfiguration configuration, ulong nowNs);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_overlay_destroy")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void DestroyOverlay(nint overlay);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_overlay_frame_view")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial NativeOverlayView OverlayFrameView(nint frame);

    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_overlay_frame_destroy")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void DestroyOverlayFrame(nint frame);

}
