using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Runtime.Versioning;
using Nashi.Core;

[assembly: SupportedOSPlatform("linux")]

namespace Nashi.Cli;

/// Prototype engine switch. `convert TARGET FILE... --json` for audio and video
/// routes runs on upstream Nashi.Core; every other command line is handed to the
/// Rust `nashi-cli` unchanged (execv), so the UI sees the same contract either way.
internal static class Program
{
    private static int Main(string[] args)
    {
        if (MediaConvert.TryPlan(args, out var plan, out var why))
        {
            Console.Error.WriteLine("nashi-cli-dotnet: media conversion on Nashi.Core");
            return MediaConvert.Run(plan);
        }
        if (Environment.GetEnvironmentVariable("NASHI_DOTNET_TRACE") == "1")
            Console.Error.WriteLine($"nashi-cli-dotnet: handing over to the Rust engine ({why})");
        return RustCli.Exec(args);
    }
}

/// Replaces this process with the Rust CLI, so its exit code, stdout/stderr and
/// signal handling (SIGTERM → cancelled, exit 130) stay exactly as they are.
internal static class RustCli
{
    [DllImport("libc", SetLastError = true)]
    private static extern int execv(string path, string?[] argv);

    public static int Exec(string[] args)
    {
        var rust = Find();
        if (rust is null)
        {
            Console.Error.WriteLine("nashi-cli-dotnet: cannot find the Rust nashi-cli (set NASHI_RUST_CLI)");
            return 2;
        }
        var argv = new string?[args.Length + 2];
        argv[0] = rust;
        Array.Copy(args, 0, argv, 1, args.Length);
        Console.Out.Flush();
        execv(rust, argv);
        Console.Error.WriteLine($"nashi-cli-dotnet: cannot start {rust} (errno {Marshal.GetLastPInvokeError()})");
        return 2;
    }

    private static string? Find()
    {
        if (Environment.GetEnvironmentVariable("NASHI_RUST_CLI") is { Length: > 0 } explicitPath)
            return File.Exists(explicitPath) ? Path.GetFullPath(explicitPath) : null;
        var self = Environment.ProcessPath is { } p ? Path.GetFullPath(p) : null;
        var beside = Path.Combine(AppContext.BaseDirectory, "nashi-cli");
        if (File.Exists(beside) && beside != self) return beside;
        return Tools.OnPath("nashi-cli", self);
    }
}

internal static class Tools
{
    public static string? OnPath(string name, string? except = null)
    {
        foreach (var dir in (Environment.GetEnvironmentVariable("PATH") ?? "").Split(':', StringSplitOptions.RemoveEmptyEntries))
        {
            var candidate = Path.Combine(dir, name);
            try
            {
                if (!File.Exists(candidate) || Path.GetFullPath(candidate) == except) continue;
                if ((File.GetUnixFileMode(candidate) & (UnixFileMode.UserExecute | UnixFileMode.GroupExecute | UnixFileMode.OtherExecute)) == 0) continue;
                return Path.GetFullPath(candidate);
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                // A dangling link or unreadable entry in PATH is skipped, never fatal.
            }
        }
        return null;
    }

    /// The same switch the Rust engine honours in tests.
    public static bool Disabled(string name) =>
        (Environment.GetEnvironmentVariable("NASHI_DISABLE_TOOLS") ?? "")
            .Split(',', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries)
            .Contains(name, StringComparer.Ordinal);

    public static string? Find(string name) => Disabled(name) ? null : OnPath(name);
}

internal sealed record MediaPlan(string Target, IReadOnlyList<string> Inputs, string? OutDir, MediaOptions Options, string Ffmpeg, string Ffprobe);

internal static class MediaConvert
{
    // Option keys this prototype maps onto MediaOptions. Any other key sends the
    // whole job to the Rust engine, so no option is ever silently dropped.
    private static readonly HashSet<string> MappedKeys = new(StringComparer.Ordinal)
        { "audio_kbps", "max_height", "gif_fps", "gif_max_width", "strip_metadata" };

    public static bool TryPlan(string[] args, out MediaPlan plan, out string why)
    {
        plan = null!;
        if (args.Length < 3 || args[0] != "convert") { why = "not a conversion"; return false; }
        string? outDir = null, options = null; var json = false; var positional = new List<string>();
        for (var i = 1; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--json": json = true; break;
                case "--out-dir" when i + 1 < args.Length: outDir = args[++i]; break;
                case "--options" when i + 1 < args.Length: options = args[++i]; break;
                default:
                    if (args[i].StartsWith('-')) { why = $"argument {args[i]}"; return false; }
                    positional.Add(args[i]);
                    break;
            }
        }
        if (!json) { why = "no --json"; return false; }
        if (positional.Count < 2) { why = "no input"; return false; }
        var target = positional[0].ToLowerInvariant();
        var audioTarget = MediaEngine.AudioFormats.Contains(target);
        if (!audioTarget && !MediaEngine.VideoFormats.Contains(target)) { why = $"target {target} is not media"; return false; }
        var inputs = positional.Skip(1).Select(Path.GetFullPath).ToList();
        foreach (var input in inputs)
        {
            if (!File.Exists(input)) { why = "an input is missing"; return false; }
            var ext = FeatureCatalog.Extension(input);
            // Audio becomes audio; video and GIF become audio or video. Same-format and
            // audio → video routes stay with the Rust engine (it reports them itself).
            var route = ext != target && (FeatureCatalog.IsAudio(input) ? audioTarget : FeatureCatalog.IsVideo(input));
            if (!route) { why = $"route {ext} → {target}"; return false; }
        }
        if (!TryOptions(options, out var media, out why)) return false;
        if (Tools.Find("ffmpeg") is not { } ffmpeg || Tools.Find("ffprobe") is not { } ffprobe)
        {
            why = "ffmpeg or ffprobe missing";
            return false;
        }
        plan = new(target, inputs, outDir is null ? null : Path.GetFullPath(outDir), media, ffmpeg, ffprobe);
        return true;
    }

    private static bool TryOptions(string? text, out MediaOptions options, out string why)
    {
        // Contract defaults for GIF (CLI-PROTOCOL.md): 12 fps, 640 px wide.
        options = new MediaOptions { GifFrameRate = 12, GifWidth = 640 };
        why = "";
        if (string.IsNullOrEmpty(text)) return true;
        try
        {
            using var doc = JsonDocument.Parse(text);
            if (doc.RootElement.ValueKind != JsonValueKind.Object) { why = "options are not an object"; return false; }
            foreach (var p in doc.RootElement.EnumerateObject())
            {
                if (p.Value.ValueKind == JsonValueKind.Null) continue;
                if (!MappedKeys.Contains(p.Name)) { why = $"option {p.Name}"; return false; }
                // Only values inside both engines' ranges stay here; the Rust engine
                // validates and reports everything else.
                switch (p.Name)
                {
                    case "audio_kbps" when p.Value.TryGetInt32(out var k) && k is >= 8 and <= 512:
                        options = options with { AudioBitrate = k }; break;
                    case "max_height" when p.Value.TryGetInt32(out var h) && h is >= 2 and <= 8192:
                        options = options with { Height = h }; break;
                    case "gif_fps" when p.Value.TryGetInt32(out var f) && f is >= 1 and <= 60:
                        options = options with { GifFrameRate = f }; break;
                    case "gif_max_width" when p.Value.TryGetInt32(out var w) && w is >= 16 and <= 8192:
                        options = options with { GifWidth = w }; break;
                    case "strip_metadata" when p.Value.ValueKind is JsonValueKind.True or JsonValueKind.False:
                        options = options with { PreserveMetadata = !p.Value.GetBoolean() }; break;
                    default:
                        why = $"option {p.Name} value"; return false;
                }
            }
            return true;
        }
        catch (JsonException)
        {
            why = "options are not JSON";
            return false;
        }
    }

    public static int Run(MediaPlan plan)
    {
        using var cancel = new CancellationTokenSource();
        void OnSignal(PosixSignalContext context) { context.Cancel = true; cancel.Cancel(); }
        using var term = PosixSignalRegistration.Create(PosixSignal.SIGTERM, OnSignal);
        using var interrupt = PosixSignalRegistration.Create(PosixSignal.SIGINT, OnSignal);

        var events = new EventWriter();
        var engine = new MediaEngine(plan.Ffmpeg, plan.Ffprobe);
        var outputs = new List<string>(); var failed = 0; var cancelled = false;
        events.Write(w => { w.WriteString("event", "start"); w.WriteString("job", Random.Shared.Next(0x10000).ToString("x4")); w.WriteNumber("total", plan.Inputs.Count); });

        if (plan.OutDir is { } outDir)
        {
            try { Directory.CreateDirectory(outDir); }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                events.Error(null, null, "io", "--out-dir: " + e.Message);
                events.Done(outputs, plan.Inputs.Count, false);
                return 1;
            }
        }

        for (var index = 0; index < plan.Inputs.Count; index++)
        {
            if (cancel.IsCancellationRequested) { cancelled = true; break; }
            var input = plan.Inputs[index];
            var destination = plan.OutDir ?? Path.GetDirectoryName(input)!;
            var stem = OutputNames.Stem(input);
            var stage = Path.Combine(destination, $".{stem}.{Guid.NewGuid():N}.nashi-tmp");
            try
            {
                var progress = new LineProgress(p => events.Progress(input, index, p));
                var made = engine.ConvertAsync([input], plan.Target, stage, plan.Options, progress, cancel.Token).GetAwaiter().GetResult();
                foreach (var file in made)
                {
                    var final = OutputNames.Publish(file, destination, stem, plan.Target);
                    outputs.Add(final);
                    events.Output(input, index, final);
                }
            }
            catch (OperationCanceledException) { cancelled = true; break; }
            catch (Exception e)
            {
                if (cancel.IsCancellationRequested) { cancelled = true; break; }
                failed++;
                events.Error(input, index, Code(e), e.Message);
            }
            finally
            {
                try { if (Directory.Exists(stage)) Directory.Delete(stage, recursive: true); }
                catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
            }
        }
        if (cancel.IsCancellationRequested) cancelled = true;
        events.Done(outputs, failed, cancelled);
        if (events.Broken) { Console.Error.WriteLine("nashi-cli-dotnet: cannot write to stdout"); return 1; }
        return cancelled ? 130 : failed == 0 ? 0 : 1;
    }

    private static string Code(Exception e) => e switch
    {
        FileNotFoundException or InvalidDataException => "invalid_input",
        ArgumentException => "invalid_options",
        UnauthorizedAccessException => "io",
        IOException => "tool_failed", // ProcessRunner reports a failed ffmpeg run as IOException
        _ => "tool_failed",
    };
}

/// Output names per CLI-PROTOCOL.md: `stem.ext`, then `stem (1).ext`, …, never
/// overwriting, built from at most the first 200 bytes of the input's name.
internal static class OutputNames
{
    public static string Stem(string input)
    {
        var stem = Path.GetFileNameWithoutExtension(input);
        if (Encoding.UTF8.GetByteCount(stem) <= 200) return stem;
        var cut = new StringBuilder();
        foreach (var rune in stem.EnumerateRunes())
        {
            if (Encoding.UTF8.GetByteCount(cut.ToString() + rune) > 200) break;
            cut.Append(rune.ToString());
        }
        return cut.ToString();
    }

    public static string Publish(string staged, string directory, string stem, string ext)
    {
        for (var n = 0; n < 100_000; n++)
        {
            var name = n == 0 ? $"{stem}.{ext}" : $"{stem} ({n}).{ext}";
            var final = Path.Combine(directory, name);
            if (File.Exists(final) || Directory.Exists(final) || File.Exists(Path.Combine(directory, $".{name}.nashi-tmp"))) continue;
            try
            {
                File.Move(staged, final, overwrite: false);
                return final;
            }
            catch (IOException) when (File.Exists(final))
            {
                // Another process took the name between the check and the move: try the next one.
            }
        }
        throw new IOException("Too many files already use this name.");
    }
}

/// Reports progress on the calling thread, in order (Progress<T> would post it).
internal sealed class LineProgress(Action<OperationProgress> report) : IProgress<OperationProgress>
{
    public void Report(OperationProgress value) => report(value);
}

/// One JSON object per line on stdout; nothing else goes there.
internal sealed class EventWriter
{
    private readonly Stream stdout = Console.OpenStandardOutput();
    private readonly Lock gate = new();
    public bool Broken { get; private set; }

    public void Write(Action<Utf8JsonWriter> body)
    {
        lock (gate)
        {
            if (Broken) return;
            try
            {
                using var buffer = new MemoryStream();
                using (var w = new Utf8JsonWriter(buffer, new JsonWriterOptions { Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }))
                {
                    w.WriteStartObject(); body(w); w.WriteEndObject();
                }
                buffer.WriteByte((byte)'\n');
                stdout.Write(buffer.GetBuffer(), 0, (int)buffer.Length);
                stdout.Flush();
            }
            catch (IOException) { Broken = true; }
        }
    }

    public void Progress(string input, int index, OperationProgress p) => Write(w =>
    {
        w.WriteString("event", "progress"); w.WriteString("input", input); w.WriteNumber("index", index);
        if (p.Fraction is { } f && double.IsFinite(f)) w.WriteNumber("fraction", Math.Clamp(f, 0, 1)); else w.WriteNull("fraction");
        w.WriteString("message", p.Label);
    });

    public void Output(string input, int index, string path) => Write(w =>
    {
        w.WriteString("event", "output"); w.WriteString("input", input); w.WriteNumber("index", index); w.WriteString("path", path);
    });

    public void Error(string? input, int? index, string code, string message) => Write(w =>
    {
        w.WriteString("event", "error");
        if (input is null) w.WriteNull("input"); else w.WriteString("input", input);
        if (index is { } i) w.WriteNumber("index", i); else w.WriteNull("index");
        w.WriteString("code", code); w.WriteString("message", message);
    });

    public void Done(IReadOnlyList<string> outputs, int failed, bool cancelled) => Write(w =>
    {
        w.WriteString("event", "done");
        w.WriteStartArray("outputs"); foreach (var o in outputs) w.WriteStringValue(o); w.WriteEndArray();
        w.WriteNumber("failed", failed); w.WriteBoolean("cancelled", cancelled);
    });
}
