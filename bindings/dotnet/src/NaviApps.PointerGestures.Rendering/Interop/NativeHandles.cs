using Microsoft.Win32.SafeHandles;

namespace NaviApps.PointerGestures.Rendering;

internal sealed class GeometryHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    internal GeometryHandle(nint value) : base(true) => SetHandle(value);
    protected override bool ReleaseHandle() { Native.Destroy(handle); return true; }
}

internal sealed class TraceHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    internal TraceHandle(nint value) : base(true) { SetHandle(value); }
    protected override bool ReleaseHandle() { Native.DestroyTrace(handle); return true; }
}
internal sealed class OverlayHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    internal OverlayHandle(nint value) : base(true) { SetHandle(value); }
    protected override bool ReleaseHandle() { Native.DestroyOverlay(handle); return true; }
}
