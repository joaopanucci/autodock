//! Expo projects the user asked AutoDock to keep an eye on, and the alert that
//! tells when a newer SDK is out.
//!
//! The list lives in a small local file (`~/.autodock/expo.json`) and only holds
//! folder paths. A project is added only when the user asks for it. The latest SDK
//! is looked up on the npm registry only on an explicit "check now" or, when the
//! user turned it on, at most once a day when the app opens.
use crate::{
    expo::{self, ExpoProject, LatestSdk},
    ports::autodock_home,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const FILE: &str = "expo.json";
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_PROJECTS: usize = 50;
/// The automatic check runs at most once in this many seconds.
const AUTO_CHECK_EVERY: u64 = 24 * 60 * 60;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Store {
    #[serde(default)]
    check_on_start: bool,
    #[serde(default)]
    latest: Option<LatestSdk>,
    #[serde(default)]
    projects: Vec<String>,
    /// Project path -> latest SDK the user already saw the alert for and dismissed.
    #[serde(default)]
    dismissed: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_sdk: u32,
    pub latest_sdk: u32,
    /// SDKs are upgraded one at a time: this is the next one.
    pub next_sdk: u32,
    pub steps_behind: u32,
    /// The user dismissed the alert for this latest SDK.
    pub dismissed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchedProject {
    pub path: String,
    pub name: String,
    /// `None` when the folder is gone or is not an Expo project anymore.
    pub project: Option<ExpoProject>,
    pub update: Option<UpdateInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchState {
    pub schema_version: u32,
    pub check_on_start: bool,
    pub latest: Option<LatestSdk>,
    pub projects: Vec<WatchedProject>,
    /// Projects behind the latest SDK whose alert was not dismissed.
    pub alerts: u32,
    /// Why the automatic check could not run (offline, no curl…).
    pub check_error: Option<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Same folder, whatever the spelling: Windows extended prefix, trailing separators
/// and (on Windows) letter case and the kind of slash do not matter.
fn same_path(a: &str, b: &str) -> bool {
    let plain = |value: &str| {
        let value = value.strip_prefix(r"\\?\").unwrap_or(value);
        let value = value.trim_end_matches(['/', '\\']).to_string();
        if cfg!(windows) {
            value.replace('/', "\\").to_lowercase()
        } else {
            value
        }
    };
    plain(a) == plain(b)
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

fn update_info(
    project: &ExpoProject,
    latest: Option<&LatestSdk>,
    store: &Store,
) -> Option<UpdateInfo> {
    let current = project.sdk?;
    let latest = latest?;
    (latest.latest_sdk > current).then(|| UpdateInfo {
        current_sdk: current,
        latest_sdk: latest.latest_sdk,
        next_sdk: current + 1,
        steps_behind: latest.latest_sdk - current,
        dismissed: store
            .dismissed
            .iter()
            .any(|(path, seen)| same_path(path, &project.path) && *seen >= latest.latest_sdk),
    })
}

fn build_state(store: &Store, check_error: Option<String>) -> WatchState {
    let projects: Vec<WatchedProject> = store
        .projects
        .iter()
        .map(|path| {
            let project = expo::inspect(path).ok().flatten();
            let update = project
                .as_ref()
                .and_then(|p| update_info(p, store.latest.as_ref(), store));
            WatchedProject {
                path: path.clone(),
                name: project.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| {
                    Path::new(path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.clone())
                }),
                project,
                update,
            }
        })
        .collect();
    let alerts = projects
        .iter()
        .filter(|p| p.update.as_ref().is_some_and(|u| !u.dismissed))
        .count() as u32;
    WatchState {
        schema_version: 1,
        check_on_start: store.check_on_start,
        latest: store.latest.clone(),
        projects,
        alerts,
        check_error,
    }
}

pub fn state() -> Result<WatchState, String> {
    Ok(build_state(&load(&autodock_home()), None))
}

/// Starts watching a project. It must be an Expo project.
pub fn add_project(path: &str) -> Result<WatchState, String> {
    let project = expo::inspect(path)?.ok_or_else(|| {
        tr!(
            "Esta pasta não é um projeto Expo (o package.json não tem a dependência expo).",
            "This folder is not an Expo project (package.json has no expo dependency)."
        )
    })?;
    let home = autodock_home();
    let mut store = load(&home);
    if !store.projects.iter().any(|p| same_path(p, &project.path)) {
        if store.projects.len() >= MAX_PROJECTS {
            return Err(tr!(
                "No máximo {MAX_PROJECTS} projetos acompanhados.",
                "At most {MAX_PROJECTS} watched projects."
            ));
        }
        store.projects.push(project.path.clone());
        save(&home, &store)?;
    }
    Ok(build_state(&store, None))
}

pub fn remove_project(path: &str) -> Result<WatchState, String> {
    let home = autodock_home();
    let mut store = load(&home);
    // The folder may be gone, so the paths are compared as text.
    store.projects.retain(|p| !same_path(p, path));
    store.dismissed.retain(|p, _| !same_path(p, path));
    save(&home, &store)?;
    Ok(build_state(&store, None))
}

pub fn set_check_on_start(enabled: bool) -> Result<WatchState, String> {
    let home = autodock_home();
    let mut store = load(&home);
    store.check_on_start = enabled;
    save(&home, &store)?;
    Ok(build_state(&store, None))
}

/// Hides the alert of a project until a newer SDK than the current latest appears.
pub fn dismiss_update(path: &str) -> Result<WatchState, String> {
    let home = autodock_home();
    let mut store = load(&home);
    let Some(stored) = store.projects.iter().find(|p| same_path(p, path)).cloned() else {
        return Err(tr!(
            "Este projeto não está na lista de acompanhados.",
            "This project is not in the watched list."
        ));
    };
    let Some(latest) = store.latest.as_ref().map(|l| l.latest_sdk) else {
        return Err(tr!(
            "Consulte o último SDK antes de dispensar o aviso.",
            "Look up the latest SDK before dismissing the alert."
        ));
    };
    store.dismissed.retain(|p, _| !same_path(p, &stored));
    store.dismissed.insert(stored, latest);
    save(&home, &store)?;
    Ok(build_state(&store, None))
}

/// Explicit check: asks the npm registry and remembers the answer.
pub fn refresh_latest() -> Result<WatchState, String> {
    refresh_with(expo::fetch_latest_sdk)
}

fn refresh_with(fetch: impl FnOnce() -> Result<LatestSdk, String>) -> Result<WatchState, String> {
    let home = autodock_home();
    let mut store = load(&home);
    store.latest = Some(fetch()?);
    save(&home, &store)?;
    Ok(build_state(&store, None))
}

/// Run when the app opens: checks the registry only if the user turned it on and
/// the last answer is older than a day. A failure never gets in the way.
pub fn auto_check() -> Result<WatchState, String> {
    auto_check_with(expo::fetch_latest_sdk, now())
}

fn auto_check_with(
    fetch: impl FnOnce() -> Result<LatestSdk, String>,
    now: u64,
) -> Result<WatchState, String> {
    let home = autodock_home();
    let mut store = load(&home);
    let due = store.check_on_start
        && !store.projects.is_empty()
        && store
            .latest
            .as_ref()
            .is_none_or(|l| now.saturating_sub(l.checked_at) >= AUTO_CHECK_EVERY);
    if !due {
        return Ok(build_state(&store, None));
    }
    match fetch() {
        Ok(latest) => {
            store.latest = Some(latest);
            save(&home, &store)?;
            Ok(build_state(&store, None))
        }
        Err(error) => Ok(build_state(&store, Some(error))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expo_project(version: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            format!(r#"{{"name":"meu-app","dependencies":{{"expo":"{version}"}}}}"#),
        )
        .unwrap();
        dir
    }

    fn path(dir: &tempfile::TempDir) -> String {
        dir.path().to_string_lossy().into_owned()
    }

    fn latest(sdk: u32, checked_at: u64) -> LatestSdk {
        LatestSdk {
            checked_at,
            latest_version: format!("{sdk}.0.1"),
            latest_sdk: sdk,
            available_sdks: vec![sdk - 1, sdk],
        }
    }

    #[test]
    fn only_expo_projects_can_be_watched_and_adding_twice_keeps_one() {
        let plain = tempfile::tempdir().unwrap();
        fs::write(
            plain.path().join("package.json"),
            r#"{"dependencies":{"react":"19"}}"#,
        )
        .unwrap();
        assert!(add_project(&path(&plain))
            .unwrap_err()
            .contains("not an Expo project"));
        assert!(add_project("/definitely/not/a/folder").is_err());

        let app = expo_project("~57.0.0");
        add_project(&path(&app)).unwrap();
        let state = add_project(&path(&app)).unwrap();
        assert_eq!(state.projects.len(), 1);
        assert_eq!(state.projects[0].name, "meu-app");
        assert_eq!(state.projects[0].project.as_ref().unwrap().sdk, Some(57));
        assert!(
            state.latest.is_none() && state.alerts == 0,
            "no alert before any check"
        );
    }

    #[test]
    fn the_alert_appears_when_the_latest_sdk_is_ahead_and_can_be_dismissed() {
        let app = expo_project("~57.0.0");
        add_project(&path(&app)).unwrap();
        let state = refresh_with(|| Ok(latest(58, 100))).unwrap();
        let update = state.projects[0].update.clone().unwrap();
        assert_eq!(
            (
                update.current_sdk,
                update.latest_sdk,
                update.next_sdk,
                update.steps_behind,
                update.dismissed
            ),
            (57, 58, 58, 1, false)
        );
        assert_eq!(state.alerts, 1);

        let state = dismiss_update(&path(&app)).unwrap();
        assert!(state.projects[0].update.as_ref().unwrap().dismissed);
        assert_eq!(state.alerts, 0);

        // a newer SDK brings the alert back
        let state = refresh_with(|| Ok(latest(59, 200))).unwrap();
        let update = state.projects[0].update.clone().unwrap();
        assert_eq!(
            (update.steps_behind, update.next_sdk, update.dismissed),
            (2, 58, false)
        );
        assert_eq!(state.alerts, 1);
    }

    #[test]
    fn dismissing_needs_a_watched_project_and_a_known_latest_sdk() {
        let app = expo_project("~57.0.0");
        assert!(dismiss_update(&path(&app))
            .unwrap_err()
            .contains("not in the watched list"));
        add_project(&path(&app)).unwrap();
        assert!(dismiss_update(&path(&app))
            .unwrap_err()
            .contains("Look up the latest SDK"));
        refresh_with(|| Ok(latest(58, 1))).unwrap();
        dismiss_update(&path(&app)).unwrap();
        // dismissing twice keeps a single entry
        dismiss_update(&path(&app)).unwrap();
        assert_eq!(load(&autodock_home()).dismissed.len(), 1);
    }

    #[test]
    fn paths_are_stored_plainly_and_matched_whatever_the_slashes() {
        let app = expo_project("~57.0.0");
        let state = add_project(&path(&app)).unwrap();
        let stored = &state.projects[0].path;
        assert!(
            !stored.starts_with(r"\\?\"),
            "the verbatim prefix is not kept: {stored}"
        );
        // the same folder spelled with the other kind of slash, a trailing separator or other case
        let mut spellings = vec![format!("{}/", path(&app))];
        if cfg!(windows) {
            spellings.push(path(&app).replace('\\', "/"));
            spellings.push(path(&app).to_uppercase());
        }
        for spelling in spellings {
            assert_eq!(
                add_project(&spelling).unwrap().projects.len(),
                1,
                "{spelling}"
            );
        }
        assert!(remove_project(&path(&app).replace('\\', "/"))
            .unwrap()
            .projects
            .is_empty());
    }

    #[test]
    fn up_to_date_and_ahead_projects_get_no_alert() {
        let current = expo_project("^58.0.0");
        let ahead = expo_project("^59.0.0");
        add_project(&path(&current)).unwrap();
        add_project(&path(&ahead)).unwrap();
        let state = refresh_with(|| Ok(latest(58, 1))).unwrap();
        assert!(state.projects.iter().all(|p| p.update.is_none()));
        assert_eq!(state.alerts, 0);
    }

    #[test]
    fn removing_a_project_forgets_it_even_when_the_folder_is_gone() {
        let app = expo_project("~57.0.0");
        let kept = expo_project("~57.0.0");
        add_project(&path(&app)).unwrap();
        add_project(&path(&kept)).unwrap();
        let gone = path(&app);
        drop(app);
        let state = state().unwrap();
        assert!(
            state.projects.iter().any(|p| p.project.is_none()),
            "a missing folder is listed without details"
        );
        let state = remove_project(&gone).unwrap();
        assert_eq!(state.projects.len(), 1);
        assert_eq!(state.projects[0].project.as_ref().unwrap().sdk, Some(57));
    }

    #[test]
    fn the_automatic_check_is_off_by_default_and_runs_at_most_once_a_day() {
        let app = expo_project("~57.0.0");
        add_project(&path(&app)).unwrap();
        let calls = std::cell::Cell::new(0);
        let fetch = |sdk: u32, at: u64| {
            calls.set(calls.get() + 1);
            Ok(latest(sdk, at))
        };

        // off: nothing is asked
        let state = auto_check_with(|| fetch(58, 1_000), 1_000).unwrap();
        assert_eq!((calls.get(), state.latest.is_none()), (0, true));

        set_check_on_start(true).unwrap();
        let state = auto_check_with(|| fetch(58, 1_000), 1_000).unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(state.latest.unwrap().latest_sdk, 58);

        // asked again within a day: answered from the stored value
        auto_check_with(|| fetch(59, 2_000), 2_000).unwrap();
        assert_eq!(calls.get(), 1);
        // a day later it asks again
        let state = auto_check_with(
            || fetch(59, 1_000 + AUTO_CHECK_EVERY),
            1_000 + AUTO_CHECK_EVERY,
        )
        .unwrap();
        assert_eq!((calls.get(), state.latest.unwrap().latest_sdk), (2, 59));
    }

    #[test]
    fn a_failed_automatic_check_reports_the_reason_and_keeps_the_last_answer() {
        let app = expo_project("~57.0.0");
        add_project(&path(&app)).unwrap();
        set_check_on_start(true).unwrap();
        refresh_with(|| Ok(latest(58, 1))).unwrap();
        let state = auto_check_with(|| Err("offline".into()), 1 + AUTO_CHECK_EVERY).unwrap();
        assert_eq!(state.check_error.as_deref(), Some("offline"));
        assert_eq!(state.latest.unwrap().latest_sdk, 58);
        assert!(
            refresh_with(|| Err("offline".into())).is_err(),
            "an explicit check reports the error"
        );
    }

    #[test]
    fn a_corrupted_file_is_kept_aside_and_never_crashes() {
        let home = autodock_home();
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(FILE), "{ broken").unwrap();
        assert!(state().unwrap().projects.is_empty());
        assert!(home.join("expo.json.invalid").is_file());
        let app = expo_project("~57.0.0");
        assert_eq!(add_project(&path(&app)).unwrap().projects.len(), 1);
    }

    #[test]
    fn the_list_has_a_limit() {
        let apps: Vec<tempfile::TempDir> = (0..=MAX_PROJECTS)
            .map(|_| expo_project("~57.0.0"))
            .collect();
        for app in apps.iter().take(MAX_PROJECTS) {
            add_project(&path(app)).unwrap();
        }
        assert!(add_project(&path(&apps[MAX_PROJECTS]))
            .unwrap_err()
            .contains("At most"));
    }
}
