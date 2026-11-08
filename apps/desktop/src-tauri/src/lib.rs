use autodock_core::i18n::{with_lang, Lang};

mod terminal;
mod updater;

/// Runs `task` on a blocking thread with the caller's language, so every message
/// the core produces (errors, notes, descriptions) comes back in that language.
pub(crate) async fn localized<T: Send + 'static>(
    lang: Option<String>,
    task: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let lang = Lang::parse(lang.as_deref());
    tauri::async_runtime::spawn_blocking(move || with_lang(lang, task))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn query_container_logs(
    request: autodock_core::LogsRequest,
    lang: Option<String>,
) -> Result<autodock_core::LogsReport, String> {
    localized(lang, move || autodock_core::query_container_logs(request)).await?
}

#[tauri::command]
async fn collect_container_metrics(
    lang: Option<String>,
) -> Result<autodock_core::MonitoringReport, String> {
    localized(lang, autodock_core::collect_container_metrics).await?
}

#[tauri::command]
async fn list_docker_containers(
    lang: Option<String>,
) -> Result<autodock_core::DockerContainersReport, String> {
    localized(lang, autodock_core::list_docker_containers).await?
}

#[tauri::command]
async fn detect_stacks(
    path: String,
    lang: Option<String>,
) -> Result<Vec<autodock_core::generate::Suggestion>, String> {
    localized(lang, move || autodock_core::generate::detect_stacks(&path)).await?
}

#[tauri::command]
async fn inspect_folder(path: String, lang: Option<String>) -> Result<bool, String> {
    localized(lang, move || {
        autodock_core::generate::folder_is_empty(&path)
    })
    .await?
}

#[tauri::command]
async fn detect_ides(lang: Option<String>) -> Result<Vec<autodock_core::ide::IdeInfo>, String> {
    localized(lang, autodock_core::ide::detect_ides).await
}

#[tauri::command]
async fn open_in_ide(id: String, path: String, lang: Option<String>) -> Result<(), String> {
    localized(lang, move || autodock_core::ide::open_in_ide(&id, &path)).await?
}

#[tauri::command]
async fn reveal_folder(path: String, lang: Option<String>) -> Result<(), String> {
    localized(lang, move || autodock_core::ide::reveal_folder(&path)).await?
}

#[tauri::command]
async fn scaffold_plan(
    request: autodock_core::scaffold::ScaffoldRequest,
    lang: Option<String>,
) -> Result<autodock_core::scaffold::ScaffoldPlan, String> {
    localized(lang, move || autodock_core::scaffold::plan(&request)).await?
}

#[tauri::command]
async fn scaffold_run(
    request: autodock_core::scaffold::ScaffoldRequest,
    lang: Option<String>,
) -> Result<autodock_core::scaffold::ScaffoldReport, String> {
    localized(lang, move || autodock_core::scaffold::run(&request)).await?
}

#[tauri::command]
async fn preview_docker_setup(
    request: autodock_core::generate::GenerateRequest,
    lang: Option<String>,
) -> Result<autodock_core::generate::GeneratePlan, String> {
    localized(lang, move || autodock_core::generate::preview(&request)).await?
}

#[tauri::command]
async fn apply_docker_setup(
    request: autodock_core::generate::GenerateRequest,
    files: Vec<String>,
    lang: Option<String>,
) -> Result<autodock_core::generate::ApplyReport, String> {
    localized(lang, move || {
        autodock_core::generate::apply(&request, &files)
    })
    .await?
}

#[tauri::command]
async fn detect_apps(
    path: String,
    lang: Option<String>,
) -> Result<autodock_core::apps::AppsReport, String> {
    localized(lang, move || autodock_core::apps::detect_apps(&path)).await?
}

#[tauri::command]
async fn preview_multi_setup(
    request: autodock_core::multi::MultiRequest,
    lang: Option<String>,
) -> Result<autodock_core::generate::GeneratePlan, String> {
    localized(lang, move || autodock_core::multi::preview(&request)).await?
}

#[tauri::command]
async fn apply_multi_setup(
    request: autodock_core::multi::MultiRequest,
    files: Vec<String>,
    lang: Option<String>,
) -> Result<autodock_core::generate::ApplyReport, String> {
    localized(lang, move || autodock_core::multi::apply(&request, &files)).await?
}

#[tauri::command]
async fn compose_files(path: String, lang: Option<String>) -> Result<Vec<String>, String> {
    localized(lang, move || {
        autodock_core::compose_run::compose_files(&path)
    })
    .await?
}

#[tauri::command]
async fn compose_status(
    path: String,
    file: String,
    lang: Option<String>,
) -> Result<autodock_core::compose_run::ComposeStatus, String> {
    localized(lang, move || {
        autodock_core::compose_run::compose_status(&path, &file)
    })
    .await?
}

#[tauri::command]
async fn compose_run(
    request: autodock_core::compose_run::ComposeRequest,
    lang: Option<String>,
) -> Result<autodock_core::compose_run::ComposeReport, String> {
    localized(lang, move || {
        autodock_core::compose_run::run_compose(request)
    })
    .await?
}

#[tauri::command]
async fn expo_inspect(
    path: String,
    lang: Option<String>,
) -> Result<Option<autodock_core::expo::ExpoProject>, String> {
    localized(lang, move || autodock_core::expo::inspect(&path)).await?
}

#[tauri::command]
async fn expo_watch_state(
    lang: Option<String>,
) -> Result<autodock_core::expo_watch::WatchState, String> {
    localized(lang, autodock_core::expo_watch::state).await?
}

#[tauri::command]
async fn expo_watch_add(
    path: String,
    lang: Option<String>,
) -> Result<autodock_core::expo_watch::WatchState, String> {
    localized(lang, move || autodock_core::expo_watch::add_project(&path)).await?
}

#[tauri::command]
async fn expo_watch_remove(
    path: String,
    lang: Option<String>,
) -> Result<autodock_core::expo_watch::WatchState, String> {
    localized(lang, move || {
        autodock_core::expo_watch::remove_project(&path)
    })
    .await?
}

#[tauri::command]
async fn expo_watch_dismiss(
    path: String,
    lang: Option<String>,
) -> Result<autodock_core::expo_watch::WatchState, String> {
    localized(lang, move || {
        autodock_core::expo_watch::dismiss_update(&path)
    })
    .await?
}

#[tauri::command]
async fn expo_set_check_on_start(
    enabled: bool,
    lang: Option<String>,
) -> Result<autodock_core::expo_watch::WatchState, String> {
    localized(lang, move || {
        autodock_core::expo_watch::set_check_on_start(enabled)
    })
    .await?
}

/// Explicit "check now": asks the npm registry for the latest stable SDK.
#[tauri::command]
async fn expo_check_latest(
    lang: Option<String>,
) -> Result<autodock_core::expo_watch::WatchState, String> {
    localized(lang, autodock_core::expo_watch::refresh_latest).await?
}

/// When the app opens: only reaches the network if the user turned that on.
#[tauri::command]
async fn expo_auto_check(
    lang: Option<String>,
) -> Result<autodock_core::expo_watch::WatchState, String> {
    localized(lang, autodock_core::expo_watch::auto_check).await?
}

#[tauri::command]
async fn expo_upgrade_plan(
    path: String,
    target_sdk: Option<u32>,
    lang: Option<String>,
) -> Result<autodock_core::expo_upgrade::UpgradePlan, String> {
    localized(lang, move || {
        autodock_core::expo_upgrade::plan(&path, target_sdk)
    })
    .await?
}

#[tauri::command]
async fn expo_upgrade_run(
    request: autodock_core::expo_upgrade::UpgradeRequest,
    lang: Option<String>,
) -> Result<autodock_core::expo_upgrade::UpgradeReport, String> {
    localized(lang, move || autodock_core::expo_upgrade::run(request)).await?
}

#[tauri::command]
async fn expo_backups(
    path: String,
    lang: Option<String>,
) -> Result<Vec<autodock_core::expo_upgrade::BackupInfo>, String> {
    localized(lang, move || autodock_core::expo_upgrade::backups(&path)).await?
}

#[tauri::command]
async fn editor_open_root(
    path: String,
    lang: Option<String>,
) -> Result<autodock_core::editor_fs::EditorRoot, String> {
    localized(lang, move || autodock_core::editor_fs::open_root(&path)).await?
}

#[tauri::command]
async fn editor_list(
    root: String,
    path: String,
    lang: Option<String>,
) -> Result<autodock_core::editor_fs::EditorListing, String> {
    localized(lang, move || autodock_core::editor_fs::list(&root, &path)).await?
}

#[tauri::command]
async fn editor_read(
    root: String,
    path: String,
    lang: Option<String>,
) -> Result<autodock_core::editor_fs::EditorFile, String> {
    localized(lang, move || autodock_core::editor_fs::read(&root, &path)).await?
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn editor_write(
    root: String,
    path: String,
    text: String,
    bom: bool,
    expected_ms: Option<u64>,
    overwrite: bool,
    lang: Option<String>,
) -> Result<autodock_core::editor_fs::SaveReport, String> {
    localized(lang, move || {
        autodock_core::editor_fs::write(&root, &path, &text, bom, expected_ms, overwrite)
    })
    .await?
}

#[tauri::command]
async fn editor_search(
    root: String,
    query: String,
    case_sensitive: bool,
    include_ignored: bool,
    lang: Option<String>,
) -> Result<autodock_core::editor_fs::SearchReport, String> {
    localized(lang, move || {
        autodock_core::editor_fs::search(&root, &query, case_sensitive, include_ignored)
    })
    .await?
}

#[tauri::command]
async fn editor_create(
    root: String,
    parent: String,
    name: String,
    kind: String,
    lang: Option<String>,
) -> Result<autodock_core::editor_fs::EditorEntry, String> {
    localized(lang, move || {
        autodock_core::editor_fs::create(&root, &parent, &name, &kind)
    })
    .await?
}

#[tauri::command]
async fn java_inspect(
    path: String,
    lang: Option<String>,
) -> Result<Option<autodock_core::java::JavaProject>, String> {
    localized(lang, move || autodock_core::java::inspect(&path)).await?
}

#[tauri::command]
async fn java_map(
    path: String,
    lang: Option<String>,
) -> Result<autodock_core::java_map::JavaMap, String> {
    localized(lang, move || autodock_core::java_map::map(&path)).await?
}

#[tauri::command]
async fn java_convert_plan(
    path: String,
    options: autodock_core::java_convert::ConvertOptions,
    lang: Option<String>,
) -> Result<autodock_core::java_convert::ConvertPlan, String> {
    localized(lang, move || {
        autodock_core::java_convert::plan(&path, &options)
    })
    .await?
}

#[tauri::command]
async fn java_convert_run(
    request: autodock_core::java_convert::ConvertRequest,
    lang: Option<String>,
) -> Result<autodock_core::java_convert::ConvertReport, String> {
    localized(lang, move || autodock_core::java_convert::run(request)).await?
}

#[tauri::command]
async fn java_upgrade_plan(
    path: String,
    destination: Option<String>,
    lang: Option<String>,
) -> Result<autodock_core::java_upgrade::UpgradePlan, String> {
    localized(lang, move || {
        autodock_core::java_upgrade::plan_in(&path, destination.as_deref())
    })
    .await?
}

#[tauri::command]
async fn java_upgrade_run(
    request: autodock_core::java_upgrade::UpgradeRequest,
    lang: Option<String>,
) -> Result<autodock_core::java_upgrade::UpgradeReport, String> {
    localized(lang, move || autodock_core::java_upgrade::run(request)).await?
}

#[tauri::command]
async fn expo_restore(
    path: String,
    backup_id: String,
    confirmed: bool,
    lang: Option<String>,
) -> Result<autodock_core::expo_upgrade::RestoreReport, String> {
    localized(lang, move || {
        autodock_core::expo_upgrade::restore(&path, &backup_id, confirmed)
    })
    .await?
}

#[tauri::command]
async fn scan_project(
    path: String,
    lang: Option<String>,
) -> Result<autodock_core::ScanReport, String> {
    localized(lang, move || {
        autodock_core::scan(std::path::Path::new(&path))
    })
    .await?
}

#[tauri::command]
async fn doctor_environment(lang: Option<String>) -> Result<autodock_core::DoctorReport, String> {
    localized(lang, autodock_core::doctor).await
}

#[tauri::command]
async fn execute_console_operation(
    operation: autodock_core::ConsoleOperation,
    lang: Option<String>,
) -> Result<autodock_core::ConsoleReport, String> {
    localized(lang, move || {
        autodock_core::execute_console_operation(operation)
    })
    .await
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::PendingUpdate::default())
        .manage(terminal::Terminals::default())
        .invoke_handler(tauri::generate_handler![
            scan_project,
            detect_stacks,
            inspect_folder,
            detect_ides,
            scaffold_plan,
            scaffold_run,
            open_in_ide,
            reveal_folder,
            preview_docker_setup,
            apply_docker_setup,
            detect_apps,
            preview_multi_setup,
            apply_multi_setup,
            compose_files,
            compose_status,
            compose_run,
            expo_inspect,
            expo_watch_state,
            expo_watch_add,
            expo_watch_remove,
            expo_watch_dismiss,
            expo_set_check_on_start,
            expo_check_latest,
            expo_auto_check,
            expo_upgrade_plan,
            expo_upgrade_run,
            expo_backups,
            expo_restore,
            editor_open_root,
            editor_list,
            editor_read,
            editor_write,
            editor_create,
            editor_search,
            terminal::terminal_images,
            terminal::terminal_default_image,
            terminal::terminal_start,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_stop,
            java_inspect,
            java_map,
            java_convert_plan,
            java_convert_run,
            java_upgrade_plan,
            java_upgrade_run,
            updater::update_auto_check,
            updater::update_check_now,
            updater::update_install,
            updater::update_restart,
            updater::update_get_prefs,
            updater::update_set_check_on_start,
            updater::update_skip_version,
            doctor_environment,
            list_docker_containers,
            collect_container_metrics,
            execute_console_operation,
            query_container_logs,
        ])
        .build(tauri::generate_context!())
        .expect("Failed to start AutoDock")
        .run(|app, event| {
            // The terminals run in containers: they must not outlive the app.
            if let tauri::RunEvent::Exit = event {
                use tauri::Manager;
                app.state::<terminal::Terminals>().stop_all();
            }
        });
}
