//! Preferences of the in-app update check, kept in `~/.autodock/update.json`.
//!
//! The check itself (a request to the release's `latest.json`) lives in the desktop app;
//! this module only decides *when* it may run by itself and remembers what the user chose:
//! whether to check when the app opens (on by default, at most once a day) and which
//! version they asked to skip. Nothing is ever installed without the user clicking.
use crate::ports::autodock_home;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const FILE: &str = "update.json";
const MAX_BYTES: u64 = 16 * 1024;
/// The automatic check runs at most once in this many seconds.
const CHECK_EVERY: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Store {
    check_on_start: bool,
    /// Seconds since the Unix epoch of the last successful check.
    last_check: Option<u64>,
    skipped_version: Option<String>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            check_on_start: true,
            last_check: None,
            skipped_version: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePrefs {
    pub schema_version: u32,
    pub check_on_start: bool,
    pub last_check: Option<u64>,
    pub skipped_version: Option<String>,
}

impl From<&Store> for UpdatePrefs {
    fn from(store: &Store) -> Self {
        Self {
            schema_version: 1,
            check_on_start: store.check_on_start,
            last_check: store.last_check,
            skipped_version: store.skipped_version.clone(),
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn load(home: &Path) -> Store {
    let path = home.join(FILE);
    let Ok(meta) = fs::symlink_metadata(&path) else {
        return Store::default();
    };
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Store::default();
    }
    match fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Store>(&text).ok())
    {
        Some(store) => store,
        None => {
            // Never overwrite a file we cannot read: keep it aside.
            let _ = fs::rename(&path, home.join(format!("{FILE}.invalid")));
            Store::default()
        }
    }
}

fn save(home: &Path, store: &Store) -> Result<(), String> {
    fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    let temporary = home.join(format!("{FILE}.tmp"));
    fs::write(&temporary, text).map_err(|e| e.to_string())?;
    fs::rename(&temporary, home.join(FILE)).map_err(|e| e.to_string())
}

/// `1.2.3` or `1.2.3-beta.1`: what a release tag looks like without the `v`.
fn valid_version(value: &str) -> bool {
    let (core, pre) = match value.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (value, None),
    };
    let numbers: Vec<&str> = core.split('.').collect();
    numbers.len() == 3
        && numbers
            .iter()
            .all(|n| !n.is_empty() && n.len() <= 9 && n.bytes().all(|b| b.is_ascii_digit()))
        && pre.is_none_or(|p| {
            !p.is_empty()
                && p.len() <= 30
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        })
}

pub fn prefs() -> Result<UpdatePrefs, String> {
    Ok(UpdatePrefs::from(&load(&autodock_home())))
}

pub fn set_check_on_start(enabled: bool) -> Result<UpdatePrefs, String> {
    let home = autodock_home();
    let mut store = load(&home);
    store.check_on_start = enabled;
    save(&home, &store)?;
    Ok(UpdatePrefs::from(&store))
}

/// The automatic check may run now: it is on and the last one is over a day old.
pub fn auto_check_due() -> bool {
    due(&load(&autodock_home()), now())
}

fn due(store: &Store, now: u64) -> bool {
    store.check_on_start
        && store
            .last_check
            // A clock set back counts as due rather than blocking the check for days.
            .is_none_or(|last| now < last || now - last >= CHECK_EVERY)
}

/// Remembers that a check just succeeded.
pub fn record_check() -> Result<(), String> {
    let home = autodock_home();
    let mut store = load(&home);
    store.last_check = Some(now());
    save(&home, &store)
}

/// The user asked not to be told about this version again (a newer one is offered).
pub fn skip_version(version: &str) -> Result<UpdatePrefs, String> {
    if !valid_version(version) {
        return Err(tr!("Versão inválida.", "Invalid version."));
    }
    let home = autodock_home();
    let mut store = load(&home);
    store.skipped_version = Some(version.to_string());
    save(&home, &store)?;
    Ok(UpdatePrefs::from(&store))
}

pub fn is_skipped(version: &str) -> bool {
    load(&autodock_home()).skipped_version.as_deref() == Some(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean() {
        let home = autodock_home();
        for name in [
            FILE.to_string(),
            format!("{FILE}.invalid"),
            format!("{FILE}.tmp"),
        ] {
            let _ = fs::remove_file(home.join(name));
        }
    }

    #[test]
    fn the_check_is_on_by_default_and_can_be_turned_off() {
        clean();
        let prefs = prefs().unwrap();
        assert!(
            prefs.check_on_start && prefs.last_check.is_none() && prefs.skipped_version.is_none()
        );
        assert!(auto_check_due(), "never checked: due");
        assert!(!set_check_on_start(false).unwrap().check_on_start);
        assert!(!auto_check_due(), "turned off: never due");
        assert!(set_check_on_start(true).unwrap().check_on_start);
    }

    #[test]
    fn the_automatic_check_runs_at_most_once_a_day() {
        let mut store = Store::default();
        assert!(due(&store, 1_000));
        store.last_check = Some(1_000);
        assert!(!due(&store, 1_000 + CHECK_EVERY - 1));
        assert!(due(&store, 1_000 + CHECK_EVERY));
        assert!(
            due(&store, 500),
            "a clock set back does not block the check"
        );
        store.check_on_start = false;
        assert!(!due(&store, 1_000 + 10 * CHECK_EVERY));

        clean();
        assert!(auto_check_due());
        record_check().unwrap();
        assert!(!auto_check_due(), "just checked");
        assert!(prefs().unwrap().last_check.is_some());
    }

    #[test]
    fn a_skipped_version_stays_skipped_until_a_newer_one() {
        clean();
        assert!(!is_skipped("0.4.0"));
        assert_eq!(
            skip_version("0.4.0").unwrap().skipped_version.as_deref(),
            Some("0.4.0")
        );
        assert!(is_skipped("0.4.0"));
        assert!(!is_skipped("0.4.1"), "a newer version is offered again");
        skip_version("0.4.1").unwrap();
        assert!(!is_skipped("0.4.0") && is_skipped("0.4.1"));
    }

    #[test]
    fn only_release_like_versions_are_accepted() {
        for good in ["0.4.0", "1.22.333", "0.5.0-beta.1", "2.0.0-rc-1"] {
            assert!(valid_version(good), "{good}");
        }
        for bad in [
            "",
            "0.4",
            "v0.4.0",
            "0.4.0.1",
            "a.b.c",
            "0.4.0-",
            "0.4.0-b ad",
            "../1.0.0",
            "1.0.0\n",
            "0..1",
        ] {
            assert!(!valid_version(bad), "{bad:?}");
            assert!(skip_version(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_corrupted_file_is_kept_aside_and_never_crashes() {
        clean();
        let home = autodock_home();
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(FILE), "{ not json").unwrap();
        assert!(prefs().unwrap().check_on_start, "defaults");
        assert!(home.join(format!("{FILE}.invalid")).is_file());
        // unknown or missing fields fall back to the defaults too
        fs::write(home.join(FILE), r#"{"lastCheck": 5}"#).unwrap();
        let prefs = prefs().unwrap();
        assert!(prefs.check_on_start);
        assert_eq!(prefs.last_check, Some(5));
    }
}
