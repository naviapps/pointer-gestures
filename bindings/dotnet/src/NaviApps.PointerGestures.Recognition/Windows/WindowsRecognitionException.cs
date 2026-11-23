namespace NaviApps.PointerGestures.Recognition.Windows;

/// <summary>A structured native recognition failure. Configuration, disposal and
/// wait cancellation retain their standard .NET exception types.</summary>
public sealed class WindowsRecognitionException : IOException
{
    /// <summary>Stable reason for programmatic handling; never parse Message.</summary>
    public WindowsRecognitionFailure Failure { get; }
    /// <summary>Native Windows error code. Zero does not imply successful recovery.</summary>
    public uint NativeErrorCode { get; }
    /// <summary>Recovery events accepted by Windows; zero outside IncompleteRecovery.</summary>
    public uint AcceptedEventCount { get; }
    /// <summary>Requested recovery events; zero outside IncompleteRecovery.</summary>
    public uint RequestedEventCount { get; }

    /// <summary>Create a failure with its native details and optional causal exception.</summary>
    public WindowsRecognitionException(WindowsRecognitionFailure failure, uint nativeErrorCode = 0,
        uint acceptedEventCount = 0, uint requestedEventCount = 0, Exception? innerException = null)
        : base(Describe(failure, nativeErrorCode, acceptedEventCount, requestedEventCount), innerException)
    {
        Failure = failure;
        NativeErrorCode = nativeErrorCode;
        AcceptedEventCount = acceptedEventCount;
        RequestedEventCount = requestedEventCount;
    }

    private static string Describe(WindowsRecognitionFailure failure, uint code, uint accepted, uint requested) => failure switch
    {
        WindowsRecognitionFailure.AlreadyRunning => "A Windows desktop recognizer already runs in this process.",
        WindowsRecognitionFailure.System => $"Windows input hook operation failed (system error {code}).",
        WindowsRecognitionFailure.IncompleteRecovery => $"Windows accepted {accepted} of {requested} recovery events (system error {code}).",
        WindowsRecognitionFailure.ConsumerLagged => "Windows observation queue filled; capture stopped and input recovery was attempted.",
        WindowsRecognitionFailure.WorkerStopped => "Windows input message loop stopped.",
        WindowsRecognitionFailure.ReplayTargetChanged => "The original Windows input target changed; no new press was replayed into its replacement.",
        _ => throw new ArgumentOutOfRangeException(nameof(failure)),
    };
}
