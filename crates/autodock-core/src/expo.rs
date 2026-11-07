//! Expo projects: which SDK a project is on, and which SDK is the latest stable.
//!
//! Inspecting a project only reads `package.json`, the lock file, `.gitignore` and
//! `node_modules/expo/package.json` as data. Nothing is executed. The latest SDK
//! comes from the public npm registry and is only asked for on an explicit action
//! (or when the user turned the automatic check on).
use crate::{
    doctor::{resolve, ToolStatus},
    generate::{self, read_manifest, regular_file},
    memory_capture,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Public endpoint with the dist-tags of the `expo` package (a few hundred bytes).
const DIST_TAGS_URL: &str = "https://registry.npmjs.org/-/package/expo/dist-tags";
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;
const CHECK_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeFolder {
    pub name: String,
    /// The folder is listed in `.gitignore`, so it is generated (Continuous Native
    /// Generation) and recreated on the next build.
    pub ignored_by_git: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpoProject {
    pub schema_version: u32,
    /// Folder of the project.
    pub path: String,
    pub name: String,
    /// What `package.json` asks for, e.g. `~57.0.3`.
    pub declared_version: String,
    /// What is installed in `node_modules`, when it is there.
    pub installed_version: Option<String>,
    /// SDK number: the major version of the `expo` package.
    pub sdk: Option<u32>,
    pub react_native: Option<String>,
    pub uses_router: bool,
    /// `npm`, `yarn`, `pnpm` or `bun`, from the lock file.
    pub package_manager: Option<String>,
    pub lock_file: Option<String>,
    pub native_folders: Vec<NativeFolder>,
    /// AutoDock can upgrade this project (see `unsupported_reason` otherwise).
    pub upgrade_supported: bool,
    pub unsupported_reason: Option<String>,
}

/// Major version in a dependency spec such as `^57.0.1`, `~57.0.0`, `57.0.0`, `>=57`.
pub(crate) fn major_of(spec: &str) -> Option<u32> {
    let trimmed = spec
        .trim()
        .trim_start_matches(['^', '~', '>', '=', 'v', ' ']);
    let digits: String = trimmed.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    // "57" and "57.x" count; "5.7" is a different major, which the digits above handle.
    match trimmed[digits.len()..].chars().next() {
        None | Some('.') | Some(' ') | Some('x') | Some('X') | Some('-') | Some('|') => {
            digits.parse().ok()
        }
        _ => None,
    }
}

fn dependency(json: &serde_json::Value, name: &str) -> Option<String> {
    ["dependencies", "devDependencies"]
        .iter()
        .find_map(|group| {
            json.get(group)
                .and_then(|d| d.get(name))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
}

/// True when `.gitignore` lists the folder (the usual spellings only).
fn ignored_by_git(root: &Path, folder: &str) -> bool {
    read_manifest(root, ".gitignore").is_some_and(|text| {
        text.lines().map(str::trim).any(|line| {
            !line.starts_with('#')
                && !line.starts_with('!')
                && [
                    folder.to_string(),
                    format!("/{folder}"),
                    format!("{folder}/"),
                    format!("/{folder}/"),
                ]
                .contains(&line.to_string())
        })
    })
}

fn lock_file(root: &Path) -> (Option<String>, Option<String>) {
    for (file, manager) in [
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "yarn"),
        ("package-lock.json", "npm"),
        ("bun.lock", "bun"),
        ("bun.lockb", "bun"),
    ] {
        if regular_file(root, file) {
            return (Some(file.into()), Some(manager.into()));
        }
    }
    (None, None)
}

/// A parent folder whose `package.json` declares workspaces (the project is a package of a monorepo).
fn workspace_root_above(root: &Path) -> Option<PathBuf> {
    let mut current = root.parent();
    for _ in 0..4 {
        let folder = current?;
        let has_workspaces = read_manifest(folder, "package.json")
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .is_some_and(|json| json.get("workspaces").is_some());
        if has_workspaces {
            return Some(folder.to_path_buf());
        }
        current = folder.parent();
    }
    None
}

/// Reads an Expo project. `None` when the folder is not one (no `expo` dependency).
pub fn inspect(path: &str) -> Result<Option<ExpoProject>, String> {
    let root = generate::root_of(path)?;
    let Some(text) = read_manifest(&root, "package.json") else {
        return Ok(None);
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(None);
    };
    let Some(declared) = dependency(&json, "expo") else {
        return Ok(None);
    };
    let installed = read_manifest(&root.join("node_modules").join("expo"), "package.json")
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("version").and_then(|v| v.as_str()).map(str::to_owned));
    // The installed version is what actually runs; the declared one is the fallback.
    let sdk = installed
        .as_deref()
        .and_then(major_of)
        .or_else(|| major_of(&declared));
    let (lock, manager) = lock_file(&root);
    let native_folders = ["android", "ios"]
        .iter()
        .filter(|name| std::fs::symlink_metadata(root.join(name)).is_ok_and(|m| m.is_dir()))
        .map(|name| NativeFolder {
            name: (*name).into(),
            ignored_by_git: ignored_by_git(&root, name),
        })
        .collect();
    let unsupported_reason = if manager.as_deref() == Some("bun") {
        Some(tr!(
            "Projetos com bun ainda não são suportados (o upgrade roda com npm, yarn ou pnpm).",
            "Projects using bun are not supported yet (the upgrade runs with npm, yarn or pnpm)."
        ))
    } else if json.get("workspaces").is_some() {
        Some(tr!(
            "Este package.json é a raiz de um workspace: abra o app Expo dentro dele.",
            "This package.json is a workspace root: open the Expo app inside it."
        ))
    } else if workspace_root_above(&root).is_some() {
        Some(tr!(
            "O projeto faz parte de um monorepo (workspace): o upgrade de monorepos ainda não é suportado.",
            "The project is part of a monorepo (workspace): upgrading monorepos is not supported yet."
        ))
    } else if sdk.is_none() {
        Some(tr!(
            "Não foi possível ler a versão do Expo ({declared}). Use uma versão numérica, como ~57.0.0.",
            "Could not read the Expo version ({declared}). Use a numeric version, such as ~57.0.0."
        ))
    } else {
        None
    };
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|n| !n.is_empty() && n.len() <= 100)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            root.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    Ok(Some(ExpoProject {
        schema_version: 1,
        path: generate::plain_path(&root),
        name,
        declared_version: declared,
        installed_version: installed,
        sdk,
        react_native: dependency(&json, "react-native"),
        uses_router: dependency(&json, "expo-router").is_some(),
        package_manager: manager.or_else(|| Some("npm".into())),
        lock_file: lock,
        native_folders,
        upgrade_supported: unsupported_reason.is_none(),
        unsupported_reason,
    }))
}

// ───────────────────────────── latest SDK ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestSdk {
    /// Seconds since the Unix epoch.
    pub checked_at: u64,
    /// Version of the `latest` tag, e.g. `58.0.23`.
    pub latest_version: String,
    pub latest_sdk: u32,
    /// SDK numbers that have a `sdk-N` tag, plus the latest one.
    pub available_sdks: Vec<u32>,
}

fn stable_version(value: &str) -> Option<u32> {
    let mut parts = value.split('.');
    let (major, minor, patch) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some()
        || ![major, minor, patch]
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    major.parse().ok()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Reads the dist-tags JSON. Only the stable `latest` tag defines the latest SDK
/// (never `next` or `canary`).
pub(crate) fn parse_dist_tags(text: &str, checked_at: u64) -> Result<LatestSdk, String> {
    let invalid = || {
        tr!(
            "Resposta inesperada do registro do npm.",
            "Unexpected answer from the npm registry."
        )
    };
    let json: serde_json::Value = serde_json::from_str(text).map_err(|_| invalid())?;
    let tags = json.as_object().ok_or_else(invalid)?;
    let latest_version = tags
        .get("latest")
        .and_then(|v| v.as_str())
        .ok_or_else(invalid)?
        .to_string();
    let latest_sdk = stable_version(&latest_version).ok_or_else(invalid)?;
    let mut available: Vec<u32> = tags
        .iter()
        .filter_map(|(tag, version)| {
            let number = tag.strip_prefix("sdk-")?.parse::<u32>().ok()?;
            stable_version(version.as_str()?).filter(|major| *major == number)
        })
        .collect();
    available.push(latest_sdk);
    available.sort_unstable();
    available.dedup();
    Ok(LatestSdk {
        checked_at,
        latest_version,
        latest_sdk,
        available_sdks: available,
    })
}

/// Asks the npm registry which SDK is the latest stable one. The address is fixed
/// and the only thing sent is the name of the package.
pub fn fetch_latest_sdk() -> Result<LatestSdk, String> {
    fetch_latest_with(|url| {
        let Some(curl) = resolve("curl") else {
            return Err(tr!(
                "O curl não foi encontrado: não dá para consultar o npm.",
                "curl was not found: the npm registry cannot be queried."
            ));
        };
        let mut command = Command::new(curl);
        command.args([
            "--silent",
            "--show-error",
            "--fail",
            "--proto",
            "=https",
            "--max-time",
            "15",
            "--max-filesize",
            &MAX_RESPONSE_BYTES.to_string(),
            url,
        ]);
        let output = memory_capture::run(&mut command, CHECK_TIMEOUT, 20);
        if output.status == ToolStatus::Available && output.exit_code == Some(0) {
            Ok(output.stdout)
        } else {
            Err(tr!(
                "Não foi possível consultar o npm. Verifique a sua conexão.",
                "Could not query npm. Check your connection."
            ))
        }
    })
}

pub(crate) fn fetch_latest_with(
    fetch: impl FnOnce(&str) -> Result<String, String>,
) -> Result<LatestSdk, String> {
    parse_dist_tags(&fetch(DIST_TAGS_URL)?, now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let full = dir.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, content).unwrap();
        }
        dir
    }

    fn inspect_dir(dir: &tempfile::TempDir) -> ExpoProject {
        inspect(&dir.path().to_string_lossy())
            .unwrap()
            .expect("an Expo project")
    }

    const TAGS: &str = r#"{"sdk-52":"52.0.49","sdk-53":"53.0.27","canary-sdk-55":"55.0.3-canary-20260429-a5e59cf","canary":"58.0.0-canary-20260909-ea7a89a","next":"58.0.11","sdk-57":"57.0.32","sdk-56":"56.0.24","sdk-55":"55.0.32","sdk-54":"54.0.38","latest":"58.0.23"}"#;

    #[test]
    fn reads_the_sdk_from_version_specs() {
        for (spec, sdk) in [
            ("~57.0.3", Some(57)),
            ("^58.0.0", Some(58)),
            ("57.0.0", Some(57)),
            (">=57", Some(57)),
            ("v57.1.0", Some(57)),
            ("57.x", Some(57)),
            ("^5.7.0", Some(5)),
            ("latest", None),
            ("*", None),
            ("", None),
            ("workspace:*", None),
            ("github:expo/expo", None),
            ("57abc", None),
            ("12345", None),
        ] {
            assert_eq!(major_of(spec), sdk, "{spec:?}");
        }
    }

    #[test]
    fn recognises_an_expo_project_and_its_sdk() {
        let dir = project(&[
            (
                "package.json",
                r#"{"name":"meu-app","dependencies":{"expo":"~57.0.3","react-native":"0.84.1","expo-router":"~7.0.0"}}"#,
            ),
            ("package-lock.json", "{}"),
        ]);
        let expo = inspect_dir(&dir);
        assert_eq!(expo.name, "meu-app");
        assert_eq!(
            (expo.sdk, expo.declared_version.as_str()),
            (Some(57), "~57.0.3")
        );
        assert_eq!(expo.react_native.as_deref(), Some("0.84.1"));
        assert!(expo.uses_router);
        assert_eq!(expo.package_manager.as_deref(), Some("npm"));
        assert_eq!(expo.lock_file.as_deref(), Some("package-lock.json"));
        assert!(expo.upgrade_supported && expo.unsupported_reason.is_none());
    }

    #[test]
    fn the_installed_version_wins_over_the_declared_one() {
        let dir = project(&[
            ("package.json", r#"{"dependencies":{"expo":"^57.0.0"}}"#),
            ("node_modules/expo/package.json", r#"{"version":"58.0.2"}"#),
        ]);
        let expo = inspect_dir(&dir);
        assert_eq!(expo.sdk, Some(58));
        assert_eq!(expo.installed_version.as_deref(), Some("58.0.2"));
    }

    #[test]
    fn projects_without_expo_or_with_broken_manifests_are_not_expo_projects() {
        for files in [
            vec![("package.json", r#"{"dependencies":{"react":"19"}}"#)],
            vec![("package.json", "{not json")],
            vec![("README.md", "x")],
        ] {
            let dir = project(&files);
            assert!(inspect(&dir.path().to_string_lossy()).unwrap().is_none());
        }
        assert!(inspect("/definitely/not/a/folder").is_err());
    }

    #[test]
    fn detects_the_package_manager_from_the_lock_file() {
        for (lock, manager) in [
            ("pnpm-lock.yaml", "pnpm"),
            ("yarn.lock", "yarn"),
            ("package-lock.json", "npm"),
        ] {
            let dir = project(&[
                ("package.json", r#"{"dependencies":{"expo":"57.0.0"}}"#),
                (lock, ""),
            ]);
            assert_eq!(inspect_dir(&dir).package_manager.as_deref(), Some(manager));
        }
        let none = project(&[("package.json", r#"{"dependencies":{"expo":"57.0.0"}}"#)]);
        let expo = inspect_dir(&none);
        assert_eq!(
            (expo.package_manager.as_deref(), expo.lock_file),
            (Some("npm"), None)
        );
    }

    #[test]
    fn flags_what_the_upgrade_cannot_handle() {
        let bun = project(&[
            ("package.json", r#"{"dependencies":{"expo":"57.0.0"}}"#),
            ("bun.lock", ""),
        ]);
        assert!(!inspect_dir(&bun).upgrade_supported);
        assert!(inspect_dir(&bun)
            .unsupported_reason
            .unwrap()
            .contains("bun"));

        let root = project(&[(
            "package.json",
            r#"{"workspaces":["app"],"dependencies":{"expo":"57.0.0"}}"#,
        )]);
        assert!(inspect_dir(&root)
            .unsupported_reason
            .unwrap()
            .contains("workspace"));

        let nested = project(&[
            ("package.json", r#"{"workspaces":["apps/*"]}"#),
            (
                "apps/mobile/package.json",
                r#"{"dependencies":{"expo":"57.0.0"}}"#,
            ),
        ]);
        let inner = inspect(&nested.path().join("apps/mobile").to_string_lossy())
            .unwrap()
            .unwrap();
        assert!(!inner.upgrade_supported);
        assert!(inner.unsupported_reason.unwrap().contains("monorepo"));

        let vague = project(&[("package.json", r#"{"dependencies":{"expo":"latest"}}"#)]);
        let expo = inspect_dir(&vague);
        assert_eq!(expo.sdk, None);
        assert!(!expo.upgrade_supported);
    }

    #[test]
    fn native_folders_are_reported_with_their_git_status() {
        let dir = project(&[
            ("package.json", r#"{"dependencies":{"expo":"57.0.0"}}"#),
            (".gitignore", "node_modules/\n# /ios\n/android\n"),
            ("android/build.gradle", ""),
            ("ios/Podfile", ""),
        ]);
        let expo = inspect_dir(&dir);
        assert_eq!(
            expo.native_folders,
            vec![
                NativeFolder {
                    name: "android".into(),
                    ignored_by_git: true
                },
                NativeFolder {
                    name: "ios".into(),
                    ignored_by_git: false
                },
            ]
        );
        let clean = project(&[("package.json", r#"{"dependencies":{"expo":"57.0.0"}}"#)]);
        assert!(inspect_dir(&clean).native_folders.is_empty());
    }

    #[test]
    fn the_latest_sdk_comes_from_the_stable_latest_tag_only() {
        let latest = parse_dist_tags(TAGS, 1_700_000_000).unwrap();
        assert_eq!(latest.latest_sdk, 58);
        assert_eq!(latest.latest_version, "58.0.23");
        assert_eq!(latest.checked_at, 1_700_000_000);
        // canary and next never count; the sdk-N tags and latest list what exists
        assert_eq!(latest.available_sdks, [52, 53, 54, 55, 56, 57, 58]);
    }

    #[test]
    fn rejects_registry_answers_it_cannot_trust() {
        for bad in [
            "",
            "not json",
            "[]",
            "{}",
            r#"{"latest":58}"#,
            r#"{"latest":"58.0.0-canary-1"}"#,
            r#"{"latest":"latest"}"#,
            r#"{"latest":"58.0"}"#,
            r#"{"latest":"58.0.0.1"}"#,
            r#"{"latest":"99999999.0.0"}"#,
        ] {
            assert!(parse_dist_tags(bad, 0).is_err(), "{bad:?}");
        }
        // a sdk tag that points to another major is ignored
        let odd = parse_dist_tags(
            r#"{"latest":"58.0.1","sdk-57":"56.0.0","sdk-x":"1.0.0"}"#,
            0,
        )
        .unwrap();
        assert_eq!(odd.available_sdks, [58]);
    }

    #[test]
    fn the_registry_is_queried_at_a_fixed_https_address() {
        let seen = std::cell::RefCell::new(String::new());
        let result = fetch_latest_with(|url| {
            *seen.borrow_mut() = url.to_string();
            Ok(TAGS.to_string())
        })
        .unwrap();
        assert_eq!(
            seen.borrow().as_str(),
            "https://registry.npmjs.org/-/package/expo/dist-tags"
        );
        assert_eq!(result.latest_sdk, 58);
        let failed = fetch_latest_with(|_| Err("offline".into()));
        assert_eq!(failed.unwrap_err(), "offline");
    }
}
