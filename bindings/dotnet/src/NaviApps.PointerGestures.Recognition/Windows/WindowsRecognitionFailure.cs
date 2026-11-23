namespace NaviApps.PointerGestures.Recognition.Windows;

/// <summary>A native desktop recognition failure, independent of its message text.
/// Hosts decide whether and when to restart capture.</summary>
public enum WindowsRecognitionFailure
{
    /// <summary>Another recognizer owns input capture in this process.</summary>
    AlreadyRunning,
    /// <summary>A Windows operation failed; inspect NativeErrorCode.</summary>
    System,
    /// <summary>Windows accepted fewer recovery events than requested.</summary>
    IncompleteRecovery,
    /// <summary>The observation queue filled; capture stopped and recovery was attempted.</summary>
    ConsumerLagged,
    /// <summary>The input message loop could not complete a request.</summary>
    WorkerStopped,
    /// <summary>The original recipient changed; no new press was replayed into its replacement.</summary>
    ReplayTargetChanged,
}
