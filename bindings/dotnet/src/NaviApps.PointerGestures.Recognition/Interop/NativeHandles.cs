using Microsoft.Win32.SafeHandles;

namespace NaviApps.PointerGestures.Recognition;

internal sealed class FrameHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    internal FrameHandle(nint value) : base(true) => SetHandle(value);
    protected override bool ReleaseHandle() { Native.DestroyFrame(handle); return true; }
}
internal sealed class EngineHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    internal EngineHandle(nint value) : base(true) => SetHandle(value);
    protected override bool ReleaseHandle() { Native.DestroyEngine(handle); return true; }
}

internal sealed class MatcherHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    internal MatcherHandle(nint value) : base(true) => SetHandle(value);
    protected override bool ReleaseHandle() { Native.DestroyMatcher(handle); return true; }
}
