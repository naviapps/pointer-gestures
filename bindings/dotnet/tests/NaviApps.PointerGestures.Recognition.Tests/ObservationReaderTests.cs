using System.Diagnostics;
using NaviApps.PointerGestures.Recognition;
using NaviApps.PointerGestures.Recognition.Windows;

internal static class ObservationReaderTests
{
    private static readonly TimeSpan Timeout = TimeSpan.FromSeconds(5);

    internal static async Task RunAsync()
    {
        await CancellationWhileWaitingForTheGatePreservesTheFrame();
        await CancellationAfterDequeueDeliversTheOwnedFrame();
        Console.WriteLine("Managed observation cancellation preserves frame ownership at the native gate.");
    }

    private static WindowsGestureObservation FinalFrame() =>
        new(new GestureRecognitionOutput(
            Disposition.Control, InputStage.Idle, new(false, false, false, null, []),
            new(false, [], [], [], null), 0, false, null, [], null),
            Epoch: 0);

    private static async Task CancellationWhileWaitingForTheGatePreservesTheFrame()
    {
        var gate = new object();
        var notification = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        using var attemptingSecondRead = new ManualResetEventSlim();
        using var cancellation = new CancellationTokenSource();
        var queue = new Queue<WindowsGestureObservation>();
        var attempts = 0;
        var expected = FinalFrame();
        var reader = new WindowsObservationReader(
            () =>
            {
                // The loop has passed its token check and is about to contend for the monitor.
                if (Interlocked.Increment(ref attempts) == 2) attemptingSecondRead.Set();
                return gate;
            },
            () => queue.TryDequeue(out var frame) ? (frame, false) : (null, false),
            _ => notification.Task);

        var pending = reader.ReadAsync(cancellation.Token).AsTask();
        lock (gate)
        {
            queue.Enqueue(expected);
            notification.SetResult();
            Check(attemptingSecondRead.Wait(Timeout), "The woken reader did not reach the contended monitor.");
            cancellation.Cancel();
        }

        try
        {
            _ = await pending.WaitAsync(Timeout);
            throw new UnreachableException("The cancelled reader acquired a final observation.");
        }
        catch (OperationCanceledException error) when (error.CancellationToken == cancellation.Token) { }
        Check(queue.Count == 1, "Cancellation before queue admission must leave the frame available.");
        Check(await reader.ReadAsync(CancellationToken.None) == expected,
            "The next reader must receive the same frame and captured context.");
    }

    private static async Task CancellationAfterDequeueDeliversTheOwnedFrame()
    {
        using var cancellation = new CancellationTokenSource();
        var expected = FinalFrame();
        var queue = new Queue<WindowsGestureObservation>([expected]);
        var gate = new object();
        var reader = new WindowsObservationReader(
            () => gate,
            () =>
            {
                var frame = queue.Dequeue();
                cancellation.Cancel();
                return (frame, false);
            },
            _ => throw new UnreachableException("An available observation must not wait."));
        Check(await reader.ReadAsync(cancellation.Token) == expected && queue.Count == 0,
            "Once the read owns a frame, cancellation must not discard it.");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new UnreachableException(message);
    }
}
