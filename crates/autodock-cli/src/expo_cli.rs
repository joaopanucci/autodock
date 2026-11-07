//! `autodock expo …`: detect the SDK of an Expo project, watch for a newer one and
//! upgrade the project (inside Docker, after an explicit `--yes`).
use autodock_core::{
    expo,
    expo_upgrade::{self, UpgradeRequest},
    expo_watch::{self, WatchState},
};
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ExpoCommand {
    /// Shows the SDK of an Expo project (read-only, runs nothing).
    Inspect {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Lists the watched projects and what is known about the latest SDK (no network).
    List {
        #[arg(long)]
        json: bool,
    },
    /// Looks up the latest stable SDK on the npm registry and compares it with the watched projects.
    Check {
        #[arg(long)]
        json: bool,
    },
    /// Adds a project to the watched list (kept in ~/.autodock/expo.json; only the path).
    Watch { path: PathBuf },
    /// Removes a project from the watched list.
    Unwatch { path: PathBuf },
    /// Stops alerting about the SDK that is currently the newest for a project.
    Dismiss { path: PathBuf },
    /// Turns the automatic check when the app opens on or off (at most once a day).
    AutoCheck {
        /// on or off
        value: String,
    },
    /// Shows what an upgrade would do. Runs nothing and writes nothing.
    Plan {
        path: PathBuf,
        /// Target SDK (default: the next one)
        #[arg(long)]
        to: Option<u32>,
        #[arg(long)]
        json: bool,
    },
    /// Upgrades the project to a newer SDK inside Docker. Needs --yes.
    Upgrade {
        path: PathBuf,
        /// Target SDK (default: the next one)
        #[arg(long)]
        to: Option<u32>,
        /// Confirms that you want to run it
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Lists the backups made before upgrades.
    Backups {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Puts the package.json and lock file of a backup back. Needs --yes.
    Restore {
        path: PathBuf,
        /// Backup id, as `backups` prints it
        id: String,
        #[arg(long)]
        yes: bool,
    },
}

fn path_text(path: &std::path::Path) -> String {
    path.to_string_lossy().to_string()
}

fn print_json<T: serde::Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("report serializable")
    );
}

fn print_state(state: &WatchState) {
    match &state.latest {
        Some(latest) => println!(
            "{}",
            say!(
                "Último SDK estável: {} ({})",
                "Latest stable SDK: {} ({})",
                latest.latest_sdk,
                latest.latest_version
            )
        ),
        None => println!(
            "{}",
            say!(
                "Último SDK estável: ainda não consultado (use `autodock expo check`).",
                "Latest stable SDK: not looked up yet (use `autodock expo check`)."
            )
        ),
    }
    println!(
        "{}",
        say!(
            "Checar ao abrir: {}",
            "Check when the app opens: {}",
            if state.check_on_start {
                say!("ligado", "on")
            } else {
                say!("desligado", "off")
            }
        )
    );
    if let Some(error) = &state.check_error {
        eprintln!("{}", say!("Aviso: {error}", "Warning: {error}"));
    }
    if state.projects.is_empty() {
        println!(
            "{}",
            say!(
                "Nenhum projeto acompanhado (use `autodock expo watch <pasta>`).",
                "No watched projects (use `autodock expo watch <folder>`)."
            )
        );
    }
    for item in &state.projects {
        let status = match (&item.project, &item.update) {
            (None, _) => say!(
                "pasta não encontrada ou não é mais um projeto Expo",
                "folder not found or no longer an Expo project"
            ),
            (Some(_), Some(update)) => {
                let tail = if update.dismissed {
                    say!(" (aviso dispensado)", " (alert dismissed)")
                } else {
                    String::new()
                };
                say!(
                    "SDK {} → {} disponível, {} versão(ões) atrás{tail}",
                    "SDK {} → {} available, {} version(s) behind{tail}",
                    update.current_sdk,
                    update.latest_sdk,
                    update.steps_behind
                )
            }
            (Some(project), None) => match project.sdk {
                Some(sdk) => say!("SDK {sdk}, em dia", "SDK {sdk}, up to date"),
                None => say!("SDK desconhecido", "unknown SDK"),
            },
        };
        println!("- {} ({}): {status}", item.name, item.path);
    }
}

fn print_project(project: &expo::ExpoProject) {
    let sdk = project
        .sdk
        .map(|sdk| sdk.to_string())
        .unwrap_or_else(|| "?".into());
    println!(
        "{}",
        say!(
            "Projeto Expo: {} — SDK {sdk}",
            "Expo project: {} — SDK {sdk}",
            project.name
        )
    );
    let installed = project
        .installed_version
        .as_deref()
        .map(|v| say!(", instalado {v}", ", installed {v}"))
        .unwrap_or_default();
    println!("  expo {}{installed}", project.declared_version);
    if let Some(react_native) = &project.react_native {
        println!("  react-native {react_native}");
    }
    println!(
        "  {}: {}",
        say!("Gerenciador", "Package manager"),
        project.package_manager.as_deref().unwrap_or("-")
    );
    for folder in &project.native_folders {
        let kind = if folder.ignored_by_git {
            say!("gerada (no .gitignore)", "generated (in .gitignore)")
        } else {
            say!("versionada", "under version control")
        };
        println!("  {}/ — {kind}", folder.name);
    }
    match &project.unsupported_reason {
        Some(reason) => println!(
            "  {}: {reason}",
            say!("Upgrade indisponível", "Upgrade unavailable")
        ),
        None => println!(
            "  {}",
            say!(
                "Upgrade pelo AutoDock: disponível",
                "Upgrade with AutoDock: available"
            )
        ),
    }
}

fn print_plan(plan: &expo_upgrade::UpgradePlan) {
    println!(
        "{}",
        say!(
            "Upgrade de {} — SDK {} → {}",
            "Upgrade of {} — SDK {} → {}",
            plan.project.name,
            plan.from_sdk,
            plan.to_sdk
        )
    );
    println!("{}", say!("Passos:", "Steps:"));
    for (index, step) in plan.steps.iter().enumerate() {
        println!("  {}. {step}", index + 1);
    }
    println!(
        "{}: {}",
        say!("Arquivos alterados", "Files changed"),
        plan.files_touched.join(", ")
    );
    println!("{}: {}", say!("Imagem Docker", "Docker image"), plan.image);
    for warning in &plan.warnings {
        println!("{}", say!("Atenção: {warning}", "Warning: {warning}"));
    }
    for note in &plan.notes {
        println!("{}", say!("Nota: {note}", "Note: {note}"));
    }
    for link in &plan.links {
        println!("{}: {}", link.label, link.url);
    }
}

fn print_report(report: &expo_upgrade::UpgradeReport) {
    if report.succeeded {
        println!(
            "{}",
            say!(
                "Upgrade concluído: SDK {} → {} ({} s).",
                "Upgrade finished: SDK {} → {} ({} s).",
                report.from_sdk,
                report.to_sdk,
                report.duration_ms / 1000
            )
        );
        println!(
            "{}",
            say!("Dependências alteradas:", "Changed dependencies:")
        );
        for change in &report.changes {
            println!(
                "  {}: {} → {}",
                change.name,
                change.from.as_deref().unwrap_or("-"),
                change.to.as_deref().unwrap_or("-")
            );
        }
        if !report.doctor.is_empty() {
            println!("\nexpo-doctor:\n{}", report.doctor);
        }
        if !report.next_steps.is_empty() {
            println!("\n{}", say!("Próximos passos:", "Next steps:"));
            for step in &report.next_steps {
                println!("  - {step}");
            }
        }
        println!(
            "\n{}",
            say!(
                "Para desfazer: autodock expo restore <pasta> {} --yes",
                "To undo: autodock expo restore <folder> {} --yes",
                report.backup_id
            )
        );
    } else {
        println!(
            "{}",
            say!(
                "O upgrade falhou (SDK {} → {}).",
                "The upgrade failed (SDK {} → {}).",
                report.from_sdk,
                report.to_sdk
            )
        );
        if !report.output.is_empty() {
            println!("{}", report.output);
        }
    }
    if !report.detail.is_empty() {
        eprintln!("{}", report.detail);
    }
}

/// Runs a subcommand; `Ok(false)` is a failure that was already explained.
pub fn run(command: ExpoCommand) -> Result<bool, String> {
    match command {
        ExpoCommand::Inspect { path, json } => {
            let found = expo::inspect(&path_text(&path))?;
            if json {
                print_json(&found);
            } else if let Some(project) = &found {
                print_project(project);
            } else {
                eprintln!(
                    "{}",
                    say!(
                        "Esta pasta não é um projeto Expo.",
                        "This folder is not an Expo project."
                    )
                );
            }
            Ok(found.is_some())
        }
        ExpoCommand::List { json } => {
            let state = expo_watch::state()?;
            if json {
                print_json(&state);
            } else {
                print_state(&state);
            }
            Ok(true)
        }
        ExpoCommand::Check { json } => {
            let state = expo_watch::refresh_latest()?;
            if json {
                print_json(&state);
            } else {
                print_state(&state);
            }
            Ok(state.check_error.is_none())
        }
        ExpoCommand::Watch { path } => {
            print_state(&expo_watch::add_project(&path_text(&path))?);
            Ok(true)
        }
        ExpoCommand::Unwatch { path } => {
            print_state(&expo_watch::remove_project(&path_text(&path))?);
            Ok(true)
        }
        ExpoCommand::Dismiss { path } => {
            print_state(&expo_watch::dismiss_update(&path_text(&path))?);
            Ok(true)
        }
        ExpoCommand::AutoCheck { value } => {
            let enabled = match value.as_str() {
                "on" => true,
                "off" => false,
                _ => return Err(say!("Use on ou off.", "Use on or off.")),
            };
            print_state(&expo_watch::set_check_on_start(enabled)?);
            Ok(true)
        }
        ExpoCommand::Plan { path, to, json } => {
            let plan = expo_upgrade::plan(&path_text(&path), to)?;
            if json {
                print_json(&plan);
            } else {
                print_plan(&plan);
            }
            Ok(true)
        }
        ExpoCommand::Upgrade {
            path,
            to,
            yes,
            json,
        } => {
            let path = path_text(&path);
            let plan = expo_upgrade::plan(&path, to)?;
            if !json {
                print_plan(&plan);
            }
            if !yes {
                return Err(say!(
                    "Isto executa comandos no seu Docker e altera o package.json e o lock. Repita com --yes para confirmar.",
                    "This runs commands in your Docker and changes package.json and the lock file. Repeat with --yes to confirm."
                ));
            }
            let report = expo_upgrade::run(UpgradeRequest {
                path,
                target_sdk: plan.to_sdk,
                confirmed: yes,
            })?;
            if json {
                print_json(&report);
            } else {
                print_report(&report);
            }
            Ok(report.succeeded)
        }
        ExpoCommand::Backups { path, json } => {
            let list = expo_upgrade::backups(&path_text(&path))?;
            if json {
                print_json(&list);
            } else if list.is_empty() {
                println!("{}", say!("Nenhum backup.", "No backups."));
            }
            if !json {
                for backup in &list {
                    println!(
                        "{}\tSDK {} → {}\t{}\t{}{}",
                        backup.id,
                        backup.from_sdk,
                        backup.to_sdk,
                        backup.status,
                        backup.files.join(", "),
                        if backup.modified_since_upgrade {
                            say!(
                                "\t(package.json mudou depois do upgrade)",
                                "\t(package.json changed after the upgrade)"
                            )
                        } else {
                            String::new()
                        }
                    );
                }
            }
            Ok(true)
        }
        ExpoCommand::Restore { path, id, yes } => {
            if !yes {
                eprintln!(
                    "{}",
                    say!(
                        "Isto sobrescreve o package.json e o lock com o backup. Repita com --yes para confirmar.",
                        "This overwrites package.json and the lock file with the backup. Repeat with --yes to confirm."
                    )
                );
            }
            let report = expo_upgrade::restore(&path_text(&path), &id, yes)?;
            for name in &report.restored {
                println!("{}: {name}", say!("Restaurado", "Restored"));
            }
            for name in &report.removed {
                println!("{}: {name}", say!("Removido", "Removed"));
            }
            println!("{}", report.detail);
            Ok(true)
        }
    }
}
