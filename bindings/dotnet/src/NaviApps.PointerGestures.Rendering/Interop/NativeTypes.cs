using NaviApps.PointerGestures.Recognition;
using System.Runtime.InteropServices;

namespace NaviApps.PointerGestures.Rendering;

[StructLayout(LayoutKind.Sequential)]
internal struct NativeResult { public uint Code; public nint GesturePatternGeometry; }
[StructLayout(LayoutKind.Sequential)]
internal struct NativeView { public nint Path; public nuint PathCount; public nint Arrowhead; public nuint ArrowheadCount; }

[StructLayout(LayoutKind.Sequential)]
internal struct NativeBoundsResult { public uint Valid; public Bounds Bounds; }

[StructLayout(LayoutKind.Sequential)]
internal struct NativeOverlayConfiguration { internal double IntervalSeconds; internal double MaximumPointLag; }
[StructLayout(LayoutKind.Sequential)]
internal unsafe struct NativeTraceView { internal Direction* Directions; internal nuint Count; internal GesturePoint* Points; internal nuint PointCount; internal GesturePoint Tail; internal uint TailPresent; }
[StructLayout(LayoutKind.Sequential)]
internal struct NativeTraceConversion { internal nint Trace; internal uint ReusedGeometry; }
[StructLayout(LayoutKind.Sequential)]
internal struct NativeOverlayView { internal nint Trace; internal uint Changed, DeadlinePresent; internal ulong DeadlineNs; }
