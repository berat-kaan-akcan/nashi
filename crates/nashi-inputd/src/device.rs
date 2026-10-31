//! evdev device layer: discovery, hot-plug and event translation.
//!
//! Privacy: from every device only BTN_LEFT, BTN_TOUCH (finger down/up), the
//! BTN_TOOL_*TAP finger counts of touchpads, the six shift/ctrl/alt keys and
//! REL_X/REL_Y/ABS_X/ABS_Y are looked at. Every other event is dropped by a code comparison
//! without being stored or logged. Devices are never grabbed (no EVIOCGRAB),
//! so the compositor keeps receiving everything unchanged. Logs only ever
//! mention device names/paths.

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use evdev::raw_stream::RawDevice;
use evdev::{
    AbsoluteAxisCode, EventType, InputEvent, KeyCode, PropType, RelativeAxisCode,
    SynchronizationCode,
};
use futures_util::StreamExt;
use inotify::{Inotify, WatchMask};
use tokio::sync::mpsc::UnboundedSender;

use crate::chord::{DeviceId, Input, Modifier, Side};

pub const INPUT_DIR: &str = "/dev/input";

/// Absolute axes are normalised so that the full X range of the device maps
/// to this many "pixels" (the same factor is used for Y to keep the aspect).
const ABS_NOMINAL_WIDTH: f64 = 1920.0;

/// Touchpad tap-and-drag (libinput's "tap, then touch again and move"): the
/// compositor turns it into a button press, but evdev never reports BTN_LEFT
/// for it, so it is recognised here. The first touch must be a single-finger
/// tap no longer than this (libinput's tap timeout is 180 ms) ...
const TAP_MAX_US: u64 = 200_000;
/// ... that barely moved (normalised units, ~2 mm on a typical touchpad) ...
const TAP_MAX_MOVE: f64 = 40.0;
/// ... and the next touch must start this soon after the tap was released.
const TAP_GAP_US: u64 = 300_000;

/// Multi-finger touchpad taps (right/middle click) never start a tap-drag.
const MULTI_FINGER: [KeyCode; 4] = [
    KeyCode::BTN_TOOL_DOUBLETAP,
    KeyCode::BTN_TOOL_TRIPLETAP,
    KeyCode::BTN_TOOL_QUADTAP,
    KeyCode::BTN_TOOL_QUINTTAP,
];

/// The only keys ever looked at. Index = bit in [`Translator::held`].
const KEYS: [KeyCode; 7] = [
    KeyCode::BTN_LEFT,
    KeyCode::KEY_LEFTSHIFT,
    KeyCode::KEY_RIGHTSHIFT,
    KeyCode::KEY_LEFTCTRL,
    KeyCode::KEY_RIGHTCTRL,
    KeyCode::KEY_LEFTALT,
    KeyCode::KEY_RIGHTALT,
];

const MODS: [(Modifier, Side); 6] = [
    (Modifier::Shift, Side::Left),
    (Modifier::Shift, Side::Right),
    (Modifier::Ctrl, Side::Left),
    (Modifier::Ctrl, Side::Right),
    (Modifier::Alt, Side::Left),
    (Modifier::Alt, Side::Right),
];

/// What a device can contribute.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Caps {
    /// Reports BTN_LEFT (mouse, touchpad, tablet, trackpoint ...).
    pub pointer: bool,
    /// Reports KEY_LEFTSHIFT (keyboard).
    pub keyboard: bool,
    /// `(min_x, min_y, scale)` for devices with ABS_X/ABS_Y.
    pub abs: Option<(f64, f64, f64)>,
    /// Indirect touch device (touchpad): tap-and-drag is recognised.
    pub touchpad: bool,
}

impl Caps {
    pub fn relevant(&self) -> bool {
        self.pointer || self.keyboard
    }

    pub fn describe(&self) -> &'static str {
        if self.touchpad && !self.keyboard {
            return "touchpad";
        }
        match (self.pointer, self.keyboard) {
            (true, true) => "pointer+keyboard",
            (true, false) => "pointer",
            (false, true) => "keyboard",
            (false, false) => "ignored",
        }
    }
}

pub fn caps_of(dev: &RawDevice) -> Caps {
    let keys = dev.supported_keys();
    let pointer = keys.is_some_and(|k| k.contains(KeyCode::BTN_LEFT));
    let keyboard = keys.is_some_and(|k| k.contains(KeyCode::KEY_LEFTSHIFT));
    let mut abs = None;
    if pointer
        && dev.supported_absolute_axes().is_some_and(|a| {
            a.contains(AbsoluteAxisCode::ABS_X) && a.contains(AbsoluteAxisCode::ABS_Y)
        })
    {
        if let Ok(info) = dev.get_abs_state() {
            let x = info[AbsoluteAxisCode::ABS_X.0 as usize];
            let y = info[AbsoluteAxisCode::ABS_Y.0 as usize];
            let range = f64::from(x.maximum) - f64::from(x.minimum);
            if range > 0.0 {
                abs = Some((
                    f64::from(x.minimum),
                    f64::from(y.minimum),
                    ABS_NOMINAL_WIDTH / range,
                ));
            }
        }
    }
    let touchpad = abs.is_some()
        && keys.is_some_and(|k| {
            k.contains(KeyCode::BTN_TOUCH) && k.contains(KeyCode::BTN_TOOL_FINGER)
        })
        && !dev.properties().contains(PropType::DIRECT);
    Caps {
        pointer,
        keyboard,
        abs,
        touchpad,
    }
}

/// Tap-and-drag recogniser state (touchpads only). Times are event
/// timestamps in microseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tap {
    Idle,
    /// A finger is down; might become a tap.
    Touch {
        since: u64,
        origin: Option<(f64, f64)>,
        disqualified: bool,
    },
    /// A tap was released; a touch soon after starts a drag.
    Released {
        at: u64,
    },
    /// Synthetic button held until every finger lifts.
    Dragging,
}

/// Turns raw evdev events of one device into abstract [`Input`]s.
#[derive(Debug)]
pub struct Translator {
    device: DeviceId,
    caps: Caps,
    /// Bitmask over [`KEYS`] of what this device currently holds.
    held: u8,
    rel: (f64, f64),
    abs: (Option<f64>, Option<f64>),
    dropping: bool,
    /// Physical BTN_LEFT state; the reported button is `phys_left || tap drag`.
    phys_left: bool,
    tap: Tap,
    /// Last absolute position (normalised), touchpads only.
    pos: (Option<f64>, Option<f64>),
}

/// Result of feeding one event.
#[derive(Debug, PartialEq, Eq)]
pub enum Feed {
    Ok,
    /// A SYN_DROPPED burst just ended: call [`Translator::resync`] with the
    /// device's current key state.
    NeedsResync,
}

impl Translator {
    pub fn new(device: DeviceId, caps: Caps) -> Self {
        Translator {
            device,
            caps,
            held: 0,
            rel: (0.0, 0.0),
            abs: (None, None),
            dropping: false,
            phys_left: false,
            tap: Tap::Idle,
            pos: (None, None),
        }
    }

    /// Report the combined (physical or tap-drag) button state.
    fn sync_button(&mut self, out: &mut Vec<Input>) {
        let down = self.phys_left || self.tap == Tap::Dragging;
        self.set_key(0, down, out);
    }

    fn touch(&mut self, now: u64, down: bool, out: &mut Vec<Input>) {
        if down {
            self.tap = match self.tap {
                Tap::Released { at } if now.saturating_sub(at) <= TAP_GAP_US => Tap::Dragging,
                _ => Tap::Touch {
                    since: now,
                    origin: None,
                    disqualified: self.phys_left,
                },
            };
        } else {
            self.pos = (None, None);
            self.tap = match self.tap {
                Tap::Touch {
                    since,
                    disqualified: false,
                    ..
                } if now.saturating_sub(since) <= TAP_MAX_US => Tap::Released { at: now },
                _ => Tap::Idle,
            };
        }
        self.sync_button(out);
    }

    /// Called once per frame: a tap that moves too far is no tap.
    fn track_tap_motion(&mut self) {
        let (Some(x), Some(y)) = self.pos else {
            return;
        };
        if let Tap::Touch {
            origin,
            disqualified,
            ..
        } = &mut self.tap
        {
            match origin {
                None => *origin = Some((x, y)),
                Some((ox, oy)) => {
                    if (x - *ox).hypot(y - *oy) > TAP_MAX_MOVE {
                        *disqualified = true;
                    }
                }
            }
        }
    }

    fn key_input(&self, idx: usize, down: bool) -> Input {
        let device = self.device;
        if idx == 0 {
            return if down {
                Input::ButtonDown { device }
            } else {
                Input::ButtonUp { device }
            };
        }
        let (modifier, side) = MODS[idx - 1];
        if down {
            Input::ModifierDown {
                device,
                modifier,
                side,
            }
        } else {
            Input::ModifierUp {
                device,
                modifier,
                side,
            }
        }
    }

    fn set_key(&mut self, idx: usize, down: bool, out: &mut Vec<Input>) {
        let bit = 1u8 << idx;
        if down == (self.held & bit != 0) {
            return;
        }
        self.held ^= bit;
        out.push(self.key_input(idx, down));
    }

    fn flush_motion(&mut self, out: &mut Vec<Input>) {
        if self.rel != (0.0, 0.0) {
            out.push(Input::RelMotion {
                dx: self.rel.0,
                dy: self.rel.1,
            });
            self.rel = (0.0, 0.0);
        }
        if self.abs != (None, None) {
            out.push(Input::AbsMotion {
                device: self.device,
                x: self.abs.0,
                y: self.abs.1,
            });
            self.abs = (None, None);
        }
    }

    pub fn feed(&mut self, ev: InputEvent, out: &mut Vec<Input>) -> Feed {
        let now = ev
            .timestamp()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_micros() as u64);
        self.feed_at(ev, now, out)
    }

    /// [`Self::feed`] with an explicit event time in microseconds.
    pub fn feed_at(&mut self, ev: InputEvent, now: u64, out: &mut Vec<Input>) -> Feed {
        let ty = ev.event_type();
        let code = ev.code();
        let value = ev.value();
        if ty == EventType::SYNCHRONIZATION {
            if code == SynchronizationCode::SYN_DROPPED.0 {
                self.dropping = true;
                self.rel = (0.0, 0.0);
                self.abs = (None, None);
                return Feed::Ok;
            }
            if code == SynchronizationCode::SYN_REPORT.0 {
                if self.dropping {
                    self.dropping = false;
                    // Positions after a drop are unreliable as a reference.
                    out.push(Input::AbsReset {
                        device: self.device,
                    });
                    return Feed::NeedsResync;
                }
                if self.caps.touchpad {
                    self.track_tap_motion();
                }
                self.flush_motion(out);
            }
            return Feed::Ok;
        }
        if self.dropping {
            return Feed::Ok;
        }
        if ty == EventType::KEY {
            if code == KeyCode::BTN_LEFT.0 {
                self.phys_left = value != 0;
                if self.phys_left {
                    // A clickpad press is a click, not a tap.
                    match &mut self.tap {
                        Tap::Touch { disqualified, .. } => *disqualified = true,
                        Tap::Released { .. } => self.tap = Tap::Idle,
                        _ => {}
                    }
                }
                self.sync_button(out);
            } else if let Some(idx) = KEYS.iter().position(|k| k.0 == code) {
                // 0 = up, 1 = down, 2 = autorepeat (still down).
                self.set_key(idx, value != 0, out);
            } else if code == KeyCode::BTN_TOUCH.0 {
                if value == 0 {
                    out.push(Input::AbsReset {
                        device: self.device,
                    });
                }
                if self.caps.touchpad && value != 2 {
                    self.touch(now, value != 0, out);
                }
            } else if self.caps.touchpad && value != 0 && MULTI_FINGER.iter().any(|k| k.0 == code) {
                if let Tap::Touch { disqualified, .. } = &mut self.tap {
                    *disqualified = true;
                }
            }
        } else if ty == EventType::RELATIVE {
            if code == RelativeAxisCode::REL_X.0 {
                self.rel.0 += f64::from(value);
            } else if code == RelativeAxisCode::REL_Y.0 {
                self.rel.1 += f64::from(value);
            }
        } else if ty == EventType::ABSOLUTE {
            if let Some((min_x, min_y, scale)) = self.caps.abs {
                if code == AbsoluteAxisCode::ABS_X.0 {
                    let x = (f64::from(value) - min_x) * scale;
                    self.abs.0 = Some(x);
                    self.pos.0 = Some(x);
                } else if code == AbsoluteAxisCode::ABS_Y.0 {
                    let y = (f64::from(value) - min_y) * scale;
                    self.abs.1 = Some(y);
                    self.pos.1 = Some(y);
                }
            }
        }
        Feed::Ok
    }

    /// Re-synchronise held keys after SYN_DROPPED. `is_down` is asked only
    /// about the keys in [`KEYS`].
    pub fn resync(&mut self, is_down: impl Fn(KeyCode) -> bool, out: &mut Vec<Input>) {
        // Touch history is unreliable after a drop: end any tap-drag.
        self.phys_left = is_down(KeyCode::BTN_LEFT);
        self.tap = Tap::Idle;
        self.pos = (None, None);
        self.sync_button(out);
        for (idx, key) in KEYS.iter().enumerate().skip(1) {
            self.set_key(idx, is_down(*key), out);
        }
    }
}

fn is_event_node(name: &OsStr) -> bool {
    name.to_str()
        .and_then(|s| s.strip_prefix("event"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// All `/dev/input/eventN` nodes, sorted by N.
pub fn list_event_nodes(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut v: Vec<(u32, PathBuf)> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| is_event_node(&e.file_name()))
        .filter_map(|e| {
            let n = e
                .file_name()
                .to_str()?
                .strip_prefix("event")?
                .parse()
                .ok()?;
            Some((n, e.path()))
        })
        .collect();
    v.sort();
    Ok(v.into_iter().map(|(_, p)| p).collect())
}

/// One-line summary of a node for `--check` (no event data is read).
pub struct Probe {
    pub path: PathBuf,
    pub name: String,
    pub result: Result<Caps, io::Error>,
}

pub fn probe(path: &Path) -> Probe {
    match RawDevice::open(path) {
        Ok(dev) => Probe {
            path: path.to_owned(),
            name: dev.name().unwrap_or("?").to_owned(),
            result: Ok(caps_of(&dev)),
        },
        Err(e) => Probe {
            path: path.to_owned(),
            name: sysfs_name(path).unwrap_or_default(),
            result: Err(e),
        },
    }
}

/// Device name from sysfs (world-readable, works without `input` group).
fn sysfs_name(path: &Path) -> Option<String> {
    let node = path.file_name()?;
    let p = Path::new("/sys/class/input").join(node).join("device/name");
    Some(std::fs::read_to_string(p).ok()?.trim().to_owned())
}

pub const PERMISSION_HINT: &str = "add your user to the `input` group \
(`sudo usermod -aG input $USER`) and log out and back in";

/// Owns the set of open devices and the hot-plug watcher.
pub struct Manager {
    tx: UnboundedSender<Input>,
    dir: PathBuf,
    open: Mutex<HashMap<PathBuf, DeviceId>>,
    /// Nodes that are neither pointer nor keyboard (not reopened on ATTRIB).
    skipped: Mutex<HashSet<PathBuf>>,
    retrying: Mutex<HashSet<PathBuf>>,
    next_id: AtomicU64,
    warned_permission: AtomicBool,
}

enum OpenOutcome {
    Opened,
    AlreadyOpen,
    Skipped,
    PermissionDenied,
    Gone,
    Failed,
}

impl Manager {
    pub fn new(tx: UnboundedSender<Input>) -> Arc<Self> {
        Arc::new(Manager {
            tx,
            dir: PathBuf::from(INPUT_DIR),
            open: Mutex::new(HashMap::new()),
            skipped: Mutex::new(HashSet::new()),
            retrying: Mutex::new(HashSet::new()),
            next_id: AtomicU64::new(1),
            warned_permission: AtomicBool::new(false),
        })
    }

    fn try_open(self: &Arc<Self>, path: &Path) -> OpenOutcome {
        if self.skipped.lock().unwrap().contains(path) {
            return OpenOutcome::Skipped;
        }
        let mut open = self.open.lock().unwrap();
        if open.contains_key(path) {
            return OpenOutcome::AlreadyOpen;
        }
        let dev = match RawDevice::open(path) {
            Ok(d) => d,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                if !self.warned_permission.swap(true, Ordering::Relaxed) {
                    log::warn!(
                        "permission denied opening {}: {PERMISSION_HINT}",
                        path.display()
                    );
                } else {
                    log::debug!("permission denied opening {}", path.display());
                }
                return OpenOutcome::PermissionDenied;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound || e.raw_os_error() == Some(19) => {
                return OpenOutcome::Gone;
            }
            Err(e) => {
                log::warn!("cannot open {}: {e}", path.display());
                return OpenOutcome::Failed;
            }
        };
        let caps = caps_of(&dev);
        let name = dev.name().unwrap_or("?").to_owned();
        if !caps.relevant() {
            self.skipped.lock().unwrap().insert(path.to_owned());
            return OpenOutcome::Skipped;
        }
        let stream = match dev.into_event_stream() {
            Ok(s) => s,
            Err(e) => {
                log::warn!("cannot watch {} ({name}): {e}", path.display());
                return OpenOutcome::Failed;
            }
        };
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        open.insert(path.to_owned(), id);
        drop(open);
        log::info!(
            "watching {} ({name}) as {}",
            path.display(),
            caps.describe()
        );
        let this = Arc::clone(self);
        let path = path.to_owned();
        tokio::spawn(async move {
            this.read_device(id, path, name, caps, stream).await;
        });
        OpenOutcome::Opened
    }

    async fn read_device(
        self: Arc<Self>,
        id: DeviceId,
        path: PathBuf,
        name: String,
        caps: Caps,
        mut stream: evdev::raw_stream::EventStream,
    ) {
        let mut tr = Translator::new(id, caps);
        let mut buf = Vec::new();
        loop {
            match stream.next_event().await {
                Ok(ev) => {
                    if tr.feed(ev, &mut buf) == Feed::NeedsResync {
                        match stream.device().get_key_state() {
                            Ok(state) => tr.resync(|k| state.contains(k), &mut buf),
                            // Unknown state: assume everything released.
                            Err(_) => tr.resync(|_| false, &mut buf),
                        }
                    }
                    for input in buf.drain(..) {
                        if self.tx.send(input).is_err() {
                            return;
                        }
                    }
                }
                Err(e) => {
                    if e.raw_os_error() != Some(19) {
                        log::warn!("read error on {} ({name}): {e}", path.display());
                    }
                    break;
                }
            }
        }
        log::info!("device removed: {} ({name})", path.display());
        self.open.lock().unwrap().remove(&path);
        let _ = self.tx.send(Input::DeviceRemoved { device: id });
    }

    /// udev applies group/ACLs shortly after the node appears; retry a few
    /// times (ATTRIB events from inotify also trigger a retry).
    fn schedule_retry(self: &Arc<Self>, path: PathBuf) {
        if !self.retrying.lock().unwrap().insert(path.clone()) {
            return;
        }
        let this = Arc::clone(self);
        tokio::spawn(async move {
            for ms in [100u64, 500, 1500, 4000] {
                tokio::time::sleep(Duration::from_millis(ms)).await;
                match this.try_open(&path) {
                    OpenOutcome::PermissionDenied | OpenOutcome::Failed => continue,
                    _ => break,
                }
            }
            this.retrying.lock().unwrap().remove(&path);
        });
    }

    fn open_or_retry(self: &Arc<Self>, path: PathBuf) {
        if let OpenOutcome::PermissionDenied | OpenOutcome::Failed = self.try_open(&path) {
            self.schedule_retry(path);
        }
    }

    /// Initial scan, then follow hot-plug events forever.
    pub async fn run(self: Arc<Self>) {
        // Set up the watch first so nothing plugged in during the scan is lost.
        let watcher = Inotify::init().and_then(|ino| {
            ino.watches().add(
                &self.dir,
                WatchMask::CREATE | WatchMask::ATTRIB | WatchMask::MOVED_TO | WatchMask::DELETE,
            )?;
            ino.into_event_stream([0u8; 4096])
        });
        match list_event_nodes(&self.dir) {
            Ok(nodes) => {
                let mut opened = 0;
                let mut denied = 0;
                for p in nodes {
                    match self.try_open(&p) {
                        OpenOutcome::Opened => opened += 1,
                        OpenOutcome::PermissionDenied => denied += 1,
                        _ => {}
                    }
                }
                log::info!("initial scan: {opened} device(s) watched, {denied} permission denied");
            }
            Err(e) => log::warn!("cannot list {}: {e}", self.dir.display()),
        }
        let mut events = match watcher {
            Ok(s) => s,
            Err(e) => {
                log::warn!(
                    "hot-plug disabled: cannot watch {}: {e}",
                    self.dir.display()
                );
                return;
            }
        };
        while let Some(ev) = events.next().await {
            let ev = match ev {
                Ok(ev) => ev,
                Err(e) => {
                    log::warn!("hot-plug watcher error: {e}");
                    continue;
                }
            };
            let Some(name) = ev.name.as_deref() else {
                continue;
            };
            if !is_event_node(name) {
                continue;
            }
            let path = self.dir.join(name);
            if ev.mask.contains(inotify::EventMask::DELETE) {
                // The device task notices ENODEV and cleans up itself.
                self.skipped.lock().unwrap().remove(&path);
            } else {
                self.open_or_retry(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: KeyCode, v: i32) -> InputEvent {
        InputEvent::new(EventType::KEY.0, k.0, v)
    }
    fn syn() -> InputEvent {
        InputEvent::new(
            EventType::SYNCHRONIZATION.0,
            SynchronizationCode::SYN_REPORT.0,
            0,
        )
    }
    fn rel(c: RelativeAxisCode, v: i32) -> InputEvent {
        InputEvent::new(EventType::RELATIVE.0, c.0, v)
    }
    fn abs(c: AbsoluteAxisCode, v: i32) -> InputEvent {
        InputEvent::new(EventType::ABSOLUTE.0, c.0, v)
    }

    fn feed_all(tr: &mut Translator, evs: &[InputEvent]) -> Vec<Input> {
        let mut out = Vec::new();
        for e in evs {
            tr.feed(*e, &mut out);
        }
        out
    }

    const PTR: Caps = Caps {
        pointer: true,
        keyboard: false,
        abs: None,
        touchpad: false,
    };

    #[test]
    fn mouse_button_and_motion() {
        let mut tr = Translator::new(7, PTR);
        let out = feed_all(
            &mut tr,
            &[
                key(KeyCode::BTN_LEFT, 1),
                rel(RelativeAxisCode::REL_X, 3),
                rel(RelativeAxisCode::REL_Y, -4),
                rel(RelativeAxisCode::REL_WHEEL, 1),
                syn(),
                key(KeyCode::BTN_RIGHT, 1),
                key(KeyCode::BTN_LEFT, 0),
                syn(),
            ],
        );
        assert_eq!(
            out,
            vec![
                Input::ButtonDown { device: 7 },
                Input::RelMotion { dx: 3.0, dy: -4.0 },
                Input::ButtonUp { device: 7 },
            ]
        );
    }

    #[test]
    fn keyboard_only_modifiers_and_repeat() {
        let kb = Caps {
            pointer: false,
            keyboard: true,
            abs: None,
            touchpad: false,
        };
        let mut tr = Translator::new(2, kb);
        let out = feed_all(
            &mut tr,
            &[
                key(KeyCode::KEY_A, 1),
                key(KeyCode::KEY_A, 0),
                key(KeyCode::KEY_RIGHTSHIFT, 1),
                key(KeyCode::KEY_RIGHTSHIFT, 2),
                key(KeyCode::KEY_RIGHTSHIFT, 2),
                key(KeyCode::KEY_LEFTMETA, 1),
                key(KeyCode::KEY_RIGHTSHIFT, 0),
                key(KeyCode::KEY_LEFTALT, 1),
            ],
        );
        assert_eq!(
            out,
            vec![
                Input::ModifierDown {
                    device: 2,
                    modifier: Modifier::Shift,
                    side: Side::Right
                },
                Input::ModifierUp {
                    device: 2,
                    modifier: Modifier::Shift,
                    side: Side::Right
                },
                Input::ModifierDown {
                    device: 2,
                    modifier: Modifier::Alt,
                    side: Side::Left
                },
            ]
        );
    }

    #[test]
    fn absolute_is_normalised_and_batched() {
        let caps = Caps {
            pointer: true,
            keyboard: false,
            abs: Some((100.0, 50.0, 0.5)),
            touchpad: false,
        };
        let mut tr = Translator::new(4, caps);
        let out = feed_all(
            &mut tr,
            &[
                abs(AbsoluteAxisCode::ABS_X, 300),
                abs(AbsoluteAxisCode::ABS_Y, 150),
                abs(AbsoluteAxisCode::ABS_MT_POSITION_X, 999),
                syn(),
                abs(AbsoluteAxisCode::ABS_Y, 250),
                syn(),
                key(KeyCode::BTN_TOUCH, 0),
            ],
        );
        assert_eq!(
            out,
            vec![
                Input::AbsMotion {
                    device: 4,
                    x: Some(100.0),
                    y: Some(50.0)
                },
                Input::AbsMotion {
                    device: 4,
                    x: None,
                    y: Some(100.0)
                },
                Input::AbsReset { device: 4 },
            ]
        );
    }

    #[test]
    fn syn_dropped_resync() {
        let mut tr = Translator::new(1, PTR);
        let mut out = Vec::new();
        tr.feed(key(KeyCode::BTN_LEFT, 1), &mut out);
        tr.feed(key(KeyCode::KEY_LEFTCTRL, 1), &mut out);
        out.clear();
        tr.feed(
            InputEvent::new(
                EventType::SYNCHRONIZATION.0,
                SynchronizationCode::SYN_DROPPED.0,
                0,
            ),
            &mut out,
        );
        tr.feed(rel(RelativeAxisCode::REL_X, 50), &mut out);
        tr.feed(key(KeyCode::BTN_LEFT, 0), &mut out);
        assert!(out.is_empty());
        assert_eq!(tr.feed(syn(), &mut out), Feed::NeedsResync);
        // Kernel now says: button released, ctrl still down, shift down.
        tr.resync(
            |k| k == KeyCode::KEY_LEFTCTRL || k == KeyCode::KEY_LEFTSHIFT,
            &mut out,
        );
        assert_eq!(
            out,
            vec![
                Input::AbsReset { device: 1 },
                Input::ButtonUp { device: 1 },
                Input::ModifierDown {
                    device: 1,
                    modifier: Modifier::Shift,
                    side: Side::Left
                },
            ]
        );
    }

    const PAD: Caps = Caps {
        pointer: true,
        keyboard: false,
        abs: Some((0.0, 0.0, 1.0)),
        touchpad: true,
    };

    /// Touchpad frames as `(time_ms, events)`; returns only button inputs.
    fn pad_buttons(tr: &mut Translator, frames: &[(u64, &[InputEvent])]) -> Vec<Input> {
        let mut out = Vec::new();
        for (ms, evs) in frames {
            for e in *evs {
                tr.feed_at(*e, ms * 1000, &mut out);
            }
            tr.feed_at(syn(), ms * 1000, &mut out);
        }
        out.retain(|i| matches!(i, Input::ButtonDown { .. } | Input::ButtonUp { .. }));
        out
    }

    fn at(x: i32, y: i32) -> [InputEvent; 2] {
        [
            abs(AbsoluteAxisCode::ABS_X, x),
            abs(AbsoluteAxisCode::ABS_Y, y),
        ]
    }

    fn down_at(x: i32, y: i32) -> [InputEvent; 4] {
        let [ax, ay] = at(x, y);
        [
            ax,
            ay,
            key(KeyCode::BTN_TOUCH, 1),
            key(KeyCode::BTN_TOOL_FINGER, 1),
        ]
    }

    fn up() -> [InputEvent; 2] {
        [key(KeyCode::BTN_TOUCH, 0), key(KeyCode::BTN_TOOL_FINGER, 0)]
    }

    type Frames = Vec<(u64, Vec<InputEvent>)>;

    const D1: Input = Input::ButtonDown { device: 9 };
    const U1: Input = Input::ButtonUp { device: 9 };

    #[test]
    fn touchpad_tap_and_drag_holds_button_until_lift() {
        let mut tr = Translator::new(9, PAD);
        let out = pad_buttons(
            &mut tr,
            &[
                (0, &down_at(500, 500)),
                (80, &up()),
                (200, &down_at(505, 500)),
                (300, &at(700, 600)),
                (900, &at(900, 700)),
                (1000, &up()),
            ],
        );
        assert_eq!(out, vec![D1, U1]);
    }

    #[test]
    fn touchpad_plain_touches_and_non_taps_press_nothing() {
        let cases: [(&str, Frames); 5] = [
            (
                "plain move",
                vec![(0, down_at(0, 0).to_vec()), (900, up().to_vec())],
            ),
            (
                "first touch too long",
                vec![
                    (0, down_at(0, 0).to_vec()),
                    (400, up().to_vec()),
                    (450, down_at(0, 0).to_vec()),
                ],
            ),
            (
                "second touch too late",
                vec![
                    (0, down_at(0, 0).to_vec()),
                    (80, up().to_vec()),
                    (700, down_at(0, 0).to_vec()),
                ],
            ),
            (
                "first touch moved (swipe)",
                vec![
                    (0, down_at(0, 0).to_vec()),
                    (40, at(300, 0).to_vec()),
                    (80, up().to_vec()),
                    (150, down_at(300, 0).to_vec()),
                ],
            ),
            (
                "two-finger tap (right click)",
                vec![
                    (0, down_at(0, 0).to_vec()),
                    (10, vec![key(KeyCode::BTN_TOOL_DOUBLETAP, 1)]),
                    (80, up().to_vec()),
                    (150, down_at(0, 0).to_vec()),
                ],
            ),
        ];
        for (name, frames) in cases {
            let mut tr = Translator::new(9, PAD);
            let frames: Vec<(u64, &[InputEvent])> =
                frames.iter().map(|(t, e)| (*t, e.as_slice())).collect();
            assert_eq!(pad_buttons(&mut tr, &frames), vec![], "{name}");
        }
    }

    #[test]
    fn touchpad_ignores_tap_drag_on_non_touchpads() {
        let mut tr = Translator::new(9, PTR);
        let out = pad_buttons(
            &mut tr,
            &[(0, &down_at(0, 0)), (80, &up()), (150, &down_at(0, 0))],
        );
        assert_eq!(out, vec![]);
    }

    #[test]
    fn touchpad_click_and_tap_drag_merge_into_one_button() {
        let mut tr = Translator::new(9, PAD);
        // Physical clickpad press: reported as is, never mistaken for a tap.
        let out = pad_buttons(
            &mut tr,
            &[
                (0, &down_at(0, 0)),
                (10, &[key(KeyCode::BTN_LEFT, 1)]),
                (60, &[key(KeyCode::BTN_LEFT, 0)]),
                (80, &up()),
                (150, &down_at(0, 0)),
                (400, &up()),
            ],
        );
        assert_eq!(out, vec![D1, U1]);
        // Pressing the pad during a tap-drag does not emit a second down, and
        // the button stays down until both are released.
        let mut tr = Translator::new(9, PAD);
        let out = pad_buttons(
            &mut tr,
            &[
                (0, &down_at(0, 0)),
                (80, &up()),
                (150, &down_at(0, 0)),
                (300, &[key(KeyCode::BTN_LEFT, 1)]),
                (400, &[key(KeyCode::BTN_LEFT, 0)]),
                (500, &up()),
            ],
        );
        assert_eq!(out, vec![D1, U1]);
    }

    #[test]
    fn touchpad_resync_ends_tap_drag() {
        let mut tr = Translator::new(9, PAD);
        let mut out = pad_buttons(
            &mut tr,
            &[(0, &down_at(0, 0)), (80, &up()), (150, &down_at(0, 0))],
        );
        assert_eq!(out, vec![D1]);
        out.clear();
        tr.resync(|_| false, &mut out);
        assert_eq!(out, vec![U1]);
    }

    #[test]
    fn event_node_names() {
        assert!(is_event_node(OsStr::new("event0")));
        assert!(is_event_node(OsStr::new("event12")));
        assert!(!is_event_node(OsStr::new("event")));
        assert!(!is_event_node(OsStr::new("mouse0")));
        assert!(!is_event_node(OsStr::new("event1x")));
        assert!(!is_event_node(OsStr::new("by-id")));
    }
}
