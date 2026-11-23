namespace NaviApps.PointerGestures.Rendering;

/// <summary>A geometry planning failure reported across the Rust boundary.</summary>
public enum GestureGeometryError
{
    /// <summary>No directions were supplied.</summary>
    Empty = 1,
    /// <summary>The points did not contain exactly one more value than the directions.</summary>
    PointCount = 2,
    /// <summary>Input or projected geometry could not be represented with finite coordinates.</summary>
    NonFinite = 3,
    /// <summary>A direction value is outside the defined cardinal directions.</summary>
    InvalidDirection = 5,
    /// <summary>The native planner returned an invalid buffer status or result.</summary>
    InvalidNativeResult = -1
}
