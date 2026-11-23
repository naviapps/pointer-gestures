using NaviApps.PointerGestures.Recognition;
using System.Runtime.InteropServices;

namespace NaviApps.PointerGestures.Rendering;

/// <summary>Conservative display-space bounds. Touching edges and stroke radius count as visible.</summary>
/// <param name="MinX">Left edge.</param>
/// <param name="MinY">Top edge in downward-increasing coordinates.</param>
/// <param name="MaxX">Right edge, at least MinX for valid bounds.</param>
/// <param name="MaxY">Bottom edge, at least MinY for valid bounds.</param>
[StructLayout(LayoutKind.Sequential)]
public readonly record struct Bounds(
    double MinX,
    double MinY,
    double MaxX,
    double MaxY)
{
    /// <summary>Bound finite points; empty or non-finite input returns null.</summary>
    public static unsafe Bounds? FromPoints(ReadOnlySpan<GesturePoint> points)
    {
        Native.EnsureAbi();
        fixed (GesturePoint* data = points)
        {
            var result = Native.Bounds(data, (nuint)points.Length);
            return result.Valid == 0 ? null : result.Bounds;
        }
    }

    /// <summary>Test conservative stroke/tail visibility; invalid bounds return false and invalid widths become zero.</summary>
    public bool Intersects(Bounds viewport, double lineWidth = 0, GesturePoint? tail = null)
    {
        Native.EnsureAbi();
        return Native.Intersects(this, tail ?? default, tail.HasValue ? 1u : 0u, viewport, lineWidth) != 0;
    }
}
