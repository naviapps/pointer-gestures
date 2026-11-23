using System.Diagnostics.CodeAnalysis;
using NaviApps.PointerGestures.Recognition;

namespace NaviApps.PointerGestures.Rendering;

/// <summary>Uses the same native Rust planner as the Swift adapter; creates no UI or windows.</summary>
public static class GestureGeometryPlanner
{
    /// <summary>Fit a direction preview and optional separate arrowhead into a canvas.</summary>
    /// <param name="directions">A nonempty sequence of defined cardinal directions.</param>
    /// <param name="width">Canvas width; negative or non-finite values become zero.</param>
    /// <param name="height">Canvas height, in the same units as width.</param>
    /// <param name="padding">Nonnegative edge padding, at least half the normalized line width.</param>
    /// <param name="lineWidth">Nonnegative stroke width used when fitting the path and arrowhead.</param>
    /// <param name="showsArrowhead">Whether to include the fitted arrowhead commands.</param>
    /// <param name="cornerRadiusScale">Multiplier for one tenth of the smaller canvas dimension; zero keeps sharp turns. Negative or non-finite values become zero.</param>
    /// <exception cref="ArgumentException">Directions or computed geometry are invalid.</exception>
    public static GesturePatternGeometry PlanPattern(ReadOnlySpan<Direction> directions, double width, double height, double padding = 8, double lineWidth = 3, bool showsArrowhead = true, double cornerRadiusScale = 1) =>
        GeometryReader.Read(NativePattern(directions, width, height, padding, lineWidth, showsArrowhead, cornerRadiusScale));

    /// <summary>Fit a preview, or return the reason the Rust planner rejected the directions or fitted geometry.</summary>
    /// <remarks>Returns true with an owned geometry and no error, or false with an error and no geometry.
    /// Inputs and drawing units are the same as for <see cref="PlanPattern"/>.</remarks>
    public static bool TryPlanPattern(ReadOnlySpan<Direction> directions, double width, double height,
        [NotNullWhen(true)] out GesturePatternGeometry? geometry, out GestureGeometryError? error,
        double padding = 8, double lineWidth = 3, bool showsArrowhead = true, double cornerRadiusScale = 1) =>
        GeometryReader.TryRead(NativePattern(directions, width, height, padding, lineWidth, showsArrowhead, cornerRadiusScale), out geometry, out error);

    /// <summary>Project captured runs and the tail onto cardinal axes and round perpendicular corners.</summary>
    /// <param name="directions">A nonempty cardinal sequence.</param>
    /// <param name="points">Finite origin and one endpoint per direction.</param>
    /// <param name="radiusScale">Multiplier for the 24-unit corner limit; zero keeps sharp corners.</param>
    /// <param name="tail">Optional newest finite position, replacing the final endpoint for planning.</param>
    /// <exception cref="ArgumentException">Directions, point counts, coordinates or projected segment lengths are invalid.</exception>
    public static IReadOnlyList<PathCommand> PlanLiveTrace(ReadOnlySpan<Direction> directions, ReadOnlySpan<GesturePoint> points, double radiusScale = 1, GesturePoint? tail = null) =>
        GeometryReader.Read(NativeLiveTrace(directions, points, radiusScale, tail)).Path;

    /// <summary>Plan a live path, or return the reason the Rust planner rejected the input or its projected geometry.</summary>
    /// <remarks>Returns true with owned commands and no error, or false with an error and no commands.
    /// Inputs and drawing units are the same as for <see cref="PlanLiveTrace"/>.</remarks>
    public static bool TryPlanLiveTrace(ReadOnlySpan<Direction> directions, ReadOnlySpan<GesturePoint> points,
        [NotNullWhen(true)] out IReadOnlyList<PathCommand>? path, out GestureGeometryError? error,
        double radiusScale = 1, GesturePoint? tail = null)
    {
        if (!GeometryReader.TryRead(NativeLiveTrace(directions, points, radiusScale, tail), out var geometry, out error))
        {
            path = null;
            return false;
        }

        path = geometry.Path;
        return true;
    }

    private static unsafe NativeResult NativePattern(ReadOnlySpan<Direction> directions, double width, double height,
        double padding, double lineWidth, bool showsArrowhead, double cornerRadiusScale)
    {
        Native.EnsureAbi();
        fixed (Direction* data = directions)
            return Native.Pattern(data, (nuint)directions.Length, width, height, padding, lineWidth, showsArrowhead ? 1u : 0u, cornerRadiusScale);
    }

    private static unsafe NativeResult NativeLiveTrace(ReadOnlySpan<Direction> directions, ReadOnlySpan<GesturePoint> points, double radiusScale, GesturePoint? tail)
    {
        Native.EnsureAbi();
        fixed (Direction* data = directions)
        fixed (GesturePoint* pointData = points)
            return Native.LiveTrace(data, (nuint)directions.Length, pointData, (nuint)points.Length, radiusScale, tail ?? default, tail.HasValue ? 1u : 0u);
    }
}
