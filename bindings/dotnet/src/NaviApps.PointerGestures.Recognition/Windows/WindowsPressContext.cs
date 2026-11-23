namespace NaviApps.PointerGestures.Recognition.Windows;

/// <summary>Native lookup hints captured for an eligible pointer press.</summary>
/// <param name="Point">Press position in physical desktop pixels.</param>
/// <param name="RecipientProcessId">Process owning the input recipient, including native capture.</param>
/// <param name="ForegroundProcessId">Foreground process ID when available.</param>
public readonly record struct WindowsPressContext(
    GesturePoint Point,
    uint RecipientProcessId,
    uint? ForegroundProcessId);

/// <summary>An immutable matcher, policy and host command epoch for a press.</summary>
public sealed class WindowsSessionContext
{
    internal MatcherHandle Matcher { get; }

    /// <summary>Whether this press may start recognition.</summary>
    public bool Enabled { get; }
    /// <summary>Required modifier groups. Additional held modifiers are allowed.</summary>
    public WindowsModifierKeys RequiredModifiers { get; }
    /// <summary>Host command snapshot retained through release, interruption and shutdown.</summary>
    public ulong Epoch { get; }

    /// <summary>Capture a matcher snapshot independently of later edits and disposal.
    /// Build contexts before entering the native resolver.</summary>
    public WindowsSessionContext(GesturePatternMatcher matcher, bool enabled, WindowsModifierKeys requiredModifiers, ulong epoch)
    {
        ArgumentNullException.ThrowIfNull(matcher);
        Matcher = matcher.CloneHandle();
        Enabled = enabled;
        RequiredModifiers = requiredModifiers;
        Epoch = epoch;
    }
}

/// <summary>A copied Windows recognition frame and the host snapshot selected for it.</summary>
/// <param name="Output">Recognition and presentation effects in delivery order.</param>
/// <param name="Epoch">Captured host snapshot, absent before the first resolved press.</param>
public readonly record struct WindowsGestureObservation(GestureRecognitionOutput Output, ulong? Epoch);
