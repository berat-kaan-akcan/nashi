# C# engine prototype (`nashi-cli-dotnet`)

An experiment: can the Linux app run on upstream Nashi's shared C# engine
(`Nashi.Core`, from `vendor/nashi-app`, MIT) instead of only the Rust one,
without changing the Qt UI?

`nashi-cli-dotnet` speaks the same contract as `nashi-cli`
([CLI-PROTOCOL.md](../docs/CLI-PROTOCOL.md)):

- `convert TARGET FILE... --json` for audio and video routes (video → audio or
  video, audio → audio) runs on `Nashi.Core`'s `MediaEngine`. Output names,
  the `--out-dir` rules, the event stream, exit codes and SIGTERM cancellation
  follow the contract, not upstream's own conventions (upstream names a
  second copy `stem 2.ext`; here it is `stem (1).ext`).
- Every other command line is passed to the Rust `nashi-cli` with `execv`, so
  its output and signals are untouched. That includes conversions with options
  this prototype does not map (`crf`, `sample_depth`, `audio_compression`,
  `gif_compression`, …), values outside either engine's range, other routes,
  and a missing `ffmpeg`/`ffprobe` (the Rust CLI reports it).

Mapped options: `audio_kbps`, `max_height`, `gif_fps`, `gif_max_width`,
`strip_metadata`. GIF defaults follow the contract (12 fps, 640 px).
Without `audio_kbps`, lossy audio follows the contract defaults (192 kbit/s
AAC and WMA, 128 kbit/s Opus) instead of Nashi.Core's 256, so sizes match the
Rust engine. MP3 without `audio_kbps` goes to the Rust engine, which encodes
VBR (`-q:a 2`); Nashi.Core only has constant bitrate.

## Build and try

```sh
git submodule update --init vendor/nashi-app
cargo build --release -p nashi-cli
dotnet build dotnet/Nashi.Cli -c Release

export NASHI_RUST_CLI=$PWD/target/release/nashi-cli
NASHI_CLI=$PWD/dotnet/Nashi.Cli/bin/Release/net10.0/nashi-cli-dotnet build/ui/nashi
```

`NASHI_DOTNET_TRACE=1` prints on stderr why a command was handed to Rust.
Without `NASHI_RUST_CLI` the Rust CLI is looked up next to the executable,
then on `PATH`.

## Verify

```sh
NASHI_RUST_CLI=$PWD/target/release/nashi-cli dotnet run --project dotnet/Nashi.Cli.Verify -c Release
```

Covers every route kept on `Nashi.Core` (checked with ffprobe), naming, staging
cleanup, a new `--out-dir`, the event stream, SIGTERM and the hand-over to Rust.
Checks are skipped, not failed, without `ffmpeg` or `NASHI_RUST_CLI`.
