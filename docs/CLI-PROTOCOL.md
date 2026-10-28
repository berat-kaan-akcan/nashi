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
- `actions` are handled by the UI itself (copy, move, copy path, open terminal, open folder, show in Dolphin). The CLI only lists them.

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
