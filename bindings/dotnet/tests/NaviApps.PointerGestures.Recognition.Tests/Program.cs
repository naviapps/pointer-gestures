using System.Runtime.InteropServices;
using NaviApps.PointerGestures.Recognition;
using NaviApps.PointerGestures.Recognition.Windows;

static void Check(bool value, string description) { if (!value) throw new System.Diagnostics.UnreachableException(description); }
static void CheckSnapshot<T>(T actual, T expected, string description) where T : notnull
{
    Check(EqualityComparer<T>.Default.Equals(actual, expected) &&
        actual.GetHashCode() == expected.GetHashCode() &&
        new HashSet<T> { actual }.Contains(expected), description);
}
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
static void CheckInvalidTuning(Action validate, string field)
{
    try
    {
        validate();
        throw new System.Diagnostics.UnreachableException($"Expected a validation error for {field}");
    }
    catch (ArgumentOutOfRangeException error)
    {
        Check(error.ParamName == field, $"Expected {field}, got {error.ParamName}");
    }
}

// Exercise native-to-public error translation without installing OS input hooks.
var windowsErrorType = typeof(GestureRecognitionEngine).Assembly.GetType("NaviApps.PointerGestures.Recognition.Interop.WindowsError", true)!;
foreach (var (kind, failure) in new (uint, WindowsRecognitionFailure)[] {
    (2, WindowsRecognitionFailure.AlreadyRunning), (3, WindowsRecognitionFailure.System),
    (4, WindowsRecognitionFailure.IncompleteRecovery), (5, WindowsRecognitionFailure.ConsumerLagged),
    (6, WindowsRecognitionFailure.WorkerStopped), (7, WindowsRecognitionFailure.ReplayTargetChanged) })
{
    var nativeError = Activator.CreateInstance(windowsErrorType)!;
    foreach (var (name, value) in new (string, uint)[] { ("Kind", kind), ("System", 5), ("Sent", 2), ("Expected", 3) })
        windowsErrorType.GetField(name, System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!.SetValue(nativeError, value);
    try { windowsErrorType.GetMethod("ThrowIfFailed", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!.Invoke(nativeError, null); throw new System.Diagnostics.UnreachableException("Native failure was ignored"); }
    catch (System.Reflection.TargetInvocationException e) when (e.InnerException is WindowsRecognitionException error)
    {
        Check(error.Failure == failure, "Structured native reason");
        Check(error.NativeErrorCode == (kind is 3 or 4 ? 5u : 0u), "Structured OS error");
        Check(error.AcceptedEventCount == (kind == 4 ? 2u : 0u) && error.RequestedEventCount == (kind == 4 ? 3u : 0u), "Structured recovery counts");
    }
}
Check(Marshal.SizeOf<GesturePoint>() == 16, "Point layout");
using var matcher = new GesturePatternMatcher();
Check(matcher.IsEmpty, "Empty matcher");
Check(matcher.Register([], 42) is not null, "Reject empty pattern");
Check(matcher.Register([Direction.Right, Direction.Down], 0) is null, "Register zero ID");
Check(GesturePatternMatcher.Validate([Direction.Right, Direction.Down, Direction.Down]) == new PatternValidationError(PatternValidationReason.RepeatedDirection, 2), "Structured run index");
Check(GesturePatternMatcher.Validate([(Direction)8]) == new PatternValidationError(PatternValidationReason.InvalidDirection, 0), "Managed invalid enum uses shared ABI validation");
using var snapshot = matcher.Clone();
Check(matcher.Register([Direction.Right, Direction.Down], ulong.MaxValue) is null, "Replace with max ID");
Check(snapshot.Match([Direction.Right, Direction.Down]) == 0, "Snapshot retained");
Check(matcher.Match([Direction.Right, Direction.Down]) == ulong.MaxValue, "Replacement visible");
Check(matcher.Remove([Direction.Right, Direction.Down]) == ulong.MaxValue, "Remove max ID");
Check(matcher.Remove([Direction.Right, Direction.Down]) is null, "Absent removal");
matcher.Clear();
Check(matcher.IsEmpty && snapshot.Match([Direction.Right, Direction.Down]) == 0, "Clear retains copied registrations");
matcher.Dispose();
try { matcher.Match([Direction.Right]); throw new System.Diagnostics.UnreachableException("Disposed matcher accepted"); }
catch (ObjectDisposedException) { }
Check(Marshal.SizeOf(typeof(GestureRecognitionEngine).Assembly.GetType("NaviApps.PointerGestures.Recognition.NativeFrame", true)!) == 160, "Frame layout");
Check(Marshal.SizeOf(typeof(GestureRecognitionEngine).Assembly.GetType("NaviApps.PointerGestures.Recognition.NativeEffect", true)!) == 144, "Effect layout");
using var activeMatcher = new GesturePatternMatcher();
activeMatcher.Register([Direction.Right], 0);
var configuration = new GestureRecognitionConfiguration(10, 5, 1000, 100, 8);
using (var emptyMatcher = new GestureRecognitionEngine(configuration))
{
    emptyMatcher.Start(0);
    var origin = new GesturePoint(0, 0);
    var end = new GesturePoint(40, 0);
    foreach (var (input, now) in new (PointerEvent, ulong)[] {
        (new(PointerEventKind.Down, 1, origin), 1),
        (new(PointerEventKind.Moved, 1, end), 2),
        (new(PointerEventKind.Up, 1, end), 3) })
    {
        var output = emptyMatcher.Step(input, now);
        Check(output.Disposition == Disposition.PassThrough && output.Stage == InputStage.Idle &&
            output.Effects.Count == 0 && output.LastSessionReport is null,
            "Default empty matcher leaves unconfigured input in the original stream");
    }
    emptyMatcher.SetRecording(true, 4);
    var down = emptyMatcher.Step(new(PointerEventKind.Down, 1, origin), 5);
    var moved = emptyMatcher.Step(new(PointerEventKind.Moved, 1, end), 6);
    var recorded = emptyMatcher.Step(new(PointerEventKind.Up, 1, end), 7);
    Check(down.Disposition == Disposition.Consume && moved.Disposition == Disposition.Consume &&
        recorded.Disposition == Disposition.Consume &&
        recorded.LastSessionReport?.Outcome == SessionOutcome.Recorded &&
        recorded.Status.RecordedDirections.SequenceEqual([Direction.Right]) &&
        recorded.Effects.OfType<Effect.Drag>().Single().Points.SequenceEqual([origin, end]),
        "Recording captures and recovers the gesture with an empty matcher");
    emptyMatcher.Close(8);
}
using var engine = new GestureRecognitionEngine(configuration);
engine.UseMatcher(activeMatcher);
var initial = engine.Snapshot(0);
Check(initial.LastSessionReport is null, "No invented session report");
Check(!initial.Status.Ready && initial.Stage == InputStage.Idle && initial.Effects.Count == 0, "Initial snapshot");
Check(!engine.UpdateTuning(configuration, 0), "Unchanged tuning");
Check(engine.UpdateTuning(configuration, 0, true), "Explicit deadline restart");
try { engine.UpdateTuning(configuration with { Button = 0 }, 0); throw new System.Diagnostics.UnreachableException("Immutable policy accepted"); }
catch (ArgumentException) { }
Check(engine.Start(0).Status.Ready, "GestureRecognitionEngine start");
Check(engine.Step(new(PointerEventKind.Down, 1, new(0, 0)), 0).Disposition == Disposition.Consume, "Pending capture");
using (var expiring = new GestureRecognitionEngine(configuration with { TotalDurationNs = 1_000, IdleDurationNs = 0 }))
{
    expiring.Start(0);
    expiring.SetRecording(true, 0);
    var original = expiring.Step(new(PointerEventKind.Down, 1, new(0, 0)), 1);
    var deadline = original.DeadlineNs!.Value;
    var expiredMove = expiring.Step(new(PointerEventKind.Moved, 1, new(140, 0)), deadline);
    Check(expiredMove.Disposition == Disposition.PassThrough && expiredMove.Stage == InputStage.Idle &&
        expiredMove.Effects is [Effect.Click { Point.X: 0, Point.Y: 0 }] &&
        expiredMove.LastSessionReport?.Outcome == SessionOutcome.TotalDeadline,
        "Movement at the deadline recovers the original press before host delivery.");
    var replacement = expiring.Step(new(PointerEventKind.Down, 1, new(100, 0)), deadline);
    Check(replacement.Stage == InputStage.Pending && !replacement.Trace.Visible,
        "A replacement press started during recovery stays pending.");
    var nextMove = expiring.Step(new(PointerEventKind.Moved, 1, new(160, 0)), deadline);
    Check(nextMove.Trace.Endpoints.SequenceEqual([new GesturePoint(100, 0), new GesturePoint(160, 0)]),
        "Only the next movement can establish the replacement trace.");
    expiring.Close(deadline);
}

activeMatcher.Register([Direction.Right], ulong.MaxValue);
Check(activeMatcher.Remove([Direction.Right]) == ulong.MaxValue, "Remove future command during retained input");
activeMatcher.Clear();
engine.UseMatcher(activeMatcher);
var active = engine.Step(new(PointerEventKind.Moved, 1, new(20, 0)), 10);
Check(active.Trace.Visible && active.Trace.Endpoints[^1] == new GesturePoint(20, 0), "Live native trace");
CheckSnapshot(active.Trace, engine.Snapshot(10).Trace, "Separately copied native traces compare by value");
var completed = engine.Step(new(PointerEventKind.Up, 1, new(30, 0)), 20);
Check(completed.Effects[^1] is Effect.Match { Identifier: 0 }, "GestureRecognitionEngine matcher snapshot");
Check(completed.Effects[^2] is Effect.Release, "Release before match");
var report = completed.LastSessionReport ?? throw new System.Diagnostics.UnreachableException("Missing session report");
Check(report.Outcome == SessionOutcome.Matched && report.Generation != 0 && report.Captured && !report.Recording, "Match decision");
Check(report.ElapsedNs == 20 && report.PointCount == 3 && report.MaximumAxisDistance == 30, "Rust session measurements");
Check(report.Directions.SequenceEqual([Direction.Right]) && report.RejectedDirection is null, "Copied session directions");
CheckOwnedCollection(completed.Effects, new Effect.Match(999), "Native effect ordering survives access through ICollection.SyncRoot");
var callerEffects = completed.Effects.ToArray();
var outputFromCaller = new GestureRecognitionOutput(completed.Disposition, completed.Stage,
    completed.Status, completed.Trace, completed.InputGeneration, completed.DiscardPendingVisibleTrace,
    completed.DeadlineNs, callerEffects, report);
var outputWith = completed with { Effects = callerEffects };
callerEffects[0] = new Effect.Match(999);
CheckSnapshot(outputFromCaller, completed, "Output construction owns all ordered effects");
CheckSnapshot(outputWith, completed, "Output record update owns all ordered effects");
Check((completed with { Effects = completed.Effects.Reverse().ToArray() }) != completed,
    "Output equality includes effect order");
var completedSnapshot = engine.Snapshot(20);
CheckSnapshot(completed.Status, completedSnapshot.Status, "Separately copied native statuses compare by value");
CheckSnapshot(report, completedSnapshot.LastSessionReport!, "Separately copied native reports compare by value");

var directions = new List<Direction> { Direction.Right };
var endpoints = active.Trace.Endpoints.ToArray();
var raw = active.Trace.RawPoints.ToArray();
var traceFromCaller = new GestureTrace(Visible: true, Directions: directions.AsReadOnly(),
    Endpoints: endpoints, RawPoints: raw, Tail: active.Trace.Tail);
var statusFromCaller = new GestureRecognitionStatus(completed.Status.Ready, completed.Status.Capturing,
    completed.Status.Recording, completed.Status.Failure, directions);
var reportFromCaller = new SessionReport(report.Generation, report.Outcome, report.ElapsedNs, report.IdleNs,
    report.PointCount, report.MaximumAxisDistance, report.Captured, report.Recording, directions, report.RejectedDirection);
var replacementDirections = new[] { Direction.Right };
var traceWith = traceFromCaller with { Directions = replacementDirections };
var statusWith = statusFromCaller with { RecordedDirections = replacementDirections };
var reportWith = reportFromCaller with { Directions = replacementDirections };
directions[0] = Direction.Left;
replacementDirections[0] = Direction.Down;
endpoints[0] = new(999, 999);
raw[0] = new(999, 999);
CheckSnapshot(traceFromCaller, active.Trace, "Trace construction snapshots caller-owned collections");
CheckSnapshot(statusFromCaller, completed.Status, "Status construction snapshots caller-owned directions");
CheckSnapshot(reportFromCaller, report, "Report construction snapshots caller-owned directions");
CheckSnapshot(traceWith, active.Trace, "A with-assigned trace snapshots caller-owned directions");
CheckSnapshot(statusWith, completed.Status, "A with-assigned status snapshots caller-owned directions");
CheckSnapshot(reportWith, report, "A with-assigned report snapshots caller-owned directions");
Check(((System.Collections.ICollection)traceFromCaller.Endpoints).SyncRoot is not GesturePoint[],
    "Read-only snapshot collections do not expose their backing arrays");
var reordered = active.Trace with { Endpoints = active.Trace.Endpoints.Reverse().ToArray() };
Check(reordered != active.Trace && (report with { Generation = 0 }) != report,
    "Snapshot comparison includes sequence order and scalar values");
var (_, _, copiedEndpoints, _, _) = traceFromCaller;
Check(copiedEndpoints.SequenceEqual(active.Trace.Endpoints), "Trace deconstruction returns the owned values");

Check(engine.Close(21).LastSessionReport?.Generation == report.Generation, "Completed report persists through close");
GC.Collect(); GC.WaitForPendingFinalizers();
Check(report.Directions.SequenceEqual([Direction.Right]), "Session copy survives close/GC");
Check(active.Trace.Endpoints[^1] == new GesturePoint(20, 0), "Copied frame survives close/GC");
var publishedActive = active.Effects.OfType<Effect.Publish>().Single();
Check(publishedActive.Trace.Endpoints[^1] == new GesturePoint(20, 0), "Published copy survives close/GC");
var finalVisible = completed.Effects.OfType<Effect.Publish>().First(value => value.Trace.Visible);
Check(!completed.Trace.Visible && finalVisible.Trace.Endpoints[^1] == new GesturePoint(30, 0), "Intermediate and final traces remain distinct");
Check(finalVisible.Status.RecordedDirections.Count == 0 && completed.Status.RecordedDirections.SequenceEqual([Direction.Right]), "Intermediate and final recorded status remain distinct");
try { ((IList<GesturePoint>)publishedActive.Trace.Endpoints)[0] = new(999, 999); throw new System.Diagnostics.UnreachableException("Mutable published trace accepted"); }
catch (NotSupportedException) { }
Check(active.Trace.Endpoints[0] == new GesturePoint(0, 0), "Shared managed snapshot remains read-only");
try { engine.Tick(22); throw new System.Diagnostics.UnreachableException("Closed engine accepted"); } catch (ObjectDisposedException) { }
using var recording = new GestureRecognitionEngine(configuration);
recording.Start(0); recording.SetRecording(true, 0);
recording.Step(new(PointerEventKind.Down, 1, new(0, 0)), 10);
recording.Step(new(PointerEventKind.Moved, 1, new(20, 0)), 20);
var expired = recording.Tick(120);
Check(expired.Status.Failure == Failure.Expired && expired.Effects[0] is Effect.Release, "Expiration recovery");
recording.Step(new(PointerEventKind.Down, 1, new(40, 0)), 130);
var closed = recording.Close(140);
Check(!closed.Status.Ready && closed.Effects[0] is Effect.Click { Point.X: 40 }, "Close recovery");

foreach (var atDeadline in new[] { false, true })
{
    using var formerMatcher = new GesturePatternMatcher();
    using var nextMatcher = new GesturePatternMatcher();
    formerMatcher.Register([Direction.Right], 41);
    nextMatcher.Register([Direction.Right], 42);
    using var staged = new GestureRecognitionEngine(configuration with { TotalDurationNs = 1_000, IdleDurationNs = 0 });
    staged.UseMatcher(formerMatcher);
    staged.Start(0);
    var oldDown = staged.Step(new(PointerEventKind.Down, 1, new(0, 0)), 1);
    staged.SetPolicy(enabled: false, modifiersSatisfied: false, nowNs: 1);
    var now = atDeadline ? oldDown.DeadlineNs!.Value : 2;
    var expectedOutcome = atDeadline ? SessionOutcome.TotalDeadline : SessionOutcome.Superseded;

    var recovery = staged.PrepareReplacementPress(now);
    Check(recovery.Disposition == Disposition.Control && recovery.Stage == InputStage.Idle &&
        recovery.InputGeneration == 0 && recovery.DeadlineNs is null && recovery.DiscardPendingVisibleTrace &&
        recovery.Effects is [Effect.Click { Button: 1, Point.X: 0, Point.Y: 0 }] &&
        recovery.LastSessionReport?.Outcome == expectedOutcome,
        "Preparation returns old recovery and its reason before any new press is admitted");

    // The host delivers recovery first, then chooses the next matcher and policy.
    staged.UseMatcher(nextMatcher);
    staged.SetPolicy(enabled: true, modifiersSatisfied: true, nowNs: now);
    var newDown = staged.Step(new(PointerEventKind.Down, 1, new(100, 0)), now);
    Check(newDown.Disposition == Disposition.Consume && newDown.Stage == InputStage.Pending &&
        newDown.InputGeneration != oldDown.InputGeneration && newDown.Effects.Count == 0,
        "Submitting the prepared press captures the new policy without replaying twice");
    CheckSnapshot(newDown.LastSessionReport!, recovery.LastSessionReport!, "Old recovery report survives new admission");
    staged.Step(new(PointerEventKind.Moved, 1, new(140, 0)), now + 1);
    var released = staged.Step(new(PointerEventKind.Up, 1, new(160, 0)), now + 2);
    Check(released.Effects.OfType<Effect.Match>().Single().Identifier == 42,
        "The new gesture uses the context selected after recovery");
    staged.SetPolicy(enabled: true, modifiersSatisfied: false, nowNs: now + 3);
    var denied = staged.Step(new(PointerEventKind.Down, 1, new(0, 0)), now + 3);
    Check(denied.Disposition == Disposition.PassThrough && denied.Status.Failure == Failure.ModifiersNotSatisfied,
        "Modifier rejection maps to the managed disposition and failure");
    staged.Close(now + 4);
    Check(recovery.Effects is [Effect.Click { Point.X: 0 }] &&
        recovery.LastSessionReport?.Outcome == expectedOutcome,
        "Copied preparation output survives later input and engine closure");
}

using (var closing = new GestureRecognitionEngine(configuration))
{
    closing.Start(0); closing.SetRecording(true, 0);
    closing.Step(new(PointerEventKind.Down, 1, new(10, 0)), 1);
    var saved = closing.Step(new(PointerEventKind.Moved, 1, new(40, 0)), 2);
    var final = closing.Close(3);
    closing.Dispose(); closing.Dispose();
    Check(final.Effects is [Effect.Release { Point.X: 40 }, Effect.Publish { Trace.Visible: false }], "Active close returns recovery before hiding after native destruction");
    Check(final.LastSessionReport is { Outcome: SessionOutcome.Stopped, Captured: true } && final.LastSessionReport.Directions.SequenceEqual([Direction.Right]), "Close report survives native destruction");
    Check(saved.Trace.Endpoints[^1] == new GesturePoint(40, 0), "Active snapshot survives close and repeated disposal");
    try { closing.Close(4); throw new System.Diagnostics.UnreachableException("Closed engine accepted twice"); }
    catch (ObjectDisposedException) { }
}
var abandoned = AbandonEngine();
GC.Collect(); GC.WaitForPendingFinalizers(); GC.Collect();
Check(!HasTarget(abandoned.Engine), "Unclosed engine can be finalized");
Check(abandoned.Output.Trace.Endpoints[^1] == new GesturePoint(40, 0), "Owned output survives memory-only finalization");

Check(Timing.DurationNanoseconds(double.MaxValue) == ulong.MaxValue, "Saturating duration");

using (var replaying = new GestureRecognitionEngine(configuration))
{
    var origin = new GesturePoint(0, 0);
    var end = new GesturePoint(40, 0);
    var replacementPoint = new GesturePoint(999, 999);
    replaying.Start(0);
    replaying.SetRecording(true, 0);
    replaying.Step(new(PointerEventKind.Down, 1, origin), 1);
    replaying.Step(new(PointerEventKind.Moved, 1, end), 2);
    var drag = replaying.Step(new(PointerEventKind.Up, 1, end), 3).Effects.OfType<Effect.Drag>().Single();

    using var rejectedDirection = new GesturePatternMatcher();
    rejectedDirection.Register([Direction.Left], 42);
    replaying.UseMatcher(rejectedDirection);
    replaying.SetRecording(false, 4);
    replaying.Step(new(PointerEventKind.Down, 1, origin), 5);
    var dragStart = replaying.Step(new(PointerEventKind.Moved, 1, end), 6).Effects.OfType<Effect.DragStart>().Single();
    replaying.Close(7);

    Check(drag.Points.SequenceEqual([origin, end]) && dragStart.Points.SequenceEqual([origin, end]),
        "Both native replay variants retain their coordinates after closure");
    CheckOwnedCollection(drag.Points, replacementPoint, "Native drag points survive access through ICollection.SyncRoot");
    CheckOwnedCollection(dragStart.Points, replacementPoint, "Native drag-start points survive access through ICollection.SyncRoot");

    var callerPoints = new[] { origin, end };
    var suppliedDrag = new Effect.Drag(1, callerPoints);
    var suppliedDragStart = new Effect.DragStart(1, callerPoints);
    var dragWith = drag with { Points = callerPoints };
    var dragStartWith = dragStart with { Points = callerPoints };
    callerPoints[0] = replacementPoint;
    CheckSnapshot(suppliedDrag, drag, "Drag construction owns the ordered coordinates");
    CheckSnapshot(suppliedDragStart, dragStart, "Drag-start construction owns the ordered coordinates");
    CheckSnapshot(dragWith, drag, "Drag record update owns the ordered coordinates");
    CheckSnapshot(dragStartWith, dragStart, "Drag-start record update owns the ordered coordinates");
    var (_, points) = dragWith;
    Check(points.SequenceEqual([origin, end]), "Replay deconstruction returns the owned coordinates");
}

using var validation = new GestureRecognitionEngine(configuration);
var invalidTuning = configuration with { StartDistance = double.NaN, MaximumPoints = -1, MaximumDirections = 0 };
CheckInvalidTuning(invalidTuning.Validate, nameof(GestureRecognitionConfiguration.StartDistance));
CheckInvalidTuning((invalidTuning with { StartDistance = 0, ChangeDistance = double.PositiveInfinity }).Validate,
    nameof(GestureRecognitionConfiguration.ChangeDistance));
CheckInvalidTuning((invalidTuning with { StartDistance = 0 }).Validate, nameof(GestureRecognitionConfiguration.MaximumPoints));
CheckInvalidTuning((invalidTuning with { StartDistance = 0, MaximumPoints = 1 }).Validate, nameof(GestureRecognitionConfiguration.MaximumDirections));
(configuration with { TotalDurationNs = ulong.MaxValue, IdleDurationNs = 0 }).Validate();
CheckInvalidTuning(() => { using var rejected = new GestureRecognitionEngine(invalidTuning); },
    nameof(GestureRecognitionConfiguration.StartDistance));

validation.Start(0);
validation.SetRecording(true, 0);
var pendingInput = validation.Step(new(PointerEventKind.Down, 1, new(0, 0)), 0);
Check(pendingInput.Stage == InputStage.Pending && pendingInput.DeadlineNs is not null, "Pending input has an active deadline");
CheckInvalidTuning(() => validation.UpdateTuning(invalidTuning, 1, resetUnchanged: true),
    nameof(GestureRecognitionConfiguration.StartDistance));
var afterInvalidTuning = validation.Snapshot(1);
Check(afterInvalidTuning.InputGeneration == pendingInput.InputGeneration && afterInvalidTuning.DeadlineNs == pendingInput.DeadlineNs,
    "Rejected tuning preserves held input and its deadline");
Check(!validation.UpdateTuning(configuration, 2), "Rejected tuning leaves configuration unchanged");
validation.Close(2);
try { validation.UpdateTuning(invalidTuning, 3); throw new System.Diagnostics.UnreachableException("Disposed engine accepted tuning"); }
catch (ObjectDisposedException) { }

using (var boundedHistory = new GestureRecognitionEngine(configuration with { TotalDurationNs = 0, IdleDurationNs = 0 }))
{
    boundedHistory.Start(0);
    boundedHistory.SetRecording(true, 0);
    var origin = new GesturePoint(0, 0);
    var tail = new GesturePoint(20, 0);
    boundedHistory.Step(new(PointerEventKind.Down, 1, origin), 1);
    var formerTrace = boundedHistory.Step(new(PointerEventKind.Moved, 1, tail), 2).Trace;
    Check(boundedHistory.UpdateTuning(configuration with { TotalDurationNs = 0, IdleDurationNs = 0, MaximumPoints = 1 }, 3),
        "Native tuning acknowledges the new point budget");
    Check(boundedHistory.Snapshot(3).Trace.RawPoints.SequenceEqual([tail]) && formerTrace.RawPoints.SequenceEqual([origin, tail]),
        "Tuning publishes a new trace while the managed snapshot retains its owned points");
    boundedHistory.Close(4);
}

activeMatcher.Register([Direction.Right], ulong.MaxValue);
using (var terminal = new GestureRecognitionEngine(configuration with { TotalDurationNs = 0, IdleDurationNs = 0 }))
{
    terminal.UseMatcher(activeMatcher);
    terminal.Start(0);
    terminal.Step(new(PointerEventKind.Down, 1, new(0, 0)), 1);
    terminal.Step(new(PointerEventKind.Moved, 1, new(40, 0)), 2);
    var ended = terminal.Step(new(PointerEventKind.Up, 1, new(double.NaN, double.PositiveInfinity)), 3);
    Check(ended.Stage == InputStage.Idle && ended.Disposition == Disposition.Consume &&
        ended.DiscardPendingVisibleTrace && ended.DeadlineNs is null, "Typed malformed-release frame");
    terminal.Close(4);
    Check(ended.Effects is [Effect.Release { Button: 1, Point.X: 40, Point.Y: 0 }, Effect.Publish { Trace.Visible: false }] &&
        ended.LastSessionReport is { Outcome: SessionOutcome.InvalidRelease, Captured: true },
        "Ordered malformed-release effects and typed report survive closure");
}
using (var extremeMotion = new GestureRecognitionEngine(configuration with { TotalDurationNs = 0, IdleDurationNs = 0 }))
{
    var start = new GesturePoint(-double.MaxValue, -double.MaxValue);
    var end = new GesturePoint(double.MaxValue, double.MaxValue / 2);
    extremeMotion.UseMatcher(activeMatcher);
    extremeMotion.Start(0);
    extremeMotion.Step(new(PointerEventKind.Down, 1, start), 1);
    var extremeActive = extremeMotion.Step(new(PointerEventKind.Moved, 1, end), 2);
    var finished = extremeMotion.Step(new(PointerEventKind.Up, 1, end), 3);
    extremeMotion.Close(4);
    Check(extremeActive.Trace.Endpoints.SequenceEqual([start, end]) &&
        finished.LastSessionReport?.MaximumAxisDistance == double.MaxValue,
        "Typed endpoints and session measurement retain extreme finite values after close");
}

foreach (var limit in new int[] { 0, 4097, int.MaxValue })
{
    try { using var invalidBudget = new GestureRecognitionEngine(configuration with { MaximumDirections = limit }); throw new System.Diagnostics.UnreachableException("Invalid budget accepted"); }
    catch (ArgumentException) { }
}
using (var bounded = new GestureRecognitionEngine(configuration with { MaximumDirections = 1, TotalDurationNs = 0, IdleDurationNs = 0 }))
{
    bounded.Start(0); bounded.SetRecording(true, 0);
    bounded.Step(new(PointerEventKind.Down, 1, new(0, 0)), 1);
    bounded.Step(new(PointerEventKind.Moved, 1, new(20, 0)), 2);
    var limited = bounded.Step(new(PointerEventKind.Moved, 1, new(20, 20)), 3);
    Check(limited.LastSessionReport is { Outcome: SessionOutcome.DirectionLimit, RejectedDirection: Direction.Down }, "Budget decision");
    Check(limited.LastSessionReport!.Directions.SequenceEqual([Direction.Right]), "Bounded accepted directions");
    Check(limited.Effects.Any(effect => effect is Effect.DragStart) && !limited.Trace.Visible, "Budget recovery and hide");
    Check(bounded.Step(new(PointerEventKind.Up, 1, new(20, 20)), 4).Disposition == Disposition.PassThrough, "Physical release passes after recovery");
    bounded.Close(5);
}
Console.WriteLine("Managed recognition passed native layouts, ordered recovery, snapshots, lifetimes and invalid/disposed input checks.");

#if SOURCE_CONSUMER
await ObservationReaderTests.RunAsync();
#endif

if (OperatingSystem.IsWindows())
{
    try { using var invalid = WindowsRecognizer.StartWithContext(configuration, null!); throw new System.Diagnostics.UnreachableException("Null resolver accepted"); }
    catch (ArgumentNullException) { }
    try
    {
        using var invalid = WindowsRecognizer.StartWithContext(configuration with { StartDistance = -1 },
            static _ => throw new System.Diagnostics.UnreachableException("Invalid tuning invoked the resolver"));
        throw new System.Diagnostics.UnreachableException("Invalid contextual configuration accepted");
    }
    catch (ArgumentException) { }
}

// Global hooks are exercised only by the explicitly selected, isolated-desktop
// check. Ordinary interop tests and package consumers never install input hooks.
if (args.Contains("--windows-input"))
{
    if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Live input check requires isolated Windows.");
    using var desktopMatcher = new GesturePatternMatcher(); desktopMatcher.Register([Direction.Right], 42);
    using var desktop = new NaviApps.PointerGestures.Recognition.Windows.WindowsRecognizer(
        new GestureRecognitionConfiguration(15, 5, Timing.DurationNanoseconds(5), Timing.DurationNanoseconds(2)), desktopMatcher);
    Check(desktop.NowNs < ulong.MaxValue, "Native adapter clock");
    try { using var duplicate = new WindowsRecognizer(configuration, desktopMatcher); throw new System.Diagnostics.UnreachableException("Duplicate capture accepted"); }
    catch (WindowsRecognitionException error) { Check(error.Failure == WindowsRecognitionFailure.AlreadyRunning, "Native duplicate capture projects its typed failure"); }
    desktop.UseMatcher(desktopMatcher);
    desktop.SetPolicy(true, NaviApps.PointerGestures.Recognition.Windows.WindowsModifierKeys.Control);
    desktop.SetRecording(true); desktop.SetRecording(false); desktop.SetPolicy(true);
    desktop.Cancel();
    while (desktop.TryRead(out _)) { }
    using (var cancelled = new CancellationTokenSource())
    {
        var pending = desktop.ReadAsync(cancelled.Token).AsTask();
        await Task.Delay(30);
        Check(!pending.IsCompleted, "Idle async reader waits without timer polling");
        cancelled.Cancel();
        try { await pending; throw new System.Diagnostics.UnreachableException("Cancelled wait returned an observation"); }
        catch (OperationCanceledException) { }
    }
    var notifiedMs = new List<double>();
    var polledMs = new List<double>();
    var notificationRecording = false;
    for (int i = 0; i < 20; i++)
    {
        var pending = desktop.ReadAsync().AsTask();
        var started = System.Diagnostics.Stopwatch.StartNew();
        notificationRecording = !notificationRecording; desktop.SetRecording(notificationRecording);
        var observation = await pending.WaitAsync(TimeSpan.FromSeconds(3));
        notifiedMs.Add(started.Elapsed.TotalMilliseconds);
        Check(observation is { Status.Ready: true } && observation.Status.Recording == notificationRecording,
            "Notification delivers the changed state without losing input readiness");
        while (desktop.TryRead(out _)) { }
        started.Restart(); notificationRecording = !notificationRecording; desktop.SetRecording(notificationRecording);
        await Task.Delay(8); // Illustrative old-style poll phase, same native publication.
        Check(desktop.TryRead(out var polled) && polled.Status.Recording == notificationRecording, "Polling comparison consumes its publication");
        polledMs.Add(started.Elapsed.TotalMilliseconds);
        while (desktop.TryRead(out _)) { }
    }
    var waitingForStop = desktop.ReadAsync().AsTask();
    desktop.Stop(); desktop.Stop();
    bool stopped = false;
    var observed = await waitingForStop.WaitAsync(TimeSpan.FromSeconds(3));
    while (observed is not null)
    {
        Check(observed.Effects.All(effect => effect is Effect.Match or Effect.Publish), "Desktop input was already recovered natively");
        stopped |= !observed.Status.Ready;
        observed = await desktop.ReadAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(3));
    }
    Check(stopped, "Async reader drains final stop observation before completion");
    Check(await desktop.ReadAsync() is null, "Repeated completed read never waits again");
    desktop.Dispose();
    using (var lagged = new WindowsRecognizer(new GestureRecognitionConfiguration(15, 5, 0, 0), desktopMatcher))
    {
        bool overflowed = false;
        try { for (int i = 0; i < 512; i++) lagged.SetRecording(i % 2 == 0); }
        catch (WindowsRecognitionException error) { overflowed = error.Failure == WindowsRecognitionFailure.ConsumerLagged; }
        Check(overflowed, "Native queue overflow projects its typed failure");
        int drained = 0;
        try { while (await lagged.ReadAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(3)) is not null) drained++; throw new System.Diagnostics.UnreachableException("Lagged completion lost its failure"); }
        catch (WindowsRecognitionException error) { Check(error.Failure == WindowsRecognitionFailure.ConsumerLagged && drained > 256, "Typed terminal failure follows retained final observations"); }
    }
    using (var disposable = new NaviApps.PointerGestures.Recognition.Windows.WindowsRecognizer(
        new GestureRecognitionConfiguration(15, 5, 0, 0), desktopMatcher))
    {
        while (disposable.TryRead(out _)) { }
        var pending = disposable.ReadAsync().AsTask();
        disposable.Dispose();
        try { await pending.WaitAsync(TimeSpan.FromSeconds(3)); throw new System.Diagnostics.UnreachableException("Disposed wait remained readable"); }
        catch (ObjectDisposedException) { }
    }
    var session = new WindowsSessionContext(desktopMatcher, true, WindowsModifierKeys.None, 0);
    using (var contextual = WindowsRecognizer.StartWithContext(configuration, _ => session))
    {
        while (contextual.TryReadWithContext(out var startup))
            Check(startup.Value.Epoch is null, "No host epoch before the first eligible press");
        contextual.SetContextResolver(_ => session);
        var waiting = contextual.ReadWithContextAsync().AsTask();
        contextual.SetRecording(true);
        var updated = await waiting.WaitAsync(TimeSpan.FromSeconds(3));
        Check(updated is { Epoch: null, Output.Status.Recording: true }, "Context-aware async reads share native notifications");
        contextual.Stop();
        try
        {
            contextual.SetContextResolver(_ => session);
            throw new System.Diagnostics.UnreachableException("A stopped recognizer accepted a new resolver");
        }
        catch (InvalidOperationException) { }
        var final = await contextual.ReadWithContextAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(3));
        Check(final is { Epoch: null, Output.Status.Ready: false }, "Context-aware reads retain final stop frames");
        Check(await contextual.ReadWithContextAsync() is null, "Context-aware read completes after final frame");
    }
    notifiedMs.Sort(); polledMs.Sort();
    var measurement = new
    {
        iterations = 20,
        notificationMedianMs = notifiedMs[10],
        illustrative8msPollingMedianMs = polledMs[10],
        idleReaderCompleted = false,
        scope = "Native control publication delivery; includes managed scheduling. Not physical mouse or UI end-to-end latency."
    };
    Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(measurement));
    if (Environment.GetEnvironmentVariable("NAVI_CI_ARTIFACTS") is { } artifacts)
        File.WriteAllText(Path.Combine(artifacts, "pointer-recognition-notification.json"), System.Text.Json.JsonSerializer.Serialize(measurement));
    Console.WriteLine("Managed Windows adapter passed native start, policy, matcher, recording, observation and idempotent stop checks.");
}

// Do not let caller JIT locals extend the engine's lifetime independently of its SafeHandle.
[System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
static (WeakReference<GestureRecognitionEngine> Engine, GestureRecognitionOutput Output) AbandonEngine()
{
    var engine = new GestureRecognitionEngine(new(10, 5, 0, 0));
    engine.Start(0); engine.SetRecording(true, 0);
    engine.Step(new(PointerEventKind.Down, 1, new(0, 0)), 1);
    var output = engine.Step(new(PointerEventKind.Moved, 1, new(40, 0)), 2);
    return (new WeakReference<GestureRecognitionEngine>(engine), output);
}
[System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
static bool HasTarget(WeakReference<GestureRecognitionEngine> weak) => weak.TryGetTarget(out _);
