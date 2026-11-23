using System.Diagnostics.CodeAnalysis;

namespace NaviApps.PointerGestures.Rendering;

internal static class GeometryReader
{
    internal static GesturePatternGeometry Read(NativeResult result)
    {
        if (result.Code != 0) throw new ArgumentException($"Native geometry rejected input (code {result.Code}).");
        return ReadOwner(result);
    }

    internal static bool TryRead(NativeResult result,
        [NotNullWhen(true)] out GesturePatternGeometry? geometry, out GestureGeometryError? error)
    {
        if (result.Code == 0 && result.GesturePatternGeometry != 0)
        {
            geometry = ReadOwner(result);
            error = null;
            return true;
        }

        geometry = null;
        error = result.Code switch
        {
            1 or 2 or 3 or 5 => (GestureGeometryError)result.Code,
            _ => GestureGeometryError.InvalidNativeResult
        };
        return false;
    }

    private static unsafe GesturePatternGeometry ReadOwner(NativeResult result)
    {
        if (result.GesturePatternGeometry == 0) throw new InvalidOperationException("Native geometry result is missing its owner.");
        using var owner = new GeometryHandle(result.GesturePatternGeometry);
        var view = Native.View(owner);
        var path = new ReadOnlySpan<PathCommand>((void*)view.Path, checked((int)view.PathCount)).ToArray();
        var arrow = view.ArrowheadCount == 0 ? null : new ReadOnlySpan<PathCommand>((void*)view.Arrowhead, checked((int)view.ArrowheadCount)).ToArray();
        return GesturePatternGeometry.FromOwnedCommands(path, arrow);
    }
}
