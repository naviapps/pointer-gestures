namespace NaviApps.PointerGestures.Recognition;

internal static class FrameReader
{
    internal static GestureRecognitionOutput Read(nint pointer)
    {
        if (pointer == 0) throw new InvalidOperationException("Native recognition returned no output frame.");
        using var owner = new FrameHandle(pointer);
        var frame = Native.ViewFrame(owner);
        var recorded = Native.CopySnapshot<Direction>(frame.Recorded);
        var trace = new GestureTrace(frame.Visible != 0, Native.CopySnapshot<Direction>(frame.Directions), Native.CopySnapshot<GesturePoint>(frame.Endpoints), Native.CopySnapshot<GesturePoint>(frame.Raw), frame.TailPresent == 0 ? null : frame.Tail);
        var effects = new Effect[checked((int)frame.EffectCount)];
        for (int i = 0; i < effects.Length; i++)
        {
            var effect = Native.ViewEffect(owner, (nuint)i);
            effects[i] = effect.Kind switch
            {
                1 => new Effect.Click(effect.Button, Native.ReadFirst<GesturePoint>(effect.Points)),
                2 => new Effect.Drag(effect.Button, Native.CopySnapshot<GesturePoint>(effect.Points)),
                3 => new Effect.DragStart(effect.Button, Native.CopySnapshot<GesturePoint>(effect.Points)),
                4 => new Effect.Release(effect.Button, Native.ReadFirst<GesturePoint>(effect.Points)),
                5 => new Effect.Match(effect.MatchValue),
                6 => new Effect.Publish(CopyStatus(effect.Ready, effect.Capturing, effect.Recording, effect.Failure, CopySnapshot(effect.Recorded, frame.Recorded, recorded)), CopyTrace(effect.Trace, frame, trace)),
                _ => throw new InvalidDataException("Unknown native effect kind.")
            };
        }
        var report = Native.ViewSessionReport(owner);
        SessionReport? session = report.Present == 0 ? null : new(report.Generation, (SessionOutcome)report.Outcome,
            report.ElapsedNs, report.IdleNs, report.PointCount, report.MaximumAxisDistance,
            report.Captured != 0, report.Recording != 0, Native.CopySnapshot<Direction>(report.Directions),
            report.RejectedDirection == 0 ? null : (Direction)report.RejectedDirection);
        return new((Disposition)frame.Disposition, (InputStage)frame.Stage,
            CopyStatus(frame.Ready, frame.Capturing, frame.Recording, frame.Failure, recorded), trace,
            frame.InputGeneration, frame.DiscardVisible != 0, frame.DeadlinePresent == 0 ? null : frame.DeadlineNs, SnapshotValues<Effect>.TakeOwned(effects), session);
    }
    private static GestureRecognitionStatus CopyStatus(uint ready, uint capturing, uint recording, uint failure, IReadOnlyList<Direction> recorded) =>
        new(ready != 0, capturing != 0, recording != 0, failure == 0 ? null : (Failure)failure, recorded);
    private static GestureTrace CopyTrace(NativeTrace trace, NativeFrame frame, GestureTrace copied) =>
        new(trace.Visible != 0, CopySnapshot(trace.Directions, frame.Directions, copied.Directions), CopySnapshot(trace.Endpoints, frame.Endpoints, copied.Endpoints), CopySnapshot(trace.Raw, frame.Raw, copied.RawPoints), trace.TailPresent == 0 ? null : trace.Tail);

    // Compare borrowed buffer identities only while this frame owns both views.
    // Shared results are managed read-only copies, independent of native lifetime.
    private static IReadOnlyList<T> CopySnapshot<T>(NativeBuffer view, NativeBuffer reference, IReadOnlyList<T> copied) where T : unmanaged =>
        view.Data == reference.Data && view.Count == reference.Count ? copied : Native.CopySnapshot<T>(view);
}
