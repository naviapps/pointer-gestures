namespace NaviApps.PointerGestures.Recognition.Windows;

/// <summary>Serializes notification waits with ownership of a native queue entry.</summary>
internal sealed class WindowsObservationReader(
    Func<object> getGate,
    Func<(WindowsGestureObservation? Observation, bool Completed)> read,
    Func<CancellationToken, Task> waitForNotification)
{
    internal async ValueTask<WindowsGestureObservation?> ReadAsync(CancellationToken cancellationToken)
    {
        while (true)
        {
            cancellationToken.ThrowIfCancellationRequested();
            Task wait;
            lock (getGate())
            {
                // A native command can hold the gate while this reader is cancelled.
                cancellationToken.ThrowIfCancellationRequested();
                var result = read();
                if (result.Observation is not null || result.Completed) return result.Observation;
                wait = waitForNotification(cancellationToken);
            }
            await wait.ConfigureAwait(false);
        }
    }
}
