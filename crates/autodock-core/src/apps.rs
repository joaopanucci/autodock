//! Detection of the applications inside a folder: monorepos and microservice
//! projects with a backend, a front end, a mobile app and so on.
//!
//! Like the rest of the analysis it only reads manifests as data: nothing is
//! executed, `.env` files are never read and symbolic links are not followed.
use crate::generate::{
    self, csproj_name, gemfile_has_rails, present, read_manifest, regular_file, valid_script,
    PackageManager, PythonFramework, Stack, COMPOSE_NAMES,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// How deep below the project root applications are looked for.
const MAX_DEPTH: usize = 4;
const MAX_DIRS: usize = 3000;

/// Folders that never hold an application of their own.
const SKIP: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "vendor",
    "bin",
    "obj",
    "venv",
    "env",
    "coverage",
    "android",
    "ios",
    "__pycache__",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppRole {
    Backend,
    Frontend,
    Mobile,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppCandidate {
    /// Folder relative to the project root, with `/`; `.` for the root itself.
    pub path: String,
    /// Suggested service name (valid for compose and container names).
    pub name: String,
    pub stack: Stack,
    pub role: AppRole,
    /// `nextjs`, `nestjs`, `vite`, `express`, `expo`, `spring-boot`, `fastapi`…
    pub framework: Option<String>,
    pub evidence: String,
    /// Port the application listens on by default.
    pub port: u16,
    pub start_script: Option<String>,
    pub package_manager: Option<PackageManager>,
    pub python_framework: Option<PythonFramework>,
    pub has_dockerfile: bool,
    /// Suggested selection. `false` when the app cannot be containerised from here
    /// (see `note`) or needs more than this version supports.
    pub selected: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFolder {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsReport {
    pub schema_version: u32,
    pub project: String,
    pub project_name: String,
    pub apps: Vec<AppCandidate>,
    /// Folders with a manifest that are not applications (libraries, workspace roots…).
    pub skipped: Vec<SkippedFolder>,
    /// An existing compose file at the root, which generation never overwrites.
    pub existing_compose: Option<String>,
    pub warnings: Vec<String>,
}

pub fn detect_apps(path: &str) -> Result<AppsReport, String> {
    let root = generate::root_of(path)?;
    let project_name = generate::project_name(&root);
    let mut report = AppsReport {
        schema_version: 1,
        project: root.display().to_string(),
        project_name: project_name.clone(),
        apps: vec![],
        skipped: vec![],
        existing_compose: COMPOSE_NAMES
            .iter()
            .find(|name| present(&root, name))
            .map(|name| name.to_string()),
        warnings: vec![],
    };

    let mut visited = 0usize;
    let mut pending: Vec<(PathBuf, usize)> = vec![(root.clone(), 0)];
    while let Some((dir, depth)) = pending.pop() {
        if visited >= MAX_DIRS {
            report.warnings.push(tr!(
                "Limite de {MAX_DIRS} pastas atingido; a busca de apps é parcial.",
                "Limit of {MAX_DIRS} folders reached; the app search is partial."
            ));
            break;
        }
        visited += 1;
        let rel = relative(&root, &dir);
        classify(&dir, &rel, &project_name, &mut report);
        if depth >= MAX_DEPTH {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                !name.starts_with('.') && !SKIP.contains(&name.as_str())
            })
            .map(|entry| entry.path())
            .collect();
        // Depth-first in a stable order, so the report does not depend on the file system.
        children.sort();
        for child in children.into_iter().rev() {
            pending.push((child, depth + 1));
        }
    }

    report.apps.sort_by(|a, b| a.path.cmp(&b.path));
    report.skipped.sort_by(|a, b| a.path.cmp(&b.path));
    unique_names(&mut report.apps);
    Ok(report)
}

fn relative(root: &Path, dir: &Path) -> String {
    let rel = dir
        .strip_prefix(root)
        .unwrap_or(dir)
        .to_string_lossy()
        .replace('\\', "/");
    if rel.is_empty() {
        ".".into()
    } else {
        rel
    }
}

/// Folder name reduced to a service name: lowercase letters, digits and hyphens.
fn service_name(folder: &str, project_name: &str) -> String {
    let raw = if folder == "." {
        project_name.to_string()
    } else {
        folder.rsplit('/').next().unwrap_or(folder).to_string()
    };
    let mut name: String = raw
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while name.contains("--") {
        name = name.replace("--", "-");
    }
    let mut name = name.trim_matches('-').to_string();
    if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_lowercase()) {
        name = format!("app-{name}").trim_end_matches('-').to_string();
    }
    name.chars()
        .take(30)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

/// Two folders can share a name (`api` under two parents): add a numeric suffix.
fn unique_names(apps: &mut [AppCandidate]) {
    let mut seen: Vec<String> = vec![];
    for app in apps.iter_mut() {
        let base = app.name.clone();
        let mut candidate = base.clone();
        let mut n = 2;
        while seen.contains(&candidate) {
            candidate = format!("{base}-{n}");
            n += 1;
        }
        app.name = candidate.clone();
        seen.push(candidate);
    }
}

fn skip(report: &mut AppsReport, path: &str, reason: String) {
    report.skipped.push(SkippedFolder {
        path: path.to_string(),
        reason,
    });
}

fn classify(dir: &Path, rel: &str, project_name: &str, report: &mut AppsReport) {
    let has_dockerfile = regular_file(dir, "Dockerfile");
    let name = service_name(rel, project_name);
    let push = |report: &mut AppsReport, app: AppCandidate| {
        report.apps.push(AppCandidate {
            name: name.clone(),
            has_dockerfile,
            ..app
        })
    };
    let base =
        |stack: Stack, role: AppRole, framework: &str, evidence: String, port: u16| AppCandidate {
            path: rel.to_string(),
            name: String::new(),
            stack,
            role,
            framework: Some(framework.to_string()),
            evidence,
            port,
            start_script: None,
            package_manager: None,
            python_framework: None,
            has_dockerfile: false,
            selected: true,
            note: None,
        };

    // ── Node.js family ──
    if let Some(text) = read_manifest(dir, "package.json") {
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(json) if json.is_object() => {
                if let Some(app) = node_app(dir, rel, &json, &base, report) {
                    push(report, app);
                }
            }
            _ => skip(
                report,
                rel,
                tr!("package.json inválido", "invalid package.json"),
            ),
        }
    }

    // ── Java / Spring Boot ──
    if let Some(text) = read_manifest(dir, "pom.xml") {
        if text.contains("org.springframework.boot") {
            match spring_app(&text) {
                SpringKind::Aggregator => skip(
                    report,
                    rel,
                    tr!(
                        "pom pai (agrega módulos Maven)",
                        "parent pom (aggregates Maven modules)"
                    ),
                ),
                SpringKind::Library => skip(
                    report,
                    rel,
                    tr!(
                        "biblioteca Maven (sem spring-boot-maven-plugin)",
                        "Maven library (no spring-boot-maven-plugin)"
                    ),
                ),
                SpringKind::App { multi_module } => {
                    let mut app = base(
                        Stack::SpringBoot,
                        AppRole::Backend,
                        "spring-boot",
                        "pom.xml".into(),
                        8080,
                    );
                    if multi_module {
                        app.selected = false;
                        app.note = Some(tr!(
                            "Módulo de um projeto Maven multimódulo: o Dockerfile gerado ainda não constrói a partir do pom pai.",
                            "Module of a multi-module Maven project: the generated Dockerfile does not build from the parent pom yet."
                        ));
                    }
                    push(report, app);
                }
            }
        }
    }

    // ── Python ──
    for manifest in ["requirements.txt", "pyproject.toml"] {
        if let Some(text) = read_manifest(dir, manifest) {
            let lower = text.to_ascii_lowercase();
            let framework = if lower.contains("fastapi") {
                Some((PythonFramework::Fastapi, "fastapi"))
            } else if lower.contains("django") {
                Some((PythonFramework::Django, "django"))
            } else if lower.contains("flask") {
                Some((PythonFramework::Flask, "flask"))
            } else {
                None
            };
            match framework {
                Some((python_framework, label)) => {
                    let mut app = base(
                        Stack::Python,
                        AppRole::Backend,
                        label,
                        manifest.into(),
                        8000,
                    );
                    app.python_framework = Some(python_framework);
                    if python_framework == PythonFramework::Fastapi && !regular_file(dir, "main.py")
                    {
                        app.note = Some(tr!(
                            "Não achei main.py na pasta: confira o módulo (arquivo:objeto) no assistente.",
                            "No main.py in the folder: check the module (file:object) in the assistant."
                        ));
                    }
                    if python_framework == PythonFramework::Django
                        && !regular_file(dir, "manage.py")
                    {
                        app.selected = false;
                        app.note = Some(tr!(
                            "Django sem manage.py na pasta.",
                            "Django without manage.py in the folder."
                        ));
                    }
                    push(report, app);
                }
                None => skip(
                    report,
                    rel,
                    tr!(
                        "Python sem FastAPI, Django ou Flask",
                        "Python without FastAPI, Django or Flask"
                    ),
                ),
            }
            break;
        }
    }

    // ── PHP / Laravel ──
    if let Some(text) = read_manifest(dir, "composer.json") {
        if text.contains("laravel/framework") {
            push(
                report,
                base(
                    Stack::Laravel,
                    AppRole::Backend,
                    "laravel",
                    "composer.json (laravel/framework)".into(),
                    8000,
                ),
            );
        } else {
            skip(report, rel, tr!("PHP sem Laravel", "PHP without Laravel"));
        }
    }

    // ── Go ──
    if regular_file(dir, "go.mod") {
        let mut app = base(Stack::Go, AppRole::Backend, "go", "go.mod".into(), 8080);
        if !regular_file(dir, "main.go") {
            app.selected = false;
            app.note = Some(tr!(
                "Não achei main.go na pasta: o Dockerfile gerado compila o pacote main da raiz (go build .).",
                "No main.go in the folder: the generated Dockerfile builds the main package at the root (go build .)."
            ));
        }
        push(report, app);
    }

    // ── .NET ──
    if let Some(project) = csproj_name(dir) {
        let web = read_manifest(dir, &format!("{project}.csproj"))
            .is_some_and(|text| text.contains("Microsoft.NET.Sdk.Web"));
        if web {
            push(
                report,
                base(
                    Stack::Dotnet,
                    AppRole::Backend,
                    "aspnet",
                    format!("{project}.csproj"),
                    8080,
                ),
            );
        } else {
            skip(
                report,
                rel,
                tr!(
                    "projeto .NET que não é web (biblioteca ou console)",
                    "non-web .NET project (library or console)"
                ),
            );
        }
    }

    // ── Ruby on Rails ──
    if read_manifest(dir, "Gemfile").is_some_and(|text| gemfile_has_rails(&text)) {
        push(
            report,
            base(
                Stack::Rails,
                AppRole::Backend,
                "rails",
                "Gemfile (rails)".into(),
                3000,
            ),
        );
    }

    // A package.json next to a backend (Laravel, Rails, Django, Spring...) is that
    // project's asset tooling (Vite, webpack). Two apps in one folder would share
    // one Dockerfile, so the second would run with the wrong image and fall over.
    let backend_here = report
        .apps
        .iter()
        .any(|a| a.path == rel && a.stack != Stack::Nodejs);
    if backend_here {
        let before = report.apps.len();
        report
            .apps
            .retain(|a| !(a.path == rel && a.stack == Stack::Nodejs));
        if report.apps.len() != before {
            skip(
                report,
                rel,
                tr!(
                    "package.json do front-end (Vite, webpack) de um projeto que já tem backend nesta pasta: roda junto com ele, não como outro app",
                    "package.json of the front-end tooling (Vite, webpack) of a project that already has a backend in this folder: it runs with it, not as another app"
                ),
            );
        }
    }
}

fn node_app(
    dir: &Path,
    rel: &str,
    json: &serde_json::Value,
    base: &dyn Fn(Stack, AppRole, &str, String, u16) -> AppCandidate,
    report: &mut AppsReport,
) -> Option<AppCandidate> {
    if json.get("workspaces").is_some() {
        skip(
            report,
            rel,
            tr!(
                "raiz de workspace (agrupa pacotes)",
                "workspace root (groups packages)"
            ),
        );
        return None;
    }
    let has = |name: &str| {
        ["dependencies", "devDependencies"]
            .iter()
            .any(|group| json.get(group).and_then(|d| d.get(name)).is_some())
    };
    let scripts: Vec<String> = json
        .get("scripts")
        .and_then(|s| s.as_object())
        .map(|s| s.keys().filter(|k| valid_script(k)).cloned().collect())
        .unwrap_or_default();
    let script_text = |name: &str| {
        json.get("scripts")
            .and_then(|s| s.get(name))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let pick = |preferred: &[&str]| -> Option<String> {
        preferred
            .iter()
            .find(|name| scripts.iter().any(|s| s == *name))
            .map(|name| name.to_string())
    };

    // (role, framework, port, preferred scripts, supported, note)
    let (role, framework, port, preferred, supported, note): (
        AppRole,
        &str,
        u16,
        &[&str],
        bool,
        Option<String>,
    ) = if has("expo") || has("react-native") {
        (
            AppRole::Mobile,
            if has("expo") { "expo" } else { "react-native" },
            8081,
            &["start"],
            false,
            Some(tr!(
                "App mobile: o servidor de desenvolvimento precisa ser alcançado pelo celular e fica fora desta versão.",
                "Mobile app: the dev server must be reachable from the phone and is outside this version."
            )),
        )
    } else if has("next") {
        (AppRole::Frontend, "nextjs", 3000, &["dev"], true, None)
    } else if has("@nestjs/core") {
        (
            AppRole::Backend,
            "nestjs",
            3000,
            &["start:dev", "start"],
            true,
            None,
        )
    } else if has("nuxt") {
        (
            AppRole::Frontend,
            "nuxt",
            3000,
            &["dev"],
            false,
            Some(tr!(
                "Nuxt: o servidor de desenvolvimento precisa de --host 0.0.0.0, ainda não suportado.",
                "Nuxt: the dev server needs --host 0.0.0.0, not supported yet."
            )),
        )
    } else if has("@angular/core") {
        (
            AppRole::Frontend,
            "angular",
            4200,
            &["start"],
            false,
            Some(tr!(
                "Angular: o servidor de desenvolvimento precisa de --host 0.0.0.0, ainda não suportado.",
                "Angular: the dev server needs --host 0.0.0.0, not supported yet."
            )),
        )
    } else if has("vite") || script_text("dev").trim_start().starts_with("vite") {
        (AppRole::Frontend, "vite", 5173, &["dev"], true, None)
    } else if has("react-scripts") {
        (AppRole::Frontend, "cra", 3000, &["start"], true, None)
    } else if ["express", "fastify", "koa", "@hapi/hapi", "restify", "hono"]
        .iter()
        .any(|d| has(d))
    {
        (
            AppRole::Backend,
            "express",
            3000,
            &["start", "dev", "start:dev"],
            true,
            None,
        )
    } else if pick(&["start"]).is_some() {
        (AppRole::Backend, "node", 3000, &["start"], true, None)
    } else {
        skip(
            report,
            rel,
            tr!(
                "pacote Node.js sem script de início (biblioteca)",
                "Node.js package without a start script (library)"
            ),
        );
        return None;
    };

    let Some(start_script) = pick(preferred).or_else(|| pick(&["start", "dev", "serve"])) else {
        skip(
            report,
            rel,
            tr!(
                "pacote Node.js sem script de início (biblioteca)",
                "Node.js package without a start script (library)"
            ),
        );
        return None;
    };

    let mut app = base(Stack::Nodejs, role, framework, "package.json".into(), port);
    app.start_script = Some(start_script);
    app.package_manager = Some(package_manager(dir));
    app.selected = supported;
    app.note = note;
    Some(app)
}

/// Lock file of the folder, or of the parent when the lock lives at a monorepo root.
fn package_manager(dir: &Path) -> PackageManager {
    let mut current = Some(dir);
    for _ in 0..=MAX_DEPTH {
        let Some(folder) = current else { break };
        if regular_file(folder, "pnpm-lock.yaml") {
            return PackageManager::Pnpm;
        }
        if regular_file(folder, "yarn.lock") {
            return PackageManager::Yarn;
        }
        if regular_file(folder, "package-lock.json") {
            return PackageManager::Npm;
        }
        current = folder.parent();
    }
    PackageManager::Npm
}

enum SpringKind {
    Aggregator,
    Library,
    App { multi_module: bool },
}

fn spring_app(text: &str) -> SpringKind {
    let Ok(doc) = roxmltree::Document::parse(text) else {
        return SpringKind::Library;
    };
    let project = doc.root_element();
    let child = |parent: roxmltree::Node, tag: &str| -> Option<String> {
        parent
            .children()
            .find(|n| n.is_element() && n.has_tag_name(tag))
            .and_then(|n| n.text())
            .map(|t| t.trim().to_string())
    };
    if child(project, "packaging").as_deref() == Some("pom") {
        return SpringKind::Aggregator;
    }
    let has_plugin = project.descendants().any(|n| {
        n.is_element()
            && n.has_tag_name("artifactId")
            && n.text().map(str::trim) == Some("spring-boot-maven-plugin")
    });
    if !has_plugin {
        return SpringKind::Library;
    }
    let parent_group = project
        .children()
        .find(|n| n.is_element() && n.has_tag_name("parent"))
        .and_then(|p| child(p, "groupId"));
    SpringKind::App {
        multi_module: parent_group.as_deref() != Some("org.springframework.boot"),
    }
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

    fn detect(dir: &tempfile::TempDir) -> AppsReport {
        detect_apps(&dir.path().to_string_lossy()).unwrap()
    }

    fn app<'a>(report: &'a AppsReport, path: &str) -> &'a AppCandidate {
        report
            .apps
            .iter()
            .find(|a| a.path == path)
            .unwrap_or_else(|| panic!("no app at {path}: {:?}", report.apps))
    }

    const SPRING_POM: &str = r#"<project>
  <parent><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-parent</artifactId></parent>
  <artifactId>api</artifactId>
  <build><plugins><plugin><artifactId>spring-boot-maven-plugin</artifactId></plugin></plugins></build>
</project>"#;

    #[test]
    fn finds_every_app_of_a_microservice_project_with_roles_and_ports() {
        let dir = tree(&[
            ("api/pom.xml", SPRING_POM),
            (
                "web/package.json",
                r#"{"scripts":{"dev":"next dev"},"dependencies":{"next":"16"}}"#,
            ),
            (
                "admin/package.json",
                r#"{"scripts":{"dev":"vite"},"devDependencies":{"vite":"6"}}"#,
            ),
            ("gateway/go.mod", "module gateway"),
            ("gateway/main.go", "package main"),
            (
                "mobile/package.json",
                r#"{"scripts":{"start":"expo start"},"dependencies":{"expo":"57"}}"#,
            ),
            (
                "worker/package.json",
                r#"{"scripts":{"start":"node index.js"},"dependencies":{"express":"4"}}"#,
            ),
        ]);
        let report = detect(&dir);
        let names: Vec<&str> = report.apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            ["admin", "api", "gateway", "mobile", "web", "worker"]
        );

        let api = app(&report, "api");
        assert_eq!(
            (api.stack, api.role, api.port),
            (Stack::SpringBoot, AppRole::Backend, 8080)
        );
        let web = app(&report, "web");
        assert_eq!(
            (web.stack, web.role, web.port),
            (Stack::Nodejs, AppRole::Frontend, 3000)
        );
        assert_eq!(web.framework.as_deref(), Some("nextjs"));
        assert_eq!(web.start_script.as_deref(), Some("dev"));
        let admin = app(&report, "admin");
        assert_eq!(
            (admin.framework.as_deref(), admin.port),
            (Some("vite"), 5173)
        );
        let gateway = app(&report, "gateway");
        assert_eq!((gateway.stack, gateway.port), (Stack::Go, 8080));
        assert!(gateway.selected);
        let worker = app(&report, "worker");
        assert_eq!(
            (worker.role, worker.framework.as_deref()),
            (AppRole::Backend, Some("express"))
        );
        // the mobile app is found but not selected by default
        let mobile = app(&report, "mobile");
        assert_eq!((mobile.role, mobile.selected), (AppRole::Mobile, false));
        assert!(mobile.note.is_some());
    }

    #[test]
    fn a_backend_with_its_own_vite_tooling_is_one_app_not_two() {
        let dir = tree(&[
            (
                "composer.json",
                r#"{"require":{"laravel/framework":"^13"}}"#,
            ),
            (
                "package.json",
                r#"{"scripts":{"dev":"vite","build":"vite build"},"devDependencies":{"vite":"^7"}}"#,
            ),
            ("api/package.json", r#"{"scripts":{"start":"node ."}}"#),
        ]);
        let report = detect(&dir);
        let root: Vec<_> = report
            .apps
            .iter()
            .filter(|a| a.path.is_empty() || a.path == ".")
            .collect();
        assert_eq!(root.len(), 1, "{:?}", report.apps);
        assert_eq!(root[0].stack, Stack::Laravel);
        // A real second app in another folder is still found.
        assert!(report
            .apps
            .iter()
            .any(|a| a.path == "api" && a.stack == Stack::Nodejs));
        assert!(report
            .skipped
            .iter()
            .any(|s| s.reason.contains("front-end") || s.reason.contains("package.json of")));
    }

    #[test]
    fn libraries_and_workspace_roots_are_not_apps() {
        let dir = tree(&[
            ("package.json", r#"{"private":true,"workspaces":["apps/*","packages/*"],"scripts":{"dev":"turbo dev"}}"#),
            ("apps/web/package.json", r#"{"scripts":{"dev":"next dev"},"dependencies":{"next":"16"}}"#),
            ("apps/api/package.json", r#"{"scripts":{"start:dev":"nest start --watch"},"dependencies":{"@nestjs/core":"11"}}"#),
            ("packages/ui/package.json", r#"{"name":"@acme/ui","main":"index.js"}"#),
            ("packages/core/pom.xml", "<project><artifactId>core</artifactId><dependencies><dependency><groupId>org.springframework.boot</groupId></dependency></dependencies></project>"),
            ("services/shared/pom.xml", "<project><packaging>pom</packaging><parent><groupId>org.springframework.boot</groupId></parent></project>"),
        ]);
        let report = detect(&dir);
        let paths: Vec<&str> = report.apps.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(paths, ["apps/api", "apps/web"]);
        assert_eq!(
            app(&report, "apps/api").framework.as_deref(),
            Some("nestjs")
        );
        assert_eq!(
            app(&report, "apps/api").start_script.as_deref(),
            Some("start:dev")
        );
        let reasons: Vec<(&str, &str)> = report
            .skipped
            .iter()
            .map(|s| (s.path.as_str(), s.reason.as_str()))
            .collect();
        assert!(reasons
            .iter()
            .any(|(p, r)| *p == "." && r.contains("workspace")));
        assert!(reasons.iter().any(|(p, _)| *p == "packages/ui"));
        assert!(reasons
            .iter()
            .any(|(p, r)| *p == "services/shared" && r.contains("parent pom")));
        assert!(reasons
            .iter()
            .any(|(p, r)| *p == "packages/core" && r.contains("library")));
    }

    #[test]
    fn detects_the_other_stacks_and_flags_what_this_version_cannot_run() {
        let dir = tree(&[
            ("py/requirements.txt", "fastapi==0.115\nuvicorn\n"),
            ("dj/requirements.txt", "django==5.1\n"),
            ("dj/manage.py", ""),
            (
                "lara/composer.json",
                r#"{"require":{"laravel/framework":"^12"}}"#,
            ),
            (
                "net/Shop.Api.csproj",
                r#"<Project Sdk="Microsoft.NET.Sdk.Web"/>"#,
            ),
            (
                "lib/Shop.Core.csproj",
                r#"<Project Sdk="Microsoft.NET.Sdk"/>"#,
            ),
            ("rb/Gemfile", "gem \"rails\", \"~> 8\"\n"),
            ("goutil/go.mod", "module x"),
            (
                "ng/package.json",
                r#"{"scripts":{"start":"ng serve"},"dependencies":{"@angular/core":"20"}}"#,
            ),
        ]);
        let report = detect(&dir);
        assert_eq!(app(&report, "py").stack, Stack::Python);
        assert!(
            app(&report, "py").note.is_some(),
            "fastapi without main.py is flagged"
        );
        assert_eq!(
            app(&report, "dj").python_framework,
            Some(PythonFramework::Django)
        );
        assert_eq!(app(&report, "lara").stack, Stack::Laravel);
        assert_eq!(app(&report, "net").stack, Stack::Dotnet);
        assert!(
            report.apps.iter().all(|a| a.path != "lib"),
            "a class library is not an app"
        );
        assert_eq!(app(&report, "rb").stack, Stack::Rails);
        assert!(
            !app(&report, "goutil").selected,
            "go without main.go is not selected"
        );
        assert!(!app(&report, "ng").selected, "angular is not supported yet");
    }

    #[test]
    fn names_are_valid_and_unique_and_the_root_app_uses_the_project_name() {
        let dir = tree(&[
            ("package.json", r#"{"scripts":{"start":"node ."}}"#),
            ("a/api/go.mod", "module a"),
            ("a/api/main.go", ""),
            ("b/api/go.mod", "module b"),
            ("b/api/main.go", ""),
            ("My App_2/package.json", r#"{"scripts":{"start":"node ."}}"#),
            ("9lives/go.mod", "module n"),
            ("9lives/main.go", ""),
        ]);
        let report = detect(&dir);
        let names: Vec<&str> = report.apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(app(&report, ".").name, report.project_name);
        for name in &names {
            assert!(
                name.starts_with(|c: char| c.is_ascii_lowercase())
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    && !name.ends_with('-'),
                "invalid name {name}"
            );
        }
        let unique: std::collections::HashSet<&&str> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "duplicate names: {names:?}");
        assert!(names.contains(&"api") && names.contains(&"api-2"));
        assert!(names.contains(&"app-9lives"));
        assert!(names.contains(&"my-app-2"));
    }

    #[test]
    fn spring_multi_module_children_are_flagged_and_plugin_less_poms_are_libraries() {
        let child = r#"<project><parent><groupId>com.acme</groupId><artifactId>parent</artifactId></parent>
            <dependencies><dependency><groupId>org.springframework.boot</groupId></dependency></dependencies>
            <build><plugins><plugin><artifactId>spring-boot-maven-plugin</artifactId></plugin></plugins></build></project>"#;
        let dir = tree(&[("svc/pom.xml", child), ("api/pom.xml", SPRING_POM)]);
        let report = detect(&dir);
        assert!(app(&report, "api").selected && app(&report, "api").note.is_none());
        let svc = app(&report, "svc");
        assert!(!svc.selected);
        assert!(svc
            .note
            .as_deref()
            .unwrap()
            .to_lowercase()
            .contains("multi"));
    }

    #[test]
    fn skips_dependency_folders_hidden_folders_and_deep_trees() {
        let dir = tree(&[
            (
                "node_modules/pkg/package.json",
                r#"{"scripts":{"start":"x"}}"#,
            ),
            (".hidden/app/package.json", r#"{"scripts":{"start":"x"}}"#),
            ("target/classes/pom.xml", SPRING_POM),
            ("a/b/c/d/e/package.json", r#"{"scripts":{"start":"x"}}"#),
            ("a/b/c/d/go.mod", "module deep"),
            ("a/b/c/d/main.go", ""),
        ]);
        let report = detect(&dir);
        let paths: Vec<&str> = report.apps.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(
            paths,
            ["a/b/c/d"],
            "only the app within MAX_DEPTH levels is found"
        );
    }

    #[test]
    fn reports_existing_dockerfiles_and_compose_files() {
        let dir = tree(&[
            ("api/go.mod", "module api"),
            ("api/main.go", ""),
            ("api/Dockerfile", "FROM scratch"),
            (
                "web/package.json",
                r#"{"scripts":{"dev":"vite"},"devDependencies":{"vite":"6"}}"#,
            ),
            ("compose.yaml", "services: {}"),
        ]);
        let report = detect(&dir);
        assert!(app(&report, "api").has_dockerfile);
        assert!(!app(&report, "web").has_dockerfile);
        assert_eq!(report.existing_compose.as_deref(), Some("compose.yaml"));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_not_followed() {
        let outside = tree(&[("package.json", r#"{"scripts":{"start":"x"}}"#)]);
        let dir = tree(&[("real/go.mod", "module r"), ("real/main.go", "")]);
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        let report = detect(&dir);
        assert!(report.apps.iter().all(|a| a.path != "link"));
    }

    #[test]
    fn a_folder_without_apps_gives_an_empty_report() {
        let dir = tree(&[("README.md", "hello")]);
        let report = detect(&dir);
        assert!(report.apps.is_empty() && report.skipped.is_empty());
        assert!(detect_apps("/definitely/not/a/folder").is_err());
    }
}
