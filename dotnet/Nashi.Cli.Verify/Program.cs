using System.Diagnostics;
using System.Runtime.Versioning;
using System.Text.Json;

[assembly: SupportedOSPlatform("linux")]

// Contract checks for the prototype: every media route the C# engine answers,
// output naming, staging cleanup, cancellation and hand-over to the Rust CLI.
var cli = Path.Combine(AppContext.BaseDirectory, "nashi-cli-dotnet");
var passed = 0; var skipped = 0; var failures = new List<string>();
var root = Directory.CreateTempSubdirectory("nashi-cli-verify-").FullName;
try
{
    var ffmpeg = OnPath("ffmpeg"); var ffprobe = OnPath("ffprobe");
    if (ffmpeg is null || ffprobe is null)
    {
        Console.WriteLine("SKIP all media routes: ffmpeg/ffprobe not installed");
        return 0;
    }
    var clip = Path.Combine(root, "clip.mp4"); var tone = Path.Combine(root, "tone.wav");
    Run(ffmpeg, "-v", "error", "-f", "lavfi", "-i", "testsrc=size=320x180:rate=25:duration=2", "-f", "lavfi", "-i", "sine=frequency=440:duration=2", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", clip);
    Run(ffmpeg, "-v", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=2", tone);

    // Every route this prototype keeps on Nashi.Core: video → audio/video, audio → audio.
    string[] video = ["mp4", "mov", "mkv", "webm", "avi", "wmv", "gif", "m4v"];
    string[] audio = ["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma", "aac"];
    foreach (var target in video.Concat(audio).Where(t => t != "mp4")) Route(clip, target);
    foreach (var target in audio.Where(t => t != "wav")) Route(tone, target);

    Check("second conversion gets 'stem (1)'", () =>
    {
        var dir = Fresh("collide");
        var first = Outputs(Convert(dir, "mp3", tone)).Single();
        var second = Outputs(Convert(dir, "mp3", tone)).Single();
        Expect(Path.GetFileName(first) == "tone.mp3" && Path.GetFileName(second) == "tone (1).mp3", $"{first} / {second}");
    });
    Check("no staging left and the input is unchanged", () =>
    {
        var dir = Fresh("clean"); var before = File.ReadAllBytes(tone);
        Convert(dir, "flac", tone);
        Expect(!Directory.EnumerateFileSystemEntries(dir).Any(p => Path.GetFileName(p).StartsWith('.')), "hidden leftovers");
        Expect(File.ReadAllBytes(tone).SequenceEqual(before), "input changed");
    });
    Check("a missing --out-dir is created", () =>
    {
        var dir = Path.Combine(Fresh("nested"), "a", "b");
        Expect(Outputs(Convert(dir, "mp3", tone)).Single().StartsWith(dir + "/"), "wrong folder");
    });
    Check("event stream: start, progress 0..1, output, done", () =>
    {
        var events = Convert(Fresh("events"), "m4a", tone);
        Expect(events[0].GetProperty("event").GetString() == "start" && events[0].GetProperty("total").GetInt32() == 1, "start");
        Expect(events.Where(e => e.GetProperty("event").GetString() == "progress").All(e => e.GetProperty("fraction").ValueKind == JsonValueKind.Null || e.GetProperty("fraction").GetDouble() is >= 0 and <= 1), "fraction range");
        var done = events[^1];
        Expect(done.GetProperty("event").GetString() == "done" && done.GetProperty("failed").GetInt32() == 0 && !done.GetProperty("cancelled").GetBoolean(), "done");
    });
    Check("SIGTERM cancels: exit 130, done cancelled, no partial file", () =>
    {
        var dir = Fresh("cancel"); var longClip = Path.Combine(root, "long.mp4");
        Run(ffmpeg, "-v", "error", "-f", "lavfi", "-i", "testsrc=size=1280x720:rate=30:duration=60", "-c:v", "libx264", "-preset", "ultrafast", longClip);
        using var p = Start(["convert", "webm", longClip, "--out-dir", dir, "--json"]);
        var started = p.StandardOutput.ReadLine();
        Expect(started?.Contains("\"start\"") == true, "no start event");
        Thread.Sleep(1500);
        Process.Start("kill", ["-TERM", p.Id.ToString()])!.WaitForExit();
        var rest = p.StandardOutput.ReadToEnd(); p.WaitForExit();
        Expect(p.ExitCode == 130, $"exit {p.ExitCode}");
        Expect(rest.TrimEnd().Split('\n')[^1].Contains("\"cancelled\":true"), "done not cancelled");
        Expect(!Directory.EnumerateFileSystemEntries(dir).Any(), "files left behind");
    });
    if (Environment.GetEnvironmentVariable("NASHI_RUST_CLI") is { Length: > 0 } rust && File.Exists(rust))
    {
        Check("unmapped options and routes go to the Rust engine", () =>
        {
            var dir = Fresh("handover");
            foreach (var (target, input, options) in new[] { ("mp3", tone, "{\"crf\":20}"), ("mp3", tone, "{\"audio_kbps\":9999}"), ("mp4", tone, "{}"), ("png", clip, "{}") })
            {
                var err = CaptureError(cli, ["convert", target, input, "--out-dir", dir, "--options", options, "--json"]);
                Expect(err.Contains("handing over to the Rust engine"), $"{target} {options} stayed on Nashi.Core");
            }
        });
        Check("non-media commands match the Rust engine", () =>
        {
            Expect(Capture(cli, ["wheel", clip, "--json"]) == Capture(rust, ["wheel", clip, "--json"]), "wheel differs");
        });
    }
    else
    {
        Console.WriteLine("SKIP hand-over checks: set NASHI_RUST_CLI to the Rust nashi-cli");
        skipped += 2;
    }
}
finally
{
    Directory.Delete(root, recursive: true);
}
foreach (var f in failures) Console.WriteLine("FAIL " + f);
Console.WriteLine(JsonSerializer.Serialize(new { passed, skipped, failed = failures.Count }));
return failures.Count == 0 ? 0 : 1;

void Route(string input, string target) => Check($"{Path.GetExtension(input)[1..]} → {target}", () =>
{
    var events = Convert(Fresh("route-" + target + Path.GetExtension(input)), target, input);
    var output = Outputs(events).Single();
    Expect(Path.GetExtension(output) == "." + target, "extension");
    var probe = Capture(OnPath("ffprobe")!, ["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1", output]);
    Expect(double.TryParse(probe.Trim(), System.Globalization.CultureInfo.InvariantCulture, out var seconds) && Math.Abs(seconds - 2) < .5, $"duration {probe.Trim()}");
});

void Check(string name, Action body)
{
    if (Environment.GetEnvironmentVariable("NASHI_DISABLE_TOOLS")?.Contains("ffmpeg") == true) { skipped++; return; }
    try { body(); passed++; Console.WriteLine("ok   " + name); }
    catch (Exception e) { failures.Add($"{name}: {e.Message}"); }
}

string Fresh(string name) { var d = Path.Combine(root, "out-" + name); Directory.CreateDirectory(d); return d; }

List<JsonElement> Convert(string outDir, string target, string input, params string[] extra)
{
    var text = Capture(cli, ["convert", target, input, "--out-dir", outDir, .. extra, "--json"]);
    return text.Split('\n', StringSplitOptions.RemoveEmptyEntries).Select(l => JsonDocument.Parse(l).RootElement.Clone()).ToList();
}

static IEnumerable<string> Outputs(List<JsonElement> events) =>
    events.Where(e => e.GetProperty("event").GetString() == "output").Select(e => e.GetProperty("path").GetString()!).ToList() is { Count: > 0 } list
        ? list
        : throw new Exception("no output: " + string.Join(" ", events.Select(e => e.GetRawText())));

static void Expect(bool condition, string message) { if (!condition) throw new Exception(message); }

Process Start(string[] args)
{
    var start = new ProcessStartInfo(cli) { RedirectStandardOutput = true, RedirectStandardError = true, UseShellExecute = false };
    foreach (var a in args) start.ArgumentList.Add(a);
    var p = Process.Start(start)!;
    _ = p.StandardError.ReadToEndAsync();
    return p;
}

static string Capture(string exe, string[] args)
{
    var start = new ProcessStartInfo(exe) { RedirectStandardOutput = true, RedirectStandardError = true, UseShellExecute = false };
    foreach (var a in args) start.ArgumentList.Add(a);
    using var p = Process.Start(start)!;
    var err = p.StandardError.ReadToEndAsync();
    var output = p.StandardOutput.ReadToEnd(); p.WaitForExit(); _ = err.Result;
    return output;
}

static string CaptureError(string exe, string[] args)
{
    var start = new ProcessStartInfo(exe) { RedirectStandardOutput = true, RedirectStandardError = true, UseShellExecute = false };
    start.Environment["NASHI_DOTNET_TRACE"] = "1";
    foreach (var a in args) start.ArgumentList.Add(a);
    using var p = Process.Start(start)!;
    var output = p.StandardOutput.ReadToEndAsync();
    var error = p.StandardError.ReadToEnd(); p.WaitForExit(); _ = output.Result;
    return error;
}

static void Run(string exe, params string[] args)
{
    var start = new ProcessStartInfo(exe) { UseShellExecute = false };
    foreach (var a in args) start.ArgumentList.Add(a);
    using var p = Process.Start(start)!; p.WaitForExit();
    if (p.ExitCode != 0) throw new Exception($"{exe} failed while making fixtures");
}

static string? OnPath(string name) =>
    (Environment.GetEnvironmentVariable("PATH") ?? "").Split(':', StringSplitOptions.RemoveEmptyEntries)
        .Select(d => Path.Combine(d, name)).FirstOrDefault(File.Exists);
