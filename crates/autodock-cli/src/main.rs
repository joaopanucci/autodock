use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};
#[derive(Parser)]
#[command(
    name = "autodock",
    version,
    about = "Análise local de projetos, somente leitura"
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
}
fn main() -> ExitCode {
    match Cli::parse().command {
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
