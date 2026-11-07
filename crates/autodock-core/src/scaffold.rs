//! Creates projects with each framework's official installer, run inside a
//! throwaway Docker container so nothing needs to be installed on the host.
//!
//! Safety contract: images and commands are fixed per framework; the only
//! interpolated value is a validated project name. No shell runs on the host.
//! Nothing is executed without an explicit call, and the plan (exact command)
//! can be previewed first. Only an empty folder is ever bound into the container.
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(20 * 60);
const OUTPUT_TAIL: usize = 6 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Framework {
    Laravel,
    Django,
    ViteReact,
    ViteVue,
    #[serde(rename = "nextjs")]
    NextJs,
    #[serde(rename = "nestjs")]
    NestJs,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScaffoldRequest {
    pub path: String,
    pub framework: Framework,
    pub project_name: String,
    #[serde(default)]
    pub runtime_version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaffoldPlan {
    pub title: String,
    pub image: String,
    /// Exact command that will run, for display only.
    pub command: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaffoldReport {
    pub success: bool,
    pub duration_ms: u128,
    pub output: String,
    pub detail: String,
}

pub(crate) fn valid_project_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=40).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && !name.ends_with('-')
        && !name.contains("--")
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

const RESERVED_PYTHON_NAMES: [&str; 8] = [
    "django", "test", "tests", "manage", "site", "admin", "app", "config",
];

fn image_version(
    allowed: &[&str],
    default: &str,
    requested: &Option<String>,
) -> Result<String, String> {
    let version = requested.as_deref().unwrap_or(default);
    if allowed.contains(&version) {
        Ok(version.to_string())
    } else {
        Err(tr!(
            "Versão não suportada: {version}. Use: {}.",
            "Unsupported version: {version}. Use: {}.",
            allowed.join(", ")
        ))
    }
}

struct Spec {
    title: String,
    image: String,
    /// Arguments after the image name.
    args: Vec<String>,
    marker: &'static str,
    notes: Vec<String>,
}

fn spec(request: &ScaffoldRequest) -> Result<Spec, String> {
    let name = &request.project_name;
    if !valid_project_name(name) {
        return Err(tr!(
            "Nome do projeto inválido (use letras minúsculas, números e hífen).",
            "Invalid project name (use lowercase letters, numbers and hyphens)."
        ));
    }
    let shell = |script: String| vec!["sh".to_string(), "-c".to_string(), script];
    Ok(match request.framework {
        Framework::Laravel => Spec {
            title: tr!("Laravel (instalador oficial)", "Laravel (official installer)"),
            image: "composer:2".into(),
            args: ["create-project", "laravel/laravel", ".", "--prefer-dist", "--no-interaction"]
                .map(String::from)
                .to_vec(),
            marker: "artisan",
            notes: vec![
                tr!("Usa composer create-project laravel/laravel. Pode levar vários minutos.", "Uses composer create-project laravel/laravel. It can take several minutes."),
                tr!("O banco padrão do Laravel é SQLite; serviços escolhidos serão configurados no compose.", "Laravel's default database is SQLite; the services you choose will be configured in compose."),
            ],
        },
        Framework::Django => {
            let package = name.replace('-', "_");
            if RESERVED_PYTHON_NAMES.contains(&package.as_str()) {
                return Err(tr!("\"{package}\" conflita com um módulo Python. Escolha outro nome.", "\"{package}\" clashes with a Python module. Pick another name."));
            }
            let version = image_version(&["3.13", "3.12", "3.11"], "3.12", &request.runtime_version)?;
            Spec {
                title: tr!("Django (django-admin startproject)", "Django (django-admin startproject)"),
                image: format!("python:{version}-slim"),
                args: shell(format!(
                    "pip install --no-cache-dir --target /tmp/py django >/dev/null && \
PYTHONPATH=/tmp/py python -m django startproject {package} . && \
printf 'Django==%s\\n' \"$(PYTHONPATH=/tmp/py python -m django --version)\" > requirements.txt"
                )),
                marker: "manage.py",
                notes: vec![
                    tr!("O pacote do projeto se chamará \"{package}\".", "The project package will be called \"{package}\"."),
                    tr!("A versão instalada do Django é fixada em requirements.txt.", "The installed Django version is pinned in requirements.txt."),
                ],
            }
        }
        Framework::NextJs => {
            let version = image_version(&["24", "22", "20"], "22", &request.runtime_version)?;
            Spec {
                title: "Next.js (create-next-app)".to_string(),
                image: format!("node:{version}-alpine"),
                args: shell(format!(
                    "npx --yes create-next-app@latest . --yes --use-npm --skip-install --disable-git < /dev/null && npm pkg set name={name}"
                )),
                marker: "package.json",
                notes: vec![
                    tr!("Usa o instalador oficial create-next-app com as opções recomendadas (TypeScript, App Router).", "Uses the official create-next-app installer with the recommended options (TypeScript, App Router)."),
                    tr!("As dependências são instaladas quando a imagem Docker é construída; o servidor de desenvolvimento usa a porta 3000.", "Dependencies are installed when the Docker image is built; the development server uses port 3000."),
                ],
            }
        }
        Framework::NestJs => {
            // Nest 12 dependencies do not install with the npm of Node 22.
            let version = image_version(&["24", "22", "20"], "24", &request.runtime_version)?;
            Spec {
                title: "NestJS (Nest CLI)".to_string(),
                image: format!("node:{version}-alpine"),
                args: shell(format!(
                    "npx --yes @nestjs/cli@latest new {name} --directory . --package-manager npm --skip-git --skip-install < /dev/null"
                )),
                marker: "package.json",
                notes: vec![
                    tr!("Usa o instalador oficial @nestjs/cli. O script de desenvolvimento é start:dev.", "Uses the official @nestjs/cli installer. The development script is start:dev."),
                    tr!("As dependências são instaladas quando a imagem Docker é construída; a aplicação usa a porta 3000.", "Dependencies are installed when the Docker image is built; the application uses port 3000."),
                ],
            }
        }
        Framework::ViteReact | Framework::ViteVue => {
            let version = image_version(&["24", "22", "20"], "22", &request.runtime_version)?;
            let template = if request.framework == Framework::ViteReact { "react-ts" } else { "vue-ts" };
            Spec {
                title: if request.framework == Framework::ViteReact {
                    "Vite + React + TypeScript (create-vite)".to_string()
                } else {
                    "Vite + Vue + TypeScript (create-vite)".to_string()
                },
                image: format!("node:{version}-alpine"),
                args: shell(format!(
                    "npm create vite@latest . -- --template {template} < /dev/null && npm pkg set name={name}"
                )),
                marker: "package.json",
                notes: vec![
                    tr!("As dependências são instaladas quando a imagem Docker é construída.", "Dependencies are installed when the Docker image is built."),
                    tr!("O servidor de desenvolvimento usa a porta 5173 por padrão.", "The development server uses port 5173 by default."),
                ],
            }
        }
    })
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

fn docker_args(spec: &Spec, directory: &Path, container: &str) -> Vec<String> {
    let mut args: Vec<String> = ["run", "--rm", "--name", container]
        .map(String::from)
        .to_vec();
    #[cfg(unix)]
    {
        // Files created in the bind mount belong to the current user, not root.
        // SAFETY: geteuid/getegid have no preconditions.
        let (uid, gid) = unsafe { (libc::geteuid(), libc::getegid()) };
        args.extend(["--user".into(), format!("{uid}:{gid}")]);
    }
    args.extend(
        [
            "-e",
            "HOME=/tmp",
            "-e",
            "COMPOSER_HOME=/tmp/composer",
            "-e",
            "npm_config_cache=/tmp/npm",
            "-e",
            "npm_config_update_notifier=false",
        ]
        .map(String::from),
    );
    args.extend([
        "--mount".into(),
        format!(
            "type=bind,source={},target=/work",
            crate::generate::plain_path(directory)
        ),
        "-w".into(),
        "/work".into(),
        spec.image.clone(),
    ]);
    args.extend(spec.args.iter().cloned());
    args
}

fn checked(request: &ScaffoldRequest) -> Result<(Spec, std::path::PathBuf), String> {
    let spec = spec(request)?;
    let root = crate::generate::root_of(&request.path)?;
    crate::generate::check_mount_path(&root)?;
    if !crate::generate::folder_is_empty(&request.path)? {
        return Err(tr!(
            "A pasta não está vazia. O instalador só roda em pasta vazia.",
            "The folder is not empty. The installer only runs in an empty folder."
        ));
    }
    Ok((spec, root))
}

/// What would run, without running it.
pub fn plan(request: &ScaffoldRequest) -> Result<ScaffoldPlan, String> {
    let (spec, root) = checked(request)?;
    let command = std::iter::once("docker".to_string())
        .chain(
            docker_args(&spec, &root, "autodock-scaffold")
                .iter()
                .map(|a| quote(a)),
        )
        .collect::<Vec<_>>()
        .join(" ");
    let mut notes = spec.notes.clone();
    notes.push(tr!(
        "Requer Docker em execução e acesso à internet. A primeira vez baixa imagens.",
        "Requires a running Docker and internet access. The first run downloads images."
    ));
    notes.push(tr!(
        "A pasta é a única montada no container, e só ela é alterada.",
        "The folder is the only one mounted in the container, and only it is changed."
    ));
    Ok(ScaffoldPlan {
        title: spec.title.clone(),
        image: spec.image.clone(),
        command,
        notes,
    })
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

fn drain(
    mut pipe: impl Read + Send + 'static,
    tail: Arc<Mutex<Vec<u8>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        while let Ok(count) = pipe.read(&mut buffer) {
            if count == 0 {
                break;
            }
            let mut tail = tail.lock().unwrap();
            tail.extend_from_slice(&buffer[..count]);
            if tail.len() > OUTPUT_TAIL * 2 {
                let cut = tail.len() - OUTPUT_TAIL;
                tail.drain(..cut);
            }
        }
    })
}

/// Runs the official installer. Blocks until it finishes or times out.
pub fn run(request: &ScaffoldRequest) -> Result<ScaffoldReport, String> {
    let (spec, root) = checked(request)?;
    let docker = crate::doctor::resolve("docker").ok_or_else(|| {
        tr!(
            "Docker não foi encontrado. Instale e inicie o Docker Desktop.",
            "Docker was not found. Install and start Docker Desktop."
        )
    })?;
    let started = Instant::now();
    let container = format!(
        "autodock-scaffold-{}-{}",
        std::process::id(),
        started.elapsed().as_nanos()
            ^ (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
                & 0xffff_ffff)
    );
    let mut command = Command::new(docker.clone());
    command
        .args(docker_args(&spec, &root, &container))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| {
        tr!(
            "Não foi possível iniciar o Docker: {e}",
            "Could not start Docker: {e}"
        )
    })?;
    let tail = Arc::new(Mutex::new(Vec::new()));
    let readers = [
        drain(child.stdout.take().expect("piped stdout"), tail.clone()),
        drain(child.stderr.take().expect("piped stderr"), tail.clone()),
    ];
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() >= TIMEOUT => {
                let mut cleanup = Command::new(&docker);
                cleanup
                    .args(["rm", "-f", &container])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cleanup.creation_flags(0x0800_0000);
                }
                let _ = cleanup.status();
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(e) => {
                return Err(tr!(
                    "Falha ao acompanhar o Docker: {e}",
                    "Failed to follow Docker: {e}"
                ))
            }
        }
    };
    for reader in readers {
        let _ = reader.join();
    }
    let bytes = tail.lock().unwrap().clone();
    let start = bytes.len().saturating_sub(OUTPUT_TAIL);
    let output = strip_ansi(&String::from_utf8_lossy(&bytes[start..]));
    let duration_ms = started.elapsed().as_millis();
    let (success, detail) = match status {
        None => (
            false,
            tr!(
                "Tempo esgotado (20 minutos). O container foi removido.",
                "Timed out (20 minutes). The container was removed."
            ),
        ),
        Some(status) if !status.success() => (
            false,
            tr!(
                "O instalador terminou com erro. Veja a saída abaixo.",
                "The installer finished with an error. See the output below."
            ),
        ),
        Some(_) if !root.join(spec.marker).is_file() => (
            false,
            tr!(
                "O instalador terminou, mas {} não foi criado.",
                "The installer finished, but {} was not created.",
                spec.marker
            ),
        ),
        Some(_) => (true, tr!("Projeto criado.", "Project created.")),
    };
    Ok(ScaffoldReport {
        success,
        duration_ms,
        output,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(dir: &tempfile::TempDir, framework: Framework, name: &str) -> ScaffoldRequest {
        ScaffoldRequest {
            path: dir.path().to_string_lossy().into(),
            framework,
            project_name: name.into(),
            runtime_version: None,
        }
    }

    #[test]
    fn project_names_are_strictly_validated() {
        for ok in ["a", "meu-app", "app2", "a-b-c"] {
            assert!(valid_project_name(ok), "{ok}");
        }
        for bad in [
            "",
            "A",
            "1app",
            "-a",
            "a-",
            "a--b",
            "a b",
            "a;b",
            "a$(x)",
            "../x",
            "é",
            &"a".repeat(41),
        ] {
            assert!(!valid_project_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn plan_shows_the_exact_fixed_command_and_never_runs() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan(&request(&dir, Framework::Laravel, "loja")).unwrap();
        assert_eq!(plan.image, "composer:2");
        assert!(plan
            .command
            .starts_with("docker run --rm --name autodock-scaffold"));
        assert!(plan
            .command
            .contains("composer:2 create-project laravel/laravel ."));
        assert!(plan.command.contains("type=bind,source="));
        assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn django_name_is_a_safe_python_identifier() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan(&request(&dir, Framework::Django, "meu-site")).unwrap();
        assert!(plan.command.contains("startproject meu_site ."));
        for bad in ["django", "test", "app"] {
            assert!(plan_err(&dir, Framework::Django, bad));
        }
    }

    fn plan_err(dir: &tempfile::TempDir, framework: Framework, name: &str) -> bool {
        plan(&request(dir, framework, name)).is_err()
    }

    #[test]
    fn rejects_injection_non_empty_folders_and_bad_versions() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["x; rm -rf /", "$(id)", "a`b`", "x' y"] {
            assert!(plan_err(&dir, Framework::ViteReact, name), "{name}");
        }
        let mut req = request(&dir, Framework::ViteVue, "ok");
        req.runtime_version = Some("22\nRUN evil".into());
        assert!(plan(&req).is_err());
        fs::write(dir.path().join("keep.txt"), "x").unwrap();
        assert!(plan_err(&dir, Framework::Laravel, "ok"));
        assert!(serde_json::from_str::<ScaffoldRequest>(
            r#"{"path":".","framework":"laravel","projectName":"a","command":"x"}"#
        )
        .is_err());
    }

    #[test]
    fn refuses_paths_that_would_break_the_mount_option() {
        let parent = tempfile::tempdir().unwrap();
        for folder in ["a,b", "a,target=etc", "a,readonly"] {
            let path = parent.path().join(folder);
            fs::create_dir(&path).unwrap();
            let request = ScaffoldRequest {
                path: path.to_string_lossy().into(),
                framework: Framework::Laravel,
                project_name: "ok".into(),
                runtime_version: None,
            };
            let english = plan(&request).unwrap_err();
            assert!(english.contains("commas, quotes"), "{folder}: {english}");
            let portuguese =
                crate::i18n::with_lang(crate::i18n::Lang::Pt, || plan(&request).unwrap_err());
            assert!(portuguese.contains("vírgula"), "{folder}: {portuguese}");
        }
    }

    #[test]
    fn strips_terminal_escape_sequences() {
        assert_eq!(
            strip_ansi("\u{1b}[32;1mDONE\u{1b}[39;22m\r\nok"),
            "DONE\nok"
        );
    }

    use std::fs;
}
