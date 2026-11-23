using System.Runtime.InteropServices;

namespace NaviApps.PointerGestures.Recognition;

[StructLayout(LayoutKind.Sequential)]
internal struct NativeConfiguration
{
    public double StartDistance;
    public double ChangeDistance; public ulong TotalNs;
    public ulong IdleNs; public nuint MaximumPoints; public uint Button;
    public uint PassEmpty;
    public nuint MaximumDirections;
}
[StructLayout(LayoutKind.Sequential)]
internal struct NativeEvent
{
    public uint Kind;
    public uint Button; public double X;
    public double Y; public ulong NowNs; public uint Value;
}
[StructLayout(LayoutKind.Sequential)]
internal struct NativeBuffer { public nint Data; public nuint Count; }
[StructLayout(LayoutKind.Sequential)]
internal struct NativeTrace
{
    public uint Visible; public NativeBuffer Directions;
    public NativeBuffer Endpoints;
    public NativeBuffer Raw; public uint TailPresent; public GesturePoint Tail;
}
[StructLayout(LayoutKind.Sequential)]
internal struct NativeEffect
{
    public uint Kind;
    public uint Button; public ulong MatchValue; public NativeBuffer Points; public NativeTrace Trace; public uint Ready;
    public uint Capturing;
    public uint Recording;
    public uint Failure; public NativeBuffer Recorded;
}
[StructLayout(LayoutKind.Sequential)]
internal struct NativeFrame
{
    public uint Disposition;
    public uint Ready;
    public uint Stage;
    public uint Capturing;
    public uint Recording;
    public uint Visible;
    public uint Failure;
    public ulong InputGeneration;
    public uint DiscardVisible;
    public NativeBuffer Directions;
    public NativeBuffer Endpoints;
    public NativeBuffer Raw;
    public NativeBuffer Recorded;
    public uint TailPresent;
    public GesturePoint Tail;
    public nuint EffectCount;
    public uint DeadlinePresent;
    public ulong DeadlineNs;
}

[StructLayout(LayoutKind.Sequential)]
internal struct Lookup { public uint Present; public ulong Value; }

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSessionReport
{
    public uint Present, Outcome, Captured, Recording, RejectedDirection;
    public ulong Generation, ElapsedNs, IdleNs, PointCount;
    public double MaximumAxisDistance;
    public NativeBuffer Directions;
}
