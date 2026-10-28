//! `~/.config/nashi/inputd.toml` (respects `$XDG_CONFIG_HOME`).

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::chord::{Mode, Settings};

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub enabled: bool,
    pub settings: Settings,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enabled: true,
            settings: Settings::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    enabled: Option<bool>,
    drag_hold_ms: Option<u64>,
    drag_move_px: Option<f64>,
    chords: Option<RawChords>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChords {
    shift: Option<String>,
    ctrl: Option<String>,
    alt: Option<String>,
}

/// Default config path: `$XDG_CONFIG_HOME/nashi/inputd.toml`, falling back to
/// `~/.config/nashi/inputd.toml`.
pub fn default_path() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(v) if Path::new(&v).is_absolute() => PathBuf::from(v),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("nashi").join("inputd.toml"))
}

/// Parse config text. Missing keys take defaults. A chord mapped to `""` or
/// `"none"` is unmapped; a section `[chords]` replaces only the keys it lists.
pub fn parse(text: &str) -> Result<Config, String> {
    let raw: RawConfig = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut cfg = Config::default();
    if let Some(e) = raw.enabled {
        cfg.enabled = e;
    }
    if let Some(ms) = raw.drag_hold_ms {
        if ms > 10_000 {
            return Err(format!("drag_hold_ms = {ms} is out of range (0..=10000)"));
        }
        cfg.settings.drag_hold_ms = ms;
    }
    if let Some(px) = raw.drag_move_px {
        if !px.is_finite() || !(0.0..=10_000.0).contains(&px) {
            return Err(format!("drag_move_px = {px} is out of range (0..=10000)"));
        }
        cfg.settings.drag_move_px = px;
    }
    if let Some(ch) = raw.chords {
        let conv = |name: &str, v: Option<String>, slot: &mut Option<Mode>| {
            match v {
            None => Ok(()),
            Some(s) if s.is_empty() || s == "none" => {
                *slot = None;
                Ok(())
            }
            Some(s) => match Mode::parse(&s) {
                Some(m) => {
                    *slot = Some(m);
                    Ok(())
                }
                None => Err(format!(
                    "chords.{name} = {s:?}: expected \"convert\", \"tools\", \"actions\" or \"none\""
                )),
            },
        }
        };
        conv("shift", ch.shift, &mut cfg.settings.shift)?;
        conv("ctrl", ch.ctrl, &mut cfg.settings.ctrl)?;
        conv("alt", ch.alt, &mut cfg.settings.alt)?;
    }
    Ok(cfg)
}

/// Outcome of loading the config file.
pub enum Loaded {
    /// File parsed fine.
    Ok(Config),
    /// File does not exist: defaults apply.
    Missing,
    /// File exists but could not be read or parsed.
    Invalid(String),
}

pub fn load(path: &Path) -> Loaded {
    match std::fs::read_to_string(path) {
        Ok(text) => match parse(&text) {
            Ok(c) => Loaded::Ok(c),
            Err(e) => Loaded::Invalid(e),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Loaded::Missing,
        Err(e) => Loaded::Invalid(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_default() {
        assert_eq!(parse("").unwrap(), Config::default());
    }

    #[test]
    fn documented_example_is_default() {
        let text = r#"
enabled = true
drag_hold_ms = 120
drag_move_px = 8
[chords]
shift = "convert"
ctrl  = "tools"
alt   = "actions"
"#;
        assert_eq!(parse(text).unwrap(), Config::default());
    }

    #[test]
    fn overrides() {
        let c = parse(
            "enabled = false\ndrag_hold_ms = 300\ndrag_move_px = 12.5\n[chords]\nshift = \"tools\"\nalt = \"none\"\n",
        )
        .unwrap();
        assert!(!c.enabled);
        assert_eq!(c.settings.drag_hold_ms, 300);
        assert_eq!(c.settings.drag_move_px, 12.5);
        assert_eq!(c.settings.shift, Some(Mode::Tools));
        assert_eq!(c.settings.ctrl, Some(Mode::Tools));
        assert_eq!(c.settings.alt, None);
    }

    #[test]
    fn invalid() {
        for bad in [
            "enabled = 3",
            "drag_hold_ms = -1",
            "drag_move_px = -2.0",
            "[chords]\nshift = \"explode\"",
            "unknown_key = 1",
            "not toml at all [",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn load_missing_and_invalid() {
        let dir = std::env::temp_dir().join(format!("nashi-inputd-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(matches!(load(&dir.join("nope.toml")), Loaded::Missing));
        let bad = dir.join("bad.toml");
        std::fs::write(&bad, "drag_hold_ms = \"x\"").unwrap();
        assert!(matches!(load(&bad), Loaded::Invalid(_)));
        let good = dir.join("good.toml");
        std::fs::write(&good, "drag_hold_ms = 50").unwrap();
        assert!(matches!(load(&good), Loaded::Ok(c) if c.settings.drag_hold_ms == 50));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
