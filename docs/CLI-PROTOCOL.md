# `nashi-cli` protocol (contract between UI and engine)

Stable contract. Changing it requires updating `ui/` and this file in the same change.

## Commands

| Command | Purpose |
|---|---|
| `nashi-cli catalog --json` | Full catalog (formats, targets, tools, actions, availability) |
| `nashi-cli wheel FILE... --json` | Items to show on the wheel for exactly these files |
| `nashi-cli inspect FILE... --json` | Per-file category, extension, size; media duration/size; pdf pages |
| `nashi-cli deps --json` | External tool availability, version, and Arch package name |
| `nashi-cli convert TARGET FILE... [--out-dir DIR] [--options JSON] --json` | Convert each file to `TARGET` extension |
| `nashi-cli tool TOOL_ID FILE... [--out-dir DIR] [--options JSON] --json` | Run a tool (single-input tools run per file; multi-input tools take all files) |

Without `--json` the CLI prints human-readable text. Exit codes: `0` all succeeded, `1` one or more inputs failed, `2` usage error, `3` required external tool missing, `130` cancelled (SIGINT/SIGTERM).

## `wheel` output

```json
{
  "files": [{"path": "/home/u/a.mp4", "ext": "mp4", "category": "video"}],
  "convert": [{"id": "mp3", "title": "MP3", "available": true}],
  "tools":   [{"id": "compress", "title": {"en": "Compress", "tr": "Sıkıştır"}, "icon": "compress", "available": true, "needs_editor": false}],
  "actions": [{"id": "copyPath", "title": {"en": "Copy Path", "tr": "Yolu Kopyala"}, "icon": "copy"}]
}
```

- `convert`: targets valid for **every** input (intersection). Empty when inputs mix categories with no common target.
- `tools`: for one file → `singleTools[ext]`; for several files → `multipleTools[ext]` when all share the ext, otherwise tools valid for all.
- `needs_editor: true` means the UI must collect options (crop rect, trim range…) before running; until editors exist the UI may hide such tools.
- `available: false` when a required external tool is missing; include `"missing": ["ffmpeg"]`.
- `actions` are handled by the UI itself. The CLI only lists them. Fixed ids: `copy` (put the files on the clipboard as `text/uri-list`), `copyPath` (paths as text), `terminal` (open a terminal in the file's folder), `openFolder`, `reveal` (show in the file manager, selected). There is no `move` action: Nashi never moves user files.
- `title` for formats is a plain string (the uppercase extension); for tools and actions it is `{"en","tr"}`.
- `icon` is a freedesktop icon-theme name (e.g. `document-export`, `edit-copy`, `utilities-terminal`); the UI may fall back to its own icons.

## `deps` output

```json
{"tools": [
  {"id": "ffmpeg", "available": true, "version": "7.1", "path": "/usr/bin/ffmpeg",
   "package": "ffmpeg", "optional": false, "used_for": ["audio", "video", "gif"]},
  {"id": "exiftool", "available": false, "version": null, "path": null,
   "package": "perl-image-exiftool", "optional": false, "used_for": ["removeMetadata"]}
]}
```

`package` is the Arch Linux package name. Tool ids: `ffmpeg`, `ffprobe`, `magick`, `soffice`, `exiftool`, `qpdf`, `pdftoppm`, `pdftotext`, `gs`, `7z`, `zbarimg`, `pandoc`, `rsvg-convert`, `yt-dlp`.

## `inspect` output

```json
{"files": [{"path": "/a.mp4", "ext": "mp4", "category": "video", "size": 1048576,
            "duration": 12.5, "width": 1920, "height": 1080, "pages": null}]}
```

Categories: `image`, `audio`, `video`, `gif`, `pdf`, `document`, `presentation`, `spreadsheet`, `text`, `subtitle`, `archive`, `unknown`. Unknown/missing fields are `null`.

## `catalog` output

```json
{"formats": [{"ext": "jpg", "category": "image", "title": "JPG", "targets": ["png", "..."],
              "single_tools": ["compress", "..."], "multiple_tools": ["compress", "..."]}],
 "tools": [{"id": "compress", "title": {"en": "Compress", "tr": "Sıkıştır"}, "icon": "...",
            "needs_editor": false, "multi_input": false, "available": true, "missing": []}],
 "actions": [{"id": "copyPath", "title": {"en": "Copy Path", "tr": "Yolu Kopyala"}, "icon": "edit-copy"}]}
```

Non-`--json` errors and usage errors: a JSON-mode usage error still prints one `{"event":"error","code":"invalid_options",...}` line plus `done` when it happens after `start`; before `start` it prints only to stderr and exits 2.

## Event stream (`convert` / `tool` with `--json`)

One JSON object per line on **stdout**; nothing else is printed to stdout. Diagnostics go to stderr.

```json
{"event":"start","job":"7f3c","total":2}
{"event":"progress","input":"/a.mp4","index":0,"fraction":0.42,"message":"Encoding"}
{"event":"output","input":"/a.mp4","index":0,"path":"/a.mp3"}
{"event":"error","input":"/b.mov","index":1,"code":"tool_failed","message":"ffmpeg exited with 1"}
{"event":"done","outputs":["/a.mp3"],"failed":1,"cancelled":false}
```

- `fraction` is 0..1 for the current input, or `null` when unknown (indeterminate).
- Multi-input tools (merge, join, collage, createPDF) use `index: null` and `input: null` for the combined output.
- Error codes: `unsupported_route`, `missing_dependency`, `tool_failed`, `invalid_input`, `invalid_options`, `io`, `cancelled`.
- On SIGTERM/SIGINT: kill child processes, delete temp files, emit `done` with `"cancelled": true`, exit 130.

## Output naming

- Default directory: the input's directory. `--out-dir` overrides (created if missing).
- Name: `stem.target` → if taken, `stem (1).target`, `stem (2).target`, … Never overwrite.
- Tools keep the extension and add a suffix: `stem-compressed.ext`, `stem-clean.ext` (metadata), `stem-muted.mp4`, `stem-normalized.mp3`; multi-output tools create a folder `stem-pages/`, `stem-snapshots/`, `stem-extracted/`; combined outputs: `merged.pdf`, `joined.mp4`, `collage.png`, `images.pdf` in the first input's directory.
- Write to `.<name>.nashi-tmp` in the destination directory, then rename atomically.

## `--options` JSON

Optional; unknown keys are ignored. Defaults are sensible so every route works without options.

```json
{"quality": 82, "crf": 26, "pdf_preset": "ebook", "start": 0.0, "end": null,
 "speed": 1.0, "rect": {"x":0,"y":0,"w":100,"h":100}, "pages": "1-3,5",
 "channels": "mono", "fps": 1, "segment_seconds": 60, "rotate": 0, "flip": null,
 "resize": {"w": null, "h": null}, "border": {"px": 40, "color": "#ffffff"},
 "rects": [{"x":0,"y":0,"w":10,"h":10,"t0":null,"t1":null}], "ranges": [[1.0,2.5]]}
```

## Additions (v0.1)

- `output` events may carry an optional `"message"` string (e.g. `readQRCode` puts the decoded text there; the text is also written to `stem-qr.txt`).
- `deps` also lists `libheif` (HEIC support inside ImageMagick) and may include `"alt_packages": ["libreoffice-still"]`. `missing` lists may name `libheif`.
- `catalog` formats keep the full upstream `targets` and add `unsupported_targets` (routes that cannot work on Linux, e.g. `→ pages`, `pdf → xlsx`). `wheel` never lists those.
- `readQRCode` is a Linux addition to `single_tools` for raster images.
- Extra output suffixes: `stem-parts/` (splitVideo), `stem-qr.txt`, `stem-<speed>x.ext`, `stem-trimmed|-cropped|-edited|-framed|-redacted|-bleeped|-organized.ext`, `stem-visualizer.mp4`, `stem-mono|-stereo.ext`, and `stem.tar.gz` when the target is `gz`.
- Tools with `needs_editor: true` return `invalid_options` when called without their options.
- Testing: `NASHI_DISABLE_TOOLS=a,b` makes those tools appear missing. Child processes run with `LC_ALL=C.UTF-8` (ImageMagick breaks under `tr_TR`).
