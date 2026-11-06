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
    /// Creates a project with the framework's official installer, inside Docker.
    /// Without --run it only shows the command.
    Scaffold {
        path: PathBuf,
        /// laravel, django, vite-react ou vite-vue
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
        /// nodejs, spring-boot, python ou laravel
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
