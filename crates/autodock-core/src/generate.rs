//! Docker scaffolding for existing projects: Dockerfile, compose, .dockerignore.
//!
//! Safety contract: `preview` never writes. `apply` regenerates the files from the
//! validated request (the UI never supplies file contents), creates files with
//! exclusive creation and never overwrites or follows symlinks. `.env` files are
//! never read. Every interpolated value is validated against an allowlist.
use crate::{
    ports::{self, PortAssignment, Wanted},
    secrets::{self, Seed},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const COMPOSE_NAMES: [&str; 4] = [
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];
pub const DOCKERFILE: &str = "Dockerfile";
pub const COMPOSE_FILE: &str = "docker-compose.yml";
pub const DOCKERIGNORE: &str = ".dockerignore";
pub const ENV_EXAMPLE: &str = ".env.docker.example";
pub const ENV_FILE: &str = ".env.docker";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stack {
    Nodejs,
    SpringBoot,
    Python,
    Laravel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    Postgres,
    Mysql,
    Mongodb,
    Redis,
    Rabbitmq,
    Adminer,
    Mailhog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PythonFramework {
    Fastapi,
    Django,
    Flask,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateRequest {
    pub path: String,
    pub stack: Stack,
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
    #[serde(default)]
    pub services: Vec<Service>,
    /// Creates a starter project in an empty folder before the Docker files.
    #[serde(default)]
    pub new_project: bool,
    /// Name used in the starter manifests; defaults to the folder name.
    #[serde(default)]
    pub project_name: Option<String>,
    /// Seed returned by a previous preview. Sending it back makes the written
    /// passwords identical to the previewed ones; without it a new one is created.
    #[serde(default)]
    pub seed: Option<String>,
    /// Port assignments returned by a previous preview (validated again).
    #[serde(default)]
    pub ports: Option<Vec<PortAssignment>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedFile {
    pub name: String,
    pub description: String,
    pub content: String,
    /// The file holds secrets: it must not be versioned and is created private.
    pub sensitive: bool,
    /// True when writing would collide with an existing file (never overwritten).
    pub exists: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratePlan {
    pub schema_version: u32,
    pub project: String,
    pub files: Vec<GeneratedFile>,
    pub notes: Vec<String>,
    /// Pass back with the request when applying, so the content stays identical.
    pub seed: String,
    pub ports: Vec<PortAssignment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub written: Vec<String>,
    pub skipped: Vec<SkippedFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub stack: Stack,
    pub evidence: String,
    pub scripts: Vec<String>,
    pub package_manager: Option<PackageManager>,
    pub python_framework: Option<PythonFramework>,
    /// The project uses the Vite dev server (default port 5173).
    pub vite: bool,
}

// ---------------------------------------------------------------- detection

pub(crate) fn root_of(path: &str) -> Result<PathBuf, String> {
    let root = Path::new(path)
        .canonicalize()
        .map_err(|e| format!("{path}: {e}"))?;
    if !root.is_dir() {
        return Err(format!("Não é diretório: {}", root.display()));
    }
    Ok(root)
}

/// Entries that do not make a folder "non-empty" for starting a new project.
const IGNORABLE_ENTRIES: [&str; 3] = [".git", ".DS_Store", "Thumbs.db"];

/// True when the folder holds nothing but harmless entries (see above).
pub fn folder_is_empty(path: &str) -> Result<bool, String> {
    let root = root_of(path)?;
    let entries = fs::read_dir(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if !IGNORABLE_ENTRIES.contains(&entry.file_name().to_string_lossy().as_ref()) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn regular_file(root: &Path, name: &str) -> bool {
    fs::symlink_metadata(root.join(name))
        .map(|m| m.is_file() && !m.file_type().is_symlink())
        .unwrap_or(false)
}

fn present(root: &Path, name: &str) -> bool {
    fs::symlink_metadata(root.join(name)).is_ok()
}

fn read_manifest(root: &Path, name: &str) -> Option<String> {
    if !regular_file(root, name) {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(root.join(name))
        .ok()?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Suggests stacks from top-level manifests only. Reads manifests as data; never
/// executes anything and never reads `.env` files.
pub fn detect_stacks(path: &str) -> Result<Vec<Suggestion>, String> {
    let root = root_of(path)?;
    let mut found = Vec::new();
    if let Some(text) = read_manifest(&root, "package.json") {
        let scripts = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("scripts").and_then(|s| s.as_object().cloned()))
            .map(|s| s.keys().filter(|k| valid_script(k)).cloned().collect())
            .unwrap_or_default();
        let vite = text.contains("\"vite\"");
        let package_manager = if regular_file(&root, "pnpm-lock.yaml") {
            Some(PackageManager::Pnpm)
        } else if regular_file(&root, "yarn.lock") {
            Some(PackageManager::Yarn)
        } else {
            Some(PackageManager::Npm)
        };
        found.push(Suggestion {
            stack: Stack::Nodejs,
            evidence: "package.json".into(),
            scripts,
            package_manager,
            python_framework: None,
            vite,
        });
    }
    if let Some(text) = read_manifest(&root, "pom.xml") {
        if text.contains("org.springframework.boot") {
            found.push(Suggestion {
                stack: Stack::SpringBoot,
                evidence: "pom.xml (referência a org.springframework.boot)".into(),
                scripts: vec![],
                package_manager: None,
                python_framework: None,
                vite: false,
            });
        }
    }
    for name in ["requirements.txt", "pyproject.toml"] {
        if let Some(text) = read_manifest(&root, name) {
            let lower = text.to_ascii_lowercase();
            let framework = if lower.contains("fastapi") {
                Some(PythonFramework::Fastapi)
            } else if lower.contains("django") {
                Some(PythonFramework::Django)
            } else if lower.contains("flask") {
                Some(PythonFramework::Flask)
            } else {
                None
            };
            found.push(Suggestion {
                stack: Stack::Python,
                evidence: name.into(),
                scripts: vec![],
                package_manager: None,
                python_framework: framework,
                vite: false,
            });
            break;
        }
    }
    if let Some(text) = read_manifest(&root, "composer.json") {
        if text.contains("laravel/framework") {
            found.push(Suggestion {
                stack: Stack::Laravel,
                evidence: "composer.json (laravel/framework)".into(),
                scripts: vec![],
                package_manager: None,
                python_framework: None,
                vite: false,
            });
        }
    }
    Ok(found)
}

// --------------------------------------------------------------- validation

pub(crate) fn valid_script(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b":_.-".contains(&b))
}

fn valid_module(value: &str) -> bool {
    let mut parts = value.splitn(2, ':');
    let ok = |part: &str| {
        !part.is_empty()
            && part.len() <= 64
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.".contains(&b))
    };
    ok(parts.next().unwrap_or("")) && parts.next().map(ok).unwrap_or(true)
}

fn runtime_version(stack: Stack, requested: &Option<String>) -> Result<String, String> {
    let (allowed, default): (&[&str], &str) = match stack {
        Stack::Nodejs => (&["24", "22", "20"], "22"),
        Stack::SpringBoot => (&["21", "17"], "21"),
        Stack::Python => (&["3.13", "3.12", "3.11"], "3.12"),
        Stack::Laravel => (&["8.5", "8.4", "8.3", "8.2"], "8.4"),
    };
    let version = requested.as_deref().unwrap_or(default);
    if allowed.contains(&version) {
        Ok(version.to_string())
    } else {
        Err(format!(
            "Versão não suportada: {version}. Use: {}.",
            allowed.join(", ")
        ))
    }
}

const SERVICE_ORDER: [Service; 7] = [
    Service::Postgres,
    Service::Mysql,
    Service::Mongodb,
    Service::Redis,
    Service::Rabbitmq,
    Service::Adminer,
    Service::Mailhog,
];

/// `(name, container port, preferred host port)` for every port a service publishes.
fn service_ports(service: Service) -> &'static [(&'static str, u16, u16)] {
    match service {
        Service::Postgres => &[("postgres", 5432, 5432)],
        Service::Mysql => &[("mysql", 3306, 3306)],
        Service::Mongodb => &[("mongodb", 27017, 27017)],
        Service::Redis => &[("redis", 6379, 6379)],
        Service::Rabbitmq => &[
            ("rabbitmq", 5672, 5672),
            ("rabbitmq-management", 15672, 15672),
        ],
        Service::Adminer => &[("adminer", 8080, 8088)],
        Service::Mailhog => &[("mailhog", 8025, 8025)],
    }
}

fn display_name(service: Service) -> &'static str {
    match service {
        Service::Postgres => "o PostgreSQL",
        Service::Mysql => "o MySQL",
        Service::Mongodb => "o MongoDB",
        Service::Redis => "o Redis",
        Service::Rabbitmq => "o RabbitMQ",
        Service::Adminer => "o Adminer",
        Service::Mailhog => "o MailHog",
    }
}

fn name_of(service: Service) -> &'static str {
    match service {
        Service::Postgres => "postgres",
        Service::Mysql => "mysql",
        Service::Mongodb => "mongodb",
        Service::Redis => "redis",
        Service::Rabbitmq => "rabbitmq",
        Service::Adminer => "adminer",
        Service::Mailhog => "mailhog",
    }
}

// --------------------------------------------------------------- generation

struct Resolved {
    stack: Stack,
    port: u16,
    version: String,
    package_manager: PackageManager,
    start_script: String,
    framework: PythonFramework,
    module: String,
    services: Vec<Service>,
    python_uses_pyproject: bool,
    new_project: bool,
    project_name: String,
    /// The chosen npm script runs the Vite dev server, which must bind 0.0.0.0.
    vite: bool,
    seed: Seed,
    ports: Vec<PortAssignment>,
    /// `.gitignore` already keeps `.env.docker` out of version control.
    gitignore_covers_env: bool,
}

fn resolve(request: &GenerateRequest, root: &Path) -> Result<Resolved, String> {
    if request.port == 0 {
        return Err("Porta inválida.".into());
    }
    let mut version = runtime_version(request.stack, &request.runtime_version)?;
    if request.stack == Stack::Laravel {
        if let Some(min) = laravel_min_php_minor(root) {
            let chosen: u32 = version
                .split('.')
                .nth(1)
                .and_then(|m| m.parse().ok())
                .unwrap_or(0);
            let required = format!("8.{min}");
            if chosen < min {
                if request.runtime_version.is_none()
                    && runtime_version(Stack::Laravel, &Some(required.clone())).is_ok()
                {
                    version = required;
                } else {
                    return Err(format!("As dependências do projeto exigem PHP {required} ou superior. Escolha uma versão mais nova."));
                }
            }
        }
    }
    let services: Vec<Service> = SERVICE_ORDER
        .iter()
        .copied()
        .filter(|s| request.services.contains(s))
        .collect();
    let mut wanted: Vec<Wanted> = vec![("app".to_string(), request.port, request.port)];
    for service in &services {
        for (name, container, preferred) in service_ports(*service) {
            wanted.push((name.to_string(), *container, *preferred));
        }
    }
    let assigned_ports = match &request.ports {
        Some(given) => ports::validate_given(root, &wanted, given)?,
        None => ports::allocate(root, &wanted)?,
    };
    let seed = match &request.seed {
        Some(hex) => secrets::seed_from_hex(hex)?,
        None => secrets::new_seed()?,
    };
    let start_script = match &request.start_script {
        Some(script) if !valid_script(script) => {
            return Err("Nome de script inválido.".into());
        }
        Some(script) => script.clone(),
        None => "start".into(),
    };
    let framework = request.python_framework.unwrap_or(PythonFramework::Fastapi);
    if request.new_project {
        match (request.stack, framework) {
            (Stack::Laravel, _) => {
                return Err(
                    "Projeto novo em Laravel ainda não está disponível. Use composer create-project e depois o assistente.".into(),
                )
            }
            (Stack::Python, PythonFramework::Django) => {
                return Err(
                    "Projeto novo em Django ainda não está disponível. Use django-admin startproject e depois o assistente.".into(),
                )
            }
            _ => {}
        }
        if !folder_is_empty(&request.path)? {
            return Err("A pasta não está vazia. Projeto novo só é criado em pasta vazia.".into());
        }
    }
    let module = match (&request.python_module, framework) {
        _ if request.new_project => match framework {
            PythonFramework::Fastapi => "main:app".to_string(),
            _ => "app".to_string(),
        },
        (Some(module), _) if !valid_module(module) => {
            return Err("Módulo Python inválido (use formato modulo:objeto).".into());
        }
        (Some(module), _) => module.clone(),
        (None, PythonFramework::Fastapi) => "main:app".into(),
        (None, _) => "app".into(),
    };
    let mut python_uses_pyproject = false;
    match request.stack {
        _ if request.new_project => {}
        Stack::Nodejs if !regular_file(root, "package.json") => {
            return Err("package.json não encontrado na raiz do projeto.".into());
        }
        Stack::SpringBoot if !regular_file(root, "pom.xml") => {
            return Err("pom.xml não encontrado na raiz. Gradle ainda não é suportado.".into());
        }
        Stack::Python => {
            if regular_file(root, "requirements.txt") {
            } else if regular_file(root, "pyproject.toml") {
                python_uses_pyproject = true;
            } else {
                return Err("requirements.txt ou pyproject.toml não encontrado na raiz.".into());
            }
        }
        Stack::Laravel if !regular_file(root, "composer.json") => {
            return Err("composer.json não encontrado na raiz do projeto.".into());
        }
        _ => {}
    }
    let start_script_value = start_script.clone();
    Ok(Resolved {
        stack: request.stack,
        port: request.port,
        version,
        package_manager: request.package_manager.unwrap_or(PackageManager::Npm),
        start_script,
        framework,
        module,
        services,
        python_uses_pyproject,
        new_project: request.new_project,
        seed,
        ports: assigned_ports,
        gitignore_covers_env: gitignore_covers_env(root),
        vite: request.stack == Stack::Nodejs
            && !request.new_project
            && script_runs_vite(root, &start_script_value),
        project_name: match &request.project_name {
            Some(name) if !crate::scaffold::valid_project_name(name) => {
                return Err(
                    "Nome do projeto inválido (use letras minúsculas, números e hífen).".into(),
                );
            }
            Some(name) => name.clone(),
            None => project_name(root),
        },
    })
}

/// Lowest PHP 8.x minor mentioned by a composer constraint (ignores upper bounds).
fn lowest_php_minor(constraint: &str) -> Option<u32> {
    constraint
        .split(|c: char| c == '|' || c == ',' || c.is_whitespace())
        .filter(|token| !token.starts_with('<'))
        .filter_map(|token| {
            let token = token.trim_start_matches(|c| "^~>=v".contains(c));
            token
                .strip_prefix("8.")?
                .split(['.', '*', '-'])
                .next()?
                .parse()
                .ok()
        })
        .min()
}

/// Highest minimum PHP version required by the locked dependencies.
fn laravel_min_php_minor(root: &Path) -> Option<u32> {
    if !regular_file(root, "composer.lock") {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(root.join("composer.lock"))
        .ok()?
        .take(8 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .ok()?;
    let lock: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let mut min = 0;
    for key in ["packages", "packages-dev"] {
        for package in lock.get(key)?.as_array()? {
            if let Some(minor) = package
                .pointer("/require/php")
                .and_then(|c| c.as_str())
                .and_then(lowest_php_minor)
            {
                min = min.max(minor);
            }
        }
    }
    (min > 0).then_some(min)
}

/// Whether `.gitignore` already ignores `.env.docker` (reads that file only).
fn gitignore_covers_env(root: &Path) -> bool {
    read_manifest(root, ".gitignore")
        .map(|text| {
            text.lines().map(str::trim).any(|line| {
                matches!(
                    line,
                    ".env.docker" | "/.env.docker" | ".env.*" | ".env*" | "*.env.docker"
                )
            })
        })
        .unwrap_or(false)
}

fn script_runs_vite(root: &Path, script: &str) -> bool {
    read_manifest(root, "package.json")
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|v| v.get("scripts")?.get(script)?.as_str().map(str::to_owned))
        .map(|command| command.trim_start().starts_with("vite"))
        .unwrap_or(false)
}

/// Folder name reduced to a safe lowercase identifier for manifests.
fn project_name(root: &Path) -> String {
    let raw = root
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut name: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    name = name.trim_matches('-').to_string();
    while name.contains("--") {
        name = name.replace("--", "-");
    }
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        name = format!("app-{name}").trim_end_matches('-').to_string();
    }
    name.chars()
        .take(40)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

fn starter_files(r: &Resolved) -> Vec<(&'static str, String, &'static str)> {
    let name = &r.project_name;
    let port = r.port;
    let host = host_port(r, "app");
    let mut files: Vec<(&'static str, String, &'static str)> = vec![];
    match r.stack {
        Stack::Nodejs => {
            files.push((
                "package.json",
                format!(
                    "{{\n  \"name\": \"{name}\",\n  \"version\": \"1.0.0\",\n  \"private\": true,\n  \"scripts\": {{\n    \"start\": \"node index.js\",\n    \"dev\": \"node --watch index.js\"\n  }},\n  \"dependencies\": {{\n    \"express\": \"^4.21.2\"\n  }}\n}}\n"
                ),
                "Manifesto do projeto Node.js com Express.",
            ));
            files.push((
                "index.js",
                format!(
                    "const express = require(\"express\");\n\nconst app = express();\nconst port = process.env.PORT || {port};\n\napp.get(\"/\", (_req, res) => {{\n  res.json({{ message: \"{name} está no ar\" }});\n}});\n\napp.get(\"/health\", (_req, res) => {{\n  res.json({{ status: \"ok\" }});\n}});\n\napp.listen(port, process.env.HOST || \"0.0.0.0\", () => {{\n  console.log(`Servidor em http://localhost:${{port}}`);\n}});\n"
                ),
                "Servidor HTTP de exemplo.",
            ));
        }
        Stack::SpringBoot => {
            let java = &r.version;
            files.push((
                "pom.xml",
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\"\n         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n         xsi:schemaLocation=\"http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd\">\n  <modelVersion>4.0.0</modelVersion>\n  <parent>\n    <groupId>org.springframework.boot</groupId>\n    <artifactId>spring-boot-starter-parent</artifactId>\n    <version>3.3.5</version>\n    <relativePath/>\n  </parent>\n  <groupId>com.example</groupId>\n  <artifactId>{name}</artifactId>\n  <version>0.0.1</version>\n  <name>{name}</name>\n  <properties>\n    <java.version>{java}</java.version>\n  </properties>\n  <dependencies>\n    <dependency>\n      <groupId>org.springframework.boot</groupId>\n      <artifactId>spring-boot-starter-web</artifactId>\n    </dependency>\n  </dependencies>\n  <build>\n    <plugins>\n      <plugin>\n        <groupId>org.springframework.boot</groupId>\n        <artifactId>spring-boot-maven-plugin</artifactId>\n      </plugin>\n    </plugins>\n  </build>\n</project>\n"
                ),
                "Projeto Maven com Spring Boot.",
            ));
            files.push((
                "src/main/java/com/example/app/Application.java",
                "package com.example.app;\n\nimport org.springframework.boot.SpringApplication;\nimport org.springframework.boot.autoconfigure.SpringBootApplication;\n\n@SpringBootApplication\npublic class Application {\n    public static void main(String[] args) {\n        SpringApplication.run(Application.class, args);\n    }\n}\n".to_string(),
                "Classe principal.",
            ));
            files.push((
                "src/main/java/com/example/app/HelloController.java",
                format!(
                    "package com.example.app;\n\nimport java.util.Map;\nimport org.springframework.web.bind.annotation.GetMapping;\nimport org.springframework.web.bind.annotation.RestController;\n\n@RestController\npublic class HelloController {{\n    @GetMapping(\"/\")\n    public Map<String, String> hello() {{\n        return Map.of(\"message\", \"{name} está no ar\");\n    }}\n\n    @GetMapping(\"/health\")\n    public Map<String, String> health() {{\n        return Map.of(\"status\", \"ok\");\n    }}\n}}\n"
                ),
                "Endpoints de exemplo.",
            ));
        }
        Stack::Python => {
            if r.framework == PythonFramework::Flask {
                files.push((
                    "requirements.txt",
                    "flask==3.0.3\n".to_string(),
                    "Dependências Python.",
                ));
                files.push((
                    "app.py",
                    format!(
                        "from flask import Flask, jsonify\n\napp = Flask(__name__)\n\n\n@app.get(\"/\")\ndef root():\n    return jsonify(message=\"{name} está no ar\")\n\n\n@app.get(\"/health\")\ndef health():\n    return jsonify(status=\"ok\")\n"
                    ),
                    "Aplicação Flask de exemplo.",
                ));
            } else {
                files.push((
                    "requirements.txt",
                    "fastapi==0.115.5\nuvicorn[standard]==0.32.1\n".to_string(),
                    "Dependências Python.",
                ));
                files.push((
                    "main.py",
                    format!(
                        "from fastapi import FastAPI\n\napp = FastAPI(title=\"{name}\")\n\n\n@app.get(\"/\")\ndef root():\n    return {{\"message\": \"{name} está no ar\"}}\n\n\n@app.get(\"/health\")\ndef health():\n    return {{\"status\": \"ok\"}}\n"
                    ),
                    "Aplicação FastAPI de exemplo.",
                ));
            }
        }
        Stack::Laravel => {}
    }
    let ignore = match r.stack {
        Stack::Nodejs => "node_modules/\n.env\n.env.docker\n",
        Stack::SpringBoot => "target/\n.idea/\n*.iml\n.env\n.env.docker\n",
        Stack::Python => "__pycache__/\n*.pyc\n.venv/\n.env\n.env.docker\n",
        Stack::Laravel => ".env\n.env.docker\n",
    };
    files.push((
        ".gitignore",
        ignore.to_string(),
        "Arquivos que o Git deve ignorar.",
    ));
    files.push((
        "README.md",
        format!(
            "# {name}\n\nProjeto criado com o AutoDock.\n\n## Rodar com Docker\n\n```bash\ndocker compose up --build\n```\n\nDepois acesse http://localhost:{host}.\n"
        ),
        "Instruções para rodar o projeto.",
    ));
    files
}

fn json_array(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|i| format!("\"{i}\"")).collect();
    format!("[{}]", quoted.join(", "))
}

fn dockerfile(r: &Resolved) -> String {
    let port = r.port;
    let v = &r.version;
    match r.stack {
        Stack::Nodejs => {
            let (setup, copy, install, run) = match r.package_manager {
                PackageManager::Npm => (
                    "",
                    "COPY package.json package-lock.json* ./",
                    "RUN if [ -f package-lock.json ]; then npm ci; else npm install; fi",
                    "npm",
                ),
                PackageManager::Pnpm => (
                    "RUN corepack enable\n",
                    "COPY package.json pnpm-lock.yaml* ./",
                    "RUN if [ -f pnpm-lock.yaml ]; then pnpm install --frozen-lockfile; else pnpm install; fi",
                    "pnpm",
                ),
                PackageManager::Yarn => (
                    "RUN corepack enable\n",
                    "COPY package.json yarn.lock* ./",
                    "RUN yarn install",
                    "yarn",
                ),
            };
            let mut command = vec![run.to_string(), "run".into(), r.start_script.clone()];
            if r.vite {
                if r.package_manager == PackageManager::Npm {
                    command.push("--".into());
                }
                command.extend([
                    "--host".into(),
                    "0.0.0.0".into(),
                    "--port".into(),
                    port.to_string(),
                ]);
            }
            let cmd = json_array(&command);
            format!(
                "# Gerado pelo AutoDock. Ambiente de desenvolvimento.\n\
FROM node:{v}-alpine\n\
{setup}WORKDIR /app\n\
{copy}\n\
{install}\n\
COPY . .\n\
EXPOSE {port}\n\
CMD {cmd}\n"
            )
        }
        Stack::SpringBoot => format!(
            "# Gerado pelo AutoDock. Build em duas etapas (Maven).\n\
FROM maven:3.9-eclipse-temurin-{v} AS build\n\
WORKDIR /workspace\n\
COPY pom.xml ./\n\
RUN mvn -B -q dependency:go-offline\n\
COPY src ./src\n\
RUN mvn -B -q -DskipTests package\n\
\n\
FROM eclipse-temurin:{v}-jre\n\
RUN useradd --system --create-home app\n\
WORKDIR /app\n\
COPY --from=build /workspace/target/*.jar app.jar\n\
USER app\n\
EXPOSE {port}\n\
ENTRYPOINT [\"java\", \"-jar\", \"/app/app.jar\"]\n"
        ),
        Stack::Python => {
            let (copy, install) = if r.python_uses_pyproject {
                ("COPY . .", "RUN pip install --no-cache-dir .")
            } else {
                (
                    "COPY requirements.txt ./\nRUN pip install --no-cache-dir -r requirements.txt\nCOPY . .",
                    "",
                )
            };
            let cmd = match r.framework {
                PythonFramework::Fastapi => json_array(&[
                    "uvicorn".into(),
                    r.module.clone(),
                    "--host".into(),
                    "0.0.0.0".into(),
                    "--port".into(),
                    port.to_string(),
                ]),
                PythonFramework::Django => json_array(&[
                    "python".into(),
                    "manage.py".into(),
                    "runserver".into(),
                    format!("0.0.0.0:{port}"),
                ]),
                PythonFramework::Flask => json_array(&[
                    "flask".into(),
                    "--app".into(),
                    r.module.clone(),
                    "run".into(),
                    "--host".into(),
                    "0.0.0.0".into(),
                    "--port".into(),
                    port.to_string(),
                ]),
            };
            let install_block = if install.is_empty() {
                String::new()
            } else {
                format!("{install}\n")
            };
            format!(
                "# Gerado pelo AutoDock. Ambiente de desenvolvimento.\n\
FROM python:{v}-slim\n\
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1\n\
WORKDIR /app\n\
{copy}\n\
{install_block}\
RUN useradd --system --create-home app && chown -R app /app\n\
USER app\n\
EXPOSE {port}\n\
CMD {cmd}\n"
            )
        }
        Stack::Laravel => {
            let mut packages = vec!["git", "unzip", "libzip-dev"];
            let mut extensions = vec!["zip"];
            if r.services.contains(&Service::Postgres) {
                packages.push("libpq-dev");
                extensions.push("pdo_pgsql");
            }
            if r.services.contains(&Service::Mysql) {
                extensions.push("pdo_mysql");
            }
            let redis = if r.services.contains(&Service::Redis) {
                "RUN pecl install redis && docker-php-ext-enable redis\n"
            } else {
                ""
            };
            let packages = packages.join(" ");
            let extensions = extensions.join(" ");
            format!(
                "# Gerado pelo AutoDock. Ambiente de desenvolvimento.\n\
FROM php:{v}-cli\n\
RUN apt-get update \\\n    && apt-get install -y --no-install-recommends {packages} \\\n    && docker-php-ext-install {extensions} \\\n    && rm -rf /var/lib/apt/lists/*\n\
{redis}\
COPY --from=composer:2 /usr/bin/composer /usr/bin/composer\n\
WORKDIR /var/www/html\n\
COPY composer.json composer.lock* ./\n\
RUN composer install --no-interaction --no-scripts --prefer-dist\n\
COPY . .\n\
RUN composer dump-autoload --optimize\n\
EXPOSE {port}\n\
CMD [\"php\", \"artisan\", \"serve\", \"--host=0.0.0.0\", \"--port={port}\"]\n"
            )
        }
    }
}

fn dockerignore(stack: Stack) -> String {
    let mut lines = vec![
        ".git",
        ".idea",
        ".vscode",
        "*.log",
        ".env",
        ".env.*",
        "!.env.example",
        "Dockerfile",
        "docker-compose*.yml",
        "compose*.y*ml",
    ];
    lines.extend(match stack {
        Stack::Nodejs => vec!["node_modules", "dist", "coverage"],
        Stack::SpringBoot => vec!["target", "build", ".gradle"],
        Stack::Python => vec!["__pycache__", "*.pyc", ".venv", ".pytest_cache"],
        Stack::Laravel => vec!["vendor", "node_modules", "storage/logs"],
    });
    format!("# Gerado pelo AutoDock.\n{}\n", lines.join("\n"))
}

fn host_port(r: &Resolved, name: &str) -> u16 {
    r.ports
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.host_port)
        .unwrap_or(0)
}

/// Services whose containers need a password.
fn has_secrets(services: &[Service]) -> bool {
    services.iter().any(|s| {
        matches!(
            s,
            Service::Postgres | Service::Mysql | Service::Mongodb | Service::Rabbitmq
        )
    })
}

type Passwords = BTreeMap<&'static str, String>;

/// Strong generated passwords, one per credential, derived from the seed.
fn passwords(r: &Resolved) -> Passwords {
    let mut map = BTreeMap::new();
    for service in &r.services {
        match service {
            Service::Postgres => {
                map.insert("postgres", secrets::password(&r.seed, "postgres"));
            }
            Service::Mysql => {
                map.insert("mysql", secrets::password(&r.seed, "mysql"));
                map.insert("mysql-root", secrets::password(&r.seed, "mysql-root"));
            }
            Service::Mongodb => {
                map.insert("mongodb", secrets::password(&r.seed, "mongodb"));
            }
            Service::Rabbitmq => {
                map.insert("rabbitmq", secrets::password(&r.seed, "rabbitmq"));
            }
            _ => {}
        }
    }
    map
}

fn placeholder_passwords(passwords: &Passwords) -> Passwords {
    passwords
        .keys()
        .map(|key| (*key, "change-me".to_string()))
        .collect()
}

/// Variables the service containers read (names fixed by the official images).
fn service_env_vars(r: &Resolved, pw: &Passwords) -> Vec<(String, String)> {
    let mut vars = vec![];
    for service in &r.services {
        match service {
            Service::Postgres => vars.push(("POSTGRES_PASSWORD".into(), pw["postgres"].clone())),
            Service::Mysql => {
                vars.push(("MYSQL_PASSWORD".into(), pw["mysql"].clone()));
                vars.push(("MYSQL_ROOT_PASSWORD".into(), pw["mysql-root"].clone()));
            }
            Service::Mongodb => {
                vars.push(("MONGO_INITDB_ROOT_PASSWORD".into(), pw["mongodb"].clone()))
            }
            Service::Rabbitmq => {
                vars.push(("RABBITMQ_DEFAULT_PASS".into(), pw["rabbitmq"].clone()))
            }
            _ => {}
        }
    }
    vars
}

/// Returns `(plain, secret)` variables for the application container. Secret ones
/// embed a password, so they go to `.env.docker` and never into the compose file.
#[allow(clippy::type_complexity)]
fn app_environment(r: &Resolved, pw: &Passwords) -> (Vec<(String, String)>, Vec<(String, String)>) {
    fn push(list: &mut Vec<(String, String)>, key: &str, value: &str) {
        list.push((key.into(), value.into()));
    }
    let mut plain: Vec<(String, String)> = vec![("PORT".into(), r.port.to_string())];
    let mut secret: Vec<(String, String)> = vec![];
    match r.stack {
        Stack::Nodejs => push(&mut plain, "HOST", "0.0.0.0"),
        Stack::SpringBoot => push(&mut plain, "SERVER_PORT", &r.port.to_string()),
        _ => {}
    }
    let has = |s: Service| r.services.contains(&s);
    // The first SQL service is the application's primary database.
    let primary = [Service::Postgres, Service::Mysql]
        .into_iter()
        .find(|s| has(*s));
    for sql in [Service::Postgres, Service::Mysql] {
        if !has(sql) {
            continue;
        }
        let (scheme, host, db_port, password) = match sql {
            Service::Postgres => ("postgresql", "postgres", 5432, pw["postgres"].as_str()),
            _ => ("mysql", "mysql", 3306, pw["mysql"].as_str()),
        };
        let url = format!("{scheme}://app:{password}@{host}:{db_port}/app");
        if Some(sql) != primary {
            push(
                &mut secret,
                &format!("{}_URL", host.to_ascii_uppercase()),
                &url,
            );
            continue;
        }
        push(&mut secret, "DATABASE_URL", &url);
        match r.stack {
            Stack::SpringBoot => {
                push(
                    &mut plain,
                    "SPRING_DATASOURCE_URL",
                    &format!("jdbc:{scheme}://{host}:{db_port}/app"),
                );
                push(&mut plain, "SPRING_DATASOURCE_USERNAME", "app");
                push(&mut secret, "SPRING_DATASOURCE_PASSWORD", password);
            }
            Stack::Laravel => {
                let connection = if sql == Service::Postgres {
                    "pgsql"
                } else {
                    "mysql"
                };
                push(&mut plain, "DB_CONNECTION", connection);
                push(&mut plain, "DB_HOST", host);
                push(&mut plain, "DB_PORT", &db_port.to_string());
                push(&mut plain, "DB_DATABASE", "app");
                push(&mut plain, "DB_USERNAME", "app");
                push(&mut secret, "DB_PASSWORD", password);
            }
            _ => {}
        }
    }
    if has(Service::Mongodb) {
        push(
            &mut secret,
            "MONGODB_URI",
            &format!(
                "mongodb://app:{}@mongodb:27017/app?authSource=admin",
                pw["mongodb"]
            ),
        );
    }
    if has(Service::Redis) {
        push(&mut plain, "REDIS_URL", "redis://redis:6379");
        match r.stack {
            Stack::SpringBoot => push(&mut plain, "SPRING_DATA_REDIS_HOST", "redis"),
            Stack::Laravel => push(&mut plain, "REDIS_HOST", "redis"),
            _ => {}
        }
    }
    if has(Service::Rabbitmq) {
        push(
            &mut secret,
            "AMQP_URL",
            &format!("amqp://app:{}@rabbitmq:5672", pw["rabbitmq"]),
        );
        if r.stack == Stack::SpringBoot {
            push(&mut plain, "SPRING_RABBITMQ_HOST", "rabbitmq");
            push(&mut plain, "SPRING_RABBITMQ_USERNAME", "app");
            push(&mut secret, "SPRING_RABBITMQ_PASSWORD", &pw["rabbitmq"]);
        }
    }
    if has(Service::Mailhog) {
        push(&mut plain, "SMTP_HOST", "mailhog");
        push(&mut plain, "SMTP_PORT", "1025");
        match r.stack {
            Stack::Laravel => {
                push(&mut plain, "MAIL_HOST", "mailhog");
                push(&mut plain, "MAIL_PORT", "1025");
            }
            Stack::SpringBoot => {
                push(&mut plain, "SPRING_MAIL_HOST", "mailhog");
                push(&mut plain, "SPRING_MAIL_PORT", "1025");
            }
            _ => {}
        }
    }
    (plain, secret)
}

/// Content of `.env.docker` (real passwords) or of the example file (placeholders).
fn env_content(r: &Resolved, pw: &Passwords, example: bool) -> String {
    let mut out = if example {
        String::from(
            "# Gerado pelo AutoDock. Copie para .env.docker e troque cada \"change-me\" por uma senha forte.\n# Este arquivo é só um modelo, sem segredos, e pode ser versionado.\n",
        )
    } else {
        String::from(
            "# Gerado pelo AutoDock. Contém senhas fortes: NÃO versione este arquivo.\n# Lido pelo docker-compose.yml (env_file).\n",
        )
    };
    let (_, secret) = app_environment(r, pw);
    for (key, value) in service_env_vars(r, pw).into_iter().chain(secret) {
        out.push_str(&format!("{key}={value}\n"));
    }
    out
}

fn service_block(r: &Resolved, service: Service) -> String {
    let hp = |name: &str| host_port(r, name);
    match service {
        Service::Postgres => format!(
            "  postgres:\n    image: postgres:16-alpine\n    env_file:\n      - {ENV_FILE}\n    environment:\n      POSTGRES_USER: app\n      POSTGRES_DB: app\n    ports:\n      - \"127.0.0.1:{}:5432\"\n    volumes:\n      - postgres_data:/var/lib/postgresql/data\n    healthcheck:\n      test: [\"CMD-SHELL\", \"pg_isready -U app -d app\"]\n      interval: 5s\n      timeout: 5s\n      retries: 10\n",
            hp("postgres")
        ),
        Service::Mysql => format!(
            "  mysql:\n    image: mysql:8.4\n    env_file:\n      - {ENV_FILE}\n    environment:\n      MYSQL_DATABASE: app\n      MYSQL_USER: app\n    ports:\n      - \"127.0.0.1:{}:3306\"\n    volumes:\n      - mysql_data:/var/lib/mysql\n    healthcheck:\n      test: [\"CMD-SHELL\", \"mysqladmin ping -h 127.0.0.1 -uroot -p\\\"$$MYSQL_ROOT_PASSWORD\\\"\"]\n      interval: 5s\n      timeout: 5s\n      retries: 20\n",
            hp("mysql")
        ),
        Service::Mongodb => format!(
            "  mongodb:\n    image: mongo:7\n    env_file:\n      - {ENV_FILE}\n    environment:\n      MONGO_INITDB_ROOT_USERNAME: app\n    ports:\n      - \"127.0.0.1:{}:27017\"\n    volumes:\n      - mongodb_data:/data/db\n    healthcheck:\n      test: [\"CMD\", \"mongosh\", \"--quiet\", \"--eval\", \"db.adminCommand('ping')\"]\n      interval: 5s\n      timeout: 5s\n      retries: 10\n",
            hp("mongodb")
        ),
        Service::Redis => format!(
            "  redis:\n    image: redis:7-alpine\n    ports:\n      - \"127.0.0.1:{}:6379\"\n    volumes:\n      - redis_data:/data\n    healthcheck:\n      test: [\"CMD\", \"redis-cli\", \"ping\"]\n      interval: 5s\n      timeout: 3s\n      retries: 10\n",
            hp("redis")
        ),
        Service::Rabbitmq => format!(
            "  rabbitmq:\n    image: rabbitmq:3-management-alpine\n    env_file:\n      - {ENV_FILE}\n    environment:\n      RABBITMQ_DEFAULT_USER: app\n    ports:\n      - \"127.0.0.1:{}:5672\"\n      - \"127.0.0.1:{}:15672\"\n    volumes:\n      - rabbitmq_data:/var/lib/rabbitmq\n    healthcheck:\n      test: [\"CMD\", \"rabbitmq-diagnostics\", \"-q\", \"ping\"]\n      interval: 10s\n      timeout: 10s\n      retries: 10\n",
            hp("rabbitmq"),
            hp("rabbitmq-management")
        ),
        Service::Adminer => format!(
            "  adminer:\n    image: adminer:4\n    ports:\n      - \"127.0.0.1:{}:8080\"\n",
            hp("adminer")
        ),
        Service::Mailhog => format!(
            "  mailhog:\n    image: mailhog/mailhog:v1.0.1\n    ports:\n      - \"127.0.0.1:{}:8025\"\n",
            hp("mailhog")
        ),
    }
}

fn compose(r: &Resolved) -> String {
    let pw = passwords(r);
    let (plain, _) = app_environment(r, &pw);
    let mut out = format!(
        "# Gerado pelo AutoDock. Revise antes de usar.\n# Nenhuma senha fica neste arquivo: elas estão em {ENV_FILE} (não versione).\nservices:\n  app:\n    build: .\n"
    );
    out.push_str(&format!(
        "    ports:\n      - \"{}:{}\"\n",
        host_port(r, "app"),
        r.port
    ));
    if has_secrets(&r.services) {
        out.push_str(&format!("    env_file:\n      - {ENV_FILE}\n"));
    }
    out.push_str("    environment:\n");
    for (key, value) in plain {
        out.push_str(&format!("      {key}: \"{value}\"\n"));
    }
    match r.stack {
        Stack::Nodejs => out.push_str("    volumes:\n      - .:/app\n      - /app/node_modules\n"),
        Stack::Python => out.push_str("    volumes:\n      - .:/app\n"),
        Stack::Laravel => {
            out.push_str("    volumes:\n      - .:/var/www/html\n      - /var/www/html/vendor\n")
        }
        Stack::SpringBoot => {}
    }
    let waits: Vec<Service> = r
        .services
        .iter()
        .copied()
        .filter(|s| !matches!(s, Service::Adminer | Service::Mailhog))
        .collect();
    if !waits.is_empty() || r.services.contains(&Service::Mailhog) {
        out.push_str("    depends_on:\n");
        for service in &r.services {
            match service {
                Service::Adminer => {}
                Service::Mailhog => {
                    out.push_str("      mailhog:\n        condition: service_started\n")
                }
                other => out.push_str(&format!(
                    "      {}:\n        condition: service_healthy\n",
                    name_of(*other)
                )),
            }
        }
    }
    let mut volumes: Vec<&str> = vec![];
    for service in &r.services {
        out.push('\n');
        out.push_str(&service_block(r, *service));
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

fn notes(r: &Resolved) -> Vec<String> {
    let mut notes =
        vec!["Nada foi gravado ainda. Revise os arquivos antes de confirmar.".to_string()];
    if r.new_project {
        notes.push("Um projeto inicial mínimo será criado nesta pasta vazia, já pronto para rodar no Docker.".to_string());
    }
    for assigned in &r.ports {
        if assigned.host_port != assigned.preferred_port {
            let who = if assigned.name == "app" {
                "a aplicação".to_string()
            } else {
                SERVICE_ORDER
                    .iter()
                    .find(|s| assigned.name.starts_with(name_of(**s)))
                    .map(|s| display_name(*s).to_string())
                    .unwrap_or_else(|| assigned.name.clone())
            };
            notes.push(format!(
                "A porta {} está em uso ou reservada por outro projeto: {who} usará a {} no seu computador (dentro do Docker continua {}).",
                assigned.preferred_port, assigned.host_port, assigned.container_port
            ));
        }
    }
    notes.push(format!(
        "Depois de subir, acesse a aplicação em http://localhost:{}.",
        host_port(r, "app")
    ));
    notes.push("Os serviços auxiliares publicam portas apenas em 127.0.0.1.".to_string());
    match r.stack {
        Stack::Nodejs => notes.push(format!(
            "A aplicação precisa escutar em 0.0.0.0 e na porta da variável PORT ({}), senão não ficará acessível fora do container.",
            r.port
        )),
        Stack::SpringBoot => notes.push(
            "O build usa Maven e espera um único JAR executável em target/. Projetos multimódulo podem exigir ajuste.".into(),
        ),
        Stack::Python => {
            if r.framework == PythonFramework::Fastapi {
                notes.push("FastAPI usa uvicorn: confirme que ele está nas dependências.".into());
            }
        }
        Stack::Laravel => notes.push(
            "Crie o .env do Laravel (copie .env.example) e rode php artisan key:generate dentro do container.".into(),
        ),
    }
    if r.services.contains(&Service::Adminer)
        && !r.services.contains(&Service::Postgres)
        && !r.services.contains(&Service::Mysql)
    {
        notes.push("Adminer foi incluído sem banco SQL no plano.".into());
    }
    if r.services.contains(&Service::Postgres) && r.services.contains(&Service::Mysql) {
        notes.push(
            "Com Postgres e MySQL, DATABASE_URL aponta para o Postgres; o MySQL fica em MYSQL_URL."
                .into(),
        );
    }
    if has_secrets(&r.services) {
        notes.push(format!(
            "Senhas fortes de 32 caracteres foram geradas agora e ficam em {ENV_FILE} (permissão restrita). Não versione esse arquivo."
        ));
        if !r.new_project && !r.gitignore_covers_env {
            notes.push(format!(
                "Adicione {ENV_FILE} ao seu .gitignore para não versionar as senhas."
            ));
        }
    }
    notes
}

/// Builds the plan. Pure with respect to the file system except for existence checks.
pub fn preview(request: &GenerateRequest) -> Result<GeneratePlan, String> {
    let root = root_of(&request.path)?;
    let resolved = resolve(request, &root)?;
    let compose_exists = COMPOSE_NAMES.iter().any(|name| present(&root, name));
    let file = |name: &str, description: &str, content: String, exists: bool, sensitive: bool| {
        GeneratedFile {
            name: name.into(),
            description: description.into(),
            content,
            sensitive,
            exists,
        }
    };
    let mut files: Vec<GeneratedFile> = vec![];
    if resolved.new_project {
        for (name, content, description) in starter_files(&resolved) {
            files.push(file(
                name,
                description,
                content,
                present(&root, name),
                false,
            ));
        }
    }
    files.push(file(
        DOCKERFILE,
        "Como a imagem da aplicação é construída.",
        dockerfile(&resolved),
        present(&root, DOCKERFILE),
        false,
    ));
    files.push(file(
        COMPOSE_FILE,
        "Orquestra a aplicação e os serviços escolhidos. Não contém senhas.",
        compose(&resolved),
        compose_exists,
        false,
    ));
    files.push(file(
        DOCKERIGNORE,
        "Evita copiar arquivos desnecessários ou sensíveis para a imagem.",
        dockerignore(resolved.stack),
        present(&root, DOCKERIGNORE),
        false,
    ));
    if has_secrets(&resolved.services) {
        let real = passwords(&resolved);
        files.push(file(
            ENV_FILE,
            "Senhas fortes de 32 caracteres geradas agora. Não versione este arquivo.",
            env_content(&resolved, &real, false),
            present(&root, ENV_FILE),
            true,
        ));
        files.push(file(
            ENV_EXAMPLE,
            "Modelo das variáveis, sem segredos, para quem clonar o projeto.",
            env_content(&resolved, &placeholder_passwords(&real), true),
            present(&root, ENV_EXAMPLE),
            false,
        ));
    }
    Ok(GeneratePlan {
        schema_version: 1,
        project: root.display().to_string(),
        files,
        notes: notes(&resolved),
        seed: secrets::seed_to_hex(&resolved.seed),
        ports: resolved.ports.clone(),
    })
}

fn create_new(path: &Path, content: &str, private: bool) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(if private { 0o600 } else { 0o644 })
            .custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()
}

/// Writes only the selected files of a freshly generated plan. Existing files are
/// skipped, never overwritten.
pub fn apply(request: &GenerateRequest, selected: &[String]) -> Result<ApplyReport, String> {
    if selected.is_empty() {
        return Err("Nenhum arquivo selecionado.".into());
    }
    let plan = preview(request)?;
    let root = root_of(&request.path)?;
    for name in selected {
        if !plan.files.iter().any(|f| &f.name == name) {
            return Err(format!("Arquivo fora do plano: {name}"));
        }
    }
    let mut report = ApplyReport {
        written: vec![],
        skipped: vec![],
    };
    for file in &plan.files {
        if !selected.contains(&file.name) {
            continue;
        }
        if file.exists {
            report.skipped.push(SkippedFile {
                name: file.name.clone(),
                reason: "Já existe; não foi sobrescrito.".into(),
            });
            continue;
        }
        if let Some(parent) = Path::new(&file.name).parent() {
            // File names come from the plan (fixed, relative, no "..").
            if let Err(e) = fs::create_dir_all(root.join(parent)) {
                report.skipped.push(SkippedFile {
                    name: file.name.clone(),
                    reason: format!("Falha ao criar pasta: {e}"),
                });
                continue;
            }
        }
        match create_new(&root.join(&file.name), &file.content, file.sensitive) {
            Ok(()) => report.written.push(file.name.clone()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                report.skipped.push(SkippedFile {
                    name: file.name.clone(),
                    reason: "Já existe; não foi sobrescrito.".into(),
                })
            }
            Err(e) => report.skipped.push(SkippedFile {
                name: file.name.clone(),
                reason: format!("Falha ao gravar: {e}"),
            }),
        }
    }
    if !report.written.is_empty() {
        // Best effort: reserving ports must never block writing the project files.
        let _ = ports::register(&root, &plan.ports);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, content) in files {
            fs::write(dir.path().join(name), content).unwrap();
        }
        dir
    }

    fn request(dir: &tempfile::TempDir, stack: Stack) -> GenerateRequest {
        GenerateRequest {
            path: dir.path().to_string_lossy().into(),
            stack,
            port: 3000,
            runtime_version: None,
            package_manager: None,
            start_script: None,
            python_framework: None,
            python_module: None,
            services: vec![],
            new_project: false,
            project_name: None,
            seed: None,
            ports: None,
        }
    }

    fn host(plan: &GeneratePlan, name: &str) -> u16 {
        plan.ports
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .host_port
    }

    fn file<'a>(plan: &'a GeneratePlan, name: &str) -> &'a GeneratedFile {
        plan.files.iter().find(|f| f.name == name).unwrap()
    }

    #[test]
    fn node_plan_has_expected_files_and_no_writes() {
        let dir = project(&[("package.json", r#"{"scripts":{"dev":"vite"}}"#)]);
        let mut req = request(&dir, Stack::Nodejs);
        req.start_script = Some("dev".into());
        req.services = vec![Service::Postgres, Service::Redis];
        let plan = preview(&req).unwrap();
        let dockerfile = &file(&plan, DOCKERFILE).content;
        assert!(dockerfile.contains("FROM node:22-alpine"));
        // vite binds localhost by default: the container needs 0.0.0.0 and the port
        assert!(dockerfile
            .contains(r#"CMD ["npm", "run", "dev", "--", "--host", "0.0.0.0", "--port", "3000"]"#));
        let compose = &file(&plan, COMPOSE_FILE).content;
        // host ports come from the allocation (3000 may be busy on this machine)
        assert!(compose.contains(&format!("\"{}:3000\"", host(&plan, "app"))));
        assert!(compose.contains("condition: service_healthy"));
        assert!(compose.contains(&format!("127.0.0.1:{}:5432", host(&plan, "postgres"))));
        // connection strings embed the password, so they live in .env.docker only
        assert!(!compose.contains("DATABASE_URL"));
        assert!(file(&plan, ENV_FILE)
            .content
            .contains("DATABASE_URL=postgresql://app:"));
        assert!(!compose.contains("version:"));
        assert!(plan.files.iter().any(|f| f.name == ENV_EXAMPLE));
        // preview must not create anything
        assert!(!dir.path().join(DOCKERFILE).exists());
    }

    #[test]
    fn vite_host_flags_depend_on_the_package_manager_and_only_apply_to_vite() {
        let dir = project(&[(
            "package.json",
            r#"{"scripts":{"dev":"vite","start":"node x.js"}}"#,
        )]);
        let cmd = |pm: PackageManager, script: &str| {
            let mut req = request(&dir, Stack::Nodejs);
            req.package_manager = Some(pm);
            req.start_script = Some(script.into());
            file(&preview(&req).unwrap(), DOCKERFILE).content.clone()
        };
        assert!(cmd(PackageManager::Pnpm, "dev")
            .contains(r#"CMD ["pnpm", "run", "dev", "--host", "0.0.0.0", "--port", "3000"]"#));
        assert!(cmd(PackageManager::Yarn, "dev")
            .contains(r#"CMD ["yarn", "run", "dev", "--host", "0.0.0.0", "--port", "3000"]"#));
        assert!(cmd(PackageManager::Npm, "start").contains(r#"CMD ["npm", "run", "start"]"#));
    }

    #[test]
    fn laravel_php_version_follows_the_locked_dependencies() {
        let lock = r#"{"packages":[{"require":{"php":"^8.2"}},{"require":{"php":">=8.4.1"}},
            {"require":{"php":"^7.4|^8.0"}},{"require":{"php":">=7.2 <8.9"}}],"packages-dev":[]}"#;
        let dir = project(&[("composer.json", "{}"), ("composer.lock", lock)]);
        // default is raised to the minimum the dependencies need
        let plan = preview(&request(&dir, Stack::Laravel)).unwrap();
        assert!(file(&plan, DOCKERFILE).content.contains("FROM php:8.4-cli"));
        // an explicit older version is refused with a clear message
        let mut req = request(&dir, Stack::Laravel);
        req.runtime_version = Some("8.3".into());
        let error = preview(&req).unwrap_err();
        assert!(error.contains("PHP 8.4"), "{error}");
        // without a lock file the default (8.4) applies
        let dir = project(&[("composer.json", "{}")]);
        let plan = preview(&request(&dir, Stack::Laravel)).unwrap();
        assert!(file(&plan, DOCKERFILE).content.contains("FROM php:8.4-cli"));
        assert_eq!(lowest_php_minor("^8.2|^8.3"), Some(2));
        assert_eq!(lowest_php_minor("<8.5"), None);
    }

    #[test]
    fn project_name_is_validated_and_used_in_starters() {
        let dir = project(&[]);
        let mut req = request(&dir, Stack::Nodejs);
        req.new_project = true;
        req.project_name = Some("loja-online".into());
        let plan = preview(&req).unwrap();
        assert!(file(&plan, "package.json")
            .content
            .contains("\"name\": \"loja-online\""));
        for bad in ["Loja", "a b", "x\"y", "$(id)", ""] {
            req.project_name = Some(bad.into());
            assert!(preview(&req).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn spring_laravel_and_python_templates() {
        let dir = project(&[
            ("pom.xml", "<project/>"),
            ("composer.json", "{}"),
            ("requirements.txt", "fastapi\n"),
        ]);
        let mut spring = request(&dir, Stack::SpringBoot);
        spring.port = 8081;
        spring.services = vec![Service::Postgres, Service::Rabbitmq];
        let plan = preview(&spring).unwrap();
        assert!(file(&plan, DOCKERFILE)
            .content
            .contains("maven:3.9-eclipse-temurin-21"));
        assert!(file(&plan, COMPOSE_FILE)
            .content
            .contains("SPRING_DATASOURCE_URL: \"jdbc:postgresql://postgres:5432/app\""));

        let mut laravel = request(&dir, Stack::Laravel);
        laravel.services = vec![Service::Mysql, Service::Redis];
        let plan = preview(&laravel).unwrap();
        let dockerfile = &file(&plan, DOCKERFILE).content;
        assert!(dockerfile.contains("pdo_mysql") && dockerfile.contains("pecl install redis"));
        assert!(file(&plan, COMPOSE_FILE)
            .content
            .contains("DB_CONNECTION: \"mysql\""));

        let mut python = request(&dir, Stack::Python);
        python.port = 8000;
        let plan = preview(&python).unwrap();
        assert!(file(&plan, DOCKERFILE)
            .content
            .contains(r#"CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]"#));
    }

    #[test]
    fn rejects_injection_and_invalid_input() {
        let dir = project(&[("package.json", "{}"), ("requirements.txt", "")]);
        for script in ["dev\"; rm -rf /", "-x", "a b", "", "x\nRUN evil"] {
            let mut req = request(&dir, Stack::Nodejs);
            req.start_script = Some(script.into());
            assert!(preview(&req).is_err(), "{script:?}");
        }
        for module in ["main:app\"]; RUN x", "a b", ":app", "a:b:c;", "../x"] {
            let mut req = request(&dir, Stack::Python);
            req.python_module = Some(module.into());
            assert!(preview(&req).is_err(), "{module:?}");
        }
        let mut req = request(&dir, Stack::Nodejs);
        req.runtime_version = Some("22\nRUN evil".into());
        assert!(preview(&req).is_err());
        let mut req = request(&dir, Stack::Nodejs);
        req.port = 0;
        assert!(preview(&req).is_err());
        // an app port that equals a service port no longer fails: the service moves
        let mut req = request(&dir, Stack::Nodejs);
        req.port = 5432;
        req.services = vec![Service::Postgres];
        let plan = preview(&req).unwrap();
        assert_ne!(host(&plan, "app"), host(&plan, "postgres"));
        // unknown request fields are rejected at the boundary
        assert!(serde_json::from_str::<GenerateRequest>(
            r#"{"path":".","stack":"nodejs","port":1,"content":"x"}"#
        )
        .is_err());
    }

    #[test]
    fn empty_folder_detection_ignores_only_harmless_entries() {
        let dir = project(&[(".DS_Store", "")]);
        fs::create_dir(dir.path().join(".git")).unwrap();
        assert!(folder_is_empty(&dir.path().to_string_lossy()).unwrap());
        fs::write(dir.path().join("a.txt"), "x").unwrap();
        assert!(!folder_is_empty(&dir.path().to_string_lossy()).unwrap());
    }

    #[test]
    fn new_project_creates_starter_and_docker_files_only_in_empty_folders() {
        for (stack, expected) in [
            (Stack::Nodejs, "index.js"),
            (
                Stack::SpringBoot,
                "src/main/java/com/example/app/Application.java",
            ),
            (Stack::Python, "main.py"),
        ] {
            let dir = project(&[]);
            let mut req = request(&dir, stack);
            req.new_project = true;
            req.port = 8080;
            req.services = vec![Service::Postgres];
            let plan = preview(&req).unwrap();
            assert!(plan.files.iter().any(|f| f.name == expected), "{stack:?}");
            assert!(plan.files.iter().any(|f| f.name == DOCKERFILE));
            let names: Vec<String> = plan.files.iter().map(|f| f.name.clone()).collect();
            let report = apply(&req, &names).unwrap();
            assert_eq!(
                report.written.len(),
                names.len(),
                "{stack:?} {:?}",
                report.skipped
            );
            assert!(dir.path().join(expected).is_file());
            // the folder is no longer empty: a second run is refused, nothing is overwritten
            assert!(preview(&req).is_err());
        }
        // non-empty folder, and unsupported scaffolds, are refused
        let dir = project(&[("keep.txt", "x")]);
        let mut req = request(&dir, Stack::Nodejs);
        req.new_project = true;
        assert!(preview(&req).is_err());
        let dir = project(&[]);
        let mut req = request(&dir, Stack::Laravel);
        req.new_project = true;
        assert!(preview(&req).is_err());
        let mut req = request(&dir, Stack::Python);
        req.new_project = true;
        req.python_framework = Some(PythonFramework::Django);
        assert!(preview(&req).is_err());
    }

    #[test]
    fn project_names_are_sanitized_for_manifests() {
        for (folder, expected) in [
            ("Meu App!", "meu-app"),
            ("123abc", "app-123abc"),
            // `"` is not a legal file name character on Windows, so use a portable separator.
            ("a.b", "a-b"),
            ("___", "app"),
        ] {
            let parent = tempfile::tempdir().unwrap();
            let path = parent.path().join(folder);
            fs::create_dir(&path).unwrap();
            assert_eq!(
                project_name(&path.canonicalize().unwrap()),
                expected,
                "{folder}"
            );
        }
    }

    fn all_services() -> Vec<Service> {
        vec![
            Service::Postgres,
            Service::Mysql,
            Service::Mongodb,
            Service::Redis,
            Service::Rabbitmq,
            Service::Adminer,
            Service::Mailhog,
        ]
    }

    /// Values of every `*PASSWORD*`/`*_PASS` variable in an env file.
    fn secret_values(env: &str) -> Vec<String> {
        env.lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once('='))
            .filter(|(k, _)| k.contains("PASSWORD") || k.ends_with("_PASS"))
            .map(|(_, v)| v.to_string())
            .collect()
    }

    #[test]
    fn compose_never_contains_passwords_and_env_file_holds_32_char_secrets() {
        for stack in [Stack::Nodejs, Stack::SpringBoot, Stack::Laravel] {
            let dir = project(&[
                ("package.json", "{}"),
                ("pom.xml", "<project/>"),
                ("composer.json", "{}"),
            ]);
            let mut req = request(&dir, stack);
            req.services = all_services();
            let plan = preview(&req).unwrap();
            let env = file(&plan, ENV_FILE);
            assert!(env.sensitive && !file(&plan, COMPOSE_FILE).sensitive);
            let secrets = secret_values(&env.content);
            assert!(secrets.len() >= 5, "{stack:?}: {secrets:?}");
            for secret in &secrets {
                assert_eq!(secret.len(), 32, "{secret}");
                assert!(secret.bytes().all(|b| b.is_ascii_alphanumeric()));
                assert!(!file(&plan, COMPOSE_FILE).content.contains(secret.as_str()));
                assert!(!file(&plan, ENV_EXAMPLE).content.contains(secret.as_str()));
            }
            // MySQL user and root passwords differ, and every credential is unique
            let unique: std::collections::HashSet<_> = secrets.iter().collect();
            assert!(unique.len() >= 5);
            let compose = &file(&plan, COMPOSE_FILE).content;
            assert!(compose.contains("env_file:") && compose.contains(".env.docker"));
            assert!(!compose.contains("autodock"), "no default password left");
            assert!(file(&plan, ENV_EXAMPLE).content.contains("change-me"));
            assert!(env.content.contains("MONGO_INITDB_ROOT_PASSWORD="));
            assert!(env.content.contains("RABBITMQ_DEFAULT_PASS="));
            assert!(env.content.contains("MYSQL_ROOT_PASSWORD="));
        }
    }

    #[test]
    fn services_without_passwords_need_no_env_file() {
        let dir = project(&[("package.json", "{}")]);
        let mut req = request(&dir, Stack::Nodejs);
        req.services = vec![Service::Redis, Service::Mailhog, Service::Adminer];
        let plan = preview(&req).unwrap();
        assert!(plan
            .files
            .iter()
            .all(|f| f.name != ENV_FILE && f.name != ENV_EXAMPLE));
        assert!(!file(&plan, COMPOSE_FILE).content.contains("env_file"));
    }

    #[test]
    fn every_preview_gets_fresh_passwords_but_the_seed_reproduces_them() {
        let dir = project(&[("package.json", "{}")]);
        let mut req = request(&dir, Stack::Nodejs);
        req.services = vec![Service::Postgres];
        let first = preview(&req).unwrap();
        let second = preview(&req).unwrap();
        assert_ne!(first.seed, second.seed);
        assert_ne!(
            file(&first, ENV_FILE).content,
            file(&second, ENV_FILE).content
        );
        // sending the seed and the ports back reproduces the preview exactly
        req.seed = Some(first.seed.clone());
        req.ports = Some(first.ports.clone());
        let again = preview(&req).unwrap();
        for name in [ENV_FILE, COMPOSE_FILE, DOCKERFILE, ENV_EXAMPLE] {
            assert_eq!(
                file(&first, name).content,
                file(&again, name).content,
                "{name}"
            );
        }
    }

    #[test]
    fn apply_writes_exactly_what_the_preview_showed() {
        let dir = project(&[("package.json", "{}")]);
        let mut req = request(&dir, Stack::Nodejs);
        req.services = vec![Service::Postgres, Service::Mysql];
        let plan = preview(&req).unwrap();
        req.seed = Some(plan.seed.clone());
        req.ports = Some(plan.ports.clone());
        let names: Vec<String> = plan.files.iter().map(|f| f.name.clone()).collect();
        let report = apply(&req, &names).unwrap();
        assert_eq!(report.written.len(), names.len());
        for f in &plan.files {
            assert_eq!(
                fs::read_to_string(dir.path().join(&f.name)).unwrap(),
                f.content,
                "{}",
                f.name
            );
        }
        // the compose file written to disk still has no password
        let written = fs::read_to_string(dir.path().join(COMPOSE_FILE)).unwrap();
        for secret in secret_values(&file(&plan, ENV_FILE).content) {
            assert!(!written.contains(&secret));
        }
    }

    #[cfg(unix)]
    #[test]
    fn env_file_is_private_and_never_overwritten() {
        use std::os::unix::fs::PermissionsExt;
        let dir = project(&[("package.json", "{}")]);
        let mut req = request(&dir, Stack::Nodejs);
        req.services = vec![Service::Postgres];
        let report = apply(&req, &[ENV_FILE.into()]).unwrap();
        assert_eq!(report.written, vec![ENV_FILE.to_string()]);
        let mode = fs::metadata(dir.path().join(ENV_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let before = fs::read_to_string(dir.path().join(ENV_FILE)).unwrap();
        let report = apply(&req, &[ENV_FILE.into()]).unwrap();
        assert!(report.written.is_empty() && report.skipped.len() == 1);
        assert_eq!(
            fs::read_to_string(dir.path().join(ENV_FILE)).unwrap(),
            before
        );
    }

    #[test]
    fn tampered_seed_or_ports_are_rejected() {
        let dir = project(&[("package.json", "{}")]);
        let mut req = request(&dir, Stack::Nodejs);
        req.services = vec![Service::Postgres];
        let plan = preview(&req).unwrap();
        for bad in ["zz", "", &"0".repeat(63)] {
            let mut bad_req = req.clone();
            bad_req.seed = Some(bad.to_string());
            assert!(preview(&bad_req).is_err(), "{bad:?}");
        }
        let mut bad_req = req.clone();
        bad_req.ports = Some(vec![]);
        assert!(preview(&bad_req).is_err());
        let mut tampered = plan.ports.clone();
        tampered[0].host_port = 0;
        bad_req.ports = Some(tampered);
        assert!(preview(&bad_req).is_err());
        // unknown fields in a port entry are refused by the JSON boundary
        assert!(serde_json::from_str::<PortAssignment>(
            r#"{"name":"app","containerPort":1,"hostPort":2,"preferredPort":1,"x":1}"#
        )
        .is_err());
    }

    #[test]
    fn projects_created_in_sequence_never_share_host_ports() {
        let mut seen = std::collections::HashSet::new();
        let mut keep = vec![];
        for _ in 0..6 {
            let dir = project(&[("package.json", "{}")]);
            let mut req = request(&dir, Stack::Nodejs);
            req.services = vec![Service::Postgres, Service::Redis, Service::Rabbitmq];
            let plan = preview(&req).unwrap();
            req.seed = Some(plan.seed.clone());
            req.ports = Some(plan.ports.clone());
            let names: Vec<String> = plan.files.iter().map(|f| f.name.clone()).collect();
            apply(&req, &names).unwrap();
            for port in &plan.ports {
                assert!(
                    seen.insert(port.host_port),
                    "port {} reused: {:?}",
                    port.host_port,
                    plan.ports
                );
            }
            keep.push(dir); // keep the folders alive: reservations of deleted projects are released
        }
    }

    #[test]
    fn a_busy_port_is_replaced_and_the_note_says_so() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let busy = listener.local_addr().unwrap().port();
        let dir = project(&[("package.json", "{}")]);
        let mut req = request(&dir, Stack::Nodejs);
        req.port = busy;
        let plan = preview(&req).unwrap();
        assert_ne!(host(&plan, "app"), busy);
        let compose = &file(&plan, COMPOSE_FILE).content;
        assert!(compose.contains(&format!("\"{}:{busy}\"", host(&plan, "app"))));
        assert!(plan
            .notes
            .iter()
            .any(|n| n.contains(&format!("A porta {busy} está em uso"))));
        assert!(plan
            .notes
            .iter()
            .any(|n| n.contains(&format!("http://localhost:{}", host(&plan, "app")))));
        drop(listener);
    }

    #[test]
    fn gitignore_advice_depends_on_the_project_gitignore() {
        let advice = |dir: &tempfile::TempDir| {
            let mut req = request(dir, Stack::Nodejs);
            req.services = vec![Service::Postgres];
            preview(&req)
                .unwrap()
                .notes
                .iter()
                .any(|n| n.contains("ao seu .gitignore"))
        };
        assert!(advice(&project(&[("package.json", "{}")])));
        assert!(advice(&project(&[
            ("package.json", "{}"),
            (".gitignore", "node_modules\n")
        ])));
        assert!(!advice(&project(&[
            ("package.json", "{}"),
            (".gitignore", "node_modules\n.env.docker\n")
        ])));
        assert!(!advice(&project(&[
            ("package.json", "{}"),
            (".gitignore", ".env*\n")
        ])));
        // new projects already ship a .gitignore that covers it
        let dir = project(&[]);
        let mut req = request(&dir, Stack::Nodejs);
        req.new_project = true;
        req.services = vec![Service::Postgres];
        let plan = preview(&req).unwrap();
        assert!(file(&plan, ".gitignore").content.contains(".env.docker"));
        assert!(file(&plan, "README.md")
            .content
            .contains(&format!("http://localhost:{}", host(&plan, "app"))));
    }

    #[test]
    fn requires_manifest_for_selected_stack() {
        let dir = project(&[]);
        for stack in [
            Stack::Nodejs,
            Stack::SpringBoot,
            Stack::Python,
            Stack::Laravel,
        ] {
            assert!(preview(&request(&dir, stack)).is_err());
        }
    }

    #[test]
    fn apply_writes_selected_files_and_never_overwrites() {
        let dir = project(&[("package.json", "{}"), ("Dockerfile", "ORIGINAL")]);
        let req = request(&dir, Stack::Nodejs);
        let plan = preview(&req).unwrap();
        assert!(file(&plan, DOCKERFILE).exists);
        assert!(!file(&plan, COMPOSE_FILE).exists);
        let report = apply(&req, &[DOCKERFILE.into(), COMPOSE_FILE.into()]).unwrap();
        assert_eq!(report.written, vec![COMPOSE_FILE.to_string()]);
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(
            fs::read_to_string(dir.path().join(DOCKERFILE)).unwrap(),
            "ORIGINAL"
        );
        assert!(!dir.path().join(DOCKERIGNORE).exists());
        // second run: nothing left to write
        let report = apply(&req, &[COMPOSE_FILE.into()]).unwrap();
        assert!(report.written.is_empty());
    }

    #[test]
    fn existing_compose_variants_block_the_compose_file() {
        for name in COMPOSE_NAMES {
            let dir = project(&[("package.json", "{}"), (name, "services: {}")]);
            let plan = preview(&request(&dir, Stack::Nodejs)).unwrap();
            assert!(file(&plan, COMPOSE_FILE).exists, "{name}");
        }
    }

    #[test]
    fn apply_rejects_names_outside_the_plan_and_empty_selection() {
        let dir = project(&[("package.json", "{}")]);
        let req = request(&dir, Stack::Nodejs);
        assert!(apply(&req, &[]).is_err());
        for name in ["../evil", "/etc/passwd", ".env", "Dockerfile/../x"] {
            assert!(apply(&req, &[name.into()]).is_err(), "{name}");
        }
        assert!(fs::read_dir(dir.path()).unwrap().count() == 1);
    }

    #[cfg(unix)]
    #[test]
    fn apply_does_not_follow_symlinks() {
        let dir = project(&[("package.json", "{}")]);
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("victim");
        fs::write(&target, "SAFE").unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join(DOCKERFILE)).unwrap();
        let report = apply(&request(&dir, Stack::Nodejs), &[DOCKERFILE.into()]).unwrap();
        assert!(report.written.is_empty());
        assert_eq!(fs::read_to_string(&target).unwrap(), "SAFE");
    }

    #[test]
    fn never_reads_env_files_and_detects_stacks_without_executing() {
        let dir = project(&[
            ("package.json", r#"{"scripts":{"dev":"x","bad name":"y"}}"#),
            ("pnpm-lock.yaml", ""),
            (".env", "SECRET=1"),
        ]);
        let found = detect_stacks(&dir.path().to_string_lossy()).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].stack, Stack::Nodejs);
        assert_eq!(found[0].package_manager, Some(PackageManager::Pnpm));
        assert_eq!(found[0].scripts, vec!["dev".to_string()]);
        let plan = preview(&request(&dir, Stack::Nodejs)).unwrap();
        assert!(plan.files.iter().all(|f| !f.content.contains("SECRET")));
    }
}
