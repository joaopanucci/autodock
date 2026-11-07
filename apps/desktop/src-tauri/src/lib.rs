use autodock_core::i18n::{with_lang, Lang};

/// Runs `task` on a blocking thread with the caller's language, so every message
/// the core produces (errors, notes, descriptions) comes back in that language.
async fn localized<T: Send + 'static>(
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
            doctor_environment,
            list_docker_containers,
            collect_container_metrics,
            execute_console_operation,
            query_container_logs,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start AutoDock");
}
