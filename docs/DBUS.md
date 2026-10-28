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
| `Reload()` | → `()` | Re-read config |
| `Enabled` | property `b`, read/write | When false, no signals |
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

Devices: all evdev devices that report `BTN_LEFT` (mice, touchpads) or `KEY_LEFTSHIFT` (keyboards); hot-plug via inotify on `/dev/input`. Never grab devices (no `EVIOCGRAB`).

## `nashi` app → `io.github.beratkaanakcan.Nashi`

- Object `/io/github/beratkaanakcan/Nashi`, interface `io.github.beratkaanakcan.Nashi1`
- `ShowWheel(as paths, s mode)` — show the wheel at the cursor for these files (`mode` may be empty → `convert`). Used by `nashi --wheel FILE…`, the Dolphin service menu and the global shortcut.
- `ShowWheelFromClipboard(s mode)` — files from the clipboard `text/uri-list`.
- `Activate()` — raise the Home window.
