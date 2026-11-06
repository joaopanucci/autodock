#[tauri::command]
async fn query_container_logs(
    request: autodock_core::LogsRequest,
) -> Result<autodock_core::LogsReport, String> {
    tauri::async_runtime::spawn_blocking(move || autodock_core::query_container_logs(request))
        .await
        .map_err(|_| "Falha na tarefa de consulta de logs.".to_string())?
}

#[tauri::command]
async fn collect_container_metrics() -> Result<autodock_core::MonitoringReport, String> {
    tauri::async_runtime::spawn_blocking(autodock_core::collect_container_metrics)
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn list_docker_containers() -> Result<autodock_core::DockerContainersReport, String> {
    tauri::async_runtime::spawn_blocking(autodock_core::list_docker_containers)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn detect_stacks(path: String) -> Result<Vec<autodock_core::generate::Suggestion>, String> {
    tauri::async_runtime::spawn_blocking(move || autodock_core::generate::detect_stacks(&path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn inspect_folder(path: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || autodock_core::generate::folder_is_empty(&path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn detect_ides() -> Result<Vec<autodock_core::ide::IdeInfo>, String> {
    tauri::async_runtime::spawn_blocking(autodock_core::ide::detect_ides)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn open_in_ide(id: String, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || autodock_core::ide::open_in_ide(&id, &path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn reveal_folder(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || autodock_core::ide::reveal_folder(&path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn preview_docker_setup(
    request: autodock_core::generate::GenerateRequest,
) -> Result<autodock_core::generate::GeneratePlan, String> {
    tauri::async_runtime::spawn_blocking(move || autodock_core::generate::preview(&request))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn apply_docker_setup(
    request: autodock_core::generate::GenerateRequest,
    files: Vec<String>,
) -> Result<autodock_core::generate::ApplyReport, String> {
    tauri::async_runtime::spawn_blocking(move || autodock_core::generate::apply(&request, &files))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn scan_project(path: String) -> Result<autodock_core::ScanReport, String> {
    tauri::async_runtime::spawn_blocking(move || autodock_core::scan(std::path::Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn doctor_environment() -> Result<autodock_core::DoctorReport, String> {
    tauri::async_runtime::spawn_blocking(autodock_core::doctor)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn execute_console_operation(
    operation: autodock_core::ConsoleOperation,
) -> Result<autodock_core::ConsoleReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        autodock_core::execute_console_operation(operation)
    })
    .await
    .map_err(|error| format!("Falha na tarefa do Console: {error}"))
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scan_project,
            detect_stacks,
            inspect_folder,
            detect_ides,
            open_in_ide,
            reveal_folder,
            preview_docker_setup,
            apply_docker_setup,
            doctor_environment,
            list_docker_containers,
            collect_container_metrics,
            execute_console_operation,
            query_container_logs,
        ])
        .run(tauri::generate_context!())
        .expect("Falha ao iniciar AutoDock");
}
