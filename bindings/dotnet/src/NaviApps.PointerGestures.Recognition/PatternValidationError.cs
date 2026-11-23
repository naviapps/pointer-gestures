namespace NaviApps.PointerGestures.Recognition;

/// <summary>Rust-owned reasons for rejecting a pattern.</summary>
public enum PatternValidationReason
{
    /// <summary>No direction run was supplied.</summary>
    Empty = 1,
    /// <summary>More than 4096 direction runs were supplied.</summary>
    TooLong = 2,
    /// <summary>The indexed run repeats the preceding direction.</summary>
    RepeatedDirection = 3,
    /// <summary>The indexed value is not a supported direction.</summary>
    InvalidDirection = 4
}

/// <summary>Rejection reason and optional zero-based offending run. Length errors have no index.</summary>
public sealed record PatternValidationError(PatternValidationReason Reason, int? Index)
{
    internal static PatternValidationError? Decode(NativePatternValidation result) => result.Code switch
    {
        0 => null,
        >= 1 and <= 4 => new((PatternValidationReason)result.Code, result.Index == uint.MaxValue ? null : checked((int)result.Index)),
        _ => throw new InvalidDataException("Invalid native pattern buffer or validation code.")
    };
}

[System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)]
internal struct NativePatternValidation { internal uint Code; internal uint Index; }
