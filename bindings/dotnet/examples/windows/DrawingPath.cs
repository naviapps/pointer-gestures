using System.Drawing.Drawing2D;
using NaviApps.PointerGestures.Rendering;
using NaviApps.PointerGestures.Recognition;

namespace NaviApps.PointerGestures.WindowsExample;

// This host maps Rust commands to GDI+; it does not recalculate corners or layout.
internal static class DrawingPath
{
    internal static GraphicsPath Create(IReadOnlyList<PathCommand> commands)
    {
        var path = new GraphicsPath();
        PointF? current = null;
        PointF? origin = null;
        try
        {
            foreach (var command in commands)
            {
                switch (command.Kind)
                {
                    case CommandKind.Move:
                        path.StartFigure(); current = origin = Convert(command.Point); break;
                    case CommandKind.Line when current.HasValue:
                        var line = Convert(command.Point);
                        path.AddLine(current.Value, line); current = line; break;
                    case CommandKind.Cubic when current.HasValue:
                        var end = Convert(command.Point);
                        path.AddBezier(current.Value, Convert(command.Control1), Convert(command.Control2), end);
                        current = end; break;
                    case CommandKind.Close when current.HasValue:
                        path.CloseFigure(); current = origin; break;
                    default:
                        throw new InvalidDataException("Invalid drawing command sequence.");
                }
            }
            return path;
        }
        catch { path.Dispose(); throw; }
    }

    private static PointF Convert(GesturePoint point)
    {
        var result = new PointF((float)point.X, (float)point.Y);
        if (!float.IsFinite(result.X) || !float.IsFinite(result.Y))
            throw new InvalidDataException("Drawing coordinates exceed the native renderer's finite range.");
        return result;
    }
}
