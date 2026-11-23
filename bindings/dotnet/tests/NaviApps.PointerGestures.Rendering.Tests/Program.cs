using System.Runtime.InteropServices;
using NaviApps.PointerGestures.Recognition;
using NaviApps.PointerGestures.Rendering;
static void Check(bool value, string description) { if (!value) throw new System.Diagnostics.UnreachableException(description); }
static void CheckOwnedCollection<T>(IReadOnlyList<T> values, T replacement, string description)
{
    var first = values[0];
    if (values is System.Collections.ICollection collection && collection.SyncRoot is IList<T> storage)
    {
        try { storage[0] = replacement; }
        catch (NotSupportedException) { }
    }
    Check(EqualityComparer<T>.Default.Equals(values[0], first), description);
}
Check(Marshal.SizeOf<PathCommand>() == 56, "Command layout");
var input = new Direction[] { Direction.Right, Direction.Down };
var geometry = GestureGeometryPlanner.PlanPattern(input, 96, 64);
input[0] = (Direction)0;
GC.Collect(); GC.WaitForPendingFinalizers();
Check(geometry.Path.Any(c => c.Kind == CommandKind.Cubic), "Native cubic geometry");
Check(geometry.Arrowhead?[^1].Kind == CommandKind.Close, "Closed owned arrowhead");
CheckOwnedCollection(geometry.Path, default, "Native preview path keeps its values through ICollection.SyncRoot");
CheckOwnedCollection(geometry.Arrowhead!, default, "Native arrowhead keeps its values through ICollection.SyncRoot");
Check(GestureGeometryPlanner.TryPlanPattern([Direction.Right, Direction.Down], 96, 64, out var identicalGeometry, out var previewError) &&
    previewError is null && geometry == identicalGeometry && new HashSet<GesturePatternGeometry> { geometry }.Contains(identicalGeometry),
    "Independent geometry is an equal value");
Check(!GestureGeometryPlanner.TryPlanPattern([], 96, 64, out var emptyGeometry, out var emptyError) &&
    emptyGeometry is null && emptyError == GestureGeometryError.Empty, "Empty preview has a typed rejection");
Check(!GestureGeometryPlanner.TryPlanPattern(input, 96, 64, out var invalidGeometry, out var invalidError) &&
    invalidGeometry is null && invalidError == GestureGeometryError.InvalidDirection, "Invalid managed direction has a typed rejection");
var withoutArrow = GestureGeometryPlanner.PlanPattern([Direction.Right, Direction.Down], 96, 64, showsArrowhead: false);
Check(geometry.Path.SequenceEqual(withoutArrow.Path) && geometry != withoutArrow, "Geometry equality distinguishes absent arrowheads");
var sourcePath = geometry.Path.ToList();
var sourceArrow = geometry.Arrowhead!.ToList();
var owned = new GesturePatternGeometry(Path: sourcePath, Arrowhead: sourceArrow);
var replaced = withoutArrow with { Path = sourcePath, Arrowhead = sourceArrow };
sourcePath.Reverse();
sourceArrow.Clear();
var (ownedPath, ownedArrow) = owned;
Check(ownedPath.SequenceEqual(geometry.Path) && ownedArrow!.SequenceEqual(geometry.Arrowhead!), "Constructor copies both command streams");
Check(replaced == geometry, "Record updates copy both command streams");
Check(new GesturePatternGeometry(sourcePath, geometry.Arrowhead) != geometry, "Command order participates in equality");
CheckOwnedCollection(owned.Path, default, "Constructed preview path keeps its values through ICollection.SyncRoot");
CheckOwnedCollection(replaced.Arrowhead!, default, "Updated arrowhead keeps its values through ICollection.SyncRoot");
Check(withoutArrow != withoutArrow with { Arrowhead = [] }, "Absent and empty arrowheads are distinct");
var sharp = GestureGeometryPlanner.PlanPattern([Direction.Right, Direction.Down], 96, 64, cornerRadiusScale: 0);
Check(!sharp.Path.Any(c => c.Kind == CommandKind.Cubic), "Sharp preview corners cross the ABI");
var subtle = GestureGeometryPlanner.PlanPattern([Direction.Right, Direction.Down], 96, 64, cornerRadiusScale: 0.5);
Check(subtle.Path.Any(c => c.Kind == CommandKind.Cubic) && !subtle.Path.SequenceEqual(geometry.Path),
    "Distinct positive corner scales cross the ABI");
var tail = new GesturePoint(50, 20);
var origin = new GesturePoint(0, -double.MaxValue);
GesturePoint[] endpoints = [origin, new(40, origin.Y)];
var trace = GestureGeometryPlanner.PlanLiveTrace([Direction.Right], endpoints, tail: tail);
Check(trace is [{ Kind: CommandKind.Move } move, { Kind: CommandKind.Line } line] &&
    move.Point == origin && line.Point == new GesturePoint(tail.X, origin.Y), "Tail and large native coordinates survive the ABI");
CheckOwnedCollection(trace, default, "Native live path keeps its values through ICollection.SyncRoot");
Check(GestureGeometryPlanner.TryPlanLiveTrace([Direction.Right], endpoints, out var plannedTrace, out var traceError, tail: tail) &&
    traceError is null && plannedTrace.SequenceEqual(trace), "Typed live planning copies the same native commands");
Check(!GestureGeometryPlanner.TryPlanLiveTrace([Direction.Right], [new(0, 0)], out var missingPath, out var missingError) &&
    missingPath is null && missingError == GestureGeometryError.PointCount, "Mismatched endpoints have a typed rejection");
Check(!GestureGeometryPlanner.TryPlanLiveTrace([Direction.Right], [new(-double.MaxValue, 0), new(double.MaxValue, 0)],
    out var overflowingPath, out var overflowingError) && overflowingPath is null && overflowingError == GestureGeometryError.NonFinite,
    "An unrepresentable projected segment has a typed rejection");
try { GestureGeometryPlanner.PlanPattern(input, 96, 64); throw new System.Diagnostics.UnreachableException("Invalid directions accepted"); }
catch (ArgumentException) { }
try { GestureGeometryPlanner.PlanLiveTrace([Direction.Right], [new(0, 0)]); throw new System.Diagnostics.UnreachableException("Point count accepted"); }
catch (ArgumentException) { }

#if SOURCE_CONSUMER
foreach (var (status, expected) in new (uint, GestureGeometryError)[]
{
    (5, GestureGeometryError.InvalidDirection),
    (0, GestureGeometryError.InvalidNativeResult),
    (4, GestureGeometryError.InvalidNativeResult),
    (uint.MaxValue, GestureGeometryError.InvalidNativeResult)
})
{
    Check(!GeometryReader.TryRead(new NativeResult { Code = status }, out var planningResult, out var planningError) &&
        planningResult is null && planningError == expected, $"Native status {status} without geometry has a typed rejection");
}
try { _ = GeometryReader.Read(default); throw new System.Diagnostics.UnreachableException("Missing native result owner accepted"); }
catch (InvalidOperationException) { }
try { _ = GeometryReader.Read(new NativeResult { Code = uint.MaxValue }); throw new System.Diagnostics.UnreachableException("Unknown native result status accepted"); }
catch (ArgumentException) { }
#endif

Check(Marshal.SizeOf<Bounds>() == 32, "Bounds layout");
var bounds = Bounds.FromPoints([new(0, 0), new(10, 10)]) ?? throw new System.Diagnostics.UnreachableException("Missing bounds");
var viewport = new Bounds(11, 0, 20, 10);
Check(!bounds.Intersects(viewport), "Outside bounds");
Check(bounds.Intersects(viewport, 2), "Touching stroke");
Check(bounds.Intersects(viewport, tail: new GesturePoint(12, 0)), "Tail visibility");
Check(Bounds.FromPoints([]) is null, "Empty bounds");

var original = new OverlayTrace([Direction.Right], [new(0, 0), new(1, 0)]);
var equalTrace = new OverlayTrace([Direction.Right], [new(-0.0, 0), new(1, -0.0)]);
var later = new OverlayTrace([Direction.Right], [new(0, 0), new(2, 0)]);
Check(original.Equals(equalTrace) && new HashSet<OverlayTrace> { original }.Contains(equalTrace), "Independent traces compare equal across signed zero");
Check(!original.Equals(later) && !original.Equals(new OverlayTrace([Direction.Left], original.Points.ToArray())), "Trace equality includes endpoints and directions");
Check(!original.Equals(original.WithTail(new GesturePoint(0, 0))), "An absent tail differs from an explicit zero point");
using (var firstState = new OverlayState(0))
using (var secondState = new OverlayState(0))
    Check(firstState.Update(original, 0) == secondState.Update(equalTrace, 0), "Equal trace values propagate through native presentation snapshots");
Check(!original.Intersects(new Bounds(double.NaN, 0, 20, 10), 4), "Invalid viewport reaches native validation");
Check(!original.Intersects(new Bounds(2, 0, 20, 10), -4), "Negative stroke width reaches native normalization");
var tailed = original.WithTail(new GesturePoint(20, 1));
Check(ReferenceEquals(tailed.Points, original.Points) && ReferenceEquals(tailed.Directions, original.Directions), "Tail replacement shares immutable managed coordinates");
Check(original.Tail is null && tailed.Tail == new GesturePoint(20, 1), "Tail replacement preserves prior snapshots");
CheckOwnedCollection(tailed.Directions, Direction.Left, "Shared trace directions stay immutable");
CheckOwnedCollection(tailed.Points, new GesturePoint(999, 999), "Shared trace points stay immutable");
Check(original.Directions[0] == Direction.Right && original.Points[0] == new GesturePoint(0, 0) &&
    original.Intersects(new Bounds(0, -1, 1, 1)), "Shared managed coordinates still agree with cached native bounds");
try { original.WithTail(new GesturePoint(double.NaN, 0)); throw new System.Diagnostics.UnreachableException("Invalid tail accepted"); }
catch (ArgumentException) { }

var converter = new OverlayTraceConverter();
var captured = new GestureTrace(true, [Direction.Right], [new(0, 0), new(40, 0)], [new(double.NaN, 0)], new(45, 10));
var first = converter.Convert(captured)!;
Check(first.Points.SequenceEqual(captured.Endpoints) && first.Tail == captured.Tail, "Conversion uses direction endpoints and live tail without reading raw history");
var moved = converter.Convert(captured with { Tail = new(70, 10) })!;
Check(ReferenceEquals(first.Points, moved.Points) && ReferenceEquals(first.Directions, moved.Directions), "Converted tail-only updates reuse immutable coordinates");
CheckOwnedCollection(moved.Directions, Direction.Left, "Converted tail updates keep shared directions immutable");
CheckOwnedCollection(moved.Points, new GesturePoint(999, 999), "Converted tail updates keep shared points immutable");
Check(first.Tail == captured.Tail && moved.Tail == new GesturePoint(70, 10), "Previous converted trace retains its own tail");
Check(converter.Convert(captured with { Tail = new(double.NaN, 0) }) is null, "Invalid input clears the managed cache");
var recovered = converter.Convert(captured)!;
Check(!ReferenceEquals(first.Points, recovered.Points) && first.Intersects(new(44, 9, 46, 11)), "Cache release preserves earlier native ownership");
var longDirections = Enumerable.Repeat(Direction.Right, 80).ToArray();
var longEndpoints = Enumerable.Range(0, 81).Select(x => new GesturePoint(x, 0)).ToArray();
var longTrace = converter.Convert(captured with { Directions = longDirections, Endpoints = longEndpoints })!;
longDirections[0] = Direction.Left;
longEndpoints[80] = new GesturePoint(-1, 0);
Check(longTrace.Directions[0] == Direction.Right && longTrace.Points[^1] == new GesturePoint(80, 0), "Long managed buffers are copied before returning");

using (var defaults = new OverlayState(0, original))
using (var explicitTiming = new OverlayState(0, original, new OverlayTiming()))
    Check(defaults.Update(later, 1) == explicitTiming.Update(later, 1), "Omitted timing selects the explicit managed default");
using (var overlay = new OverlayState(0, original, new OverlayTiming(0.01, 24)))
{
    Check(!overlay.Update(later, 1).Changed, "Unchanged frame retains managed trace");
    var final = overlay.Update(null, 2);
    GC.Collect(); GC.WaitForPendingFinalizers();
    Check(final.Trace?.Points.Last() == new GesturePoint(2, 0), "Final pending trace copied before native frame destruction");
    var reconfigured = overlay.Configure(new OverlayTiming(0.02, 24), 3);
    var deadline = reconfigured.DeadlineNanoseconds ?? throw new InvalidDataException("Missing configured deadline.");
    Check(deadline > final.DeadlineNanoseconds, "Configure returns the deadline for the new timing");
    var hidden = overlay.Tick(deadline);
    Check(hidden.Changed && hidden.Trace is null && hidden.DeadlineNanoseconds is null, "Hidden frame clears the managed trace and deadline");
    overlay.Update(original, deadline);
    var cleared = overlay.Clear(deadline);
    Check(cleared.Changed && cleared.Trace is null && cleared.DeadlineNanoseconds is null, "Clear projects the native frame");
    overlay.Dispose();
    CheckOwnedCollection(final.Trace!.Points, new GesturePoint(999, 999), "Delivered trace points stay immutable after native disposal");
    Check(final.Trace!.Points.SequenceEqual(later.Points) && final.Trace.Intersects(new Bounds(1, -1, 3, 1)), "Snapshot and native trace outlive overlay");
    try { overlay.Tick(100); throw new System.Diagnostics.UnreachableException("Disposed overlay accepted"); }
    catch (ObjectDisposedException) { }
}
try { _ = new OverlayTrace([Direction.Right], [new(0, 0)]); throw new System.Diagnostics.UnreachableException("Mismatched trace point count accepted"); }
catch (ArgumentException) { }
Check(new OverlayTiming(double.NaN, -1).MaximumPointLagDistance == 24, "Shared configuration normalization");
var disposed = CreateDisposedOverlay();
GC.Collect(); GC.WaitForPendingFinalizers(); GC.Collect();
Check(!HasTarget(disposed.Trace), "A rooted disposed overlay must release its last trace");
GC.KeepAlive(disposed.State);
Console.WriteLine("Managed rendering passed geometry, trace conversion, overlay cadence/ownership, disposal reachability, visibility and invalid input checks.");

// Returning across a non-inlined call prevents the caller's JIT locals from
// extending the trace's lifetime independently of the disposed owner.
[System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
static (OverlayState State, WeakReference<OverlayTrace> Trace) CreateDisposedOverlay()
{
    var trace = new OverlayTrace([Direction.Right], [new(0, 0), new(10, 0)]);
    var state = new OverlayState(0, trace);
    state.Dispose(); state.Dispose();
    return (state, new WeakReference<OverlayTrace>(trace));
}

[System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
static bool HasTarget(WeakReference<OverlayTrace> weak) => weak.TryGetTarget(out _);
