using System.Runtime.InteropServices;

namespace NaviApps.PointerGestures.Recognition;

/// <summary>Cardinal directions in coordinates increasing rightward and downward.</summary>
public enum Direction : uint
{
    /// <summary>Negative vertical movement.</summary>
    Up = 1,
    /// <summary>Positive vertical movement.</summary>
    Down = 2,
    /// <summary>Negative horizontal movement.</summary>
    Left = 3,
    /// <summary>Positive horizontal movement.</summary>
    Right = 4
}

/// <summary>Display coordinates increasing rightward and downward.</summary>
/// <param name="X">Horizontal position in the host's coordinate units.</param>
/// <param name="Y">Vertical position in the same coordinate units.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly record struct GesturePoint(
    double X,
    double Y)
{
    /// <summary>Whether both coordinates are finite.</summary>
    public bool IsFinite => double.IsFinite(X) && double.IsFinite(Y);
}
