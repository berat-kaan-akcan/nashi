# nashi-inputd

Small user daemon that notices **"drag a file while holding a modifier"** anywhere on the
desktop and announces it on the session bus, so the Nashi UI can show its overlay. On
Wayland no app can see global input, so the daemon reads `/dev/input/event*` directly
(evdev). D-Bus contract: [`docs/DBUS.md`](../../docs/DBUS.md).

## Privacy guarantees

- **Never grabs devices** (no `EVIOCGRAB`): the compositor and apps get every event
  unchanged.
- **Only these events are looked at:** `BTN_LEFT`, `BTN_TOUCH` (finger lift, only to reset
  touchpad motion deltas), left/right Shift, Ctrl and Alt, and `REL_X`/`REL_Y`/`ABS_X`/`ABS_Y`.
  Everything else is dropped right after a code comparison, without being stored.
- **Never logs key codes or text.** Logs only contain device names/paths, permission
  problems and (at debug level) drag/chord state ("drag started", "chord started: convert").
- **Output is coarse:** the bus only ever sees `DragStarted`, `ChordStarted(mode)`,
  `ChordReleased`, `DragEnded` and `GetState() → (dragging, mode)`.
- Runs as your user, no root, no network (the systemd unit restricts sockets to `AF_UNIX`).

Caveat: to read evdev your user must be in the `input` group, which lets *any* program you
run read all input devices. That is a property of the group, not of this daemon.

## Setup

```sh
sudo usermod -aG input "$USER"        # then log out and back in
nashi-inputd --check                  # lists devices + permission status (no key data)
systemctl --user enable --now nashi-inputd.service
```

Installed files (`dist/`):

- `70-nashi-inputd.rules` → `/usr/lib/udev/rules.d/`: re-asserts `GROUP="input"`,
  `MODE="0660"` on event nodes (already the Arch default).
- `nashi-inputd.service` → `/usr/lib/systemd/user/`: `Type=dbus`, bound to
  `graphical-session.target`, `ExecReload` sends `SIGHUP`.

## Usage

```
nashi-inputd [--config PATH] [--check] [--version]
```

Config: `$XDG_CONFIG_HOME/nashi/inputd.toml` (default `~/.config/nashi/inputd.toml`).
Missing file → defaults. Invalid file → warning, defaults at startup / previous settings on
reload. Re-read via `Reload()` on D-Bus or `SIGHUP` (`systemctl --user reload nashi-inputd`).

```toml
enabled = true
drag_hold_ms = 120     # 0..=10000
drag_move_px = 8       # 0..=10000, see "Motion units"
[chords]               # "convert" | "tools" | "actions" | "none"
shift = "convert"
ctrl  = "tools"
alt   = "actions"
```

Logging: `NASHI_INPUTD_LOG=debug` (or `RUST_LOG`).

## Behaviour details

- A drag starts when the primary button has been held `drag_hold_ms` **and** the pointer
  has travelled `drag_move_px` (path length) since the press. A press that never gets there
  emits nothing.
- The primary button is counted across devices; the drag ends when the last device holding
  it releases it (or the device is unplugged).
- Several modifiers held: the **most recently pressed** one decides the mode; releasing it
  falls back to the newest one still held. Left/right keys are tracked separately, so
  releasing one of two held Shifts keeps the chord.
- Releasing the button ends with `DragEnded` only (no `ChordReleased` before it).
- `Enabled` (D-Bus property) is a runtime switch and is not written to the config file;
  `Reload()` resets it to the file's `enabled`. Disabling during a drag emits a final
  `DragEnded`. The daemon also emits `DragEnded` when it is stopped mid-drag.
- `Reload()` fails with `org.freedesktop.DBus.Error.Failed` if the file is invalid (the
  previous settings stay active).

### Motion units

Relative devices (mice) count raw device counts (≈ pixels with flat acceleration).
Absolute devices (touchpads, tablets) are normalised so the full X range of the device
equals 1920 units.

## Limitations

- **Touchpad tap-to-click / tap-and-drag is invisible**: libinput synthesizes those clicks
  in the compositor; evdev only shows `BTN_LEFT` for physical clicks (clickpad press).
  Dragging with a pressed clickpad works; tap-drag does not.
- Software-only input (virtual keyboards, remote desktop injected via the compositor) is
  not seen. Devices from `uinput` (e.g. key remappers) are seen like real ones.
- AltGr is `KEY_RIGHTALT` and therefore counts as Alt.
