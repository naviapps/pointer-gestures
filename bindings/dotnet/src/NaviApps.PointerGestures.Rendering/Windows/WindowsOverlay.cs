using System.Runtime.Versioning;

namespace NaviApps.PointerGestures.Rendering.Windows;

/// <summary>Borrowed native window identity and physical desktop bounds of the last OS submission.
/// Visibility is not a compositor completion fence. Never mutate or destroy Window.</summary>
/// <param name="Window">Borrowed HWND invalidated by worker failure, deactivation or disposal.</param>
/// <param name="Visible">Whether visible geometry was submitted.</param>
/// <param name="Bounds">Physical desktop bounds, absent when hidden.</param>
public sealed record WindowsOverlaySnapshot(nuint Window, bool Visible, Bounds? Bounds);

/// <summary>Optional nonactivating, click-through desktop overlay. Rust owns geometry,
/// rasterization, cadence and the dedicated native thread. Hosts own activation,
/// appearance and error presentation. Calls wait for native completion; never call from an input hook.</summary>
[SupportedOSPlatform("windows")]
public sealed class WindowsOverlay : IDisposable
{
    private readonly object gate = new();
    private readonly WindowsOverlayHandle handle;

    /// <summary>Create an inactive overlay. No window, thread or input hook starts here.
    /// Appearance and cadence are host policy.</summary>
    public WindowsOverlay(WindowsOverlayConfiguration configuration)
    {
        ArgumentNullException.ThrowIfNull(configuration);
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Desktop overlay requires Windows.");
        Native.EnsureAbi();
        if (Native.WindowsOverlayAbi() != 4) throw new NotSupportedException("Unsupported Windows overlay ABI.");
        var pointer = Native.CreateWindowsOverlay(configuration.NativeValue, out var error);
        error.ThrowIfFailed();
        handle = new(pointer);
        if (handle.IsInvalid) throw new InvalidOperationException("Native overlay returned no owner.");
    }

    /// <summary>Create a hidden window on a dedicated DPI-aware thread. Repeated calls are harmless.</summary>
    public void Activate() { lock (gate) { EnsureOpen(); Native.ActivateWindowsOverlay(handle, out var error); error.ThrowIfFailed(); } }

    /// <summary>Update appearance/cadence in place, preserving pending geometry and
    /// final hiding. Inactive owners retain configuration for activation. Invalid
    /// values leave the current configuration and native resources intact.</summary>
    public void Configure(WindowsOverlayConfiguration configuration)
    {
        ArgumentNullException.ThrowIfNull(configuration);
        lock (gate)
        {
            EnsureOpen(); Native.ConfigureWindowsOverlay(handle, configuration.NativeValue, out var error);
            error.ThrowIfFailed();
        }
    }

    /// <summary>Submit physical-pixel trace geometry, or schedule final-frame hiding with null.
    /// Rust copies/shares the trace and owns timing. At most 4096 directions and 16M raster pixels are accepted.</summary>
    public void Update(OverlayTrace? trace)
    {
        lock (gate)
        {
            EnsureOpen();
            NativeWindowsOverlayError error;
            if (trace is null) Native.UpdateWindowsOverlayNull(handle, 0, out error);
            else Native.UpdateWindowsOverlay(handle, trace.Handle, out error);
            error.ThrowIfFailed();
        }
    }

    /// <summary>Discard pending geometry and hide immediately, e.g. on cancellation.</summary>
    public void Clear() { lock (gate) { EnsureOpen(); Native.ClearWindowsOverlay(handle, out var error); error.ThrowIfFailed(); } }

    /// <summary>Read submitted native state. Requires activation; the HWND is borrowed.</summary>
    public WindowsOverlaySnapshot Snapshot
    {
        get
        {
            lock (gate)
            {
                EnsureOpen(); Native.ViewWindowsOverlay(handle, out var view, out var error); error.ThrowIfFailed();
                return new(view.Window, view.Visible != 0, view.Visible == 0 ? null : view.Bounds);
            }
        }
    }

    /// <summary>Hide, destroy resources and join. Repeated calls are harmless; activate can start again.</summary>
    public void Deactivate() { lock (gate) { EnsureOpen(); Native.DeactivateWindowsOverlay(handle, out var error); error.ThrowIfFailed(); } }
    /// <summary>Join the native worker and release the owner. Use Deactivate first when errors must be reported.</summary>
    public void Dispose() { lock (gate) handle.Dispose(); }
    private void EnsureOpen() => ObjectDisposedException.ThrowIf(handle.IsClosed, this);
}
