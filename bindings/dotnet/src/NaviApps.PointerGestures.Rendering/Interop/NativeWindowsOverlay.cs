using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace NaviApps.PointerGestures.Rendering;

[StructLayout(LayoutKind.Sequential)]
internal struct NativeWindowsOverlayConfiguration
{
    internal uint Rgba;
    internal double LineWidth, RadiusScale;
    internal NativeOverlayConfiguration Timing;
}
[StructLayout(LayoutKind.Sequential)]
internal struct NativeWindowsOverlayView { internal nuint Window; internal uint Visible; internal Bounds Bounds; }
[StructLayout(LayoutKind.Sequential)]
internal struct NativeWindowsOverlayError
{
    internal uint Kind, System;
    internal readonly void ThrowIfFailed()
    {
        switch (Kind)
        {
            case 0: return;
            case 1: throw new ArgumentException("Invalid appearance or excessive/unrepresentable overlay geometry.");
            case 2: throw new InvalidOperationException("Activate the overlay before submitting or querying geometry.");
            case 3: throw new Win32Exception((int)System, "Windows overlay operation failed.");
            case 4: throw new InvalidOperationException("Windows overlay owner stopped before completing the request.");
            default: throw new InvalidDataException("Unknown native overlay error.");
        }
    }
}
internal sealed class WindowsOverlayHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    internal WindowsOverlayHandle(nint value) : base(true) => SetHandle(value);
    protected override bool ReleaseHandle() { Native.DestroyWindowsOverlay(handle); return true; }
}
internal static partial class Native
{
    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_windows_abi_version")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint WindowsOverlayAbi();
    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_windows_create")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial nint CreateWindowsOverlay(NativeWindowsOverlayConfiguration configuration, out NativeWindowsOverlayError error);
    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_windows_activate")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint ActivateWindowsOverlay(WindowsOverlayHandle overlay, out NativeWindowsOverlayError error);
    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_windows_configure")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint ConfigureWindowsOverlay(WindowsOverlayHandle overlay, NativeWindowsOverlayConfiguration configuration, out NativeWindowsOverlayError error);
    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_windows_update")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint UpdateWindowsOverlay(WindowsOverlayHandle overlay, TraceHandle trace, out NativeWindowsOverlayError error);
    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_windows_update")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint UpdateWindowsOverlayNull(WindowsOverlayHandle overlay, nint trace, out NativeWindowsOverlayError error);
    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_windows_clear")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint ClearWindowsOverlay(WindowsOverlayHandle overlay, out NativeWindowsOverlayError error);
    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_windows_view")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint ViewWindowsOverlay(WindowsOverlayHandle overlay, out NativeWindowsOverlayView view, out NativeWindowsOverlayError error);
    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_windows_deactivate")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial uint DeactivateWindowsOverlay(WindowsOverlayHandle overlay, out NativeWindowsOverlayError error);
    [LibraryImport(Library, EntryPoint = "pointer_gesture_rendering_windows_destroy")]
    [UnmanagedCallConv(CallConvs = [typeof(CallConvCdecl)])]
    internal static partial void DestroyWindowsOverlay(nint overlay);
}
