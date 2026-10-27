//! The environment variables the app reads, read once at startup into [`Env`]. Library crates
//! never read these; `main` passes the values down. Platform variables (`HOME`, `DISPLAY`, ...)
//! and test-only ones stay with the code that needs them.
//!
//! `RUST_LOG` is not a field: the logger reads it ([`rust_log`]) before anything else, so that
//! the warnings [`Switch`] logs for a malformed value reach the log file.

use std::ffi::OsString;
use std::path::PathBuf;

use soundcraft_ui_egui::switch::Switch;

pub(crate) const CONFIG_DIR: &str = "SOUNDCRAFT_CONFIG_DIR";
pub(crate) const NO_PREFS: &str = "SOUNDCRAFT_NO_PREFS";
pub(crate) const NO_SYSTEM_FONTS: &str = "SOUNDCRAFT_NO_SYSTEM_FONTS";
pub(crate) const WAYLAND: &str = "SOUNDCRAFT_WAYLAND";
pub(crate) const RUST_LOG: &str = "RUST_LOG";

/// The variables and their `--help` lines, in the order `--help` lists them.
pub(crate) const VARS: &[(&str, &str)] = &[
    // Read by clap (`env =` on `--control`), not by `Env`.
    ("SOUNDCRAFT_CONTROL_PORT", "The --control fallback: a port or host:port, ignored for another host"),
    (CONFIG_DIR, "The settings folder (ui.json, Autosave, Presets, logs); empty: none"),
    (NO_PREFS, "1 doesn't read or write preferences and logs to stderr only; 0 or unset is off"),
    (NO_SYSTEM_FONTS, "1 uses the built-in fonts only; 0 or unset is off"),
    (WAYLAND, "Linux: 1 keeps the native Wayland backend instead of XWayland; 0 or unset is off"),
    (RUST_LOG, "Log filter, env_logger style: debug, warn,soundcraft_mix=trace"),
];

/// The `--help` Environment section, generated from [`VARS`].
pub(crate) fn help() -> String {
    let width = VARS.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
    let mut out = String::from("Environment:\n");
    for (name, line) in VARS {
        out.push_str(&format!("  {name:<width$}  {line}\n"));
        if *name == WAYLAND {
            out.push_str("  The on/off variables take 1/on/true/yes or 0/off/false/no, in any case; any other value is\n  ignored with a warning in the log.\n");
        }
    }
    out.truncate(out.trim_end().len());
    out
}

/// The app's variables, as read at startup.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Env {
    /// `SOUNDCRAFT_CONFIG_DIR`: `Some("")` disables the settings folder.
    pub(crate) config_dir: Option<PathBuf>,
    pub(crate) no_prefs: Switch,
    pub(crate) no_system_fonts: Switch,
    pub(crate) wayland: Switch,
}

impl Env {
    /// Reads the process environment. Call after the logger is installed, so malformed switches are logged.
    pub(crate) fn read() -> Env {
        Env::from_lookup(|name| std::env::var_os(name))
    }

    pub(crate) fn from_lookup(get: impl Fn(&str) -> Option<OsString>) -> Env {
        let switch = |name: &str| match get(name) {
            None => Switch::Default,
            Some(v) => Switch::parse(name, Some(&v.to_string_lossy())),
        };
        Env {
            config_dir: get(CONFIG_DIR).map(PathBuf::from),
            no_prefs: switch(NO_PREFS),
            no_system_fonts: switch(NO_SYSTEM_FONTS),
            wayland: switch(WAYLAND),
        }
    }

    /// Runs without preferences (`SOUNDCRAFT_NO_PREFS`, agents' test runs) neither read nor write them.
    pub(crate) fn prefs_enabled(&self) -> bool {
        self.no_prefs != Switch::On
    }

    pub(crate) fn system_fonts(&self) -> bool {
        self.no_system_fonts != Switch::On
    }

    /// Whether to run through XWayland: winit has no file drag-and-drop on Wayland, so dropping
    /// files from the file manager showed a "no" cursor. Needs a nonempty `DISPLAY`
    /// (`display`); `SOUNDCRAFT_WAYLAND=1` keeps the native backend.
    #[cfg(any(test, all(unix, not(target_os = "macos"))))]
    pub(crate) fn use_xwayland(&self, display: Option<&OsString>) -> bool {
        display.is_some_and(|d| !d.is_empty()) && self.wayland != Switch::On
    }
}

/// The `RUST_LOG` filter, `None` when unset or blank.
pub(crate) fn rust_log() -> Option<String> {
    std::env::var(RUST_LOG).ok().filter(|s| !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(vars: &'static [(&'static str, &'static str)]) -> Env {
        Env::from_lookup(|name| vars.iter().find(|(n, _)| *n == name).map(|(_, v)| OsString::from(v)))
    }

    #[test]
    fn variables_are_read_into_typed_fields() {
        let env = lookup(&[(CONFIG_DIR, "/c"), (NO_PREFS, "yes"), (NO_SYSTEM_FONTS, "0"), (WAYLAND, "bogus")]);
        assert_eq!(env.config_dir, Some(PathBuf::from("/c")));
        assert_eq!((env.no_prefs, env.no_system_fonts, env.wayland), (Switch::On, Switch::Off, Switch::Default));
        assert_eq!(lookup(&[]), Env::default());
        assert_eq!(lookup(&[(CONFIG_DIR, "")]).config_dir, Some(PathBuf::new()));
    }

    #[test]
    fn no_prefs_and_no_system_fonts_turn_features_off() {
        assert!(Env::default().prefs_enabled() && Env::default().system_fonts());
        let env = Env { no_prefs: Switch::On, no_system_fonts: Switch::On, ..Env::default() };
        assert!(!env.prefs_enabled() && !env.system_fonts());
        assert!(Env { no_prefs: Switch::Off, ..Env::default() }.prefs_enabled());
    }

    #[test]
    fn xwayland_needs_a_display_and_wayland_not_forced() {
        let display = OsString::from(":0");
        assert!(Env::default().use_xwayland(Some(&display)));
        assert!(Env { wayland: Switch::Off, ..Env::default() }.use_xwayland(Some(&display)));
        assert!(!Env { wayland: Switch::On, ..Env::default() }.use_xwayland(Some(&display)));
        assert!(!Env::default().use_xwayland(None));
        assert!(!Env::default().use_xwayland(Some(&OsString::new())));
    }

    #[test]
    fn help_lists_every_variable() {
        let help = help();
        for (name, line) in VARS {
            assert!(help.contains(name) && help.contains(line), "{name} missing from\n{help}");
        }
        assert!(help.contains("0/off/false/no"), "{help}");
    }
}
