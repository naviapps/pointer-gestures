using System.Text.Json;

namespace NaviApps.PointerGestures.WindowsExample;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        if (args.Length != 0 && (args.Length != 2 || args[0] != "--verify"))
            throw new ArgumentException("Usage: PointerGestures.WindowsExample [--verify result-directory]");
        using var window = new ExampleWindow(args.Length != 0);
        if (args.Length != 0)
        {
            var directory = Path.GetFullPath(args[1]);
            Directory.CreateDirectory(directory);
            window.Shown += async (_, _) =>
            {
                object result;
                try { result = await Verification.Run(window, directory); }
                catch (Exception error)
                {
                    Environment.ExitCode = 1;
                    result = new { state = "failed", error = error.ToString() };
                }
                finally { if (!window.IsDisposed) window.Close(); }
                var json = JsonSerializer.Serialize(result);
                File.WriteAllText(Path.Combine(directory, "result.json"), json);
                Console.WriteLine(json);
            };
        }
        Application.Run(window);
    }
}

internal sealed class ExampleWindow : Form
{
    internal GestureCanvas Canvas { get; }
    internal CheckBox Recording { get; } = new() { Text = "&Record any pattern", AutoSize = true };

    internal ExampleWindow(bool verification)
    {
        Text = "Pointer Gestures";
        ClientSize = new(620, 480);
        MinimumSize = new(460, 380);
        AutoScaleMode = AutoScaleMode.Dpi;
        Canvas = new GestureCanvas(verification ? 0.05 : 1.0 / 60.0) { Dock = DockStyle.Fill };
        var instructions = new Label { Text = "Right-drag: Right → Down matches. Escape cancels.", Dock = DockStyle.Top, Height = 28, AutoSize = false };
        var status = new Label { Text = "Ready", Dock = DockStyle.Bottom, Height = 28, AccessibleName = "Recognition status" };
        var toolbar = new FlowLayoutPanel { Dock = DockStyle.Top, Height = 36 };
        var reset = new Button { Text = "&Reset", AutoSize = true };
        reset.Click += (_, _) => Canvas.ClearFeedback();
        Recording.CheckedChanged += (_, _) => Canvas.SetRecording(Recording.Checked);
        Canvas.StatusChanged += text => status.Text = text;
        toolbar.Controls.Add(Recording); toolbar.Controls.Add(reset);
        Controls.Add(Canvas); Controls.Add(status); Controls.Add(toolbar); Controls.Add(instructions);
        FormClosing += (_, _) => Canvas.Stop();
        Deactivate += (_, _) => Canvas.CancelInput();
    }
}
