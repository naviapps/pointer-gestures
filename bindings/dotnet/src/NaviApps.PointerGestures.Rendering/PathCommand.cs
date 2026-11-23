using NaviApps.PointerGestures.Recognition;
using System.Runtime.InteropServices;

namespace NaviApps.PointerGestures.Rendering;

/// <summary>Renderer-independent commands for native Apple or Windows drawing APIs.</summary>
public enum CommandKind : uint
{
    /// <summary>Begin a subpath at the destination point.</summary>
    Move = 1,
    /// <summary>Draw a straight segment to the destination point.</summary>
    Line = 2,
    /// <summary>Draw a cubic Bezier segment using both control points.</summary>
    Cubic = 3,
    /// <summary>Close the current subpath; coordinate fields are unused.</summary>
    Close = 4
}

/// <summary>An immutable, copied drawing command. No native pointer escapes planning.</summary>
/// <param name="Kind">How to interpret this command's coordinates.</param>
/// <param name="Point">Destination for move, line or cubic commands.</param>
/// <param name="Control1">First control point, used only for a cubic segment.</param>
/// <param name="Control2">Second control point, used only for a cubic segment.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly record struct PathCommand(
    CommandKind Kind,
    GesturePoint Point,
    GesturePoint Control1,
    GesturePoint Control2);
