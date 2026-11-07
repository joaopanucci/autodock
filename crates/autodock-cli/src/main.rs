use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};
#[derive(Parser)]
#[command(
    name = "autodock",
    version,
    about = "Local project analysis and Docker generation, with preview"
)]
struct Cli {
    /// Language of the messages: en or pt (default: from LC_ALL/LANG, English otherwise)
    #[arg(long, global = true)]
    lang: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

/// `say!("português", "english")` picks the message for the current language.
macro_rules! say {
    ($pt:literal, $en:literal $(, $($arg:tt)*)?) => {
        if autodock_core::i18n::current() == autodock_core::i18n::Lang::Pt {
            format!($pt $(, $($arg)*)?)
        } else {
            format!($en $(, $($arg)*)?)
        }
    };
}
#[derive(Subcommand)]
enum Commands {
    Scan {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Finds the applications of a monorepo or microservice project (read-only).
    Apps {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// One Dockerfile per app plus a compose file at the root. Without --write it only previews.
    Multi {
        path: PathBuf,
        /// postgres, mysql, mongodb, redis, rabbitmq, adminer, mailhog (repeatable)
        #[arg(long = "service")]
        services: Vec<String>,
        /// Only these apps, by the names `autodock apps` prints (default: the suggested ones)
        #[arg(long = "app")]
        only: Vec<String>,
        #[arg(long)]
        project_name: Option<String>,
        /// Writes the new files; never overwrites existing ones
        #[arg(long)]
        write: bool,
    },
    /// Runs the compose project of a folder: up, stop, down (keeps volumes) or status.
    Compose {
        path: PathBuf,
        /// up, stop, down or status
        action: String,
        /// Compose file at the project root (default: the only one found)
        #[arg(long)]
        file: Option<String>,
        /// Confirms that you want to run it (required for up, stop and down)
        #[arg(long)]
        yes: bool,
    },
    /// Creates a project with the framework's official installer, inside Docker.
    /// Without --run it only shows the command.
    Scaffold {
        path: PathBuf,
        /// laravel, django, vite-react, vite-vue, nextjs ou nestjs
        #[arg(long)]
        framework: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        run: bool,
    },
    /// Generates Dockerfile and docker-compose.yml. Without --write it only previews.
    Generate {
        path: PathBuf,
        /// nodejs, spring-boot, python, laravel, go, dotnet ou rails
        #[arg(long)]
        stack: String,
        #[arg(long)]
        port: u16,
        /// postgres, mysql, mongodb, redis, rabbitmq, adminer, mailhog (repeatable)
        #[arg(long = "service")]
        services: Vec<String>,
        #[arg(long)]
        runtime_version: Option<String>,
        /// npm, pnpm ou yarn (Node.js)
        #[arg(long)]
        package_manager: Option<String>,
        /// npm script used as the start command (Node.js)
        #[arg(long)]
        start_script: Option<String>,
        /// fastapi, django ou flask (Python)
        #[arg(long)]
        python_framework: Option<String>,
        #[arg(long)]
        python_module: Option<String>,
        /// Creates a starter project in an empty folder before the Docker files
        #[arg(long = "new")]
        new_project: bool,
        /// Writes the new files; never overwrites existing ones
        #[arg(long)]
        write: bool,
    },
}
fn enum_from(value: &str) -> Result<serde_json::Value, String> {
    Ok(serde_json::Value::String(value.to_string()))
}
#[allow(clippy::too_many_arguments)]
fn generate_command(
    path: PathBuf,
    stack: String,
    port: u16,
    services: Vec<String>,
    runtime_version: Option<String>,
    package_manager: Option<String>,
    start_script: Option<String>,
    python_framework: Option<String>,
    python_module: Option<String>,
    new_project: bool,
    write: bool,
) -> Result<(), String> {
    let request = serde_json::json!({
        "path": path.to_string_lossy(),
        "stack": enum_from(&stack)?,
        "port": port,
        "services": services,
        "runtimeVersion": runtime_version,
        "packageManager": package_manager,
        "startScript": start_script,
        "pythonFramework": python_framework,
        "pythonModule": python_module,
        "newProject": new_project,
    });
    let request: autodock_core::generate::GenerateRequest = serde_json::from_value(request)
        .map_err(|e| say!("Opção inválida: {e}", "Invalid option: {e}"))?;
    let plan = autodock_core::generate::preview(&request)?;
    if !write {
        for file in &plan.files {
            let state = if file.exists {
                &say!(
                    " (já existe, não será gravado)",
                    " (already exists, will not be written)"
                )
            } else {
                ""
            };
            println!("===== {}{state} =====\n{}", file.name, file.content);
        }
        for note in &plan.notes {
            eprintln!("{}", say!("Nota: {note}", "Note: {note}"));
        }
        eprintln!(
            "{}",
            say!(
                "Preview apenas. Use --write para gravar.",
                "Preview only. Use --write to write the files."
            )
        );
        return Ok(());
    }
    let names: Vec<String> = plan.files.iter().map(|f| f.name.clone()).collect();
    let report = autodock_core::generate::apply(&request, &names)?;
    for name in &report.written {
        println!("{}", say!("Gravado: {name}", "Written: {name}"));
    }
    for skipped in &report.skipped {
        eprintln!(
            "{}",
            say!(
                "Ignorado: {} ({})",
                "Skipped: {} ({})",
                skipped.name,
                skipped.reason
            )
        );
    }
    Ok(())
}
fn print_apps(report: &autodock_core::apps::AppsReport) {
    println!("{}", say!("Projeto: {}", "Project: {}", report.project));
    if report.apps.is_empty() {
        println!("{}", say!("Nenhum app encontrado.", "No apps found."));
    }
    for app in &report.apps {
        let mark = if app.selected { "x" } else { " " };
        println!(
            "[{mark}] {}  {}  {:?}/{}  porta {}  ({})",
            app.name,
            app.path,
            app.stack,
            app.framework.as_deref().unwrap_or("-"),
            app.port,
            app.evidence
        );
        if let Some(note) = &app.note {
            println!("      {note}");
        }
    }
    for skipped in &report.skipped {
        println!(
            "{}",
            say!(
                "ignorado: {} ({})",
                "skipped: {} ({})",
                skipped.path,
                skipped.reason
            )
        );
    }
    if let Some(compose) = &report.existing_compose {
        println!(
            "{}",
            say!(
                "Já existe {compose}: ele não será tocado.",
                "{compose} already exists: it will not be touched."
            )
        );
    }
    for warning in &report.warnings {
        eprintln!("{}", say!("Aviso: {warning}", "Warning: {warning}"));
    }
}

fn multi_command(
    path: PathBuf,
    services: Vec<String>,
    only: Vec<String>,
    project_name: Option<String>,
    write: bool,
) -> Result<(), String> {
    let path_text = path.to_string_lossy().to_string();
    let report = autodock_core::apps::detect_apps(&path_text)?;
    let chosen: Vec<&autodock_core::apps::AppCandidate> = if only.is_empty() {
        report.apps.iter().filter(|a| a.selected).collect()
    } else {
        only.iter()
            .map(|name| {
                report.apps.iter().find(|a| &a.name == name).ok_or_else(|| {
                    say!(
                        "App desconhecido: {name}. Use autodock apps para ver os nomes.",
                        "Unknown app: {name}. Use autodock apps to see the names."
                    )
                })
            })
            .collect::<Result<_, _>>()?
    };
    if chosen.is_empty() {
        return Err(say!("Nenhum app para gerar.", "No app to generate."));
    }
    let apps: Vec<serde_json::Value> = chosen
        .iter()
        .map(|a| {
            serde_json::json!({
                "path": a.path,
                "name": a.name,
                "stack": a.stack,
                "role": a.role,
                "framework": a.framework,
                "port": a.port,
                "packageManager": a.package_manager,
                "startScript": a.start_script,
                "pythonFramework": a.python_framework,
            })
        })
        .collect();
    let request: autodock_core::multi::MultiRequest = serde_json::from_value(serde_json::json!({
        "path": path_text,
        "projectName": project_name,
        "apps": apps,
        "services": services,
    }))
    .map_err(|e| say!("Opção inválida: {e}", "Invalid option: {e}"))?;
    let plan = autodock_core::multi::preview(&request)?;
    if !write {
        for file in &plan.files {
            let state = if file.exists {
                say!(
                    " (já existe, não será gravado)",
                    " (already exists, will not be written)"
                )
            } else {
                String::new()
            };
            println!("===== {}{state} =====\n{}", file.name, file.content);
        }
        for note in &plan.notes {
            eprintln!("{}", say!("Nota: {note}", "Note: {note}"));
        }
        eprintln!(
            "{}",
            say!(
                "Preview apenas. Use --write para gravar.",
                "Preview only. Use --write to write the files."
            )
        );
        return Ok(());
    }
    let names: Vec<String> = plan.files.iter().map(|f| f.name.clone()).collect();
    let applied = autodock_core::multi::apply(&request, &names)?;
    for name in &applied.written {
        println!("{}", say!("Gravado: {name}", "Written: {name}"));
    }
    for skipped in &applied.skipped {
        eprintln!(
            "{}",
            say!(
                "Ignorado: {} ({})",
                "Skipped: {} ({})",
                skipped.name,
                skipped.reason
            )
        );
    }
    Ok(())
}

fn compose_command(
    path: PathBuf,
    action: String,
    file: Option<String>,
    yes: bool,
) -> Result<bool, String> {
    let path_text = path.to_string_lossy().to_string();
    let file = match file {
        Some(file) => file,
        None => {
            let mut found = autodock_core::compose_run::compose_files(&path_text)?;
            match found.len() {
                1 => found.remove(0),
                0 => {
                    return Err(say!(
                        "Nenhum arquivo compose no projeto.",
                        "No compose file in the project."
                    ))
                }
                _ => {
                    return Err(say!(
                        "Há mais de um arquivo compose ({}): escolha com --file.",
                        "There is more than one compose file ({}): pick one with --file.",
                        found.join(", ")
                    ))
                }
            }
        }
    };
    if action == "status" {
        let status = autodock_core::compose_run::compose_status(&path_text, &file)?;
        if !status.detail.is_empty() {
            eprintln!("{}", status.detail);
        }
        for service in &status.services {
            println!(
                "{}\t{}\t{}\t{}",
                service.service,
                service.state,
                service.health.as_deref().unwrap_or("-"),
                service.ports.join(", ")
            );
        }
        return Ok(status.status == autodock_core::ToolStatus::Available);
    }
    let action = match action.as_str() {
        "up" => autodock_core::compose_run::ComposeAction::Up,
        "stop" => autodock_core::compose_run::ComposeAction::Stop,
        "down" => autodock_core::compose_run::ComposeAction::Down,
        other => {
            return Err(say!(
                "Ação desconhecida: {other} (use up, stop, down ou status).",
                "Unknown action: {other} (use up, stop, down or status)."
            ))
        }
    };
    let request = autodock_core::compose_run::ComposeRequest {
        path: path_text,
        file,
        action,
        confirmed: yes,
    };
    if !yes {
        eprintln!(
            "{}",
            say!(
                "Isto executa comandos no seu Docker. Repita com --yes para confirmar.",
                "This runs commands in your Docker. Repeat with --yes to confirm."
            )
        );
    }
    let report = autodock_core::compose_run::run_compose(request)?;
    println!("$ {}", report.command);
    if !report.output.is_empty() {
        println!("{}", report.output);
    }
    if !report.detail.is_empty() {
        eprintln!("{}", report.detail);
    }
    Ok(report.succeeded)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let lang = match cli.lang.as_deref() {
        Some(value) => autodock_core::i18n::Lang::parse(Some(value)),
        None => autodock_core::i18n::Lang::from_environment(),
    };
    autodock_core::i18n::with_lang(lang, move || run(cli.command))
}

fn run(command: Commands) -> ExitCode {
    match command {
        Commands::Scaffold {
            path,
            framework,
            name,
            run,
        } => {
            let request = serde_json::from_value::<autodock_core::scaffold::ScaffoldRequest>(
                serde_json::json!({
                    "path": path.to_string_lossy(),
                    "framework": framework,
                    "projectName": name,
                }),
            )
            .map_err(|e| say!("Opção inválida: {e}", "Invalid option: {e}"))
            .and_then(|request| {
                let plan = autodock_core::scaffold::plan(&request)?;
                println!("{}\n{}", plan.title, plan.command);
                for note in &plan.notes {
                    eprintln!("{}", say!("Nota: {note}", "Note: {note}"));
                }
                if !run {
                    eprintln!(
                        "{}",
                        say!(
                            "Preview apenas. Use --run para executar.",
                            "Preview only. Use --run to run it."
                        )
                    );
                    return Ok(true);
                }
                let report = autodock_core::scaffold::run(&request)?;
                println!("{}\n{}", report.output, report.detail);
                Ok(report.success)
            });
            match request {
                Ok(true) => ExitCode::SUCCESS,
                Ok(false) => ExitCode::FAILURE,
                Err(error) => {
                    eprintln!("{}", say!("Erro: {error}", "Error: {error}"));
                    ExitCode::FAILURE
                }
            }
        }
        Commands::Generate {
            path,
            stack,
            port,
            services,
            runtime_version,
            package_manager,
            start_script,
            python_framework,
            python_module,
            new_project,
            write,
        } => match generate_command(
            path,
            stack,
            port,
            services,
            runtime_version,
            package_manager,
            start_script,
            python_framework,
            python_module,
            new_project,
            write,
        ) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{}", say!("Erro: {error}", "Error: {error}"));
                ExitCode::FAILURE
            }
        },
        Commands::Apps { path, json } => {
            match autodock_core::apps::detect_apps(&path.to_string_lossy()) {
                Ok(report) => {
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&report).expect("report serializable")
                        );
                    } else {
                        print_apps(&report);
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    if json {
                        println!("{}", serde_json::json!({"error": error}));
                    } else {
                        eprintln!("{}", say!("Erro: {error}", "Error: {error}"));
                    }
                    ExitCode::FAILURE
                }
            }
        }
        Commands::Multi {
            path,
            services,
            only,
            project_name,
            write,
        } => match multi_command(path, services, only, project_name, write) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{}", say!("Erro: {error}", "Error: {error}"));
                ExitCode::FAILURE
            }
        },
        Commands::Compose {
            path,
            action,
            file,
            yes,
        } => match compose_command(path, action, file, yes) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(error) => {
                eprintln!("{}", say!("Erro: {error}", "Error: {error}"));
                ExitCode::FAILURE
            }
        },
        Commands::Scan { path, json } => match autodock_core::scan(&path) {
            Ok(report) => {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).expect("report serializable")
                    );
                } else {
                    println!("{}", say!("Projeto: {}", "Project: {}", report.project));
                    if report.components.is_empty() {
                        println!(
                            "{}",
                            say!(
                                "Nenhuma tecnologia reconhecida.",
                                "No technology recognized."
                            )
                        );
                    }
                    for c in report.components {
                        println!("- {}: {} [{}]", c.path, c.technology, c.evidence.join(", "));
                    }
                    for warning in report.warnings {
                        eprintln!("{}", say!("Aviso: {warning}", "Warning: {warning}"));
                    }
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                if json {
                    println!("{}", serde_json::json!({"error": error}));
                } else {
                    eprintln!("{}", say!("Erro: {error}", "Error: {error}"));
                }
                ExitCode::FAILURE
            }
        },
        Commands::Doctor { json } => {
            let report = autodock_core::doctor();
            let healthy = report
                .checks
                .iter()
                .all(|c| c.status == autodock_core::ToolStatus::Available);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("report serializable")
                );
            } else {
                for c in report.checks {
                    println!(
                        "{}: {} | {} {} {}",
                        c.name,
                        c.status.label(),
                        c.stdout,
                        c.stderr,
                        c.detail
                    );
                }
                for note in report.notes {
                    println!("{}", say!("Nota: {note}", "Note: {note}"));
                }
            }
            if healthy {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            }
        }
    }
}
