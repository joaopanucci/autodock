use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};
#[derive(Parser)]
#[command(
    name = "autodock",
    version,
    about = "Análise local de projetos e geração de Docker, com preview"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
    /// Cria um projeto com o instalador oficial do framework, dentro do Docker.
    /// Sem --run apenas mostra o comando.
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
    /// Gera Dockerfile e docker-compose.yml. Sem --write apenas mostra o preview.
    Generate {
        path: PathBuf,
        /// nodejs, spring-boot, python ou laravel
        #[arg(long)]
        stack: String,
        #[arg(long)]
        port: u16,
        /// postgres, mysql, mongodb, redis, rabbitmq, adminer, mailhog (repetível)
        #[arg(long = "service")]
        services: Vec<String>,
        #[arg(long)]
        runtime_version: Option<String>,
        /// npm, pnpm ou yarn (Node.js)
        #[arg(long)]
        package_manager: Option<String>,
        /// Script npm usado como comando de início (Node.js)
        #[arg(long)]
        start_script: Option<String>,
        /// fastapi, django ou flask (Python)
        #[arg(long)]
        python_framework: Option<String>,
        #[arg(long)]
        python_module: Option<String>,
        /// Cria um projeto inicial em pasta vazia antes dos arquivos Docker
        #[arg(long = "new")]
        new_project: bool,
        /// Grava os arquivos novos; nunca sobrescreve existentes
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
    let request: autodock_core::generate::GenerateRequest =
        serde_json::from_value(request).map_err(|e| format!("Opção inválida: {e}"))?;
    let plan = autodock_core::generate::preview(&request)?;
    if !write {
        for file in &plan.files {
            let state = if file.exists {
                " (já existe, não será gravado)"
            } else {
                ""
            };
            println!("===== {}{state} =====\n{}", file.name, file.content);
        }
        for note in &plan.notes {
            eprintln!("Nota: {note}");
        }
        eprintln!("Preview apenas. Use --write para gravar.");
        return Ok(());
    }
    let names: Vec<String> = plan.files.iter().map(|f| f.name.clone()).collect();
    let report = autodock_core::generate::apply(&request, &names)?;
    for name in &report.written {
        println!("Gravado: {name}");
    }
    for skipped in &report.skipped {
        eprintln!("Ignorado: {} ({})", skipped.name, skipped.reason);
    }
    Ok(())
}
fn main() -> ExitCode {
    match Cli::parse().command {
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
            .map_err(|e| format!("Opção inválida: {e}"))
            .and_then(|request| {
                let plan = autodock_core::scaffold::plan(&request)?;
                println!("{}\n{}", plan.title, plan.command);
                for note in &plan.notes {
                    eprintln!("Nota: {note}");
                }
                if !run {
                    eprintln!("Preview apenas. Use --run para executar.");
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
                    eprintln!("Erro: {error}");
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
                eprintln!("Erro: {error}");
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
                    println!("Projeto: {}", report.project);
                    if report.components.is_empty() {
                        println!("Nenhuma tecnologia reconhecida.");
                    }
                    for c in report.components {
                        println!("- {}: {} [{}]", c.path, c.technology, c.evidence.join(", "));
                    }
                    for warning in report.warnings {
                        eprintln!("Aviso: {warning}");
                    }
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                if json {
                    println!("{}", serde_json::json!({"error": error}));
                } else {
                    eprintln!("Erro: {error}");
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
                    println!("Nota: {note}");
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
