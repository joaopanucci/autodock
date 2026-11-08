//! `autodock java …`: inspect a legacy Maven project and upgrade a copy of it to the
//! current JDK and Spring Boot (inside Docker, after an explicit `--yes`).
use autodock_core::{
    java,
    java_convert::{self, ConvertOptions, ConvertRequest},
    java_map,
    java_upgrade::{self, UpgradeRequest},
};
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum JavaCommand {
    /// Maps a folder with Java code, whatever its layout (no pom.xml needed): build files,
    /// where the sources are, libraries, main classes, the Java level. Read-only.
    Map {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Shows the Java level and Spring Boot version of a Maven project (read-only, runs nothing).
    Inspect {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Turns a folder with Java code but no pom.xml into a Maven project, in a copy next to it
    /// (or in --into): the Maven layout, a generated pom.xml and the libraries as dependencies.
    /// Without --yes it only shows the plan. The folder itself is never modified.
    Convert {
        path: PathBuf,
        /// Folder that will hold the copy (default: next to the project)
        #[arg(long)]
        into: Option<PathBuf>,
        #[arg(long)]
        group_id: Option<String>,
        #[arg(long)]
        artifact_id: Option<String>,
        #[arg(long)]
        version: Option<String>,
        /// Java level the project was written for (default: what its files declare, else 8)
        #[arg(long)]
        java_level: Option<u32>,
        /// Makes the copy
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Shows what an upgrade would do. Runs nothing and writes nothing.
    Plan {
        path: PathBuf,
        /// Folder that will hold the upgraded copy (default: next to the project)
        #[arg(long)]
        into: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Creates an upgraded copy next to the project (or in --into), inside Docker. The project
    /// itself is never modified. Needs --yes.
    Upgrade {
        path: PathBuf,
        /// Folder that will hold the upgraded copy (default: next to the project)
        #[arg(long)]
        into: Option<PathBuf>,
        /// Confirms that you want to run it
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
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

fn print_map(map: &java_map::JavaMap) {
    println!(
        "{}",
        say!(
            "Pasta Java: {} (layout: {})",
            "Java folder: {} (layout: {})",
            map.name,
            map.layout
        )
    );
    let counts = &map.counts;
    println!(
        "  {}",
        say!(
            "{} arquivo(s) .java, {} .jar, {} .xml, {} .properties, {} outros",
            "{} .java file(s), {} .jar, {} .xml, {} .properties, {} others",
            counts.java,
            counts.jars,
            counts.xml,
            counts.properties,
            counts.other
        )
    );
    for system in &map.build_systems {
        println!("  Build: {} ({})", system.kind, system.files.join(", "));
    }
    if let Some(level) = &map.java_level {
        println!("  Java: {} ({})", level.declared, level.source);
    }
    for root in &map.source_roots {
        println!(
            "  {} {}: {} {} ({} {})",
            say!("Fontes", "Sources"),
            if root.dir.is_empty() { "." } else { &root.dir },
            root.files,
            say!("arquivo(s)", "file(s)"),
            root.kind,
            if root.top_package.is_empty() {
                say!("sem package comum", "no common package")
            } else {
                root.top_package.clone()
            }
        );
    }
    for main in &map.main_classes {
        println!("  main: {} ({})", main.class, main.file);
    }
    for app in &map.web_apps {
        println!(
            "  {}: {}",
            say!("Aplicação web", "Web application"),
            if app.is_empty() { "." } else { app }
        );
    }
    if map.library_count > 0 {
        println!(
            "  {}: {}",
            say!("Bibliotecas", "Libraries"),
            map.library_count
        );
        for library in map.libraries.iter().take(10) {
            println!(
                "    {} ({})",
                library.path,
                library.version.as_deref().unwrap_or("?")
            );
        }
        if map.library_count > 10 {
            println!("    …");
        }
    }
    for note in &map.notes {
        println!("- {note}");
    }
}

fn print_convert_plan(plan: &java_convert::ConvertPlan) {
    println!(
        "{}",
        say!(
            "Conversão de {} para Maven ({}, Java {})",
            "Conversion of {} to Maven ({}, Java {})",
            plan.name,
            plan.packaging,
            plan.java_level
        )
    );
    println!("  {}:{}:{}", plan.group_id, plan.artifact_id, plan.version);
    let counts = &plan.counts;
    println!(
        "  {}",
        say!(
            "{} fonte(s), {} de teste, {} recurso(s), {} arquivo(s) web, {} movido(s) para a pasta do package",
            "{} source(s), {} test(s), {} resource(s), {} web file(s), {} moved to their package folder",
            counts.main_sources,
            counts.test_sources,
            counts.main_resources + counts.test_resources,
            counts.webapp_files,
            counts.moved_to_their_package
        )
    );
    for dependency in &plan.dependencies {
        println!(
            "  + {}:{}:{} ({}){}",
            dependency.group_id,
            dependency.artifact_id,
            dependency.version,
            dependency.scope,
            if dependency.known {
                String::new()
            } else {
                say!(" — jar local em lib/", " — local jar in lib/")
            }
        );
    }
    println!(
        "{}: {}",
        say!("A cópia vai para", "The copy goes to"),
        plan.destination
    );
    for warning in &plan.warnings {
        println!("{} {warning}", say!("Atenção:", "Warning:"));
    }
    for note in &plan.notes {
        println!("- {note}");
    }
}

fn print_project(project: &java::JavaProject) {
    println!(
        "{}",
        say!(
            "Projeto Java: {} ({})",
            "Java project: {} ({})",
            project.name,
            project.build_tool
        )
    );
    let level = project
        .java_declared
        .clone()
        .unwrap_or_else(|| say!("não declarado", "not declared"));
    println!("  Java: {level}");
    if let Some(boot) = &project.spring_boot {
        println!("  Spring Boot: {boot}");
    }
    if !project.modules.is_empty() {
        println!(
            "  {}: {}",
            say!("Módulos", "Modules"),
            project.modules.join(", ")
        );
    }
    for api in &project.jdk_apis {
        let kind = if api.fixable {
            say!(
                "o AutoDock adiciona a dependência",
                "AutoDock adds the dependency"
            )
        } else {
            say!("precisa de reescrita manual", "needs a manual rewrite")
        };
        println!(
            "  {} — {} {} ({kind})",
            api.name,
            api.files,
            say!("arquivo(s)", "file(s)")
        );
    }
    match &project.unsupported_reason {
        Some(reason) => println!(
            "  {}: {reason}",
            say!("Upgrade indisponível", "Upgrade unavailable")
        ),
        None if project.upgrade_available => println!(
            "  {}",
            say!(
                "Upgrade pelo AutoDock: disponível",
                "Upgrade with AutoDock: available"
            )
        ),
        None => println!("  {}", say!("Já está atualizado.", "Already up to date.")),
    }
}

fn print_plan(plan: &java_upgrade::UpgradePlan) {
    println!(
        "{}",
        say!(
            "Upgrade de {} — Java {} → {}",
            "Upgrade of {} — Java {} → {}",
            plan.project.name,
            plan.from_java
                .map_or_else(|| "?".to_string(), |l| l.to_string()),
            plan.to_java
        )
    );
    if let (Some(from), Some(to)) = (&plan.from_spring_boot, &plan.to_spring_boot) {
        println!("Spring Boot {from} → {to}");
    }
    println!("{}", say!("Passos:", "Steps:"));
    for (index, hop) in plan.hops.iter().enumerate() {
        let compile = if hop.checked {
            say!(" (compila depois)", " (compiles after)")
        } else {
            String::new()
        };
        println!("  {}. {}{compile}", index + 1, hop.label);
    }
    for fixup in &plan.fixups {
        println!(
            "  + {} {} ({})",
            fixup.dependency,
            if fixup.module.is_empty() {
                String::new()
            } else {
                format!("[{}]", fixup.module)
            },
            fixup.because
        );
    }
    println!(
        "{}: {}",
        say!("A cópia vai para", "The copy goes to"),
        plan.destination
    );
    for warning in &plan.warnings {
        println!("{} {warning}", say!("Atenção:", "Warning:"));
    }
    for note in &plan.notes {
        println!("- {note}");
    }
    println!("{}", say!("Comando:", "Command:"));
    println!("  {}", plan.command);
}

fn print_report(report: &java_upgrade::UpgradeReport) {
    if report.succeeded {
        println!(
            "{}",
            say!(
                "Cópia atualizada criada em {}",
                "Upgraded copy created at {}",
                report.destination
            )
        );
    } else {
        println!(
            "{}",
            say!("O upgrade não terminou.", "The upgrade did not finish.")
        );
    }
    for hop in &report.hops {
        let mark = match hop.status.as_str() {
            "ok" => "ok",
            "failed" => "FAILED",
            _ => "-",
        };
        println!("  [{mark}] {}", hop.label);
    }
    for fixup in &report.fixups {
        println!("  + {} ({})", fixup.dependency, fixup.because);
    }
    match &report.tests {
        Some(t) => println!(
            "{}",
            say!(
                "Testes na cópia: {} executados, {} falha(s), {} erro(s), {} ignorado(s) — {}",
                "Tests on the copy: {} run, {} failure(s), {} error(s), {} skipped — {}",
                t.run,
                t.failures,
                t.errors,
                t.skipped,
                if t.passed {
                    say!("passaram", "passed")
                } else {
                    say!("há falhas", "failures")
                }
            )
        ),
        None if report.succeeded => println!(
            "{}",
            say!("Nenhum teste foi executado.", "No test was run.")
        ),
        None => {}
    }
    println!(
        "{}",
        say!(
            "Arquivos: {} alterado(s), {} novo(s), {} removido(s)",
            "Files: {} changed, {} new, {} removed",
            report.changes.modified.len(),
            report.changes.added.len(),
            report.changes.removed.len()
        )
    );
    if !report.detail.is_empty() {
        println!("{}", report.detail);
    }
    if !report.succeeded && !report.output.is_empty() {
        println!("---\n{}\n---", report.output);
    }
    for step in &report.next_steps {
        println!("- {step}");
    }
}

/// Returns whether the command succeeded (used for the exit code).
pub fn run(command: JavaCommand) -> Result<bool, String> {
    match command {
        JavaCommand::Map { path, json } => {
            let map = java_map::map(&path_text(&path))?;
            if json {
                print_json(&map);
            } else {
                print_map(&map);
            }
            Ok(true)
        }
        JavaCommand::Inspect { path, json } => {
            let project = java::inspect(&path_text(&path))?.ok_or_else(|| {
                say!(
                    "Esta pasta não é um projeto Java com pom.xml.",
                    "This folder is not a Java project with a pom.xml."
                )
            })?;
            if json {
                print_json(&project);
            } else {
                print_project(&project);
            }
            Ok(true)
        }
        JavaCommand::Convert {
            path,
            into,
            group_id,
            artifact_id,
            version,
            java_level,
            yes,
            json,
        } => {
            let options = ConvertOptions {
                into: into.as_deref().map(path_text),
                group_id,
                artifact_id,
                version,
                java_level,
            };
            if !yes {
                let plan = java_convert::plan(&path_text(&path), &options)?;
                if json {
                    print_json(&plan);
                } else {
                    print_convert_plan(&plan);
                    println!(
                        "{}",
                        say!(
                            "Nada foi gravado. Rode de novo com --yes para criar a cópia.",
                            "Nothing was written. Run again with --yes to create the copy."
                        )
                    );
                }
                return Ok(true);
            }
            let report = java_convert::run(ConvertRequest {
                path: path_text(&path),
                options,
                confirmed: true,
            })?;
            if json {
                print_json(&report);
            } else {
                println!(
                    "{}",
                    say!(
                        "Cópia Maven criada em {} ({} arquivo(s))",
                        "Maven copy created at {} ({} file(s))",
                        report.destination,
                        report.files_written
                    )
                );
                for step in &report.next_steps {
                    println!("- {step}");
                }
            }
            Ok(true)
        }
        JavaCommand::Plan { path, into, json } => {
            let into = into.as_deref().map(path_text);
            let plan = java_upgrade::plan_in(&path_text(&path), into.as_deref())?;
            if json {
                print_json(&plan);
            } else {
                print_plan(&plan);
            }
            Ok(true)
        }
        JavaCommand::Upgrade {
            path,
            into,
            yes,
            json,
        } => {
            let report = java_upgrade::run(UpgradeRequest {
                path: path_text(&path),
                destination: into.as_deref().map(path_text),
                confirmed: yes,
            })?;
            if json {
                print_json(&report);
            } else {
                print_report(&report);
            }
            Ok(report.succeeded)
        }
    }
}
