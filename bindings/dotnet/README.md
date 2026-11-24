# Pointer Gestures for .NET

`NaviApps.PointerGestures.Recognition` matches gestures and returns ordered effects.
`NaviApps.PointerGestures.Rendering` plans gesture geometry and live presentation.
Rendering depends on Recognition. The portable APIs run on macOS and Windows.
Types in the `Windows` namespaces provide optional desktop input and overlays.

## Install

Use .NET 10 on macOS 26 or later (ARM64 or x64) or Windows x64. On Windows 11
ARM64, use the x64 .NET SDK and runtime.

Get the packages from the same successful
[SDK distribution run](https://github.com/naviapps/sdk-tooling/actions/workflows/check-sdk-distribution.yml).
The Rendering artifact contains the exact Recognition version it needs under
`dependencies/`. Verify both archives against the SHA-256 in their receipts before
copying them into one local NuGet feed. Use a new directory so the restore starts
with an empty package cache, and substitute the downloaded package version.

```sh
dotnet new console --framework net10.0 --no-restore
dotnet package add NaviApps.PointerGestures.Rendering --version "<PACKAGE_VERSION>" --no-restore
dotnet restore --source "/path/to/pointer-feed" --packages ./.pointer-packages
```

For recognition alone, add `NaviApps.PointerGestures.Recognition` instead.
Replace `Program.cs` with one of the examples below and run `dotnet run --no-restore`.
Pass `-- --desktop-input` or `-- --desktop-overlay` to run the corresponding
native example on an interactive Windows desktop.

## Recognize gestures

Create a matcher and pass pointer events to the engine with monotonic
nanosecond timestamps. Deliver its effects in order.

```csharp
using NaviApps.PointerGestures.Recognition;

using var matcher = new GesturePatternMatcher();
matcher.Register([Direction.Right], 42);
using var engine = new GestureRecognitionEngine(new GestureRecognitionConfiguration(10, 5, Timing.DurationNanoseconds(1), 0));
engine.UseMatcher(matcher);

void Deliver(GestureRecognitionOutput output)
{
    foreach (var effect in output.Effects) Console.WriteLine(effect);
}

Deliver(engine.Start(0));
Deliver(engine.Step(new(PointerEventKind.Down, 1, new(0, 0)), 1));
Deliver(engine.Step(new(PointerEventKind.Moved, 1, new(40, 0)), 2));
Deliver(engine.Step(new(PointerEventKind.Up, 1, new(40, 0)), 3));
Deliver(engine.Close(4));
```

`GestureRecognitionConfiguration.Validate()`, construction and tuning changes
report the first invalid threshold or count as an `ArgumentOutOfRangeException`
with its property name. A rejected tuning update leaves the active input and
deadline intact.

`GestureRecognitionEngine.PrepareReplacementPress` lets custom sources deliver
previous input recovery before selecting the new matcher and policy. Submit the
press to `Step` if it is still current. An elapsed deadline takes precedence
over `Superseded`. Deliver recovery from `Close` before disposing the engine.
Disposal releases native memory. Managed snapshots own their copied data and
compare ordered collections by value.

## Capture desktop input on Windows

`WindowsRecognizer` owns hooks and ordered replay recovery. It publishes
match and trace effects to the host. Drain observations serially and call
`Stop` to observe recovery errors. Disposing attempts recovery without reporting
failures. Cancellation ends the wait. The adapter delivers recovery itself.

```csharp
using NaviApps.PointerGestures.Recognition;
using NaviApps.PointerGestures.Recognition.Windows;

// Live hooks require an explicit opt-in on an interactive Windows desktop.
if (OperatingSystem.IsWindows() && args.Contains("--desktop-input"))
{
    using var matcher = new GesturePatternMatcher();
    matcher.Register([Direction.Right], 42);
    using var input = new WindowsRecognizer(new GestureRecognitionConfiguration(10, 5, 1_000_000_000, 0), matcher);
    input.SetPolicy(true, WindowsModifierKeys.None);
    using var lifetime = new CancellationTokenSource(TimeSpan.FromSeconds(30));
    try
    {
        while (await input.ReadAsync(lifetime.Token) is { } output)
            foreach (var effect in output.Effects) Console.WriteLine(effect);
    }
    catch (OperationCanceledException) when (lifetime.IsCancellationRequested) { }
    finally { input.Stop(); } // Stop owns native input recovery; cancellation only ends this wait.
}
```

Capture validates the native button and limits at startup. For native failures,
inspect `WindowsRecognitionException.Failure`, `NativeErrorCode`,
`AcceptedEventCount` and `RequestedEventCount`. Use the
[Windows input contracts](https://github.com/naviapps/pointer-gestures/blob/main/docs/WINDOWS.md#native-input-recovery)
for failure reasons and restart policy.

## Plan and present geometry

Use `GestureGeometryPlanner` for host-owned drawing. `TryPlanPattern` and
`TryPlanLiveTrace` return a `GestureGeometryError` when input is rejected.
`PlanPattern` and `PlanLiveTrace` throw `ArgumentException` on rejection.
The managed planner reports invalid native results if the native status
is unknown or success returns without geometry.

Use one `OverlayTraceConverter` per recognition stream and pass each
`Effect.Publish.Trace` through it. Deliver effects in order so the final
visible trace reaches the reducer before it hides. Treat
`DiscardPendingVisibleTrace` as an immediate clear.

```csharp
using NaviApps.PointerGestures.Recognition;
using NaviApps.PointerGestures.Rendering;

if (GestureGeometryPlanner.TryPlanPattern([Direction.Right, Direction.Down], 96, 64, out var preview, out var error))
    Console.WriteLine(preview.Path.Count);
else
    Console.WriteLine(error);

var converter = new OverlayTraceConverter();
var captured = new GestureTrace(true, [Direction.Right], [new(0, 0), new(40, 0)], [], null);
var trace = converter.Convert(captured);
using var state = new OverlayState(0, timing: new OverlayTiming());
Console.WriteLine(state.Update(trace, 0).Trace!.Points.Count);
var pendingHide = state.Update(null, 1);
var hidden = state.Tick(pendingHide.DeadlineNanoseconds!.Value);
Console.WriteLine(hidden.Trace is null);
```

Geometry, traces and updates compare by value. Geometry copies command lists
supplied through construction and `with` expressions. Published commands and
trace points are read-only. An absent tail differs from a tail at the origin.
Signed zero compares equally.

## Draw a desktop overlay on Windows

`WindowsOverlay` draws in physical virtual-desktop pixels. Call its methods on
the host loop outside input hooks. The native owner schedules presentation, so
the host needs no timer. `Configure` updates appearance and timing while
preserving the window, trace and pending hide.

```csharp
using NaviApps.PointerGestures.Recognition;
using NaviApps.PointerGestures.Rendering;
using NaviApps.PointerGestures.Rendering.Windows;

if (OperatingSystem.IsWindows() && args.Contains("--desktop-overlay"))
{
    using var overlay = new WindowsOverlay(new(0x3D7CFFFF, 4));
    overlay.Activate(); // Explicit lifetime; constructor is inactive.
    overlay.Update(new OverlayTrace([Direction.Right], [new(100, 100), new(200, 100)]));
    overlay.Clear(); // Immediate cancellation discards pending geometry.
    overlay.Deactivate(); // Joins and destroys its resources; can activate again.
}
```

Deliver publication effects in order. A null trace schedules the final-frame
hide. Call `Clear` immediately when `DiscardPendingVisibleTrace` is set.
See the
[Windows overlay contracts](https://github.com/naviapps/pointer-gestures/blob/main/docs/WINDOWS.md#native-desktop-rendering)
for coordinates, limits and resource lifetime.

The [shared input and geometry contracts](https://github.com/naviapps/pointer-gestures/blob/main/README.md#input-and-geometry-contracts)
describe matcher identity, input recovery and presentation rules.
Use the [session report and budget contract](https://github.com/naviapps/pointer-gestures/blob/main/README.md#session-measurements-and-budgets)
when consuming `GestureRecognitionOutput.LastSessionReport`.
