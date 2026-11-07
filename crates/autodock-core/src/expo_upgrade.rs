//! Upgrading an Expo project to the next SDK, the way the Expo guide describes it:
//! install the new `expo`, align the dependencies with `expo install --fix` and
//! check the result with `expo-doctor`.
//!
//! Everything that executes project code runs **inside a Docker container** (the
//! package manager and its install scripts never run on the host). The container
//! sees the project folder, but `node_modules` is an isolated volume, so the
//! `node_modules` of the user's computer is not touched. Only `package.json` and
//! the lock file change. They are backed up first and restored automatically if
//! the upgrade fails; a backup can also be restored by hand later.
use crate::{
    doctor::{resolve, ToolStatus},
    expo::{self, ExpoProject},
    generate::{self, regular_file},
    memory_capture::{self, Output},
    ports::autodock_home,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Expo SDK 58 needs Node ^22.13 or ^24.3: the current 22 line satisfies it.
const NODE_IMAGE: &str = "node:22-bookworm-slim";
const UPGRADE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const OUTPUT_LINES: usize = 600;
const GUIDE_URL: &str = "https://docs.expo.dev/workflow/upgrading-expo-sdk-walkthrough/";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLink {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradePlan {
    pub schema_version: u32,
    pub project: ExpoProject,
    pub from_sdk: u32,
    pub to_sdk: u32,
    pub package_manager: String,
    pub image: String,
    /// What each part of the upgrade does, in order.
    pub steps: Vec<String>,
    /// The exact `docker` command, for display.
    pub command: String,
    pub files_touched: Vec<String>,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
    pub links: Vec<PlanLink>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpgradeRequest {
    pub path: String,
    pub target_sdk: u32,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyChange {
    pub name: String,
    /// `dependencies` or `devDependencies`.
    pub section: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeReport {
    pub schema_version: u32,
    pub succeeded: bool,
    /// The upgrade failed and the files were put back as they were.
    pub restored: bool,
    pub from_sdk: u32,
    pub to_sdk: u32,
    pub changes: Vec<DependencyChange>,
    /// Output of `expo-doctor`.
    pub doctor: String,
    pub doctor_passed: Option<bool>,
    /// npm could not resolve the peer dependencies and `--legacy-peer-deps` was used.
    pub used_legacy_peer_deps: bool,
    /// Tail of the container output when something went wrong.
    pub output: String,
    pub detail: String,
    pub duration_ms: u128,
    pub backup_id: String,
    pub next_steps: Vec<String>,
}

// ───────────────────────────── plan ─────────────────────────────

fn manager_of(project: &ExpoProject) -> String {
    project
        .package_manager
        .clone()
        .unwrap_or_else(|| "npm".into())
}

fn add_command(manager: &str, to_sdk: u32) -> String {
    match manager {
        "yarn" => format!("corepack enable && yarn add expo@^{to_sdk}.0.0"),
        "pnpm" => format!("corepack enable && pnpm add expo@^{to_sdk}.0.0"),
        _ => format!("npm install expo@^{to_sdk}.0.0"),
    }
}

/// The shell script run in the container. Only the manager (an allowlisted word),
/// the SDK number (an integer) and the host ids (integers) reach it.
fn script(manager: &str, to_sdk: u32, host_ids: Option<(u32, u32)>) -> String {
    let add = add_command(manager, to_sdk);
    // npm can fail to resolve the peer dependencies of many packages updated at once:
    // retry once with --legacy-peer-deps and say so.
    let align = if manager == "npm" {
        "align() { npx expo install --fix >/tmp/align.log 2>&1 && return 0; \
         if grep -q ERESOLVE /tmp/align.log; then echo 'NOTE legacy-peer-deps'; \
         npx expo install --fix -- --legacy-peer-deps >/tmp/align2.log 2>&1 && return 0; \
         tail -n 40 /tmp/align2.log; else tail -n 40 /tmp/align.log; fi; return 1; }"
    } else {
        "align() { npx expo install --fix >/tmp/align.log 2>&1 && return 0; tail -n 40 /tmp/align.log; return 1; }"
    };
    // On Linux the container runs as root: whatever it writes in the bind mount goes back
    // to the user, whichever way the script ends.
    let owner = match host_ids {
        Some((uid, gid)) => format!(
            "trap 'chown {uid}:{gid} package.json package-lock.json yarn.lock pnpm-lock.yaml 2>/dev/null' EXIT; "
        ),
        None => String::new(),
    };
    format!(
        "{owner}export CI=1 EXPO_NO_TELEMETRY=1 NO_COLOR=1 FORCE_COLOR=0 npm_config_update_notifier=false npm_config_fund=false npm_config_audit=false; \
step() {{ n=\"$1\"; shift; if sh -c \"$*\" >\"/tmp/$n.log\" 2>&1; then echo \"STEP $n ok\"; else echo \"STEP $n failed\"; tail -n 40 \"/tmp/$n.log\"; return 1; fi; }}; \
{align}; \
step install '{add}' || exit 1; \
if align; then echo 'STEP align ok'; else echo 'STEP align failed'; exit 1; fi; \
echo DOCTOR; npx expo-doctor >/tmp/doctor.log 2>&1; code=$?; tail -n 80 /tmp/doctor.log; echo \"DOCTOR_EXIT $code\""
    )
}

fn quote(arg: &str) -> String {
    if arg
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-_./:=@,+".contains(&b))
    {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

fn docker_args(root: &Path, script: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "run",
        "--rm",
        "-e",
        "HOME=/tmp",
        "-e",
        "npm_config_cache=/tmp/npm",
    ]
    .map(String::from)
    .to_vec();
    args.extend([
        "--mount".into(),
        format!(
            "type=bind,source={},target=/work",
            generate::plain_path(root)
        ),
        // An anonymous volume hides the project's node_modules: the host copy is neither
        // read nor written, and the volume disappears with the container (--rm).
        "--mount".into(),
        "type=volume,target=/work/node_modules".into(),
        "-w".into(),
        "/work".into(),
        NODE_IMAGE.into(),
        "sh".into(),
        "-c".into(),
        script.into(),
    ]);
    args
}

#[cfg(unix)]
fn host_ids() -> Option<(u32, u32)> {
    // SAFETY: geteuid/getegid have no preconditions.
    Some(unsafe { (libc::geteuid(), libc::getegid()) })
}

#[cfg(not(unix))]
fn host_ids() -> Option<(u32, u32)> {
    None
}

pub fn plan(path: &str, target: Option<u32>) -> Result<UpgradePlan, String> {
    let project = expo::inspect(path)?.ok_or_else(|| {
        tr!(
            "Esta pasta não é um projeto Expo.",
            "This folder is not an Expo project."
        )
    })?;
    if let Some(reason) = &project.unsupported_reason {
        return Err(reason.clone());
    }
    let from = project.sdk.ok_or_else(|| {
        tr!(
            "Não foi possível ler a versão do SDK.",
            "Could not read the SDK version."
        )
    })?;
    let to = target.unwrap_or(from + 1);
    if to <= from {
        return Err(tr!(
            "O projeto já está no SDK {from}: escolha um SDK mais novo.",
            "The project is already on SDK {from}: pick a newer SDK."
        ));
    }
    if to > from + 5 {
        return Err(tr!(
            "Atualize um SDK por vez (o próximo é o {}).",
            "Upgrade one SDK at a time (the next one is {}).",
            from + 1
        ));
    }
    let manager = manager_of(&project);
    let root = generate::root_of(path)?;
    generate::check_mount_path(&root)?;
    let lock = project.lock_file.clone();

    let mut warnings = vec![];
    if to > from + 1 {
        warnings.push(tr!(
            "O Expo recomenda atualizar um SDK por vez (o próximo é o {}): pular versões dificulta achar o que quebrou.",
            "Expo recommends upgrading one SDK at a time (the next one is {}): skipping versions makes it harder to find what broke.",
            from + 1
        ));
    }
    let mut notes = vec![
        tr!(
            "O upgrade roda dentro de um container Docker. A pasta node_modules do seu computador não é tocada: depois, rode {manager} install no projeto.",
            "The upgrade runs inside a Docker container. The node_modules folder on your computer is not touched: afterwards run {manager} install in the project."
        ),
        tr!(
            "Só o package.json e o arquivo de lock mudam. Antes de começar eles ganham um backup e voltam sozinhos se algo falhar.",
            "Only package.json and the lock file change. They are backed up first and restored automatically if something fails."
        ),
        tr!(
            "O Expo Go só abre projetos do SDK mais recente; um projeto em SDK antigo precisa de um development build.",
            "Expo Go only opens projects on the latest SDK; a project on an older SDK needs a development build."
        ),
        tr!(
            "Mudanças que quebram a API não são corrigidas automaticamente: leia as notas da versão e o relatório do expo-doctor.",
            "Breaking changes are not fixed automatically: read the release notes and the expo-doctor report."
        ),
    ];
    if project.lock_file.is_none() {
        notes.push(tr!(
            "O projeto não tem arquivo de lock: um novo será criado.",
            "The project has no lock file: a new one will be created."
        ));
    }
    for folder in &project.native_folders {
        if folder.ignored_by_git {
            notes.push(tr!(
                "A pasta {} é gerada (está no .gitignore). O Expo recomenda apagá-la depois do upgrade para ser recriada no próximo build; o AutoDock não apaga.",
                "The {} folder is generated (it is in .gitignore). Expo recommends deleting it after the upgrade so it is recreated on the next build; AutoDock does not delete it.",
                folder.name
            ));
        } else {
            warnings.push(tr!(
                "A pasta {} tem código nativo versionado: aplique as mudanças do upgrade helper do React Native e rode npx pod-install se houver ios.",
                "The {} folder has native code under version control: apply the React Native upgrade helper changes and run npx pod-install if there is an ios folder.",
                folder.name
            ));
        }
    }

    let command_script = script(&manager, to, host_ids());
    let command = std::iter::once("docker".to_string())
        .chain(docker_args(&root, &command_script).iter().map(|a| quote(a)))
        .collect::<Vec<_>>()
        .join(" ");
    let mut files_touched = vec!["package.json".to_string()];
    files_touched.push(lock.unwrap_or_else(|| match manager.as_str() {
        "yarn" => "yarn.lock".into(),
        "pnpm" => "pnpm-lock.yaml".into(),
        _ => "package-lock.json".into(),
    }));
    let steps = vec![
        tr!(
            "Backup do package.json e do arquivo de lock.",
            "Backup of package.json and the lock file."
        ),
        tr!(
            "{}: instala o expo do SDK {to}.",
            "{}: installs the expo of SDK {to}.",
            add_command(&manager, to)
        ),
        tr!(
            "npx expo install --fix: alinha cada dependência com a versão esperada pelo SDK {to}.",
            "npx expo install --fix: aligns every dependency with the version SDK {to} expects."
        ),
        tr!(
            "npx expo-doctor: confere o projeto e gera o relatório.",
            "npx expo-doctor: checks the project and produces the report."
        ),
    ];
    Ok(UpgradePlan {
        schema_version: 1,
        links: vec![
            PlanLink {
                label: tr!("Notas da versão do SDK {to}", "SDK {to} release notes"),
                url: format!("https://expo.dev/changelog/sdk-{to}"),
            },
            PlanLink {
                label: tr!("Guia oficial de upgrade", "Official upgrade guide"),
                url: GUIDE_URL.into(),
            },
        ],
        project,
        from_sdk: from,
        to_sdk: to,
        package_manager: manager,
        image: NODE_IMAGE.into(),
        steps,
        command,
        files_touched,
        notes,
        warnings,
    })
}

// ───────────────────────────── backups ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    name: String,
    /// The file existed before the upgrade (a lock file that did not exist is removed on restore).
    existed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    id: String,
    project: String,
    from_sdk: u32,
    to_sdk: u32,
    created_at: u64,
    /// `running`, `done` or `failed`.
    status: String,
    files: Vec<BackupFile>,
    /// SHA-256 of package.json right after a successful upgrade.
    after_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub id: String,
    pub from_sdk: u32,
    pub to_sdk: u32,
    pub created_at: u64,
    pub status: String,
    pub files: Vec<String>,
    /// package.json changed after the upgrade: restoring would discard that work.
    pub modified_since_upgrade: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub schema_version: u32,
    pub restored: Vec<String>,
    pub removed: Vec<String>,
    pub detail: String,
}

fn backups_root() -> PathBuf {
    autodock_home().join("backups").join("expo")
}

fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('-').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 12 && p.bytes().all(|b| b.is_ascii_digit()))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hash_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

fn read_manifest(dir: &Path) -> Option<Manifest> {
    let text = fs::read_to_string(dir.join("manifest.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_manifest(dir: &Path, manifest: &Manifest) -> Result<(), String> {
    let text = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    fs::write(dir.join("manifest.json"), text).map_err(|e| e.to_string())
}

fn same_project(a: &str, b: &str) -> bool {
    let plain = |value: &str| {
        let value = value.strip_prefix(r"\\?\").unwrap_or(value);
        let value = value.trim_end_matches(['/', '\\']).to_string();
        if cfg!(windows) {
            value.to_lowercase()
        } else {
            value
        }
    };
    plain(a) == plain(b)
}

fn make_backup(
    root: &Path,
    project: &ExpoProject,
    from: u32,
    to: u32,
) -> Result<(PathBuf, Manifest), String> {
    let id = format!("{}-{from}-{to}", now());
    let dir = backups_root().join(&id);
    fs::create_dir_all(&dir).map_err(|e| {
        tr!(
            "Não foi possível criar o backup: {e}",
            "Could not create the backup: {e}"
        )
    })?;
    let mut files = vec![];
    let mut wanted = vec!["package.json".to_string()];
    wanted.extend(project.lock_file.clone());
    for name in wanted {
        if regular_file(root, &name) {
            fs::copy(root.join(&name), dir.join(&name)).map_err(|e| {
                tr!(
                    "Não foi possível copiar {name} para o backup: {e}",
                    "Could not copy {name} to the backup: {e}"
                )
            })?;
            files.push(BackupFile {
                name,
                existed: true,
            });
        }
    }
    let manifest = Manifest {
        id,
        project: project.path.clone(),
        from_sdk: from,
        to_sdk: to,
        created_at: now(),
        status: "running".into(),
        files,
        after_hash: None,
    };
    write_manifest(&dir, &manifest)?;
    Ok((dir, manifest))
}

/// Puts the backed up files back; a lock file that did not exist before is removed.
fn put_back(
    root: &Path,
    dir: &Path,
    manifest: &Manifest,
    lock_names: &[&str],
) -> Result<RestoreReport, String> {
    let mut restored = vec![];
    let mut removed = vec![];
    for file in &manifest.files {
        fs::copy(dir.join(&file.name), root.join(&file.name)).map_err(|e| {
            tr!(
                "Não foi possível restaurar {}: {e}",
                "Could not restore {}: {e}",
                file.name
            )
        })?;
        restored.push(file.name.clone());
    }
    // A lock file the upgrade created (none was there before) goes away again.
    for lock in lock_names {
        let had = manifest.files.iter().any(|f| f.name == *lock);
        if !had && regular_file(root, lock) {
            let _ = fs::remove_file(root.join(lock));
            removed.push((*lock).to_string());
        }
    }
    Ok(RestoreReport {
        schema_version: 1,
        restored,
        removed,
        detail: String::new(),
    })
}

const LOCK_FILES: [&str; 3] = ["package-lock.json", "yarn.lock", "pnpm-lock.yaml"];

/// Backups of a project, newest first.
pub fn backups(path: &str) -> Result<Vec<BackupInfo>, String> {
    let root = generate::root_of(path)?;
    let project = root.display().to_string();
    let mut found: Vec<BackupInfo> = vec![];
    let Ok(entries) = fs::read_dir(backups_root()) else {
        return Ok(found);
    };
    for entry in entries.filter_map(Result::ok) {
        let dir = entry.path();
        if !dir.is_dir() || !valid_id(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let Some(manifest) = read_manifest(&dir) else {
            continue;
        };
        if !same_project(&manifest.project, &project) {
            continue;
        }
        let modified = manifest
            .after_hash
            .as_ref()
            .is_some_and(|hash| hash_file(&root.join("package.json")).as_ref() != Some(hash));
        found.push(BackupInfo {
            id: manifest.id.clone(),
            from_sdk: manifest.from_sdk,
            to_sdk: manifest.to_sdk,
            created_at: manifest.created_at,
            status: manifest.status.clone(),
            files: manifest.files.iter().map(|f| f.name.clone()).collect(),
            modified_since_upgrade: modified,
        });
    }
    found.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    Ok(found)
}

/// Puts the files of a backup back in the project (the node_modules is not touched).
pub fn restore(path: &str, backup_id: &str, confirmed: bool) -> Result<RestoreReport, String> {
    if !confirmed {
        return Err(tr!(
            "Confirmação explícita obrigatória.",
            "Explicit confirmation required."
        ));
    }
    if !valid_id(backup_id) {
        return Err(tr!("Backup inválido.", "Invalid backup."));
    }
    let root = generate::root_of(path)?;
    let dir = backups_root().join(backup_id);
    let manifest =
        read_manifest(&dir).ok_or_else(|| tr!("Backup não encontrado.", "Backup not found."))?;
    if !same_project(&manifest.project, &root.display().to_string()) {
        return Err(tr!(
            "Este backup é de outro projeto.",
            "This backup belongs to another project."
        ));
    }
    let mut report = put_back(&root, &dir, &manifest, &LOCK_FILES)?;
    report.detail = tr!(
        "package.json e o arquivo de lock voltaram ao estado do SDK {}. Rode o gerenciador de pacotes para refazer a node_modules.",
        "package.json and the lock file are back to the SDK {} state. Run the package manager to rebuild node_modules.",
        manifest.from_sdk
    );
    Ok(report)
}

// ───────────────────────────── run ─────────────────────────────

fn dependency_map(text: &str) -> BTreeMap<(String, String), String> {
    let mut map = BTreeMap::new();
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return map;
    };
    for section in ["dependencies", "devDependencies"] {
        if let Some(object) = json.get(section).and_then(|v| v.as_object()) {
            for (name, version) in object {
                if let Some(version) = version.as_str() {
                    map.insert((section.to_string(), name.clone()), version.to_string());
                }
            }
        }
    }
    map
}

pub(crate) fn diff_dependencies(before: &str, after: &str) -> Vec<DependencyChange> {
    let (old, new) = (dependency_map(before), dependency_map(after));
    let mut keys: Vec<&(String, String)> = old.keys().chain(new.keys()).collect();
    keys.sort_by(|a, b| (&a.1, &a.0).cmp(&(&b.1, &b.0)));
    keys.dedup();
    keys.into_iter()
        .filter(|key| old.get(*key) != new.get(*key))
        .map(|key| DependencyChange {
            name: key.1.clone(),
            section: key.0.clone(),
            from: old.get(key).cloned(),
            to: new.get(key).cloned(),
        })
        .collect()
}

struct Parsed {
    install_ok: bool,
    align_ok: bool,
    legacy: bool,
    doctor: String,
    doctor_exit: Option<i32>,
}

fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed {
        install_ok: false,
        align_ok: false,
        legacy: false,
        doctor: String::new(),
        doctor_exit: None,
    };
    let mut in_doctor = false;
    let mut doctor: Vec<&str> = vec![];
    for line in text.lines() {
        let line = line.trim_end();
        if in_doctor {
            if let Some(code) = line.strip_prefix("DOCTOR_EXIT ") {
                parsed.doctor_exit = code.trim().parse().ok();
                in_doctor = false;
            } else {
                doctor.push(line);
            }
            continue;
        }
        match line {
            "STEP install ok" => parsed.install_ok = true,
            "STEP align ok" => parsed.align_ok = true,
            "NOTE legacy-peer-deps" => parsed.legacy = true,
            "DOCTOR" => in_doctor = true,
            _ => {}
        }
    }
    // npx prints a notice when it has to fetch expo-doctor: it is not part of the report.
    parsed.doctor = doctor
        .into_iter()
        .filter(|l| !l.starts_with("npm warn exec"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    parsed
}

/// The mount point of the isolated volume is created on the host: remove it when it
/// was not there before and is still empty.
fn cleanup_empty_node_modules(root: &Path, existed: bool) {
    if existed {
        return;
    }
    let folder = root.join("node_modules");
    if fs::symlink_metadata(&folder).is_ok_and(|m| m.is_dir())
        && fs::read_dir(&folder).is_ok_and(|mut entries| entries.next().is_none())
    {
        let _ = fs::remove_dir(&folder);
    }
}

pub fn run(request: UpgradeRequest) -> Result<UpgradeReport, String> {
    run_with(request, |args, timeout| {
        let Some(docker) = resolve("docker") else {
            return Output::missing();
        };
        let mut command = Command::new(docker);
        command.args(args);
        memory_capture::run(&mut command, timeout, OUTPUT_LINES)
    })
}

fn run_with(
    request: UpgradeRequest,
    runner: impl FnOnce(&[String], Duration) -> Output,
) -> Result<UpgradeReport, String> {
    if !request.confirmed {
        return Err(tr!(
            "Confirmação explícita obrigatória.",
            "Explicit confirmation required."
        ));
    }
    let plan = plan(&request.path, Some(request.target_sdk))?;
    let root = generate::root_of(&request.path)?;
    let node_modules_existed = fs::symlink_metadata(root.join("node_modules")).is_ok();
    let (backup_dir, mut manifest) = make_backup(&root, &plan.project, plan.from_sdk, plan.to_sdk)?;
    let before = fs::read_to_string(root.join("package.json")).unwrap_or_default();

    let script = script(&plan.package_manager, plan.to_sdk, host_ids());
    let output = runner(&docker_args(&root, &script), UPGRADE_TIMEOUT);
    cleanup_empty_node_modules(&root, node_modules_existed);

    let text = [output.stdout.trim(), output.stderr.trim()]
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    let parsed = parse(&text);
    let ran = output.status == ToolStatus::Available && output.exit_code == Some(0);
    let succeeded = ran && parsed.install_ok && parsed.align_ok;

    let mut report = UpgradeReport {
        schema_version: 1,
        succeeded,
        restored: false,
        from_sdk: plan.from_sdk,
        to_sdk: plan.to_sdk,
        changes: vec![],
        doctor: parsed.doctor.clone(),
        doctor_passed: parsed.doctor_exit.map(|code| code == 0),
        used_legacy_peer_deps: parsed.legacy,
        output: String::new(),
        detail: String::new(),
        duration_ms: output.duration_ms,
        backup_id: manifest.id.clone(),
        next_steps: vec![],
    };

    if succeeded {
        let after = fs::read_to_string(root.join("package.json")).unwrap_or_default();
        report.changes = diff_dependencies(&before, &after);
        manifest.status = "done".into();
        manifest.after_hash = hash_file(&root.join("package.json"));
        let _ = write_manifest(&backup_dir, &manifest);
        report.next_steps = next_steps(&plan, report.doctor_passed == Some(false));
        if parsed.legacy {
            report.detail = tr!(
                "O npm não conseguiu resolver as peer dependencies e o upgrade usou --legacy-peer-deps. Confira o resultado com npm ls.",
                "npm could not resolve the peer dependencies and the upgrade used --legacy-peer-deps. Check the result with npm ls."
            );
        }
        return Ok(report);
    }

    // Something went wrong: the project goes back to how it was.
    let restore = put_back(&root, &backup_dir, &manifest, &LOCK_FILES);
    report.restored = restore.is_ok();
    manifest.status = "failed".into();
    let _ = write_manifest(&backup_dir, &manifest);
    report.output = text
        .lines()
        .rev()
        .take(60)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    report.detail = match output.status {
        ToolStatus::Missing => output.detail.clone(),
        ToolStatus::Timeout => tr!(
            "Prazo excedido. O projeto foi restaurado; tente de novo.",
            "Time limit exceeded. The project was restored; try again."
        ),
        _ => tr!(
            "O upgrade não terminou. O package.json e o lock voltaram ao estado anterior.",
            "The upgrade did not finish. package.json and the lock file are back to how they were."
        ),
    };
    if restore.is_err() {
        report.detail.push(' ');
        report.detail.push_str(&tr!(
            "Atenção: não foi possível restaurar os arquivos automaticamente; use o backup {}.",
            "Warning: the files could not be restored automatically; use backup {}.",
            manifest.id
        ));
    }
    Ok(report)
}

fn next_steps(plan: &UpgradePlan, doctor_failed: bool) -> Vec<String> {
    let manager = &plan.package_manager;
    let mut steps = vec![tr!(
        "Rode {manager} install no projeto para atualizar a node_modules do seu computador.",
        "Run {manager} install in the project to update the node_modules on your computer."
    )];
    if doctor_failed {
        steps.push(tr!(
            "O expo-doctor apontou problemas: leia o relatório e corrija antes de seguir.",
            "expo-doctor reported problems: read the report and fix them before moving on."
        ));
    }
    for folder in &plan.project.native_folders {
        steps.push(if folder.ignored_by_git {
            tr!(
                "Apague a pasta {} (é gerada) para ela ser recriada no próximo build.",
                "Delete the {} folder (it is generated) so it is recreated on the next build.",
                folder.name
            )
        } else {
            tr!(
                "Aplique as mudanças nativas do upgrade helper na pasta {}.",
                "Apply the native changes of the upgrade helper to the {} folder.",
                folder.name
            )
        });
    }
    steps.push(tr!(
        "Leia as notas da versão do SDK {} e teste o app.",
        "Read the SDK {} release notes and test the app.",
        plan.to_sdk
    ));
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEFORE: &str = r#"{"name":"meu-app","dependencies":{"expo":"~57.0.32","react":"19.2.3","react-native":"0.86.3","expo-router":"~57.0.30"},"devDependencies":{"@types/react":"~19.2.2"}}"#;
    const AFTER: &str = r#"{"name":"meu-app","dependencies":{"expo":"^58.0.23","react":"19.3.0","react-native":"0.88.1","expo-router":"~58.0.30","expo-new":"~58.0.1"},"devDependencies":{"@types/react":"~19.3.0"}}"#;

    fn app(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let full = dir.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, content).unwrap();
        }
        dir
    }

    fn project() -> tempfile::TempDir {
        app(&[
            ("package.json", BEFORE),
            ("package-lock.json", r#"{"lockfileVersion":3}"#),
        ])
    }

    fn path(dir: &tempfile::TempDir) -> String {
        dir.path().to_string_lossy().into_owned()
    }

    fn request(dir: &tempfile::TempDir, target: u32, confirmed: bool) -> UpgradeRequest {
        UpgradeRequest {
            path: path(dir),
            target_sdk: target,
            confirmed,
        }
    }

    fn output(status: ToolStatus, exit: Option<i32>, stdout: &str) -> Output {
        Output {
            status,
            exit_code: exit,
            stdout: stdout.into(),
            stderr: String::new(),
            truncated: false,
            detail: "no docker".into(),
            duration_ms: 1234,
        }
    }

    const SUCCESS: &str = "STEP install ok\nSTEP align ok\nDOCTOR\nnpm warn exec The following package was not found and will be installed: expo-doctor@1.21.6\nRunning 20 checks on your project...\n20/20 checks passed. No issues detected!\nDOCTOR_EXIT 0\n";

    #[test]
    fn plans_the_next_sdk_by_default_with_the_official_steps_and_links() {
        let dir = project();
        let plan = plan(&path(&dir), None).unwrap();
        assert_eq!((plan.from_sdk, plan.to_sdk), (57, 58));
        assert_eq!(plan.package_manager, "npm");
        assert_eq!(plan.image, "node:22-bookworm-slim");
        assert_eq!(plan.files_touched, ["package.json", "package-lock.json"]);
        let steps = plan.steps.join("\n");
        assert!(
            steps.contains("npm install expo@^58.0.0")
                && steps.contains("expo install --fix")
                && steps.contains("expo-doctor")
        );
        assert!(plan
            .links
            .iter()
            .any(|l| l.url == "https://expo.dev/changelog/sdk-58"));
        assert!(plan
            .links
            .iter()
            .any(|l| l.url.contains("upgrading-expo-sdk-walkthrough")));
        assert!(plan.warnings.is_empty());
        // the command is the docker invocation with an isolated node_modules
        assert!(plan.command.starts_with("docker run --rm"));
        assert!(plan
            .command
            .contains("type=volume,target=/work/node_modules"));
        assert!(plan.notes.iter().any(|n| n.contains("node_modules")));
        assert!(plan.notes.iter().any(|n| n.contains("Expo Go")));
    }

    #[test]
    fn upgrades_one_sdk_at_a_time_and_rejects_nonsense_targets() {
        let dir = project();
        assert!(plan(&path(&dir), Some(57))
            .unwrap_err()
            .contains("already on SDK 57"));
        assert!(plan(&path(&dir), Some(40)).is_err());
        assert!(plan(&path(&dir), Some(99))
            .unwrap_err()
            .contains("one SDK at a time"));
        let skipping = plan(&path(&dir), Some(59)).unwrap();
        assert!(skipping
            .warnings
            .iter()
            .any(|w| w.contains("one SDK at a time")));
        assert!(plan("/definitely/not/a/folder", None).is_err());
        let plain = app(&[("package.json", r#"{"dependencies":{"react":"19"}}"#)]);
        assert!(plan(&path(&plain), None)
            .unwrap_err()
            .contains("not an Expo project"));
    }

    #[test]
    fn unsupported_projects_are_refused_with_the_reason() {
        let bun = app(&[("package.json", BEFORE), ("bun.lock", "")]);
        assert!(plan(&path(&bun), None).unwrap_err().contains("bun"));
        let root = app(&[(
            "package.json",
            r#"{"workspaces":["a"],"dependencies":{"expo":"~57.0.0"}}"#,
        )]);
        assert!(plan(&path(&root), None).unwrap_err().contains("workspace"));
    }

    #[test]
    fn native_folders_become_notes_or_warnings_and_are_never_touched() {
        let dir = app(&[
            ("package.json", BEFORE),
            (".gitignore", "/android\n"),
            ("android/build.gradle", "x"),
            ("ios/Podfile", "x"),
        ]);
        let plan = plan(&path(&dir), None).unwrap();
        assert!(plan
            .notes
            .iter()
            .any(|n| n.contains("android") && n.contains("does not delete")));
        assert!(plan
            .warnings
            .iter()
            .any(|w| w.contains("ios") && w.contains("pod-install")));
        assert!(
            dir.path().join("android/build.gradle").is_file()
                && dir.path().join("ios/Podfile").is_file()
        );
    }

    #[test]
    fn the_script_follows_the_package_manager_and_only_embeds_validated_values() {
        let npm = script("npm", 58, None);
        assert!(npm.contains("'npm install expo@^58.0.0'"));
        assert!(
            npm.contains("ERESOLVE") && npm.contains("--legacy-peer-deps"),
            "npm gets the peer dependency fallback"
        );
        let yarn = script("yarn", 58, None);
        assert!(
            yarn.contains("'corepack enable && yarn add expo@^58.0.0'")
                && !yarn.contains("legacy-peer-deps")
        );
        let pnpm = script("pnpm", 59, None);
        assert!(pnpm.contains("pnpm add expo@^59.0.0"));
        for text in [&npm, &yarn, &pnpm] {
            assert!(text.contains("npx expo install --fix") && text.contains("npx expo-doctor"));
            assert!(text.contains("EXPO_NO_TELEMETRY=1") && !text.contains("chown"));
        }
        // on unix the files written in the bind mount go back to the user
        let owned = script("npm", 58, Some((1000, 1000)));
        assert!(owned.starts_with("trap 'chown 1000:1000 package.json package-lock.json"));
        assert!(owned.contains("' EXIT; export CI=1"));
    }

    #[test]
    fn the_container_sees_the_project_but_not_its_node_modules() {
        let dir = project();
        let root = generate::root_of(&path(&dir)).unwrap();
        let args = docker_args(&root, "echo hi");
        assert_eq!(&args[..3], ["run", "--rm", "-e"]);
        assert!(args.contains(&format!(
            "type=bind,source={},target=/work",
            generate::plain_path(&root)
        )));
        assert!(
            !args.iter().any(|a| a.contains(r"\\?\")),
            "Docker does not accept the verbatim Windows prefix"
        );
        assert!(args.contains(&"type=volume,target=/work/node_modules".to_string()));
        assert_eq!(args.last().unwrap(), "echo hi");
        assert!(args.iter().any(|a| a == "node:22-bookworm-slim"));
        assert!(!args
            .iter()
            .any(|a| a == "--privileged" || a.starts_with("--network") || a == "-v"));
    }

    #[test]
    fn nothing_runs_without_confirmation() {
        let dir = project();
        let error = run_with(request(&dir, 58, false), |_, _| panic!("must not run")).unwrap_err();
        assert!(error.contains("confirmation"));
        assert!(backups(&path(&dir)).unwrap().is_empty(), "no backup either");
    }

    #[test]
    fn a_successful_upgrade_reports_the_changes_the_doctor_and_the_next_steps() {
        let dir = project();
        let target = dir.path().to_path_buf();
        let report = run_with(request(&dir, 58, true), move |args, timeout| {
            assert_eq!(timeout, Duration::from_secs(30 * 60));
            assert!(args.last().unwrap().contains("npm install expo@^58.0.0"));
            // the container rewrites the files of the bind mount
            fs::write(target.join("package.json"), AFTER).unwrap();
            fs::write(
                target.join("package-lock.json"),
                r#"{"lockfileVersion":3,"new":true}"#,
            )
            .unwrap();
            // and leaves the empty mount point of the isolated volume behind
            fs::create_dir(target.join("node_modules")).unwrap();
            output(ToolStatus::Available, Some(0), SUCCESS)
        })
        .unwrap();
        assert!(report.succeeded && !report.restored);
        assert_eq!((report.from_sdk, report.to_sdk), (57, 58));
        assert_eq!(report.doctor_passed, Some(true));
        assert!(
            report.doctor.contains("20/20 checks passed")
                && !report.doctor.contains("npm warn exec")
        );
        assert!(!report.used_legacy_peer_deps && report.detail.is_empty());
        let names: Vec<(&str, Option<&str>, Option<&str>)> = report
            .changes
            .iter()
            .map(|c| (c.name.as_str(), c.from.as_deref(), c.to.as_deref()))
            .collect();
        assert!(names.contains(&("expo", Some("~57.0.32"), Some("^58.0.23"))));
        assert!(names.contains(&("react-native", Some("0.86.3"), Some("0.88.1"))));
        assert!(
            names.contains(&("expo-new", None, Some("~58.0.1"))),
            "added packages are listed"
        );
        assert!(names.contains(&("@types/react", Some("~19.2.2"), Some("~19.3.0"))));
        assert!(report.next_steps[0].contains("npm install"));
        assert!(report
            .next_steps
            .last()
            .unwrap()
            .contains("SDK 58 release notes"));
        assert!(
            !dir.path().join("node_modules").exists(),
            "the empty mount point is cleaned up"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("package.json")).unwrap(),
            AFTER
        );

        // the backup was kept and can bring the old files back
        let list = backups(&path(&dir)).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            (list[0].from_sdk, list[0].to_sdk, list[0].status.as_str()),
            (57, 58, "done")
        );
        assert!(!list[0].modified_since_upgrade);
        assert_eq!(list[0].files, ["package.json", "package-lock.json"]);
    }

    #[test]
    fn a_failed_upgrade_puts_the_files_back() {
        let dir = project();
        let target = dir.path().to_path_buf();
        let report = run_with(request(&dir, 58, true), move |_, _| {
            // npm half-way: package.json already changed, then the alignment failed
            fs::write(target.join("package.json"), AFTER).unwrap();
            output(
                ToolStatus::Available,
                Some(1),
                "STEP install ok\nSTEP align failed\nnpm error code ERESOLVE\nnpm error could not resolve",
            )
        })
        .unwrap();
        assert!(!report.succeeded && report.restored);
        assert!(report.output.contains("ERESOLVE") && !report.detail.is_empty());
        assert_eq!(
            fs::read_to_string(dir.path().join("package.json")).unwrap(),
            BEFORE
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("package-lock.json")).unwrap(),
            r#"{"lockfileVersion":3}"#
        );
        assert_eq!(backups(&path(&dir)).unwrap()[0].status, "failed");
    }

    #[test]
    fn missing_docker_and_timeouts_leave_the_project_untouched() {
        for (status, expected) in [
            (ToolStatus::Missing, "no docker"),
            (ToolStatus::Timeout, "Time limit"),
        ] {
            let dir = project();
            let report = run_with(request(&dir, 58, true), move |_, _| {
                output(status, None, "")
            })
            .unwrap();
            assert!(!report.succeeded && report.restored);
            assert!(report.detail.contains(expected), "{}", report.detail);
            assert_eq!(
                fs::read_to_string(dir.path().join("package.json")).unwrap(),
                BEFORE
            );
        }
    }

    #[test]
    fn a_zero_exit_without_the_steps_is_not_a_success() {
        let dir = project();
        let report = run_with(request(&dir, 58, true), |_, _| {
            output(ToolStatus::Available, Some(0), "something else")
        })
        .unwrap();
        assert!(!report.succeeded && report.restored);
    }

    #[test]
    fn the_legacy_peer_deps_fallback_is_reported_and_a_failing_doctor_does_not_fail_the_upgrade() {
        let dir = project();
        let target = dir.path().to_path_buf();
        let report = run_with(request(&dir, 58, true), move |_, _| {
            fs::write(target.join("package.json"), AFTER).unwrap();
            output(
                ToolStatus::Available,
                Some(0),
                "STEP install ok\nNOTE legacy-peer-deps\nSTEP align ok\nDOCTOR\n1 check failed\nDOCTOR_EXIT 1\n",
            )
        })
        .unwrap();
        assert!(report.succeeded && report.used_legacy_peer_deps);
        assert!(report.detail.contains("--legacy-peer-deps"));
        assert_eq!(report.doctor_passed, Some(false));
        assert!(report
            .next_steps
            .iter()
            .any(|s| s.contains("expo-doctor reported problems")));
    }

    #[test]
    fn a_lock_file_created_by_a_failed_upgrade_is_removed_again() {
        let dir = app(&[("package.json", BEFORE)]);
        let target = dir.path().to_path_buf();
        let report = run_with(request(&dir, 58, true), move |_, _| {
            fs::write(target.join("package-lock.json"), "{}").unwrap();
            output(ToolStatus::Available, Some(1), "STEP install failed")
        })
        .unwrap();
        assert!(report.restored);
        assert!(!dir.path().join("package-lock.json").exists());
    }

    #[test]
    fn restoring_a_backup_needs_confirmation_and_the_right_project() {
        let dir = project();
        let target = dir.path().to_path_buf();
        run_with(request(&dir, 58, true), move |_, _| {
            fs::write(target.join("package.json"), AFTER).unwrap();
            output(ToolStatus::Available, Some(0), SUCCESS)
        })
        .unwrap();
        let id = backups(&path(&dir)).unwrap()[0].id.clone();

        assert!(restore(&path(&dir), &id, false).is_err());
        assert!(restore(&path(&dir), "../escape", true).is_err());
        assert!(restore(&path(&dir), "1-2", true).is_err());
        assert!(restore(&path(&dir), "999999999-1-2", true)
            .unwrap_err()
            .contains("not found"));
        let other = project();
        assert!(restore(&path(&other), &id, true)
            .unwrap_err()
            .contains("another project"));

        // work done after the upgrade is flagged before the user restores
        fs::write(dir.path().join("package.json"), format!("{AFTER} ")).unwrap();
        assert!(backups(&path(&dir)).unwrap()[0].modified_since_upgrade);

        let report = restore(&path(&dir), &id, true).unwrap();
        assert_eq!(report.restored, ["package.json", "package-lock.json"]);
        assert!(report.detail.contains("SDK 57"));
        assert_eq!(
            fs::read_to_string(dir.path().join("package.json")).unwrap(),
            BEFORE
        );
    }

    #[test]
    fn dependency_diffs_cover_changed_added_and_removed_packages() {
        let changes = diff_dependencies(BEFORE, AFTER);
        let find = |name: &str| changes.iter().find(|c| c.name == name).unwrap();
        assert_eq!(
            (find("expo").from.as_deref(), find("expo").to.as_deref()),
            (Some("~57.0.32"), Some("^58.0.23"))
        );
        assert_eq!(find("expo-new").from, None);
        let removed = diff_dependencies(AFTER, BEFORE);
        assert_eq!(
            removed.iter().find(|c| c.name == "expo-new").unwrap().to,
            None
        );
        assert!(diff_dependencies(BEFORE, BEFORE).is_empty());
        assert!(diff_dependencies("not json", "{}").is_empty());
        assert_eq!(find("@types/react").section, "devDependencies");
    }

    #[test]
    fn container_output_is_parsed_into_steps_notes_and_the_doctor_report() {
        let ok = parse(SUCCESS);
        assert!(ok.install_ok && ok.align_ok && !ok.legacy && ok.doctor_exit == Some(0));
        assert_eq!(
            ok.doctor,
            "Running 20 checks on your project...\n20/20 checks passed. No issues detected!"
        );
        let failed = parse("STEP install ok\nSTEP align failed\nboom");
        assert!(
            failed.install_ok
                && !failed.align_ok
                && failed.doctor.is_empty()
                && failed.doctor_exit.is_none()
        );
        let empty = parse("");
        assert!(!empty.install_ok && !empty.align_ok && !empty.legacy);
    }
}
