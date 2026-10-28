# Architecture

```
            evdev (/dev/input/event*)
                   │  (user in `input` group)
          ┌────────▼─────────┐   session D-Bus signals
          │  nashi-inputd     │ ─────────────────────────────┐
          │  Rust, zbus       │  ChordStarted(mode) / ...    │
          └───────────────────┘                              │
                                                             ▼
┌───────────────────────────────────────────────────────────────────────┐
│ nashi  (Qt 6 / QML app, single instance, tray)                        │
│  ├─ Overlay: full-screen transparent LayerShellQt surface (Overlay    │
│  │   layer, keyboard none, anchors all) shown only while a chord is   │
│  │   active; receives the drag, draws the wheel at the drag position  │
│  ├─ Wheel (QML): stage 1 = drop target, stage 2 = type-specific items │
│  ├─ JobRunner: spawns `nashi-cli … --json`, parses JSON lines         │
│  ├─ Toasts, Home window (drop zone, recent outputs, settings, deps)   │
│  └─ D-Bus service io.github.beratkaanakcan.Nashi (ShowWheel, …)       │
└───────────────┬───────────────────────────────────────────────────────┘
                │ QProcess, argv only (no shell)
       ┌────────▼─────────┐
       │  nashi-cli        │  Rust; JSON-lines events on stdout
       │  └─ nashi-core    │  catalog, detection, planning, execution
       └────────┬─────────┘
                │ std::process::Command (no shell)
   ffmpeg/ffprobe · magick|convert · soffice · exiftool · qpdf · pdftoppm/pdftotext · gs · 7z · (pandoc, yt-dlp, rsvg-convert)
```

## Repository layout

```
crates/
  nashi-core/     library: catalog, detect, deps, jobs, engines (image, media, document, pdf, archive, subtitle, qr, metadata)
  nashi-cli/      binary `nashi-cli`
  nashi-inputd/   binary `nashi-inputd`
ui/               CMake + Qt6/QML app `nashi`
packaging/        arch/PKGBUILD, install.sh, udev, systemd, dolphin service menu, nautilus script
data/             .desktop, icons, metainfo
docs/             PLAN, ARCHITECTURE, CLI-PROTOCOL, DBUS
reference/        upstream Nashi FeatureCatalog.json (MIT) used as the source of the catalog
```

## Principles

- **Never destroy user data.** Outputs go next to the input with a unique name (`name (1).ext`), written to a temp file in the same directory then renamed. Inputs are never modified or deleted. Drops are accepted as CopyAction only.
- **No shell.** Every external tool is invoked with an argv array. File names starting with `-` are passed as `./-name` or after `--` where supported.
- **Local only.** No network except the explicit web-download tool (yt-dlp, later phase).
- **Graceful degradation.** Missing tools never crash; routes needing them are reported as unavailable with the package name to install (`nashi-cli deps`).
- **i18n.** UI strings are English source with Turkish translation (Qt `.ts`). Catalog titles have `en` and `tr`.
- **Security of inputd.** It reads raw input devices but emits only chord state (mode, drag started/ended). It never logs or exports key codes beyond modifiers + primary button.
