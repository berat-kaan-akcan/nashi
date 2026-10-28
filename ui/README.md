# nashi — Qt 6 / QML UI

The `nashi` app: drag overlay with the wheel, toasts, tray, Home window and the
`io.github.beratkaanakcan.Nashi` D-Bus service. Engine work is delegated to
`nashi-cli … --json` (see `docs/CLI-PROTOCOL.md`); drag chords come from
`nashi-inputd` (see `docs/DBUS.md`).

## Build

```sh
cmake -S ui -B build/ui -G Ninja
cmake --build build/ui
QT_QPA_PLATFORM=offscreen ctest --test-dir build/ui --output-on-failure
```

Requirements: Qt ≥ 6.5 (Core, Gui, Qml, Quick, QuickControls2, Widgets, DBus,
LinguistTools, Test), python3 for the test fixture.

| Option | Default | Meaning |
|---|---|---|
| `NASHI_WITH_LAYERSHELL` | ON if `LayerShellQt` is found | Use layer-shell surfaces on Wayland. Without it (or on X11) the overlay is a frameless, transparent, always-on-top full-screen window per screen. |
| `BUILD_TESTING` | ON | Build the Qt Test suites. |

`cmake --build build/ui --target update_translations` refreshes
`i18n/nashi_tr.ts` (lupdate); `.qm` files are compiled into the resources
(`:/i18n`). A test fails if any Turkish string is unfinished.

Only the `nashi` binary is installed (`${CMAKE_INSTALL_BINDIR}`); desktop
files, D-Bus activation and autostart entries live in `data/`.

## Layout

- `src/` — C++ services, all owned by `App` (QML singleton `App`):
  `JobRunner`/`JobManager` (QProcess, argv only, JSON-lines parsing,
  SIGTERM → kill after 3 s), `WheelQuery`/`Selection`/`WheelModel`
  (`nashi-cli wheel`, cached per file set, `needs_editor` tools hidden),
  `WheelGeometry` (pie hit-testing), `OverlayController` (state machine) +
  `OverlaySurfaces` (windows), `InputdClient`, `ToastController`, `Notifier`,
  `FileActions`, `DepsModel`, `RecentModel`, `Settings`
  (`~/.config/nashi/nashi.conf`), `Language`, `Tray`, D-Bus adaptors.
- `qml/` — `OverlayWindow`, `Wheel`, `WheelSlice`, `ToastWindow`, `JobCard`,
  `HomeWindow` and its pages.
- `tests/` — Qt Test suites and `fake-nashi-cli` (canned wheel/deps/event
  stream, partial lines, errors, a slow job for cancellation).

## Overlay design

One transparent full-screen overlay window per screen. With layer-shell it is
an `Overlay`-layer surface anchored to all edges, exclusive zone −1, keyboard
interactivity none, scope `nashi-overlay`. All of them are shown only while a
chord is active, so the drag can enter on any monitor. The wheel is drawn
where the drag first enters (on X11 it is placed at the cursor up front).

```
Hidden ─ChordStarted─▶ Target ─URLs readable at enter + wheel ready─▶ Live
                         │ drop (URLs unknown before)                 │ drop on slice → run
                         ▼                                            │ drop on centre/far away → cancel
                       Pick (click a slice; Esc / click outside cancels)
```

- **Single stage (Live):** `drag.urls` can be read in `onEntered`, so
  `nashi-cli wheel` is queried right away and the user drops on a slice.
- **Two stage:** if the URLs can't be read before the drop, stage 1 is a
  drop target labelled with the mode. After the drop the wheel is queried
  and stage 2 takes clicks. Keyboard interactivity switches to on-demand
  so Escape works.
- `DragEnded` without a drop hides the overlay after 300 ms. A drop inside
  that window keeps it open. A new `ChordStarted` mode mid-drag switches
  the list. Losing inputd, or chords being disabled, hides an overlay that
  has not received a drop.
- **Drops are always `Qt::CopyAction`** on enter, move and drop (a Shift-drag
  from Dolphin proposes Move). A headless test sends synthetic Move-proposing
  DnD events through the real QML and checks that Copy is returned.
- Hit-testing: slice 0 is centred at 12 o'clock and slices go clockwise. A
  centre disc of radius 52 px cancels. Points beyond 1.4 × the outer radius
  count as outside.

## Notifications policy

The toast (layer-shell `Top` layer, top-right, never takes focus) is the
primary feedback. A freedesktop notification (with Open / Show in folder
actions) is also sent when a job finishes and either the toast is not on
screen or the job ran ≥ 10 s. You can turn it off in Settings. Cancelled jobs
never notify.

## Single instance

`nashi` exports `/io/github/beratkaanakcan/Nashi` and then tries to own
`io.github.beratkaanakcan.Nashi`. If the name is taken, it forwards the request
and exits:

- `nashi` → `Nashi1.Activate`
- `--wheel [--mode m] FILE…` → `ShowWheel`
- `--wheel` without files → `ShowWheelFromClipboard`
- `nashi FILE…` → `org.freedesktop.Application.Open`, because Nashi1 has no
  method for this
- `--background` → nothing
