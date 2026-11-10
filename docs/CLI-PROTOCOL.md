# `nashi-cli` protocol (contract between UI and engine)

Stable contract. Changing it requires updating `ui/` and this file in the same change.

## Commands

| Command | Purpose |
|---|---|
| `nashi-cli catalog --json` | Full catalog (formats, targets, tools, actions, availability) |
| `nashi-cli wheel FILE... --json` | Items to show on the wheel for exactly these files |
| `nashi-cli inspect FILE... --json` | Per-file category, extension, size; media duration/size; pdf pages |
| `nashi-cli deps --json` | External tool availability, version, and Arch package name |
| `nashi-cli convert TARGET FILE... [--out-dir DIR] [--options JSON] [--password-stdin] --json` | Convert each file to `TARGET` extension |
| `nashi-cli tool TOOL_ID FILE... [--out-dir DIR] [--options JSON] [--password-stdin] --json` | Run a tool (single-input tools run per file; multi-input tools take all files) |
| `nashi-cli preview FILE --kind frame\|waveform\|pages --out-dir DIR [...] --json` | Render editor preview PNGs (frame, waveform, PDF pages) into `DIR` |
| `nashi-cli suggest FILE [--at SEC] [--kinds LIST] [--lang LANGS] --json` | Find places in a picture or video frame that look like personal data (OCR) |
| `nashi-cli download URL [--kind auto\|video\|audio\|thumbnail\|image] [--out-dir DIR] --json` | Download an `http`/`https` URL via `yt-dlp` or `curl` |

Without `--json` the CLI prints human-readable text. Exit codes: `0` all succeeded, `1` one or more inputs failed, `2` usage error, `3` required external tool missing, `130` cancelled (SIGINT/SIGTERM).

## `wheel` output

```json
{
  "files": [{"path": "/home/u/a.mp4", "ext": "mp4", "category": "video"}],
  "convert": [{"id": "mp3", "title": "MP3", "available": true}],
  "tools":   [{"id": "compress", "title": {"en": "Compress", "tr": "Sıkıştır", "fr": "Compresser"}, "icon": "compress", "available": true, "needs_editor": false}],
  "actions": [{"id": "copyPath", "title": {"en": "Copy Path", "tr": "Yolu Kopyala", "fr": "Copier le chemin"}, "icon": "copy"}]
}
```

- `convert`: targets valid for **every** input (intersection). Empty when inputs mix categories with no common target.
- `tools`: for one file → `singleTools[ext]`; for several files → `multipleTools[ext]` when all share the ext, otherwise tools valid for all.
- `needs_editor: true` means the UI must collect options (crop rect, trim range…) before running. The UI shows such tools when it has an options form for them (`OptionsWindow.qml`: speed, trim, crop, edit, frame, redact, organizePDF) and hides the rest (`annotateImage`) until a visual editor exists.
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

`package` is the Arch Linux package name. Tool ids: `ffmpeg`, `ffprobe`, `magick`, `soffice`, `exiftool`, `qpdf`, `pdftoppm`, `pdftotext`, `gs`, `7z`, `zbarimg`, `tesseract`, `opencv` (Python's OpenCV, run through `python3`), `pandoc`, `rsvg-convert`, `yt-dlp`, `curl`.

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
 "tools": [{"id": "compress", "title": {"en": "Compress", "tr": "Sıkıştır", "fr": "Compresser"}, "icon": "...",
            "needs_editor": false, "multi_input": false, "available": true, "missing": []}],
 "actions": [{"id": "copyPath", "title": {"en": "Copy Path", "tr": "Yolu Kopyala", "fr": "Copier le chemin"}, "icon": "edit-copy"}]}
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
- Error codes: `unsupported_route`, `missing_dependency`, `tool_failed`, `invalid_input`, `invalid_options`, `io`, `cancelled`, `password_required`.
- A PDF with no extractable text (a scan) converted to `txt`, `md` or `epub` is `invalid_input`; no output file is produced.
- On SIGTERM/SIGINT: kill child processes, delete temp files, emit `done` with `"cancelled": true`, exit 130.

## Passwords (archives and PDFs)

- `password_required` means an archive needs a password (`This archive is password protected.`) or the one given did not open it (`The password is incorrect, or the archive is damaged.`; encrypted data cannot tell the two apart). No output is produced. It applies to `extractArchive` and to converting an archive to another archive format.
- A PDF that needs a password to open gives `password_required` too (`This PDF is password protected.` / `The password is incorrect.`) for every conversion and tool, merge included (one password for the whole job). With the right password, `qpdf` decrypts it into a folder only this user can open (mode 0700, in `$XDG_RUNTIME_DIR` when set, which is memory-backed) and the job runs on that copy, which is deleted when it ends. Outputs are named after the original and are **not** password protected. A PDF with only an owner password (printing or editing limits) opens without one. Decrypting needs `qpdf` (`missing_dependency` otherwise). `preview` does not take a password, so editors cannot show the pages of a protected PDF.
- `--password-stdin` reads the password from the first line of stdin (without its `\n` or `\r\n`, at most 4096 bytes, no NUL byte; otherwise exit 2). An empty line means no password. Without the flag stdin is never read.
- The password is never accepted on the command line: `--options` is an argument, which any user can read in `/proc/PID/cmdline`, so a `password` key there is ignored. The engine passes it to `7z`, `bsdtar` or `qpdf` (`--password-file=-`) on their stdin, in a session of their own (no controlling terminal, so they cannot prompt there), never as `-p`. `unzip` only takes `-P PASSWORD`, so a password with only `unzip` installed returns `missing_dependency` (`7zip`).
- The engine keeps the password out of debug output and overwrites it in memory when done (best effort). The app writes it to nashi-cli through a pipe of its own, not through Qt's buffers, and clears the text field once it is sent.

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

## Additions (v0.2 — editors, download)

### Orientation

`cropImage`, `redactImage`, `annotateImage`, `frameImage` and `editImage` work on the **auto-oriented** image (`-auto-orient` first). `inspect` reports image `width`/`height` after EXIF orientation and video `width`/`height` after rotation metadata (what ffmpeg filters see), so editor coordinates always match what the user sees.

### `preview` command (editor/preview helpers)

```
nashi-cli preview FILE --kind frame|waveform|pages --out-dir DIR
          [--at SEC] [--width PX] [--height PX] [--pages SPEC] [--color #RRGGBB] --json
```

Writes PNGs into `DIR` (created if missing; the caller owns and deletes it) and prints **one** JSON object:

```json
{"kind": "pages", "files": ["/DIR/page-001.png", "/DIR/page-002.png"], "width": 240, "height": 339}
```

- `frame`: images (any readable format, incl. HEIC/SVG/TIFF, first frame, auto-oriented) or video/gif (frame at `--at`, default 0, clamped to the duration). Scaled to fit `--width` (default 1600), never upscaled. `width`/`height` are those of the PNG.
- `waveform`: audio or video → one transparent PNG `--width`×`--height` (defaults 1200×160), mono waveform in `--color` (default `#30543b`).
- `pages`: PDF → one PNG per page, `--width` default 240, `--pages` limits the set (same syntax as the `pages` option). `files` is in page order; `width`/`height` are those of the first PNG.
- Errors: prints `{"error": {"code": "invalid_input", "message": "..."}}` (codes as in the event stream) and exits 1 (3 for a missing tool, 2 for usage).

### `annotateImage` options

`{"overlay": "/abs/path/overlay.png"}` — a PNG with alpha drawn by the UI at the pixel size of the auto-oriented input; it is composited over the input (scaled to the input size if it differs). The path must be an existing regular `.png` file and must not be the input itself. Output: `stem-annotated.ext`. The UI writes overlays to `~/.cache/nashi/annotate/` and deletes them after the job.

### `organizePDF` options

`{"pages": "3,1,2", "rotations": [0, 90, 270]}` — `rotations` (optional) is parallel to the **expanded** output page list (one entry per output page), degrees clockwise, each a multiple of 90 (0/90/180/270; negative values are normalised). Length must match the page count of the result, otherwise `invalid_options`.

### `redactImage` / `redactVideo` styles

`rects[].style` (optional): `"black"` (default), `"blur"` or `"pixelate"`.

### `suggest` command

```
nashi-cli suggest FILE [--at SEC] [--kinds LIST] [--lang LANGS] --json
```

Reads the text of a picture, or of one video frame (`--at`, seconds), with `tesseract` and returns the places that look like personal data; it also finds faces with OpenCV. It changes nothing and writes nothing next to `FILE`.

```json
{"width": 1000, "height": 400, "language": "tur+eng",
 "rects": [{"x": 284, "y": 34, "w": 330, "h": 36, "kind": "email"}]}
```

- `width`/`height` are the size of the picture the rectangles refer to, after orientation (the same pixels `redactImage`/`redactVideo` use). Video frames are read at full size.
- `kind`: `iban` (checksum), `email`, `url` (`http://`, `https://`, `www.`), `card` (13–19 digits, Luhn), `id` (11-digit Turkish identity number, checksums), `phone` (10–15 digits, also when split into groups), `face` (front-facing faces, box widened by about 12 % each side). `--kinds` takes a comma-separated subset; anything else returns `invalid_options`. Without `--kinds` it looks for everything the installed tools can find (text kinds need `tesseract`, `face` needs OpenCV); with `--kinds` every tool it names must be installed.
- `--lang` takes tesseract languages joined with `+` (default `tur+eng`). Languages that are not installed are left out; when none is installed an explicit `--lang` returns `invalid_options`, and the default falls back to whatever is installed.
- The matched text never appears in the output, only its kind and place. OCR can miss text or misread it, so the result is a suggestion for the user to check, not a guarantee.
- Pictures, GIFs and videos only; other files return `invalid_input`. Missing `tesseract` or OpenCV (`python-opencv`) when needed → `missing_dependency` (exit 3). `deps` lists `opencv` as available when `python3` can import `cv2`; this is only checked when asked, because importing it takes about 0.2 s. Errors are printed as `{"error": {"code", "message"}}`, exit 1.

### `download` command

```
nashi-cli download URL [--kind auto|video|audio|thumbnail|image] [--out-dir DIR] --json
```

- Only `http`/`https` URLs (else `invalid_input`); the URL is passed after `--` and never interpreted as an option.
- Default `--out-dir`: the XDG download directory (`$XDG_DOWNLOAD_DIR` / `user-dirs.dirs`, else `~/Downloads`).
- `auto`: a URL whose path ends in a known image/video/audio/document extension is fetched directly (like `image`); everything else goes through `yt-dlp` as `video`.
- `video` (yt-dlp, best video+audio merged to mp4 when possible), `audio` (yt-dlp `-x --audio-format mp3`), `thumbnail` (yt-dlp, thumbnail only, converted to png/jpg), `image` (direct file via `curl`, http/https only, also for redirects, size limit 2 GiB).
- yt-dlp runs with `--ignore-config --no-playlist --newline`; downloads go to a temp dir inside `DIR` and are renamed into place with a unique name (never overwrite). Title-based names are sanitised (no `/`, no leading `-`/`.`, ≤ 150 bytes).
- Emits the normal event stream (`start` total 1, `progress` with `fraction` from yt-dlp/curl, `output`, `error`, `done`), `input` is the URL, `index` 0. Cancel: SIGTERM as for jobs.
- Missing `yt-dlp` / `curl` → `missing_dependency` (exit 3). `deps` lists `curl`.

## Additions (v0.3 — upstream sync)

- Exit code 1 also means stdout could not be written (for example a full disk); stderr says so.
- `removeMetadata` on a PDF also removes annotation authors and dates, keeping form field names. Without `qpdf` the `output` event carries a `message` saying they may remain.
- `compress` on a PDF returns a copy of the input with a `message` when the rewrite would not be smaller.
- `inspect` measures the duration of MP3 files without a length header instead of reporting ffprobe's bitrate guess.
- Output names are built from at most the first 200 bytes of the input's name, so suffixes and `" (n)"` still fit the 255-byte limit.
- `trimAudio` options: `trim_silence` (bool) also removes silence from both ends of the selection; `silence_threshold_db` (-120..0, default -50) sets what counts as silence.
- `redactAudio` option `audio_mode`: `mute` (default) or `bleep` (a 1 kHz tone). Other values return `invalid_options`.
- `dpi` (36..1200, default 150) sets the resolution of PDF and document → `jpg`/`png` page images.
- `actions` gains the fixed ids `share` and `float`.
- `compress` options: `target_bytes` (positive) or `percentage` (0 < p < 100, of the file a symlink points to) ask for a size; an unreachable one returns `invalid_options` and no file, and a file already small enough is copied with a `message`. `target_bytes` wins when both are given. Images also take `max_edge` (1..32000) and `image_scale` (0 < s <= 1), which only shrink, and `resize`. Video takes `max_height` (2..16384), which only shrinks. Lossy audio takes `audio_kbps` (8..512). A size target on `wav`, `flac` or `aiff` writes `.m4a`, since those have no bitrate to lower.
- `splitVideo` options: `split_points` (seconds, any order, at least 0.05 s apart and from either end) or `equal_clips` (2..10000) choose the cuts, and re-encode so they land exactly. `discard_clips` (zero-based) leaves clips out, and the rest keep their numbers in `stem-parts/stem-NNN.ext`. `join_clips: true` exports the kept clips as one `stem-split.ext` instead. With none of these, `segment_seconds` alone still stream-copies. Impossible values return `invalid_options` and no file.
- `videoSnapshots` option `snapshot_times` (seconds) saves one PNG per time as `stem-frame-N.png`, each with its own `output` event. A time at the very end saves the last frame. `fps` alone still writes JPEGs into `stem-snapshots/`.
- `nashi-cli metadata FILE --json` prints `{"fields": [{"key": "XMP-dc:Title", "value": "…", "editable": true}]}`, sorted by key. Images, GIFs and PDFs use exiftool keys (`Group:Tag`) and leave out facts about the file itself. Audio and video use ffmpeg tag names. Errors print `{"error": {"code", "message"}}` and exit 1, or 3 for a missing tool.
- `removeMetadata` options `metadata_set` (`{"key": "value"}`) and `metadata_remove` (`["key"]`) change only those fields, write `stem-metadata.ext`, and copy audio and video streams without re-encoding. Keys that cannot be written, or that hold anything but letters, digits, `_`, `-`, `.` and (for exiftool) `:`, return `invalid_options` and no file. Without these options the tool still removes everything into `stem-clean.ext`.
- `editImage` option `adjust`: `exposure` (-5..5 stops), `brightness`, `contrast`, `saturation`, `hue`, `temperature`, `tint`, `vibrance`, `highlights`, `shadows`, `whites`, `blacks`, `clarity`, `texture`, `dehaze`, `vignette` (-100..100), `gamma` (0.1..5), `sharpen`, `blur`, `grain`, `fade`, `noise_reduction` (0..100), `filter` (`grayscale` or `sepia`). Out-of-range values return `invalid_options`. Neutral values count as no change. Transparency is kept.
- Direct and site downloads are limited to 20 GB each (matching upstream).
- `preview --kind frame --adjust JSON` applies the same object to an image preview.
- `frameImage` option `frame`: `backdrop` (`solid`, `gradient`, `image`), `color`, `color2`, `color3` (optional end colour; `color2` is then the middle), `angle`, `background_image`, `background_blur` (0..100), `padding` (px, default 40), `corner_radius`, `shadow` (0..200), `shadow_opacity` (0..100, default 30), `aspect` (0.1..10), `image_scale` (0 < s <= 1), `position` (`{"x","y"}`, 0..1). Invalid values return `invalid_options`. `border` alone still works as before.
- `createCollage` option `collage`: `layout` (`grid`, `row`, `column`, `featured`), `columns`, `width` (default 2048), `aspect` (cell width over height, default 1), `spacing`, `padding`, `corner_radius`, `color`, `fit` (`fill` or `fit`), `order` (the inputs' indices in display order). Invalid values return `invalid_options`. Without this option the tool still builds its simple grid.
- Archives may expand to at most 20 GB, counting hard links as full copies; past that the job returns `invalid_input` and no output. Testing: `NASHI_ARCHIVE_LIMIT_BYTES` lowers the limit.
- Conversion options, per target: images take `quality` (`jpg`, `webp`, `heic`, `avif`). Audio and video targets take `audio_kbps` (8..512), ignored by lossless and PCM codecs. Video targets take `crf` (mapped to a fixed quantizer for `avi` and `wmv`) and `max_height` (2..16384), which only shrinks. `gif` takes `gif_fps` (1..60, default 12), `gif_max_width` (2..16384, default 640), which only shrinks, and `gif_compression` (`fast`, `balanced` (default), `maximum` for the smallest file). Out-of-range values return `invalid_options`.
- `strip_metadata` (bool) on a conversion of an image, audio or video leaves out tags, chapters and album art. On images it uses `magick -strip`, which also drops the colour profile. Other targets ignore it; use `removeMetadata` for PDFs.
- `splitPDF` option `pages_per_file` (at least 1, default 1) writes parts named `stem-first-last.pdf`, or `stem-page.pdf` for a part of one page. It applies to the pages chosen by `pages`.
- `mergePDF` and `organizePDF` option `uniform_width` (bool) scales every page to the width of the narrowest as shown, keeping its shape. It needs `gs` and `pdfinfo`. Pages that are already that wide are copied as they are; Ghostscript rewrites the others: links keep working and are scaled, but form fields on those pages stop being fillable (they still show). When that happens the `output` event carries a `message` saying so.
- `normalizeAudio` options: `target_lufs` (-70..-5, default -16), `loudness_range` (1..50, default 11), `true_peak` (-9..0, default -1.5).
- `wav` and `aiff` outputs of conversions and audio tools keep the source's sample depth (16, 24 or 32-bit integer, or float). Conversion option `sample_depth` (`16`, `24`, `32`, `float32`) chooses another. `audio_compression` (`fast`, `balanced`, `maximum`) sets the FLAC compression level. Other values return `invalid_options`.
- `mergePDF` and `joinVideos` use the files in the order given on the command line.
- PDF to `epub` or `docx` option `pdf_layout`: `editable` (default) or `pages`, which writes each page as a JPEG in a fixed-layout EPUB or a DOCX section of the page's size. It needs `pdftoppm` and `pdfinfo`, works for PDFs with no text, and uses `dpi` (default 150). A page over 100 megapixels, a file over 20 gigapixels or pictures over 2 GB return `invalid_input`.
- `normalizeAudio` measures the file first and then changes the level evenly where the target allows, which keeps its dynamics. It falls back to dynamic normalizing where that would exceed the true peak limit.
- PDF to pictures, `jpg`/`png` and `pdf_layout: pages`, render each page's visible box (CropBox). Size limits apply to both.
