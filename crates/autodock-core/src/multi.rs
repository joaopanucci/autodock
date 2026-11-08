//! Docker files for a project with several applications (monorepo or
//! microservices): one Dockerfile per application and one compose file at the
//! root that runs them together with the shared services.
//!
//! It reuses the single-application generator for each app, so the Dockerfile of
//! every stack is exactly what the assistant would write for that folder alone.
//! Nothing is overwritten and nothing is written before the plan is confirmed.
use crate::{
    apps::AppRole,
    generate::{
        self, ApplyReport, GeneratePlan, GenerateRequest, GeneratedFile, PackageManager,
        PythonFramework, Resolved, Service, Stack, COMPOSE_NAMES, SERVICE_ORDER,
    },
    ports::{self, PortAssignment, Wanted},
    secrets,
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

const COMPOSE_FILE: &str = "docker-compose.yml";
/// Used when the project already has a compose file, which is never overwritten.
const COMPOSE_ALTERNATIVE: &str = "docker-compose.autodock.yml";
const ENV_FILE: &str = ".env.docker";
const ENV_EXAMPLE: &str = ".env.docker.example";
const MAX_APPS: usize = 12;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppRequest {
    /// Folder of the app relative to the project root (`.` for the root itself).
    pub path: String,
    /// Service name in the compose file; also part of the container name.
    pub name: String,
    pub stack: Stack,
    pub role: AppRole,
    /// Framework label from the detection (`nextjs`, `vite`…); only used to name
    /// the variable that carries the API address.
    #[serde(default)]
    pub framework: Option<String>,
    pub port: u16,
    #[serde(default)]
    pub runtime_version: Option<String>,
    #[serde(default)]
    pub package_manager: Option<PackageManager>,
    #[serde(default)]
    pub start_script: Option<String>,
    #[serde(default)]
    pub python_framework: Option<PythonFramework>,
    #[serde(default)]
    pub python_module: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MultiRequest {
    pub path: String,
    #[serde(default)]
    pub project_name: Option<String>,
    pub apps: Vec<AppRequest>,
    #[serde(default)]
    pub services: Vec<Service>,
    /// Seed and ports returned by a previous preview (the written files then match it).
    #[serde(default)]
    pub seed: Option<String>,
    #[serde(default)]
    pub ports: Option<Vec<PortAssignment>>,
}

struct AppPlan<'a> {
    request: &'a AppRequest,
    resolved: Resolved,
    folder: PathBuf,
    host_port: u16,
}

/// True for a regular file whose first lines say AutoDock generated it.
fn generated_by_autodock(path: &Path) -> bool {
    use std::io::Read;
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    let mut buffer = [0u8; 400];
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let read = file.read(&mut buffer).unwrap_or(0);
    String::from_utf8_lossy(&buffer[..read]).contains("AutoDock")
}

/// A relative folder that stays inside the project: no `..`, no absolute paths,
/// only plain characters (the value ends up in YAML) and no symbolic links.
fn safe_app_dir(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let invalid = || {
        tr!(
            "Pasta de app inválida: {relative}. Use um caminho relativo ao projeto, só com letras, números, ponto, hífen, sublinhado e barra.",
            "Invalid app folder: {relative}. Use a path relative to the project with only letters, numbers, dots, hyphens, underscores and slashes."
        )
    };
    if relative.is_empty()
        || relative.len() > 200
        || relative.starts_with('/')
        || relative.ends_with('/')
        || relative.contains("//")
        || !relative
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
    {
        return Err(invalid());
    }
    if relative == "." {
        return Ok(root.to_path_buf());
    }
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(part) = component else {
            return Err(invalid());
        };
        current.push(part);
        let meta = fs::symlink_metadata(&current).map_err(|_| {
            tr!(
                "A pasta {relative} não existe no projeto.",
                "The folder {relative} does not exist in the project."
            )
        })?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(invalid());
        }
    }
    let canonical = current.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(root) {
        return Err(invalid());
    }
    Ok(canonical)
}

fn valid_frameworks(framework: &Option<String>) -> Option<&str> {
    const KNOWN: [&str; 7] = [
        "nextjs",
        "vite",
        "cra",
        "nuxt",
        "expo",
        "react-native",
        "angular",
    ];
    framework.as_deref().filter(|f| KNOWN.contains(f))
}

fn env_name(service: &str) -> String {
    service
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Prefix the front end's tooling needs for a variable to reach the browser.
fn public_prefix(framework: Option<&str>) -> &'static str {
    match framework {
        Some("nextjs") => "NEXT_PUBLIC_",
        Some("vite") => "VITE_",
        Some("cra") => "REACT_APP_",
        Some("nuxt") => "NUXT_PUBLIC_",
        Some("expo") | Some("react-native") => "EXPO_PUBLIC_",
        _ => "PUBLIC_",
    }
}

fn service_names() -> Vec<&'static str> {
    let mut names: Vec<&str> = SERVICE_ORDER
        .iter()
        .map(|s| generate::name_of(*s))
        .collect();
    names.push("rabbitmq-management");
    names
}

fn validate(request: &MultiRequest) -> Result<(), String> {
    if request.apps.is_empty() {
        return Err(tr!("Nenhum app selecionado.", "No app selected."));
    }
    if request.apps.len() > MAX_APPS {
        return Err(tr!(
            "No máximo {MAX_APPS} apps por projeto.",
            "At most {MAX_APPS} apps per project."
        ));
    }
    let reserved = service_names();
    let mut names: Vec<&str> = vec![];
    let mut folders: Vec<(&str, Stack)> = vec![];
    for app in &request.apps {
        if !crate::scaffold::valid_project_name(&app.name) {
            return Err(tr!(
                "Nome de app inválido: {} (use letras minúsculas, números e hífen).",
                "Invalid app name: {} (use lowercase letters, numbers and hyphens).",
                app.name
            ));
        }
        if reserved.contains(&app.name.as_str()) || names.contains(&app.name.as_str()) {
            return Err(tr!(
                "O nome \"{}\" já é usado por outro app ou por um serviço.",
                "The name \"{}\" is already used by another app or by a service.",
                app.name
            ));
        }
        if folders.contains(&(app.path.as_str(), app.stack)) {
            return Err(tr!(
                "A pasta {} foi escolhida duas vezes com a mesma tecnologia.",
                "The folder {} was chosen twice with the same technology.",
                app.path
            ));
        }
        if app.port == 0 {
            return Err(tr!("Porta inválida.", "Invalid port."));
        }
        names.push(&app.name);
        folders.push((&app.path, app.stack));
    }
    Ok(())
}

fn dockerfile_path(folder: &str) -> String {
    if folder == "." {
        "Dockerfile".into()
    } else {
        format!("{folder}/Dockerfile")
    }
}

fn dockerignore_path(folder: &str) -> String {
    if folder == "." {
        ".dockerignore".into()
    } else {
        format!("{folder}/.dockerignore")
    }
}

fn build_context(folder: &str) -> String {
    if folder == "." {
        ".".into()
    } else {
        format!("./{folder}")
    }
}

/// Mount of the app's source folder for the stacks that run from source.
fn dev_volumes(app: &AppPlan) -> Option<String> {
    let src = build_context(&app.request.path);
    match app.request.stack {
        Stack::Nodejs => Some(format!(
            "    volumes:\n      - {src}:/app\n      - /app/node_modules\n"
        )),
        Stack::Python => Some(format!("    volumes:\n      - {src}:/app\n")),
        Stack::Laravel => Some(format!(
            "    volumes:\n      - {src}:/var/www/html\n      - /var/www/html/vendor\n"
        )),
        Stack::Rails => Some(format!("    volumes:\n      - {src}:/rails\n")),
        Stack::SpringBoot | Stack::Go | Stack::Dotnet => None,
    }
}

/// Plain variables that tell each front end where the backends are: the service
/// address for server-side code and the published address for the browser.
fn wiring(apps: &[AppPlan], current: &AppPlan) -> Vec<(String, String)> {
    if current.request.role == AppRole::Backend {
        return vec![];
    }
    let backends: Vec<&AppPlan> = apps
        .iter()
        .filter(|a| a.request.role == AppRole::Backend)
        .collect();
    let prefix = public_prefix(valid_frameworks(&current.request.framework));
    let mut vars = vec![];
    if let [only] = backends.as_slice() {
        // One backend: the plain names every front end already expects.
        vars.push((
            "API_URL".to_string(),
            format!("http://{}:{}", only.request.name, only.request.port),
        ));
        vars.push((
            format!("{prefix}API_URL"),
            format!("http://localhost:{}", only.host_port),
        ));
    } else {
        for backend in &backends {
            let key = env_name(&backend.request.name);
            vars.push((
                format!("{key}_URL"),
                format!("http://{}:{}", backend.request.name, backend.request.port),
            ));
            vars.push((
                format!("{prefix}{key}_API_URL"),
                format!("http://localhost:{}", backend.host_port),
            ));
        }
    }
    vars
}

fn collect_env(
    apps: &[AppPlan],
    pw: &generate::Passwords,
) -> Result<Vec<(String, String)>, String> {
    let mut merged: BTreeMap<String, String> = BTreeMap::new();
    let mut put = |key: String, value: String| -> Result<(), String> {
        match merged.get(&key) {
            Some(existing) if existing != &value => Err(tr!(
                "Dois apps definem {key} com valores diferentes.",
                "Two apps define {key} with different values."
            )),
            _ => {
                merged.insert(key, value);
                Ok(())
            }
        }
    };
    let first = &apps[0].resolved;
    for (key, value) in generate::service_env_vars(first, pw) {
        put(key, value)?;
    }
    for app in apps {
        let (_, secret) = generate::app_environment(&app.resolved, pw);
        for (key, value) in secret {
            put(key, value)?;
        }
    }
    Ok(merged.into_iter().collect())
}

fn env_file_content(vars: &[(String, String)], example: bool) -> String {
    let mut out = if example {
        tr!(
            "# Gerado pelo AutoDock. Copie para .env.docker e troque cada \"change-me\" por uma senha forte.\n# Este arquivo é só um modelo, sem segredos, e pode ser versionado.\n",
            "# Generated by AutoDock. Copy to .env.docker and replace each \"change-me\" with a strong password.\n# This file is only a template, has no secrets and can be committed.\n"
        )
    } else {
        tr!(
            "# Gerado pelo AutoDock. Contém senhas fortes: NÃO versione este arquivo.\n# Lido pelo docker-compose.yml (env_file) por todos os apps e serviços.\n",
            "# Generated by AutoDock. Contains strong passwords: DO NOT commit this file.\n# Read by docker-compose.yml (env_file) by every app and service.\n"
        )
    };
    for (key, value) in vars {
        out.push_str(&format!("{key}={value}\n"));
    }
    out
}

fn compose(
    project: &str,
    apps: &[AppPlan],
    services: &[Service],
    pw: &generate::Passwords,
) -> String {
    let secrets_used = generate::has_secrets(services);
    let mut out = tr!(
        "# Gerado pelo AutoDock. Revise antes de usar.\n# Nenhuma senha fica neste arquivo: elas estão em {ENV_FILE} (não versione).\nname: {project}\nservices:\n",
        "# Generated by AutoDock. Review before use.\n# No password lives in this file: they are in {ENV_FILE} (do not commit).\nname: {project}\nservices:\n"
    );
    for app in apps {
        let (plain, _) = generate::app_environment(&app.resolved, pw);
        out.push_str(&format!(
            "  {name}:\n    build: {context}\n    container_name: {project}-{name}\n    ports:\n      - \"{host}:{container}\"\n",
            name = app.request.name,
            context = build_context(&app.request.path),
            host = app.host_port,
            container = app.request.port,
        ));
        if secrets_used {
            out.push_str(&format!("    env_file:\n      - {ENV_FILE}\n"));
        }
        out.push_str("    environment:\n");
        for (key, value) in plain.into_iter().chain(wiring(apps, app)) {
            out.push_str(&format!("      {key}: \"{value}\"\n"));
        }
        if let Some(volumes) = dev_volumes(app) {
            out.push_str(&volumes);
        }
        out.push_str(&generate::watch_block(
            app.request.stack,
            &build_context(&app.request.path),
        ));
        let mut depends = String::new();
        for service in services {
            match service {
                Service::Adminer => {}
                Service::Mailhog => {
                    depends.push_str("      mailhog:\n        condition: service_started\n")
                }
                other => depends.push_str(&format!(
                    "      {}:\n        condition: service_healthy\n",
                    generate::name_of(*other)
                )),
            }
        }
        if app.request.role != AppRole::Backend {
            for backend in apps.iter().filter(|a| a.request.role == AppRole::Backend) {
                depends.push_str(&format!(
                    "      {}:\n        condition: service_started\n",
                    backend.request.name
                ));
            }
        }
        if !depends.is_empty() {
            out.push_str("    depends_on:\n");
            out.push_str(&depends);
        }
        out.push('\n');
    }
    let first = &apps[0].resolved;
    let mut volumes: Vec<&str> = vec![];
    for (index, service) in services.iter().enumerate() {
        let block = generate::service_block(first, *service);
        match block.split_once('\n') {
            Some((head, rest)) => out.push_str(&format!(
                "{head}\n    container_name: {project}-{}\n{rest}",
                generate::name_of(*service)
            )),
            None => out.push_str(&block),
        }
        if index + 1 < services.len() {
            out.push('\n');
        }
        match service {
            Service::Postgres => volumes.push("postgres_data"),
            Service::Mysql => volumes.push("mysql_data"),
            Service::Mongodb => volumes.push("mongodb_data"),
            Service::Redis => volumes.push("redis_data"),
            Service::Rabbitmq => volumes.push("rabbitmq_data"),
            _ => {}
        }
    }
    if !volumes.is_empty() {
        out.push_str("\nvolumes:\n");
        for volume in volumes {
            out.push_str(&format!("  {volume}:\n"));
        }
    }
    out
}

fn notes(
    apps: &[AppPlan],
    services: &[Service],
    ports: &[PortAssignment],
    compose_name: &str,
    compose_exists: bool,
    workspace_root: bool,
    existing_dockerfiles: &[String],
) -> Vec<String> {
    let mut notes = vec![tr!(
        "Nada foi gravado ainda. Revise os arquivos antes de confirmar.",
        "Nothing has been written yet. Review the files before confirming."
    )];
    if compose_exists {
        notes.push(tr!(
            "O projeto já tem um arquivo compose, que não será tocado. Os apps ficam em {compose_name}: suba com docker compose -f {compose_name} up --build.",
            "The project already has a compose file, which will not be touched. The apps go in {compose_name}: start them with docker compose -f {compose_name} up --build."
        ));
    }
    for assigned in ports {
        if assigned.host_port != assigned.preferred_port {
            notes.push(tr!(
                "A porta {} está em uso ou reservada por outro projeto: {} usará a {} no seu computador (dentro do Docker continua {}).",
                "Port {} is in use or reserved by another project: {} will use {} on your computer (inside Docker it stays {}).",
                assigned.preferred_port,
                assigned.name,
                assigned.host_port,
                assigned.container_port
            ));
        }
    }
    let urls: Vec<String> = apps
        .iter()
        .map(|a| format!("{} → http://localhost:{}", a.request.name, a.host_port))
        .collect();
    notes.push(tr!(
        "Depois de subir: {}.",
        "Once it is up: {}.",
        urls.join(" · ")
    ));
    let backends = apps
        .iter()
        .filter(|a| a.request.role == AppRole::Backend)
        .count();
    if backends > 0 && apps.iter().any(|a| a.request.role != AppRole::Backend) {
        notes.push(tr!(
            "Os front ends recebem o endereço dos backends nas variáveis API_URL (entre containers) e no prefixo do seu framework, por exemplo NEXT_PUBLIC_API_URL (para o navegador). Se o seu código usa outros nomes, ajuste no compose.",
            "The front ends get the backends' address in API_URL (container to container) and in the prefix of their framework, for example NEXT_PUBLIC_API_URL (for the browser). If your code uses other names, adjust them in the compose file."
        ));
    }
    if !services.is_empty() {
        notes.push(tr!(
            "Os serviços auxiliares publicam portas apenas em 127.0.0.1.",
            "Auxiliary services publish ports on 127.0.0.1 only."
        ));
    }
    if generate::has_secrets(services) {
        notes.push(tr!(
            "Senhas fortes de 32 caracteres foram geradas agora e ficam em {ENV_FILE} (permissão restrita), lido por todos os apps. Não versione esse arquivo.",
            "Strong 32-character passwords were just generated and live in {ENV_FILE} (restricted permissions), read by every app. Do not commit that file."
        ));
    }
    if apps.iter().any(|a| a.request.stack == Stack::Nodejs) {
        notes.push(tr!(
            "Os apps Node.js precisam escutar em 0.0.0.0 e na porta da variável PORT, senão não ficam acessíveis fora do container.",
            "Node.js apps must listen on 0.0.0.0 and on the port in the PORT variable, otherwise they are not reachable outside the container."
        ));
    }
    if apps.iter().any(|a| {
        matches!(
            a.request.stack,
            Stack::Go | Stack::Dotnet | Stack::SpringBoot
        )
    }) {
        notes.push(tr!(
            "Apps compilados (Go, .NET, Spring Boot): alterações no código exigem docker compose up --build.",
            "Compiled apps (Go, .NET, Spring Boot): code changes need docker compose up --build."
        ));
    }
    for folder in existing_dockerfiles {
        notes.push(tr!(
            "{folder} já tem um Dockerfile: ele será usado como está (não é sobrescrito). Confira se o app escuta na porta escolhida.",
            "{folder} already has a Dockerfile: it will be used as is (it is not overwritten). Check that the app listens on the chosen port."
        ));
    }
    if workspace_root {
        notes.push(tr!(
            "O projeto é um workspace Node.js: apps que dependem de pacotes locais do workspace podem exigir ajuste no Dockerfile.",
            "The project is a Node.js workspace: apps that depend on local workspace packages may need Dockerfile adjustments."
        ));
    }
    notes
}

/// Builds the plan. Pure with respect to the file system except for existence checks.
pub fn preview(request: &MultiRequest) -> Result<GeneratePlan, String> {
    validate(request)?;
    let root = generate::root_of(&request.path)?;
    let project = match &request.project_name {
        Some(name) if !crate::scaffold::valid_project_name(name) => {
            return Err(tr!(
                "Nome do projeto inválido (use letras minúsculas, números e hífen).",
                "Invalid project name (use lowercase letters, numbers and hyphens)."
            ))
        }
        Some(name) => name.clone(),
        None => generate::project_name(&root),
    };
    let services: Vec<Service> = SERVICE_ORDER
        .iter()
        .copied()
        .filter(|s| request.services.contains(s))
        .collect();

    // Ports and passwords are decided once for the whole project.
    let mut wanted: Vec<Wanted> = request
        .apps
        .iter()
        .map(|a| (a.name.clone(), a.port, a.port))
        .collect();
    for service in &services {
        for (name, container, preferred) in generate::service_ports(*service) {
            wanted.push((name.to_string(), *container, *preferred));
        }
    }
    let assigned = match &request.ports {
        Some(given) => ports::validate_given(&root, &wanted, given)?,
        None => ports::allocate(&root, &wanted)?,
    };
    let seed = match &request.seed {
        Some(hex) => secrets::seed_from_hex(hex)?,
        None => secrets::new_seed()?,
    };
    let service_ports: Vec<PortAssignment> = assigned
        .iter()
        .filter(|p| !request.apps.iter().any(|a| a.name == p.name))
        .cloned()
        .collect();

    let mut plans: Vec<AppPlan> = vec![];
    for app in &request.apps {
        let folder = safe_app_dir(&root, &app.path)?;
        let own = assigned
            .iter()
            .find(|p| p.name == app.name)
            .ok_or_else(|| tr!("Porta não encontrada.", "Port not found."))?;
        let mut app_ports = vec![PortAssignment {
            name: "app".into(),
            container_port: app.port,
            host_port: own.host_port,
            preferred_port: app.port,
        }];
        app_ports.extend(service_ports.iter().cloned());
        let single = GenerateRequest {
            path: folder.to_string_lossy().into_owned(),
            stack: app.stack,
            port: app.port,
            runtime_version: app.runtime_version.clone(),
            package_manager: app.package_manager,
            start_script: app.start_script.clone(),
            python_framework: app.python_framework,
            python_module: app.python_module.clone(),
            services: services.clone(),
            new_project: false,
            project_name: Some(project.clone()),
            seed: None,
            ports: None,
        };
        let resolved = generate::resolve_app(&single, &folder, app_ports, seed)?;
        plans.push(AppPlan {
            request: app,
            resolved,
            folder,
            host_port: own.host_port,
        });
    }

    let file =
        |name: String, description: String, content: String, exists: bool, sensitive: bool| {
            GeneratedFile {
                name,
                description,
                content,
                sensitive,
                exists,
            }
        };
    let mut files: Vec<GeneratedFile> = vec![];
    let mut existing_dockerfiles: Vec<String> = vec![];
    for plan in &plans {
        let folder = &plan.request.path;
        let exists = generate::present(&plan.folder, generate::DOCKERFILE);
        if exists {
            existing_dockerfiles.push(folder.clone());
        }
        files.push(file(
            dockerfile_path(folder),
            tr!(
                "Como a imagem de {} é construída.",
                "How the image of {} is built.",
                plan.request.name
            ),
            generate::dockerfile(&plan.resolved),
            exists,
            false,
        ));
        files.push(file(
            dockerignore_path(folder),
            tr!(
                "Evita copiar arquivos desnecessários de {} para a imagem.",
                "Keeps unneeded files of {} out of the image.",
                plan.request.name
            ),
            generate::dockerignore(plan.request.stack),
            generate::present(&plan.folder, generate::DOCKERIGNORE),
            false,
        ));
    }
    // A compose file written by a previous run of AutoDock is ours: it keeps the usual
    // name (and is skipped as "already exists"). Somebody else's file is never touched.
    let existing: Vec<&str> = COMPOSE_NAMES
        .iter()
        .copied()
        .filter(|name| generate::present(&root, name))
        .collect();
    let ours = existing == [COMPOSE_FILE] && generated_by_autodock(&root.join(COMPOSE_FILE));
    let compose_exists = !existing.is_empty() && !ours;
    let compose_name = if compose_exists {
        COMPOSE_ALTERNATIVE
    } else {
        COMPOSE_FILE
    };
    let real = generate::passwords(&plans[0].resolved);
    files.push(file(
        compose_name.into(),
        tr!(
            "Roda todos os apps e os serviços escolhidos. Não contém senhas.",
            "Runs every app and the chosen services. Contains no passwords."
        ),
        compose(&project, &plans, &services, &real),
        generate::present(&root, compose_name),
        false,
    ));
    if generate::has_secrets(&services) {
        let placeholder = generate::placeholder_passwords(&real);
        files.push(file(
            ENV_FILE.into(),
            tr!(
                "Senhas fortes de 32 caracteres geradas agora. Não versione este arquivo.",
                "Strong 32-character passwords generated now. Do not commit this file."
            ),
            env_file_content(&collect_env(&plans, &real)?, false),
            generate::present(&root, ENV_FILE),
            true,
        ));
        files.push(file(
            ENV_EXAMPLE.into(),
            tr!(
                "Modelo das variáveis, sem segredos, para quem clonar o projeto.",
                "Variable template without secrets, for whoever clones the project."
            ),
            env_file_content(&collect_env(&plans, &placeholder)?, true),
            generate::present(&root, ENV_EXAMPLE),
            false,
        ));
    }

    let workspace_root = generate::read_manifest(&root, "package.json").is_some_and(|text| {
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .is_some_and(|v| v.get("workspaces").is_some())
    });
    Ok(GeneratePlan {
        schema_version: 1,
        project: root.display().to_string(),
        notes: notes(
            &plans,
            &services,
            &assigned,
            compose_name,
            compose_exists,
            workspace_root,
            &existing_dockerfiles,
        ),
        files,
        seed: secrets::seed_to_hex(&seed),
        ports: assigned,
    })
}

/// Writes the selected files of a freshly built plan. Existing files are skipped,
/// never overwritten.
pub fn apply(request: &MultiRequest, selected: &[String]) -> Result<ApplyReport, String> {
    if selected.is_empty() {
        return Err(tr!("Nenhum arquivo selecionado.", "No file selected."));
    }
    let plan = preview(request)?;
    let root = generate::root_of(&request.path)?;
    generate::write_plan(&root, &plan, selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let full = dir.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, content).unwrap();
        }
        dir
    }

    fn shop() -> tempfile::TempDir {
        tree(&[
            ("api/go.mod", "module api\n"),
            ("api/main.go", "package main\n"),
            (
                "web/package.json",
                r#"{"scripts":{"dev":"next dev"},"dependencies":{"next":"16"}}"#,
            ),
        ])
    }

    fn app(name: &str, path: &str, stack: Stack, role: AppRole, port: u16) -> AppRequest {
        AppRequest {
            path: path.into(),
            name: name.into(),
            stack,
            role,
            framework: None,
            port,
            runtime_version: None,
            package_manager: None,
            start_script: None,
            python_framework: None,
            python_module: None,
        }
    }

    fn shop_request(dir: &tempfile::TempDir) -> MultiRequest {
        let mut web = app("web", "web", Stack::Nodejs, AppRole::Frontend, 3000);
        web.framework = Some("nextjs".into());
        web.start_script = Some("dev".into());
        MultiRequest {
            path: dir.path().to_string_lossy().into(),
            project_name: Some("loja".into()),
            apps: vec![app("api", "api", Stack::Go, AppRole::Backend, 8080), web],
            services: vec![Service::Postgres, Service::Redis],
            seed: None,
            ports: None,
        }
    }

    fn file<'a>(plan: &'a GeneratePlan, name: &str) -> &'a GeneratedFile {
        plan.files
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| {
                panic!(
                    "no file {name}: {:?}",
                    plan.files.iter().map(|f| &f.name).collect::<Vec<_>>()
                )
            })
    }

    fn host(plan: &GeneratePlan, name: &str) -> u16 {
        plan.ports
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .host_port
    }

    /// The compose block of one service: from its header to the next blank line.
    fn block<'a>(compose: &'a str, header: &str) -> &'a str {
        compose
            .split(header)
            .nth(1)
            .unwrap_or_else(|| panic!("no block {header}"))
            .split("\n\n")
            .next()
            .unwrap()
    }

    #[test]
    fn one_dockerfile_per_app_and_one_compose_for_everything() {
        let dir = shop();
        let plan = preview(&shop_request(&dir)).unwrap();
        let names: Vec<&str> = plan.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "api/Dockerfile",
                "api/.dockerignore",
                "web/Dockerfile",
                "web/.dockerignore",
                "docker-compose.yml",
                ".env.docker",
                ".env.docker.example"
            ]
        );
        assert!(file(&plan, "api/Dockerfile")
            .content
            .contains("FROM golang:1.26-alpine AS build"));
        let web = &file(&plan, "web/Dockerfile").content;
        assert!(
            web.contains("FROM node:22-alpine") && web.contains(r#"CMD ["npm", "run", "dev"]"#)
        );

        let compose = &file(&plan, "docker-compose.yml").content;
        assert!(compose.contains("\nname: loja\nservices:\n"));
        assert!(compose.contains("  api:\n    build: ./api\n    container_name: loja-api\n"));
        assert!(compose.contains("  web:\n    build: ./web\n    container_name: loja-web\n"));
        assert!(compose.contains("  postgres:\n    container_name: loja-postgres\n"));
        assert!(compose.contains("  redis:\n    container_name: loja-redis\n"));
        assert!(compose.contains("      - ./web:/app\n      - /app/node_modules\n"));
        assert!(
            !compose.contains("./api:/"),
            "Go is compiled: no source mount"
        );
        assert!(file(&plan, ".env.docker").sensitive);
        assert!(!file(&plan, "docker-compose.yml").sensitive);
    }

    #[test]
    fn the_front_end_learns_where_the_backend_is_and_waits_for_it() {
        let dir = shop();
        let plan = preview(&shop_request(&dir)).unwrap();
        let compose = &file(&plan, "docker-compose.yml").content;
        let web = block(compose, "  web:\n");
        eprintln!("WEB BLOCK:\n{web}\n-----");
        assert!(web.contains("      API_URL: \"http://api:8080\"\n"));
        assert!(web.contains(&format!(
            "NEXT_PUBLIC_API_URL: \"http://localhost:{}\"",
            host(&plan, "api")
        )));
        assert!(web.contains("      api:\n        condition: service_started"));
        assert!(web.contains("      postgres:\n        condition: service_healthy\n"));
        // every variable appears once, with the plain names
        assert_eq!(web.matches("API_URL:").count(), 2, "{web}");
        assert!(!web.contains("API_API_URL"));
        let api = block(compose, "  api:\n");
        assert!(
            !api.contains("NEXT_PUBLIC"),
            "backends get no front end variables"
        );
        assert!(
            !api.contains("service_started"),
            "the backend does not wait for the front end"
        );
    }

    #[test]
    fn several_backends_get_one_variable_each_and_no_ambiguous_alias() {
        let dir = tree(&[
            ("users/go.mod", "module u\n"),
            ("users/main.go", ""),
            ("orders/go.mod", "module o\n"),
            ("orders/main.go", ""),
            (
                "site/package.json",
                r#"{"scripts":{"dev":"vite"},"devDependencies":{"vite":"6"}}"#,
            ),
        ]);
        let mut site = app("site", "site", Stack::Nodejs, AppRole::Frontend, 5173);
        site.framework = Some("vite".into());
        site.start_script = Some("dev".into());
        let request = MultiRequest {
            path: dir.path().to_string_lossy().into(),
            project_name: Some("shop".into()),
            apps: vec![
                app("users", "users", Stack::Go, AppRole::Backend, 8080),
                app("orders", "orders", Stack::Go, AppRole::Backend, 8081),
                site,
            ],
            services: vec![],
            seed: None,
            ports: None,
        };
        let plan = preview(&request).unwrap();
        let compose = &file(&plan, "docker-compose.yml").content;
        assert!(compose.contains("USERS_URL: \"http://users:8080\""));
        assert!(compose.contains("ORDERS_URL: \"http://orders:8081\""));
        assert!(compose.contains(&format!(
            "VITE_USERS_API_URL: \"http://localhost:{}\"",
            host(&plan, "users")
        )));
        assert!(
            !compose.contains("      API_URL:"),
            "no ambiguous API_URL alias"
        );
        assert!(
            !plan.files.iter().any(|f| f.name == ".env.docker"),
            "no secrets without databases"
        );
    }

    #[test]
    fn passwords_live_only_in_the_env_files_and_are_shared_by_the_apps() {
        let dir = shop();
        let plan = preview(&shop_request(&dir)).unwrap();
        let env = &file(&plan, ".env.docker").content;
        let password = env
            .lines()
            .find_map(|l| l.strip_prefix("POSTGRES_PASSWORD="))
            .unwrap();
        assert_eq!(password.len(), 32);
        assert!(env.contains(&format!(
            "DATABASE_URL=postgresql://app:{password}@postgres:5432/app"
        )));
        for f in &plan.files {
            if f.name != ".env.docker" {
                assert!(
                    !f.content.contains(password),
                    "{} leaks the password",
                    f.name
                );
            }
        }
        let example = &file(&plan, ".env.docker.example").content;
        assert!(example.contains("POSTGRES_PASSWORD=change-me"));
        // both apps and the Postgres service read the shared file; Redis has no secret
        let compose = &file(&plan, "docker-compose.yml").content;
        assert_eq!(
            compose
                .matches("    env_file:\n      - .env.docker\n")
                .count(),
            3
        );
    }

    #[test]
    fn host_ports_are_distinct_and_follow_the_assigned_plan() {
        let dir = shop();
        let plan = preview(&shop_request(&dir)).unwrap();
        let mut hosts: Vec<u16> = plan.ports.iter().map(|p| p.host_port).collect();
        let total = hosts.len();
        hosts.sort_unstable();
        hosts.dedup();
        assert_eq!(hosts.len(), total);
        let compose = &file(&plan, "docker-compose.yml").content;
        assert!(compose.contains(&format!("      - \"{}:8080\"\n", host(&plan, "api"))));
        assert!(compose.contains(&format!("      - \"{}:3000\"\n", host(&plan, "web"))));
        assert!(compose.contains(&format!(
            "      - \"127.0.0.1:{}:5432\"\n",
            host(&plan, "postgres")
        )));
    }

    #[test]
    fn previewing_again_with_the_seed_and_ports_gives_identical_files() {
        let dir = shop();
        let first = preview(&shop_request(&dir)).unwrap();
        let mut request = shop_request(&dir);
        request.seed = Some(first.seed.clone());
        request.ports = Some(first.ports.clone());
        let second = preview(&request).unwrap();
        for (a, b) in first.files.iter().zip(&second.files) {
            assert_eq!((&a.name, &a.content), (&b.name, &b.content));
        }
        // tampered ports are rejected
        request.ports.as_mut().unwrap()[0].container_port = 9;
        assert!(preview(&request).is_err());
    }

    #[test]
    fn an_existing_compose_file_is_never_touched_and_the_apps_go_to_another_file() {
        let dir = shop();
        fs::write(dir.path().join("compose.yaml"), "services: {}\n").unwrap();
        let plan = preview(&shop_request(&dir)).unwrap();
        assert!(plan.files.iter().all(|f| f.name != "docker-compose.yml"));
        assert!(!file(&plan, "docker-compose.autodock.yml").exists);
        assert!(plan
            .notes
            .iter()
            .any(|n| n.contains("docker compose -f docker-compose.autodock.yml")));
    }

    #[test]
    fn existing_dockerfiles_are_reused_and_never_overwritten() {
        let dir = shop();
        fs::write(dir.path().join("api/Dockerfile"), "FROM scratch\n").unwrap();
        let request = shop_request(&dir);
        let plan = preview(&request).unwrap();
        assert!(file(&plan, "api/Dockerfile").exists);
        assert!(!file(&plan, "web/Dockerfile").exists);
        assert!(plan
            .notes
            .iter()
            .any(|n| n.contains("api already has a Dockerfile")));
        let all: Vec<String> = plan.files.iter().map(|f| f.name.clone()).collect();
        let report = apply(&request, &all).unwrap();
        assert!(report.skipped.iter().any(|s| s.name == "api/Dockerfile"));
        assert!(report.written.contains(&"web/Dockerfile".to_string()));
        assert_eq!(
            fs::read_to_string(dir.path().join("api/Dockerfile")).unwrap(),
            "FROM scratch\n"
        );
    }

    #[test]
    fn apply_writes_every_file_once_and_a_second_run_changes_nothing() {
        let dir = shop();
        let request = shop_request(&dir);
        let names: Vec<String> = preview(&request)
            .unwrap()
            .files
            .iter()
            .map(|f| f.name.clone())
            .collect();
        let report = apply(&request, &names).unwrap();
        assert_eq!(report.written.len(), names.len());
        assert!(report.skipped.is_empty());
        for name in &names {
            assert!(dir.path().join(name).is_file(), "{name} was not written");
        }
        let again = apply(&request, &names).unwrap();
        assert!(
            again.written.is_empty(),
            "nothing new is written the second time"
        );
        assert_eq!(again.skipped.len(), names.len());
        assert!(apply(&request, &[]).is_err());
        assert!(apply(&request, &["outside.txt".to_string()]).is_err());
        assert!(apply(&request, &["../escape".to_string()]).is_err());
    }

    #[test]
    fn unsafe_folders_names_and_ports_are_rejected() {
        let dir = shop();
        let base = shop_request(&dir);
        let rejected = |edit: &dyn Fn(&mut MultiRequest)| {
            let mut request = base.clone();
            edit(&mut request);
            assert!(preview(&request).is_err());
        };
        for bad in [
            "../api",
            "/etc",
            "api/../web",
            "a\\b",
            "api;ls",
            "api web",
            "",
            "api/",
            "./api",
            "api//x",
            "nope",
            "C:/x",
        ] {
            rejected(&|r| r.apps[0].path = bad.to_string());
        }
        rejected(&|r| r.apps[0].name = "Api".into());
        rejected(&|r| r.apps[0].name = "postgres".into());
        rejected(&|r| r.apps[1].name = "api".into());
        rejected(&|r| r.apps[0].port = 0);
        rejected(&|r| r.apps.clear());
        rejected(&|r| r.project_name = Some("Bad Name".into()));
        let many: Vec<AppRequest> = (0..=MAX_APPS)
            .map(|i| {
                app(
                    &format!("a{i}"),
                    "api",
                    Stack::Go,
                    AppRole::Backend,
                    9000 + i as u16,
                )
            })
            .collect();
        rejected(&|r| r.apps = many.clone());
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_is_not_accepted_as_an_app_folder() {
        let outside = tree(&[("go.mod", "module x\n"), ("main.go", "")]);
        let dir = shop();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("linked")).unwrap();
        let mut request = shop_request(&dir);
        request.apps[0].path = "linked".into();
        assert!(preview(&request).is_err());
    }

    #[test]
    fn the_app_at_the_root_uses_plain_paths() {
        let dir = tree(&[
            ("go.mod", "module root\n"),
            ("main.go", ""),
            ("worker/go.mod", "module worker\n"),
            ("worker/main.go", ""),
        ]);
        let request = MultiRequest {
            path: dir.path().to_string_lossy().into(),
            project_name: Some("mono".into()),
            apps: vec![
                app("mono", ".", Stack::Go, AppRole::Backend, 8080),
                app("worker", "worker", Stack::Go, AppRole::Backend, 8081),
            ],
            services: vec![],
            seed: None,
            ports: None,
        };
        let plan = preview(&request).unwrap();
        assert!(plan.files.iter().any(|f| f.name == "Dockerfile"));
        assert!(plan.files.iter().any(|f| f.name == ".dockerignore"));
        assert!(plan.files.iter().any(|f| f.name == "worker/Dockerfile"));
        let compose = &file(&plan, "docker-compose.yml").content;
        assert!(compose.contains("  mono:\n    build: .\n    container_name: mono-mono\n"));
    }

    #[test]
    fn notes_cover_ports_urls_workspaces_and_rebuilds() {
        let dir = tree(&[
            ("package.json", r#"{"workspaces":["apps/*"]}"#),
            ("apps/api/go.mod", "module a\n"),
            ("apps/api/main.go", ""),
        ]);
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let busy = listener.local_addr().unwrap().port();
        let request = MultiRequest {
            path: dir.path().to_string_lossy().into(),
            project_name: Some("mono".into()),
            apps: vec![app("api", "apps/api", Stack::Go, AppRole::Backend, busy)],
            services: vec![Service::Mailhog],
            seed: None,
            ports: None,
        };
        let plan = preview(&request).unwrap();
        let notes = plan.notes.join("\n");
        assert!(notes.contains(&format!("Port {busy} is in use")), "{notes}");
        assert!(notes.contains("Once it is up: api → http://localhost:"));
        assert!(notes.contains("workspace"));
        assert!(notes.contains("docker compose up --build"));
        drop(listener);
    }

    #[test]
    fn english_output_has_no_portuguese_and_portuguese_is_available() {
        use crate::i18n::{with_lang, Lang};
        let dir = shop();
        let request = shop_request(&dir);
        let english = with_lang(Lang::En, || preview(&request).unwrap());
        let portuguese = with_lang(Lang::Pt, || preview(&request).unwrap());
        let marks = "áéíóúâêôãõçÁÉÍÓÚÂÊÔÃÕÇ";
        for note in &english.notes {
            assert!(
                !note.chars().any(|c| marks.contains(c)),
                "Portuguese in note: {note}"
            );
        }
        for f in &english.files {
            assert!(
                !f.content.chars().any(|c| marks.contains(c)),
                "Portuguese in {}",
                f.name
            );
            assert!(!f.description.chars().any(|c| marks.contains(c)));
        }
        assert!(portuguese
            .notes
            .iter()
            .any(|n| n.contains("Nada foi gravado")));
        assert_ne!(english.notes, portuguese.notes);
    }
}
