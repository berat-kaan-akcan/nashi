# D-Bus contracts (session bus)

## `nashi-inputd` → `io.github.beratkaanakcan.Nashi.Input`

- Bus name: `io.github.beratkaanakcan.Nashi.Input`
- Object: `/io/github/beratkaanakcan/Nashi/Input`
- Interface: `io.github.beratkaanakcan.Nashi.Input1`

### Signals

| Signal | Args | When |
|---|---|---|
| `DragStarted` | — | Primary button held ≥ `drag_hold_ms` (default 120) **and** pointer moved ≥ `drag_move_px` (default 8, summed relative/abs motion) |
| `ChordStarted` | `s mode` (`convert`/`tools`/`actions`) | During a drag, a mapped modifier goes down (or is already held when the drag starts). Re-emitted with a new mode if the modifier changes |
| `ChordReleased` | — | The modifier is released while still dragging (UI keeps the overlay; user may still drop) |
| `DragEnded` | — | Primary button released (any device) |

### Methods / properties

| Member | Type | Notes |
|---|---|---|
| `GetState()` | → `(b dragging, s mode)` | `mode` empty when no chord |
| `Reload()` | → `()` | Re-read config (also on `SIGHUP`). Returns `org.freedesktop.DBus.Error.Failed` and keeps the previous config when the file is invalid |
| `Enabled` | property `b`, read/write | When false, no signals (exception: switching it off mid-drag emits one final `DragEnded` so the UI never keeps a stuck overlay). Runtime only; `Reload()` resets it to the config value. Emits `PropertiesChanged` |
| `Version` | property `s` | crate version |

### Config `~/.config/nashi/inputd.toml`

```toml
enabled = true
drag_hold_ms = 120
drag_move_px = 8
[chords]
shift = "convert"
ctrl  = "tools"
alt   = "actions"
```

### Semantics (clients must handle)

- Several modifiers held: the most recently pressed one wins; releasing it falls back to the newest one still held. `ChordStarted` is re-emitted only when the mode changes, so it can arrive several times per drag.
- A drag may start without a chord; a chord may come and go during a drag.
- `DragEnded` is emitted only after `DragStarted`, may arrive without a preceding `ChordReleased`, and is also emitted when the daemon is disabled or stopped mid-drag. Treat it as "hide the overlay unless a drop arrives".
- A press that never reaches the hold/move thresholds emits nothing.
- `GetState()` returns `(false, "")` when idle.

Devices: all evdev devices that report `BTN_LEFT` (mice, touchpads) or `KEY_LEFTSHIFT` (keyboards); hot-plug via inotify on `/dev/input`. Never grab devices (no `EVIOCGRAB`).

Touchpad tap-and-drag: the compositor (libinput) turns "tap, then touch again and move" into a button press, but evdev never reports `BTN_LEFT` for it. On touchpads (`BTN_TOUCH` + `BTN_TOOL_FINGER` + `ABS_X/Y`, not `INPUT_PROP_DIRECT`) the daemon therefore treats a single-finger tap (≤ 200 ms, ≤ ~2 mm of motion, no physical click, no `BTN_TOOL_DOUBLETAP`…) followed within 300 ms by a new touch as the primary button going down, and every finger lifting as it going up. The normal `drag_hold_ms` / `drag_move_px` thresholds still apply, so a plain double-tap (double click) emits nothing. A physical click during a tap-drag is merged into the same button (one down, one up).

## `nashi` app → `io.github.beratkaanakcan.Nashi`

- Object `/io/github/beratkaanakcan/Nashi`, interface `io.github.beratkaanakcan.Nashi1`
- `ShowWheel(as paths, s mode)` — show the wheel at the cursor for these files (`mode` may be empty → `convert`). Used by `nashi --wheel FILE…`, the Dolphin service menu and the global shortcut.
- `ShowWheelFromClipboard(s mode)` — files from the clipboard `text/uri-list`.
- `Activate()` — raise the Home window.
- The same object also implements the standard `org.freedesktop.Application` interface (`Activate(a{sv})`, `Open(as uris, a{sv})`, `ActivateAction`). `nashi FILE…` forwards to `Open` (opens Home with the files queued); both read the `activation-token` platform-data key so Wayland can raise the window. `Nashi1.Activate()` is kept for scripts but cannot pass a token.
- `nashi --wheel [--mode m]` with no files maps to `ShowWheelFromClipboard(mode)`.
- On Wayland the cursor position is unknown to clients, so `ShowWheel` centres the wheel on the primary screen.
