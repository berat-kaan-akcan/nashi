//! Pure, I/O-free drag + modifier chord state machine.
//!
//! The machine is fed abstract [`Input`] events together with a monotonic
//! timestamp in milliseconds (the injected clock) and returns the D-Bus level
//! [`Output`]s described in `docs/DBUS.md`.
//!
//! Semantics (beyond the contract, decided here):
//!
//! * **Primary button**: tracked per device. A "press" starts when the first
//!   device reports the primary button down and ends when the *last* device
//!   holding it releases it (a seat-wide button count, like libinput). A press
//!   that never satisfied the drag thresholds emits **no signals at all**.
//! * **Drag threshold**: `DragStarted` is emitted once the press has lasted
//!   `>= drag_hold_ms` **and** the accumulated motion path length since the
//!   press is `>= drag_move_px`. The hold threshold can be crossed without new
//!   input via [`Machine::tick`]; [`Machine::next_deadline`] tells the caller
//!   when to call it.
//! * **Motion**: relative motion is accumulated as `sqrt(dx² + dy²)` per event.
//!   Absolute motion is converted to deltas from the previous absolute
//!   position of the same device; the first sample after a press, a
//!   [`Input::AbsReset`] (finger lifted) or a device removal only sets the
//!   reference point.
//! * **Modifiers**: shift/ctrl/alt with left and right keys tracked as separate
//!   keys per device, so releasing one of two held shifts keeps "shift" held.
//!   Only modifiers mapped in the config take part; unmapped ones are ignored.
//! * **Mode priority**: when several mapped modifiers are held, the **most
//!   recently pressed** one wins. Releasing it falls back to the most recently
//!   pressed modifier that is still held. `ChordStarted` is emitted whenever the
//!   effective mode changes during a drag; `ChordReleased` when no mapped
//!   modifier is held any more while the drag continues.
//! * **Release ordering**: releasing the primary button ends the drag with
//!   `DragEnded` only (no `ChordReleased` first). Modifiers released after
//!   the drag ended emit nothing.
//! * **Enabled**: while disabled no signals are emitted and no new press is
//!   armed. Disabling during an active drag emits a final `DragEnded` so the
//!   UI never keeps a stuck overlay. Re-enabling while the button is still
//!   held does not resume the old press; a new press is required.
//! * **Reload**: new thresholds apply immediately; new chord mappings are
//!   re-evaluated against the held modifiers, so an active chord may switch
//!   mode or be released.

use std::collections::HashMap;

/// A modifier family (left and right keys are merged at this level).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    Shift,
    Ctrl,
    Alt,
}

/// Which physical key of a modifier family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

/// Wheel mode carried by `ChordStarted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Convert,
    Tools,
    Actions,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Convert => "convert",
            Mode::Tools => "tools",
            Mode::Actions => "actions",
        }
    }

    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "convert" => Some(Mode::Convert),
            "tools" => Some(Mode::Tools),
            "actions" => Some(Mode::Actions),
            _ => None,
        }
    }
}

/// Opaque identifier of an input device (assigned by the device layer).
pub type DeviceId = u64;

/// Abstract input events.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    ButtonDown {
        device: DeviceId,
    },
    ButtonUp {
        device: DeviceId,
    },
    RelMotion {
        dx: f64,
        dy: f64,
    },
    /// Absolute position (already normalised by the device layer). `None`
    /// means "axis unchanged in this report".
    AbsMotion {
        device: DeviceId,
        x: Option<f64>,
        y: Option<f64>,
    },
    /// Forget the absolute reference point of a device (e.g. finger lifted).
    AbsReset {
        device: DeviceId,
    },
    ModifierDown {
        device: DeviceId,
        modifier: Modifier,
        side: Side,
    },
    ModifierUp {
        device: DeviceId,
        modifier: Modifier,
        side: Side,
    },
    /// Device disappeared: everything it held is released.
    DeviceRemoved {
        device: DeviceId,
    },
}

/// D-Bus level outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    DragStarted,
    ChordStarted(Mode),
    ChordReleased,
    DragEnded,
}

/// Tunables and chord mappings.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub drag_hold_ms: u64,
    pub drag_move_px: f64,
    pub shift: Option<Mode>,
    pub ctrl: Option<Mode>,
    pub alt: Option<Mode>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            drag_hold_ms: 120,
            drag_move_px: 8.0,
            shift: Some(Mode::Convert),
            ctrl: Some(Mode::Tools),
            alt: Some(Mode::Actions),
        }
    }
}

impl Settings {
    fn mode_for(&self, m: Modifier) -> Option<Mode> {
        match m {
            Modifier::Shift => self.shift,
            Modifier::Ctrl => self.ctrl,
            Modifier::Alt => self.alt,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct KeyId {
    device: DeviceId,
    modifier: Modifier,
    side: Side,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    /// Button held, thresholds not met yet.
    Pending {
        since: u64,
        moved: f64,
    },
    Dragging,
    /// Button held but this press is not tracked (pressed while disabled).
    Ignored,
}

#[derive(Debug)]
pub struct Machine {
    settings: Settings,
    enabled: bool,
    phase: Phase,
    buttons_held: Vec<DeviceId>,
    /// Held modifier keys with a press sequence number (higher = newer).
    keys: HashMap<KeyId, u64>,
    seq: u64,
    abs_last: HashMap<DeviceId, (Option<f64>, Option<f64>)>,
    /// Mode currently announced to the UI (only meaningful while dragging).
    announced: Option<Mode>,
}

impl Machine {
    pub fn new(settings: Settings, enabled: bool) -> Self {
        Machine {
            settings,
            enabled,
            phase: Phase::Idle,
            buttons_held: Vec::new(),
            keys: HashMap::new(),
            seq: 0,
            abs_last: HashMap::new(),
            announced: None,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    #[cfg(test)]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn dragging(&self) -> bool {
        self.phase == Phase::Dragging
    }

    /// `(dragging, mode)` as returned by `GetState()`; mode empty when no chord.
    pub fn state(&self) -> (bool, &'static str) {
        let mode = if self.dragging() {
            self.announced.map(Mode::as_str).unwrap_or("")
        } else {
            ""
        };
        (self.dragging(), mode)
    }

    /// Effective mode from held mapped modifiers: most recently pressed wins.
    fn effective_mode(&self) -> Option<Mode> {
        self.keys
            .iter()
            .filter_map(|(k, seq)| self.settings.mode_for(k.modifier).map(|m| (*seq, m)))
            .max_by_key(|(seq, _)| *seq)
            .map(|(_, m)| m)
    }

    fn sync_chord(&mut self, out: &mut Vec<Output>) {
        if !self.dragging() {
            return;
        }
        let eff = self.effective_mode();
        if eff != self.announced {
            match eff {
                Some(m) => out.push(Output::ChordStarted(m)),
                None => out.push(Output::ChordReleased),
            }
            self.announced = eff;
        }
    }

    fn check_threshold(&mut self, now: u64, out: &mut Vec<Output>) {
        if let Phase::Pending { since, moved } = self.phase {
            if now.saturating_sub(since) >= self.settings.drag_hold_ms
                && moved >= self.settings.drag_move_px
            {
                self.phase = Phase::Dragging;
                self.announced = None;
                out.push(Output::DragStarted);
                self.sync_chord(out);
            }
        }
    }

    fn add_motion(&mut self, dist: f64, now: u64, out: &mut Vec<Output>) {
        if let Phase::Pending { moved, .. } = &mut self.phase {
            *moved += dist;
        }
        self.check_threshold(now, out);
    }

    fn end_press(&mut self, out: &mut Vec<Output>) {
        if self.dragging() {
            out.push(Output::DragEnded);
        }
        self.phase = Phase::Idle;
        self.announced = None;
    }

    /// Feed one input event at time `now` (monotonic ms).
    pub fn handle(&mut self, now: u64, input: Input) -> Vec<Output> {
        let mut out = Vec::new();
        match input {
            Input::ButtonDown { device } => {
                if !self.buttons_held.contains(&device) {
                    self.buttons_held.push(device);
                }
                if self.phase == Phase::Idle {
                    self.phase = if self.enabled {
                        Phase::Pending {
                            since: now,
                            moved: 0.0,
                        }
                    } else {
                        Phase::Ignored
                    };
                    // Motion before the press must not count.
                    self.abs_last.clear();
                }
                self.check_threshold(now, &mut out);
            }
            Input::ButtonUp { device } => {
                self.release_button(device, &mut out);
            }
            Input::RelMotion { dx, dy } => {
                self.add_motion(dx.hypot(dy), now, &mut out);
            }
            Input::AbsMotion { device, x, y } => {
                let prev = self.abs_last.entry(device).or_insert((None, None));
                let mut dx = 0.0;
                let mut dy = 0.0;
                if let Some(x) = x {
                    if let Some(px) = prev.0 {
                        dx = x - px;
                    }
                    prev.0 = Some(x);
                }
                if let Some(y) = y {
                    if let Some(py) = prev.1 {
                        dy = y - py;
                    }
                    prev.1 = Some(y);
                }
                self.add_motion(dx.hypot(dy), now, &mut out);
            }
            Input::AbsReset { device } => {
                self.abs_last.remove(&device);
            }
            Input::ModifierDown {
                device,
                modifier,
                side,
            } => {
                let key = KeyId {
                    device,
                    modifier,
                    side,
                };
                // Key repeat / duplicate down: keep the original press order.
                if let std::collections::hash_map::Entry::Vacant(e) = self.keys.entry(key) {
                    self.seq += 1;
                    e.insert(self.seq);
                }
                self.sync_chord(&mut out);
            }
            Input::ModifierUp {
                device,
                modifier,
                side,
            } => {
                self.keys.remove(&KeyId {
                    device,
                    modifier,
                    side,
                });
                self.sync_chord(&mut out);
            }
            Input::DeviceRemoved { device } => {
                self.abs_last.remove(&device);
                self.keys.retain(|k, _| k.device != device);
                if self.buttons_held.contains(&device) {
                    self.release_button(device, &mut out);
                } else {
                    self.sync_chord(&mut out);
                }
            }
        }
        out
    }

    fn release_button(&mut self, device: DeviceId, out: &mut Vec<Output>) {
        self.buttons_held.retain(|d| *d != device);
        if self.buttons_held.is_empty() {
            self.end_press(out);
        } else {
            self.sync_chord(out);
        }
    }

    /// Advance time without input so the hold threshold can trigger.
    pub fn tick(&mut self, now: u64) -> Vec<Output> {
        let mut out = Vec::new();
        self.check_threshold(now, &mut out);
        out
    }

    /// When the caller should call [`Machine::tick`] next, if ever. Only set
    /// when the movement threshold is already met and only time is missing.
    pub fn next_deadline(&self) -> Option<u64> {
        match self.phase {
            Phase::Pending { since, moved } if moved >= self.settings.drag_move_px => {
                Some(since.saturating_add(self.settings.drag_hold_ms))
            }
            _ => None,
        }
    }

    /// Enable or disable signal emission at runtime.
    pub fn set_enabled(&mut self, enabled: bool) -> Vec<Output> {
        let mut out = Vec::new();
        if enabled == self.enabled {
            return out;
        }
        self.enabled = enabled;
        if !enabled {
            self.end_press(&mut out);
            if !self.buttons_held.is_empty() {
                self.phase = Phase::Ignored;
            }
        } else if self.phase == Phase::Idle && !self.buttons_held.is_empty() {
            self.phase = Phase::Ignored;
        }
        out
    }

    /// Apply new settings (Reload). Mappings are re-evaluated immediately.
    pub fn apply_settings(&mut self, now: u64, settings: Settings) -> Vec<Output> {
        let mut out = Vec::new();
        self.settings = settings;
        self.sync_chord(&mut out);
        self.check_threshold(now, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Input::*;
    use Output::*;

    const MOUSE: DeviceId = 1;
    const KBD: DeviceId = 2;
    const KBD2: DeviceId = 3;
    const PAD: DeviceId = 4;

    fn md(m: Modifier, s: Side) -> Input {
        ModifierDown {
            device: KBD,
            modifier: m,
            side: s,
        }
    }
    fn mu(m: Modifier, s: Side) -> Input {
        ModifierUp {
            device: KBD,
            modifier: m,
            side: s,
        }
    }
    fn rel(dx: f64) -> Input {
        RelMotion { dx, dy: 0.0 }
    }
    const DOWN: Input = ButtonDown { device: MOUSE };
    const UP: Input = ButtonUp { device: MOUSE };
    use Modifier::*;
    use Side::*;

    enum Step {
        /// Feed input at time t, expect outputs.
        At(u64, Input, &'static [Output]),
        /// Tick at time t, expect outputs.
        Tick(u64, &'static [Output]),
        Enable(bool, &'static [Output]),
        Apply(u64, fn() -> Settings, &'static [Output]),
        State(bool, &'static str),
    }
    use Step::*;

    fn run(name: &str, steps: &[Step]) {
        let mut m = Machine::new(Settings::default(), true);
        for (i, step) in steps.iter().enumerate() {
            match step {
                At(t, input, exp) => {
                    assert_eq!(&m.handle(*t, *input), exp, "{name}: step {i} {input:?}")
                }
                Tick(t, exp) => assert_eq!(&m.tick(*t), exp, "{name}: step {i} tick {t}"),
                Enable(e, exp) => assert_eq!(&m.set_enabled(*e), exp, "{name}: step {i}"),
                Apply(t, s, exp) => {
                    assert_eq!(&m.apply_settings(*t, s()), exp, "{name}: step {i}")
                }
                State(d, mode) => assert_eq!(m.state(), (*d, *mode), "{name}: step {i}"),
            }
        }
    }

    #[test]
    fn table() {
        let cases: Vec<(&str, Vec<Step>)> = vec![
            (
                "plain drag",
                vec![
                    At(0, DOWN, &[]),
                    At(50, rel(10.0), &[]),
                    Tick(119, &[]),
                    Tick(120, &[DragStarted]),
                    State(true, ""),
                    At(200, rel(5.0), &[]),
                    At(300, UP, &[DragEnded]),
                    State(false, ""),
                ],
            ),
            (
                "hold too short",
                vec![
                    At(0, DOWN, &[]),
                    At(10, rel(50.0), &[]),
                    At(100, UP, &[]),
                    Tick(500, &[]),
                ],
            ),
            (
                "movement too small",
                vec![
                    At(0, DOWN, &[]),
                    At(10, rel(3.0), &[]),
                    At(200, rel(4.0), &[]),
                    Tick(1000, &[]),
                    At(1001, UP, &[]),
                ],
            ),
            (
                "movement accumulates after hold",
                vec![
                    At(0, DOWN, &[]),
                    At(500, rel(4.0), &[]),
                    At(600, RelMotion { dx: 3.0, dy: 4.0 }, &[DragStarted]),
                ],
            ),
            (
                "press without drag emits nothing",
                vec![At(0, DOWN, &[]), At(1000, UP, &[]), State(false, "")],
            ),
            (
                "modifier before drag",
                vec![
                    At(0, md(Shift, Left), &[]),
                    At(10, DOWN, &[]),
                    At(20, rel(20.0), &[]),
                    Tick(130, &[DragStarted, ChordStarted(Mode::Convert)]),
                    State(true, "convert"),
                    At(200, UP, &[DragEnded]),
                    At(210, mu(Shift, Left), &[]),
                ],
            ),
            (
                "modifier between press and threshold",
                vec![
                    At(0, DOWN, &[]),
                    At(10, md(Ctrl, Left), &[]),
                    At(200, rel(9.0), &[DragStarted, ChordStarted(Mode::Tools)]),
                ],
            ),
            (
                "modifier during drag",
                vec![
                    At(0, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted]),
                    At(300, md(Alt, Right), &[ChordStarted(Mode::Actions)]),
                    State(true, "actions"),
                    At(400, mu(Alt, Right), &[ChordReleased]),
                    State(true, ""),
                    At(450, md(Alt, Left), &[ChordStarted(Mode::Actions)]),
                    At(500, UP, &[DragEnded]),
                ],
            ),
            (
                "switching modifiers: most recent wins, fallback on release",
                vec![
                    At(0, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted]),
                    At(210, md(Shift, Left), &[ChordStarted(Mode::Convert)]),
                    At(220, md(Ctrl, Left), &[ChordStarted(Mode::Tools)]),
                    At(230, md(Alt, Left), &[ChordStarted(Mode::Actions)]),
                    At(240, mu(Ctrl, Left), &[]),
                    At(250, mu(Alt, Left), &[ChordStarted(Mode::Convert)]),
                    At(260, mu(Shift, Left), &[ChordReleased]),
                ],
            ),
            (
                "two shifts: releasing one keeps chord",
                vec![
                    At(0, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted]),
                    At(210, md(Shift, Left), &[ChordStarted(Mode::Convert)]),
                    At(220, md(Shift, Right), &[]),
                    At(230, mu(Shift, Left), &[]),
                    State(true, "convert"),
                    At(240, mu(Shift, Right), &[ChordReleased]),
                ],
            ),
            (
                "same shift on two keyboards",
                vec![
                    At(0, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted]),
                    At(210, md(Shift, Left), &[ChordStarted(Mode::Convert)]),
                    At(
                        220,
                        ModifierDown {
                            device: KBD2,
                            modifier: Shift,
                            side: Left,
                        },
                        &[],
                    ),
                    At(230, mu(Shift, Left), &[]),
                    At(240, DeviceRemoved { device: KBD2 }, &[ChordReleased]),
                ],
            ),
            (
                "key repeat does not change priority",
                vec![
                    At(0, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted]),
                    At(210, md(Shift, Left), &[ChordStarted(Mode::Convert)]),
                    At(220, md(Ctrl, Left), &[ChordStarted(Mode::Tools)]),
                    At(230, md(Shift, Left), &[]),
                    State(true, "tools"),
                ],
            ),
            (
                "release ordering: button before modifier",
                vec![
                    At(0, md(Ctrl, Right), &[]),
                    At(1, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted, ChordStarted(Mode::Tools)]),
                    At(300, UP, &[DragEnded]),
                    At(310, mu(Ctrl, Right), &[]),
                    State(false, ""),
                ],
            ),
            (
                "release ordering: modifier before button",
                vec![
                    At(0, md(Ctrl, Right), &[]),
                    At(1, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted, ChordStarted(Mode::Tools)]),
                    At(300, mu(Ctrl, Right), &[ChordReleased]),
                    At(310, UP, &[DragEnded]),
                ],
            ),
            (
                "modifier press and release without button",
                vec![
                    At(0, md(Shift, Left), &[]),
                    At(10, mu(Shift, Left), &[]),
                    At(20, rel(100.0), &[]),
                ],
            ),
            (
                "disabled: no signals",
                vec![
                    Enable(false, &[]),
                    At(0, md(Shift, Left), &[]),
                    At(1, DOWN, &[]),
                    At(200, rel(50.0), &[]),
                    Tick(500, &[]),
                    At(600, UP, &[]),
                    State(false, ""),
                ],
            ),
            (
                "disable mid drag ends it, re-enable needs a new press",
                vec![
                    At(0, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted]),
                    Enable(false, &[DragEnded]),
                    At(210, md(Shift, Left), &[]),
                    Enable(true, &[]),
                    At(300, rel(50.0), &[]),
                    At(400, UP, &[]),
                    At(500, DOWN, &[]),
                    At(700, rel(9.0), &[DragStarted, ChordStarted(Mode::Convert)]),
                ],
            ),
            (
                "pressed while disabled is ignored after enable",
                vec![
                    Enable(false, &[]),
                    At(0, DOWN, &[]),
                    Enable(true, &[]),
                    At(300, rel(50.0), &[]),
                    Tick(500, &[]),
                    At(600, UP, &[]),
                ],
            ),
            (
                "unmapped modifier ignored",
                vec![
                    Apply(0, unmapped_alt, &[]),
                    At(0, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted]),
                    At(210, md(Alt, Left), &[]),
                    State(true, ""),
                    At(220, md(Shift, Left), &[ChordStarted(Mode::Convert)]),
                    At(230, md(Alt, Right), &[]),
                    State(true, "convert"),
                ],
            ),
            (
                "reload with new mappings re-evaluates active chord",
                vec![
                    At(0, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted]),
                    At(210, md(Shift, Left), &[ChordStarted(Mode::Convert)]),
                    Apply(220, swapped, &[ChordStarted(Mode::Actions)]),
                    State(true, "actions"),
                    Apply(230, unmapped_shift, &[ChordReleased]),
                    Apply(240, Settings::default, &[ChordStarted(Mode::Convert)]),
                ],
            ),
            (
                "reload with new thresholds",
                vec![
                    Apply(0, slow, &[]),
                    At(0, DOWN, &[]),
                    At(200, rel(9.0), &[]),
                    Tick(999, &[]),
                    Tick(1000, &[]),
                    At(1001, rel(11.0), &[DragStarted]),
                ],
            ),
            (
                "two pointer devices: drag ends on last release",
                vec![
                    At(0, DOWN, &[]),
                    At(1, ButtonDown { device: PAD }, &[]),
                    At(200, rel(9.0), &[DragStarted]),
                    At(300, UP, &[]),
                    At(310, ButtonUp { device: PAD }, &[DragEnded]),
                ],
            ),
            (
                "pointer device removed while dragging",
                vec![
                    At(0, DOWN, &[]),
                    At(200, rel(9.0), &[DragStarted]),
                    At(300, DeviceRemoved { device: MOUSE }, &[DragEnded]),
                ],
            ),
            (
                "absolute motion uses deltas",
                vec![
                    At(
                        0,
                        AbsMotion {
                            device: PAD,
                            x: Some(500.0),
                            y: Some(500.0),
                        },
                        &[],
                    ),
                    At(1, ButtonDown { device: PAD }, &[]),
                    // First sample after press only sets the reference point.
                    At(
                        10,
                        AbsMotion {
                            device: PAD,
                            x: Some(100.0),
                            y: Some(100.0),
                        },
                        &[],
                    ),
                    At(
                        200,
                        AbsMotion {
                            device: PAD,
                            x: Some(104.0),
                            y: None,
                        },
                        &[],
                    ),
                    At(
                        210,
                        AbsMotion {
                            device: PAD,
                            x: None,
                            y: Some(104.0),
                        },
                        // 4 + 4 = 8 px path length (the pre-press 500 is ignored).
                        &[DragStarted],
                    ),
                    State(true, ""),
                ],
            ),
            (
                "absolute reset avoids jumps",
                vec![
                    At(0, ButtonDown { device: PAD }, &[]),
                    At(
                        10,
                        AbsMotion {
                            device: PAD,
                            x: Some(0.0),
                            y: Some(0.0),
                        },
                        &[],
                    ),
                    At(20, AbsReset { device: PAD }, &[]),
                    At(
                        200,
                        AbsMotion {
                            device: PAD,
                            x: Some(900.0),
                            y: Some(900.0),
                        },
                        &[],
                    ),
                    At(
                        210,
                        AbsMotion {
                            device: PAD,
                            x: Some(910.0),
                            y: Some(900.0),
                        },
                        &[DragStarted],
                    ),
                ],
            ),
        ];
        for (name, steps) in &cases {
            run(name, steps);
        }
    }

    fn unmapped_alt() -> Settings {
        Settings {
            alt: None,
            ..Settings::default()
        }
    }
    fn unmapped_shift() -> Settings {
        Settings {
            shift: None,
            ..Settings::default()
        }
    }
    fn swapped() -> Settings {
        Settings {
            shift: Some(Mode::Actions),
            ..Settings::default()
        }
    }
    fn slow() -> Settings {
        Settings {
            drag_hold_ms: 1000,
            drag_move_px: 20.0,
            ..Settings::default()
        }
    }

    #[test]
    fn deadline_only_when_motion_met() {
        let mut m = Machine::new(Settings::default(), true);
        assert_eq!(m.next_deadline(), None);
        m.handle(100, DOWN);
        assert_eq!(m.next_deadline(), None);
        m.handle(110, rel(8.0));
        assert_eq!(m.next_deadline(), Some(220));
        assert_eq!(m.tick(220), vec![DragStarted]);
        assert_eq!(m.next_deadline(), None);
    }

    #[test]
    fn mode_parse_roundtrip() {
        for m in [Mode::Convert, Mode::Tools, Mode::Actions] {
            assert_eq!(Mode::parse(m.as_str()), Some(m));
        }
        assert_eq!(Mode::parse("bogus"), None);
    }
}
