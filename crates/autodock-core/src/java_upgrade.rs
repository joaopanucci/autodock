//! Upgrading a legacy Java (Maven) project to the current JDK and Spring Boot.
//!
//! The project folder is **never modified**. AutoDock copies it next to the original (or into
//! a folder the person chooses)
//! (`<name>-java25`), adds the dependencies for the APIs the JDK stopped shipping, and
//! walks the copy up one OpenRewrite recipe at a time, **inside a Docker container**:
//! Spring Boot to 2.7, Java 17, Spring Boot 3.5, Spring Boot 4.0, Java 25. After each
//! step the copy is compiled, so a failure says exactly which step broke. At the end
//! the tests run. Maven and the project's build plugins never run on the host.
use crate::{
    doctor::{resolve, ToolStatus},
    expo_upgrade::{host_ids, quote, PlanLink},
    generate::{self, regular_file},
    java::{
        self, boot_line, hops_for, scan_modules, Hop, JavaProject, ModuleScan, TARGET_BOOT,
        TARGET_JAVA,
    },
    memory_capture::{self, Output},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

/// The JDK 25 image with Maven. The rewrite plugin and the recipe libraries below are
/// pinned: this exact combination was run end to end against legacy projects.
const MAVEN_IMAGE: &str = "maven:3.9-eclipse-temurin-25";
const REWRITE_PLUGIN: &str = "org.openrewrite.maven:rewrite-maven-plugin:6.46.1";
const RECIPE_LIBRARIES: &str =
    "org.openrewrite.recipe:rewrite-spring:6.37.1,org.openrewrite.recipe:rewrite-migrate-java:3.42.1";
/// Named volume with the downloaded Maven artifacts, so the next upgrade does not
/// download them again. It never mixes with the Maven cache of the user's computer.
const CACHE_VOLUME: &str = "autodock-maven-cache";
const UPGRADE_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const OUTPUT_LINES: usize = 1200;
const MAX_COPY_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_COPY_FILES: usize = 100_000;
const MAX_LISTED_CHANGES: usize = 300;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HopPlan {
    pub id: String,
    pub label: String,
    pub recipe: String,
    /// The copy is compiled after this step.
    pub checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixupPlan {
    /// Module folder, `""` for the root.
    pub module: String,
    /// `group:artifact:version`.
    pub dependency: String,
    /// What the sources use that made it necessary.
    pub because: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradePlan {
    pub schema_version: u32,
    pub project: JavaProject,
    pub from_java: Option<u32>,
    pub to_java: u32,
    pub from_spring_boot: Option<String>,
    pub to_spring_boot: Option<String>,
    pub image: String,
    pub hops: Vec<HopPlan>,
    pub fixups: Vec<FixupPlan>,
    /// The folder the upgraded copy will be created in.
    pub destination: String,
    /// The exact `docker` command, for display.
    pub command: String,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
    pub links: Vec<PlanLink>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpgradeRequest {
    pub path: String,
    /// The folder that will hold the upgraded copy (default: the project's parent folder).
    #[serde(default)]
    pub destination: Option<String>,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HopResult {
    pub id: String,
    pub label: String,
    /// `ok`, `failed` or `skipped` (an earlier step failed).
    pub status: String,
    /// The copy compiled after this step (`None`: not checked or not reached).
    pub compiled: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSummary {
    pub run: u32,
    pub failures: u32,
    pub errors: u32,
    pub skipped: u32,
    pub passed: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Changes {
    pub modified: Vec<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// More changes than the lists hold.
    pub truncated: bool,
    pub total: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeReport {
    pub schema_version: u32,
    pub succeeded: bool,
    pub destination: String,
    pub from_java: Option<u32>,
    pub to_java: u32,
    pub from_spring_boot: Option<String>,
    pub to_spring_boot: Option<String>,
    pub hops: Vec<HopResult>,
    pub failed_hop: Option<String>,
    pub fixups: Vec<FixupPlan>,
    pub changes: Changes,
    /// The result of the project's own tests, run on the upgraded copy.
    pub tests: Option<TestSummary>,
    /// Tail of the container output when something went wrong.
    pub output: String,
    pub detail: String,
    pub duration_ms: u128,
    pub next_steps: Vec<String>,
}

// ───────────────────────────── plan ─────────────────────────────

struct HopSpec {
    hop: Hop,
    id: &'static str,
    recipe: &'static str,
}

fn spec(hop: Hop) -> HopSpec {
    let (id, recipe) = match hop {
        Hop::Boot27 => (
            "spring-boot-2.7",
            "org.openrewrite.java.spring.boot2.UpgradeSpringBoot_2_7",
        ),
        Hop::Java17 => ("java-17", "org.openrewrite.java.migrate.UpgradeToJava17"),
        Hop::Boot35 => (
            "spring-boot-3.5",
            "org.openrewrite.java.spring.boot3.UpgradeSpringBoot_3_5",
        ),
        Hop::Boot40 => (
            "spring-boot-4.0",
            "org.openrewrite.java.spring.boot4.UpgradeSpringBoot_4_0",
        ),
        Hop::Java25 => ("java-25", "org.openrewrite.java.migrate.UpgradeToJava25"),
    };
    HopSpec { hop, id, recipe }
}

fn label(hop: Hop) -> String {
    match hop {
        Hop::Boot27 => tr!(
            "Spring Boot para 2.7 (inclui JUnit 4 → 5)",
            "Spring Boot to 2.7 (includes JUnit 4 → 5)"
        ),
        Hop::Java17 => tr!(
            "Java 17 (compilador, plugins e APIs removidas do JDK)",
            "Java 17 (compiler, plugins and APIs removed from the JDK)"
        ),
        Hop::Boot35 => tr!(
            "Spring Boot para 3.5 (javax → jakarta)",
            "Spring Boot to 3.5 (javax → jakarta)"
        ),
        Hop::Boot40 => tr!("Spring Boot para 4.0", "Spring Boot to 4.0"),
        Hop::Java25 => tr!("Java 25 (a LTS atual)", "Java 25 (the current LTS)"),
    }
}

fn hop_plans(java: Option<u32>, boot: Option<(u32, u32)>) -> Vec<HopPlan> {
    let mut level = java.unwrap_or(0);
    hops_for(java, boot)
        .into_iter()
        .map(|hop| {
            match hop {
                Hop::Java17 => level = 17,
                Hop::Java25 => level = TARGET_JAVA,
                _ => {}
            }
            let spec = spec(hop);
            HopPlan {
                id: spec.id.into(),
                label: label(spec.hop),
                recipe: spec.recipe.into(),
                // Before Java 17 the old build plugins may not run on the new JDK: only the
                // steps that leave the project on Java 17 or newer are compiled.
                checked: level >= 17,
            }
        })
        .collect()
}

fn artifact_text((group, artifact, version): java::Artifact) -> String {
    format!("{group}:{artifact}:{version}")
}

/// The dependencies the sources of each module need, one per artifact.
fn fixups_for(scans: &[ModuleScan]) -> Vec<FixupPlan> {
    let mut plans = vec![];
    for module in scans.iter().filter(|m| m.packaging != "pom") {
        let mut seen = BTreeSet::new();
        for (name, (_, api)) in &module.apis {
            for artifact in api.artifacts {
                if seen.insert(*artifact) {
                    plans.push(FixupPlan {
                        module: module.dir.clone(),
                        dependency: artifact_text(*artifact),
                        because: (*name).to_string(),
                    });
                }
            }
        }
    }
    plans
}

fn docker_args(copy: &Path, script: &str) -> Vec<String> {
    vec![
        "run".into(),
        "--rm".into(),
        "-e".into(),
        "HOME=/tmp".into(),
        "--mount".into(),
        format!(
            "type=bind,source={},target=/work",
            generate::plain_path(copy)
        ),
        "--mount".into(),
        format!("type=volume,source={CACHE_VOLUME},target=/m2"),
        "-w".into(),
        "/work".into(),
        MAVEN_IMAGE.into(),
        "sh".into(),
        "-c".into(),
        script.into(),
    ]
}

/// The shell script run in the container. Only constants and integers reach it.
///
/// `reactor`: a multi-module project. `runNoFork` does not build the reactor, so a module that
/// depends on a sibling could not be resolved. Running the `compile` phase first, with the
/// compiler switched off (`-Dmaven.main.skip`), makes Maven register the siblings without
/// compiling the old code with a build setup that is about to be replaced.
fn script(hops: &[HopPlan], ids: Option<(u32, u32)>, reactor: bool) -> String {
    // On Linux the container runs as root: whatever it wrote in the copy goes back to the user.
    let owner = match ids {
        Some((uid, gid)) => format!("trap 'chown -R {uid}:{gid} /work 2>/dev/null' EXIT; "),
        None => String::new(),
    };
    let prepare = if reactor {
        "-Dmaven.main.skip=true compile"
    } else {
        ""
    };
    let mut text = format!(
        "{owner}M='mvn -B -ntp -Dmaven.repo.local=/m2'; \
hop() {{ id=\"$1\"; recipe=\"$2\"; check=\"$3\"; \
if $M {prepare} {REWRITE_PLUGIN}:runNoFork -Drewrite.recipeArtifactCoordinates={RECIPE_LIBRARIES} -Drewrite.failOnInvalidActiveRecipes=true -Drewrite.activeRecipes=\"$recipe\" >/tmp/hop.log 2>&1; \
then echo \"HOP $id ok\"; else echo \"HOP $id failed\"; grep -E 'ERROR' /tmp/hop.log | head -n 40; return 1; fi; \
if [ \"$check\" = 1 ]; then if $M -q test-compile >/tmp/check.log 2>&1; \
then echo \"CHECK $id ok\"; else echo \"CHECK $id failed\"; grep -E 'ERROR' /tmp/check.log | head -n 40; return 1; fi; fi; }}; "
    );
    for hop in hops {
        text.push_str(&format!(
            "hop {} {} {} || exit 1; ",
            hop.id,
            hop.recipe,
            u8::from(hop.checked)
        ));
    }
    text.push_str(
        "echo TESTS; $M test >/tmp/test.log 2>&1; code=$?; \
grep -E '^(\\[[A-Z]+\\] )?Tests run:.*Skipped: [0-9]+$' /tmp/test.log | sed 's/^/TESTLINE /'; \
if [ $code -ne 0 ]; then grep -E 'ERROR|FAIL' /tmp/test.log | head -n 40; fi; echo \"TESTS_EXIT $code\"",
    );
    text
}

/// The folder that will hold the copy: the one the person chose, or the project's parent.
/// A chosen folder must exist, be a real folder and lie outside the project (a copy made
/// inside the project would be copied into itself).
fn holding_folder(root: &Path, into: Option<&str>) -> Result<PathBuf, String> {
    let Some(into) = into.map(str::trim).filter(|v| !v.is_empty()) else {
        return root.parent().map(Path::to_path_buf).ok_or_else(|| {
            tr!(
                "Esta pasta não tem uma pasta pai para receber a cópia.",
                "This folder has no parent folder to hold the copy."
            )
        });
    };
    let folder = Path::new(into)
        .canonicalize()
        .map_err(|e| format!("{into}: {e}"))?;
    if !folder.is_dir() {
        return Err(tr!(
            "A pasta escolhida para a cópia não é uma pasta: {}",
            "The folder chosen for the copy is not a folder: {}",
            folder.display()
        ));
    }
    if folder.starts_with(root) {
        return Err(tr!(
            "Escolha uma pasta fora do projeto: uma cópia dentro dele seria copiada para dentro de si mesma.",
            "Choose a folder outside the project: a copy inside it would be copied into itself."
        ));
    }
    Ok(folder)
}

/// Where the upgraded copy goes: `<name>-java25` in the holding folder, never over something
/// that exists.
fn destination_for(root: &Path, into: Option<&str>) -> Result<PathBuf, String> {
    let parent = holding_folder(root, into)?;
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    for attempt in 1..=20 {
        let suffix = if attempt == 1 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let candidate = parent.join(format!("{name}-java{TARGET_JAVA}{suffix}"));
        if fs::symlink_metadata(&candidate).is_err() {
            return Ok(candidate);
        }
    }
    Err(tr!(
        "Já existem 20 cópias atualizadas nessa pasta: apague as que não precisa mais.",
        "There are already 20 upgraded copies in that folder: delete the ones you no longer need."
    ))
}

pub fn plan(path: &str) -> Result<UpgradePlan, String> {
    plan_in(path, None)
}

/// Like [`plan`], with the folder that will hold the upgraded copy (default: next to the project).
pub fn plan_in(path: &str, into: Option<&str>) -> Result<UpgradePlan, String> {
    let project = java::inspect(path)?.ok_or_else(|| {
        tr!(
            "Esta pasta não é um projeto Java com pom.xml.",
            "This folder is not a Java project with a pom.xml."
        )
    })?;
    if let Some(reason) = &project.unsupported_reason {
        return Err(reason.clone());
    }
    let boot = project.spring_boot.as_deref().and_then(boot_line);
    if project.spring_boot.is_some() && boot.is_none() {
        return Err(tr!(
            "Não foi possível ler a versão do Spring Boot ({}).",
            "Could not read the Spring Boot version ({}).",
            project.spring_boot.clone().unwrap_or_default()
        ));
    }
    let hops = hop_plans(project.java_level, boot);
    if hops.is_empty() {
        return Err(tr!(
            "O projeto já está no Java {TARGET_JAVA} e no Spring Boot mais novo que o AutoDock migra.",
            "The project is already on Java {TARGET_JAVA} and the newest Spring Boot AutoDock migrates."
        ));
    }
    let root = generate::root_of(path)?;
    generate::check_mount_path(&root)?;
    let destination = destination_for(&root, into)?;
    generate::check_mount_path(&destination)?;
    let scans = scan_modules(&root);
    let fixups = fixups_for(&scans);

    let mut warnings = vec![];
    for module in &scans {
        for (name, (files, api)) in &module.apis {
            if api.artifacts.is_empty() {
                warnings.push(tr!(
                    "{name} ({files} arquivo(s)) não existe mais no JDK e não tem substituto automático: esse código precisa ser reescrito à mão.",
                    "{name} ({files} file(s)) is gone from the JDK and has no automatic replacement: that code has to be rewritten by hand."
                ));
            }
        }
    }
    warnings.sort();
    warnings.dedup();
    if boot.is_none() && pom_uses_spring_framework(&root) {
        warnings.push(tr!(
            "O projeto usa Spring Framework sem Spring Boot: só o nível do Java é atualizado, o Spring Framework fica como está.",
            "The project uses Spring Framework without Spring Boot: only the Java level is upgraded, Spring Framework stays as it is."
        ));
    }
    if scans.iter().all(|m| !has_tests(&root, m)) {
        warnings.push(tr!(
            "O projeto não tem testes (src/test): \"compila\" é a única garantia que a cópia dá.",
            "The project has no tests (src/test): \"it compiles\" is the only guarantee the copy gives."
        ));
    }

    let mut notes = vec![
        tr!(
            "O projeto original não é alterado: o AutoDock cria uma cópia (ao lado dele ou na pasta que você escolher) e faz o upgrade nela.",
            "The original project is not modified: AutoDock creates a copy (next to it or in the folder you choose) and upgrades the copy."
        ),
        tr!(
            "Tudo roda dentro de um container Docker. O Maven e os plugins do projeto nunca rodam no seu computador.",
            "Everything runs inside a Docker container. Maven and the project's plugins never run on your computer."
        ),
        tr!(
            "A primeira vez baixa a imagem do Maven com JDK {TARGET_JAVA} e as dependências (pode levar vários minutos). As dependências ficam num volume do Docker ({CACHE_VOLUME}) para as próximas vezes.",
            "The first time it downloads the Maven image with JDK {TARGET_JAVA} and the dependencies (it can take several minutes). The dependencies are kept in a Docker volume ({CACHE_VOLUME}) for next time."
        ),
        tr!(
            "Arquivos .env, a pasta .git, node_modules, target e links simbólicos não são copiados.",
            "The .env files, the .git folder, node_modules, target and symbolic links are not copied."
        ),
        tr!(
            "Mudanças que quebram a API não são todas corrigidas automaticamente: o passo que falhar aparece no relatório, com os erros do compilador.",
            "Breaking changes are not all fixed automatically: the step that fails shows in the report, with the compiler errors."
        ),
    ];
    if project.packaging == "war" {
        notes.push(tr!(
            "O projeto gera um WAR: o servidor de aplicação (Tomcat, etc.) também precisa suportar Jakarta EE depois do upgrade.",
            "The project builds a WAR: the application server (Tomcat, etc.) must also support Jakarta EE after the upgrade."
        ));
    }
    if !project.modules.is_empty() {
        notes.push(tr!(
            "Projeto multimódulo: {} módulo(s) serão atualizados juntos.",
            "Multi-module project: {} module(s) will be upgraded together.",
            project.modules.len()
        ));
    }

    let command_script = script(&hops, host_ids(), !project.modules.is_empty());
    let command = std::iter::once("docker".to_string())
        .chain(
            docker_args(&destination, &command_script)
                .iter()
                .map(|a| quote(a)),
        )
        .collect::<Vec<_>>()
        .join(" ");
    let mut links = vec![PlanLink {
        label: tr!("Receitas do OpenRewrite", "OpenRewrite recipes"),
        url: "https://docs.openrewrite.org/recipes/java/migrate".into(),
    }];
    if boot.is_some() {
        links.push(PlanLink {
            label: tr!("Migração para o Spring Boot 4", "Migrating to Spring Boot 4"),
            url: "https://github.com/spring-projects/spring-boot/wiki/Spring-Boot-4.0-Migration-Guide".into(),
        });
    }
    Ok(UpgradePlan {
        schema_version: 1,
        from_java: project.java_level,
        to_java: TARGET_JAVA,
        from_spring_boot: project.spring_boot.clone(),
        to_spring_boot: boot.map(|_| format!("{}.{}.x", TARGET_BOOT.0, TARGET_BOOT.1)),
        project,
        image: MAVEN_IMAGE.into(),
        hops,
        fixups,
        destination: generate::plain_path(&destination),
        command,
        notes,
        warnings,
        links,
    })
}

fn pom_uses_spring_framework(root: &Path) -> bool {
    fs::read_to_string(root.join("pom.xml"))
        .is_ok_and(|text| text.contains("<groupId>org.springframework</groupId>"))
}

fn has_tests(root: &Path, module: &ModuleScan) -> bool {
    let base = if module.dir.is_empty() {
        root.to_path_buf()
    } else {
        root.join(&module.dir)
    };
    fs::read_dir(base.join("src/test")).is_ok_and(|mut entries| entries.next().is_some())
}

// ───────────────────────────── copying ─────────────────────────────

/// Folders never copied, at any depth.
const SKIPPED_DIRS: [&str; 6] = [".git", ".idea", "node_modules", ".gradle", ".svn", ".hg"];

fn skipped_file(name: &str) -> bool {
    let env = name == ".env"
        || (name.starts_with(".env.")
            && !["example", "sample", "template"]
                .iter()
                .any(|s| name.ends_with(&format!(".{s}"))));
    env || name.ends_with(".iml") || name == ".DS_Store" || name == "Thumbs.db"
}

#[derive(Default)]
struct Listing {
    dirs: Vec<String>,
    files: Vec<(String, PathBuf, u64)>,
    symlinks: usize,
    secrets: usize,
}

fn list_tree(dir: &Path, prefix: &str, out: &mut Listing) -> Result<(), String> {
    // `target` is the build output of a Maven folder; a Java package called `target` is not.
    let maven_folder = regular_file(dir, "pom.xml");
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if meta.file_type().is_symlink() {
            out.symlinks += 1;
        } else if meta.is_dir() {
            if SKIPPED_DIRS.contains(&name.as_str()) || (maven_folder && name == "target") {
                continue;
            }
            out.dirs.push(relative.clone());
            list_tree(&entry.path(), &relative, out)?;
        } else if meta.is_file() {
            if skipped_file(&name) {
                if name.starts_with(".env") {
                    out.secrets += 1;
                }
                continue;
            }
            out.files.push((relative, entry.path(), meta.len()));
            if out.files.len() > MAX_COPY_FILES {
                return Err(tr!(
                    "O projeto tem arquivos demais para copiar (mais de {MAX_COPY_FILES}).",
                    "The project has too many files to copy (more than {MAX_COPY_FILES})."
                ));
            }
        }
    }
    Ok(())
}

struct Copied {
    symlinks: usize,
    secrets: usize,
}

/// Copies the project into a folder that does not exist yet. On error the new folder is removed.
fn copy_project(root: &Path, destination: &Path) -> Result<Copied, String> {
    let mut listing = Listing::default();
    list_tree(root, "", &mut listing)?;
    let bytes: u64 = listing.files.iter().map(|(_, _, size)| size).sum();
    if bytes > MAX_COPY_BYTES {
        return Err(tr!(
            "O projeto é grande demais para copiar ({} MB, o limite é {} MB).",
            "The project is too large to copy ({} MB, the limit is {} MB).",
            bytes / 1_000_000,
            MAX_COPY_BYTES / 1_000_000
        ));
    }
    // `create_dir` fails when the folder exists: nothing is ever copied over something.
    fs::create_dir(destination).map_err(|e| format!("{}: {e}", destination.display()))?;
    let result = (|| {
        for dir in &listing.dirs {
            fs::create_dir_all(destination.join(dir)).map_err(|e| e.to_string())?;
        }
        for (relative, source, _) in &listing.files {
            fs::copy(source, destination.join(relative)).map_err(|e| format!("{relative}: {e}"))?;
        }
        Ok::<(), String>(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(destination);
        return Err(error);
    }
    Ok(Copied {
        symlinks: listing.symlinks,
        secrets: listing.secrets,
    })
}

// ───────────────────────────── fixups ─────────────────────────────

/// Adds `<dependency>` blocks to a pom, skipping the artifacts it already declares.
/// Returns the new text, or `None` when there is nothing to add.
pub(crate) fn add_dependencies(
    text: &str,
    artifacts: &[java::Artifact],
) -> Result<Option<(String, Vec<java::Artifact>)>, String> {
    let doc = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            ..Default::default()
        },
    )
    .map_err(|e| tr!("pom.xml inválido: {e}", "Invalid pom.xml: {e}"))?;
    let project = doc.root_element();
    let dependencies = project
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "dependencies");
    let declared: BTreeSet<String> = dependencies
        .iter()
        .flat_map(|d| d.children().filter(|n| n.is_element()))
        .filter_map(|d| {
            d.children()
                .find(|n| n.is_element() && n.tag_name().name() == "artifactId")
                .and_then(|n| n.text())
                .map(|t| t.trim().to_string())
        })
        .collect();
    let missing: Vec<java::Artifact> = artifacts
        .iter()
        .filter(|(_, a, _)| !declared.contains(*a))
        .copied()
        .collect();
    if missing.is_empty() {
        return Ok(None);
    }
    let blocks = |indent: &str, unit: &str| -> String {
        missing
            .iter()
            .map(|(group, artifact, version)| {
                format!(
                    "{indent}<dependency>\n{indent}{unit}<groupId>{group}</groupId>\n{indent}{unit}<artifactId>{artifact}</artifactId>\n{indent}{unit}<version>{version}</version>\n{indent}</dependency>\n"
                )
            })
            .collect()
    };
    let line_indent = |position: usize| -> String {
        let start = text[..position].rfind('\n').map_or(0, |i| i + 1);
        text[start..position]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect()
    };
    let unit_for = |indent: &str| if indent.contains('\t') { "\t" } else { "  " };

    let mut out = text.to_string();
    match dependencies {
        Some(node) => {
            let range = node.range();
            let element = &text[range.clone()];
            let base = line_indent(range.start);
            let unit = unit_for(&base);
            if element.trim_end().ends_with("/>") {
                // <dependencies/>: open it.
                let replacement = format!(
                    "<dependencies>\n{}{base}</dependencies>",
                    blocks(&format!("{base}{unit}"), unit)
                );
                out.replace_range(range, &replacement);
            } else {
                let close = range.start
                    + element
                        .rfind("</")
                        .ok_or_else(|| tr!("pom.xml inválido.", "Invalid pom.xml."))?;
                let on_own_line = text[..close]
                    .rfind('\n')
                    .is_some_and(|i| text[i + 1..close].trim().is_empty());
                let insert_at = if on_own_line {
                    text[..close].rfind('\n').map_or(close, |i| i + 1)
                } else {
                    close
                };
                let body = blocks(&format!("{base}{unit}"), unit);
                let body = if on_own_line {
                    body
                } else {
                    format!("\n{body}{base}")
                };
                out.insert_str(insert_at, &body);
            }
        }
        None => {
            let close = text
                .rfind("</project")
                .ok_or_else(|| tr!("pom.xml inválido.", "Invalid pom.xml."))?;
            let base = line_indent(project.range().start);
            let unit = unit_for(&base);
            let on_own_line = text[..close]
                .rfind('\n')
                .is_some_and(|i| text[i + 1..close].trim().is_empty());
            let insert_at = if on_own_line {
                text[..close].rfind('\n').map_or(close, |i| i + 1)
            } else {
                close
            };
            let inner = format!("{base}{unit}");
            let block = format!(
                "{inner}<dependencies>\n{}{inner}</dependencies>\n",
                blocks(&format!("{inner}{unit}"), unit)
            );
            let block = if on_own_line {
                block
            } else {
                format!("\n{block}")
            };
            out.insert_str(insert_at, &block);
        }
    }
    Ok(Some((out, missing)))
}

/// Puts the dependencies the sources need into the pom of the module that uses them.
fn apply_fixups(copy: &Path, scans: &[ModuleScan]) -> Result<Vec<FixupPlan>, String> {
    let mut applied = vec![];
    for module in scans.iter().filter(|m| m.packaging != "pom") {
        let artifacts: Vec<java::Artifact> = module
            .apis
            .values()
            .flat_map(|(_, api)| api.artifacts.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if artifacts.is_empty() {
            continue;
        }
        let pom = if module.dir.is_empty() {
            copy.join("pom.xml")
        } else {
            copy.join(&module.dir).join("pom.xml")
        };
        let text = fs::read_to_string(&pom).map_err(|e| format!("{}: {e}", pom.display()))?;
        let Some((new_text, added)) = add_dependencies(&text, &artifacts)? else {
            continue;
        };
        let temporary = pom.with_extension("xml.autodock-tmp");
        fs::write(&temporary, new_text).map_err(|e| format!("{}: {e}", temporary.display()))?;
        fs::rename(&temporary, &pom).map_err(|e| format!("{}: {e}", pom.display()))?;
        for artifact in added {
            let because = module
                .apis
                .iter()
                .find(|(_, (_, api))| api.artifacts.contains(&artifact))
                .map(|(name, _)| (*name).to_string())
                .unwrap_or_default();
            applied.push(FixupPlan {
                module: module.dir.clone(),
                dependency: artifact_text(artifact),
                because,
            });
        }
    }
    Ok(applied)
}

// ───────────────────────────── running ─────────────────────────────

#[derive(Default)]
struct Parsed {
    hops: BTreeMap<String, bool>,
    checks: BTreeMap<String, bool>,
    tests: TestSummary,
    test_lines: u32,
    tests_exit: Option<i32>,
}

fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    for line in text.lines() {
        let line = line.trim_end();
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["HOP", id, status] => {
                parsed.hops.insert((*id).into(), *status == "ok");
            }
            ["CHECK", id, status] => {
                parsed.checks.insert((*id).into(), *status == "ok");
            }
            ["TESTS_EXIT", code] => parsed.tests_exit = code.parse().ok(),
            ["TESTLINE", rest @ ..] => {
                let numbers: Vec<u32> = rest
                    .join(" ")
                    .split(|c: char| !c.is_ascii_digit())
                    .filter(|n| !n.is_empty())
                    .filter_map(|n| n.parse().ok())
                    .collect();
                if let [run, failures, errors, skipped] = numbers[..] {
                    parsed.tests.run += run;
                    parsed.tests.failures += failures;
                    parsed.tests.errors += errors;
                    parsed.tests.skipped += skipped;
                    parsed.test_lines += 1;
                }
            }
            _ => {}
        }
    }
    parsed
}

fn remove_build_output(copy: &Path, scans: &[ModuleScan]) {
    for module in scans {
        let base = if module.dir.is_empty() {
            copy.to_path_buf()
        } else {
            copy.join(&module.dir)
        };
        let target = base.join("target");
        if fs::symlink_metadata(&target).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
            let _ = fs::remove_dir_all(&target);
        }
    }
}

/// What differs between the original and the upgraded copy.
fn compare(root: &Path, copy: &Path) -> Changes {
    let list = |base: &Path| {
        let mut listing = Listing::default();
        let _ = list_tree(base, "", &mut listing);
        listing
            .files
            .into_iter()
            .map(|(relative, path, _)| (relative, path))
            .collect::<BTreeMap<_, _>>()
    };
    let before = list(root);
    let after = list(copy);
    let mut changes = Changes::default();
    for (relative, path) in &after {
        match before.get(relative) {
            None => changes.added.push(relative.clone()),
            Some(original) if fs::read(original).ok() != fs::read(path).ok() => {
                changes.modified.push(relative.clone());
            }
            _ => {}
        }
    }
    for relative in before.keys().filter(|r| !after.contains_key(*r)) {
        changes.removed.push(relative.clone());
    }
    changes.total = changes.modified.len() + changes.added.len() + changes.removed.len();
    for list in [
        &mut changes.modified,
        &mut changes.added,
        &mut changes.removed,
    ] {
        if list.len() > MAX_LISTED_CHANGES {
            list.truncate(MAX_LISTED_CHANGES);
            changes.truncated = true;
        }
    }
    changes
}

pub fn run(request: UpgradeRequest) -> Result<UpgradeReport, String> {
    run_with(request, |args, timeout| {
        let Some(docker) = resolve("docker") else {
            return Output::missing();
        };
        let mut command = Command::new(docker);
        command.args(args);
        memory_capture::run(&mut command, timeout, OUTPUT_LINES)
    })
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

fn run_with(
    request: UpgradeRequest,
    runner: impl FnOnce(&[String], Duration) -> Output,
) -> Result<UpgradeReport, String> {
    if !request.confirmed {
        return Err(tr!(
            "Confirmação explícita obrigatória.",
            "Explicit confirmation required."
        ));
    }
    let plan = plan_in(&request.path, request.destination.as_deref())?;
    let root = generate::root_of(&request.path)?;
    let copy = PathBuf::from(&plan.destination);
    let copied = copy_project(&root, &copy)?;

    let scans = scan_modules(&copy);
    let fixups = match apply_fixups(&copy, &scans) {
        Ok(fixups) => fixups,
        Err(error) => {
            let _ = fs::remove_dir_all(&copy);
            return Err(error);
        }
    };
    let output = runner(
        &docker_args(
            &copy,
            &script(&plan.hops, host_ids(), !plan.project.modules.is_empty()),
        ),
        UPGRADE_TIMEOUT,
    );
    if output.status == ToolStatus::Missing {
        // Nothing ran: an untouched copy is just clutter.
        let _ = fs::remove_dir_all(&copy);
        return Ok(failure_report(&plan, &output, String::new(), fixups));
    }
    remove_build_output(&copy, &scans);

    let text = [output.stdout.trim(), output.stderr.trim()]
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    let parsed = parse(&text);
    let ran = output.status == ToolStatus::Available && output.exit_code == Some(0);
    let hop_ok = |id: &str| parsed.hops.get(id) == Some(&true);
    let all_ok = plan
        .hops
        .iter()
        .all(|h| hop_ok(&h.id) && (!h.checked || parsed.checks.get(&h.id) == Some(&true)));
    let succeeded = ran && all_ok && parsed.tests_exit.is_some();

    let mut failed_hop = None;
    let mut hops = vec![];
    for hop in &plan.hops {
        let step_ok =
            hop_ok(&hop.id) && (!hop.checked || parsed.checks.get(&hop.id) == Some(&true));
        let status = if failed_hop.is_some() {
            "skipped"
        } else if step_ok {
            "ok"
        } else {
            failed_hop = Some(hop.id.clone());
            "failed"
        };
        hops.push(HopResult {
            id: hop.id.clone(),
            label: hop.label.clone(),
            status: status.into(),
            compiled: hop
                .checked
                .then(|| parsed.checks.get(&hop.id).copied())
                .flatten(),
        });
    }
    if succeeded {
        failed_hop = None;
    }

    let tests = (parsed.test_lines > 0).then(|| TestSummary {
        passed: parsed.tests_exit == Some(0)
            && parsed.tests.failures == 0
            && parsed.tests.errors == 0,
        ..parsed.tests.clone()
    });
    let mut report = UpgradeReport {
        schema_version: 1,
        succeeded,
        destination: plan.destination.clone(),
        from_java: plan.from_java,
        to_java: plan.to_java,
        from_spring_boot: plan.from_spring_boot.clone(),
        to_spring_boot: plan.to_spring_boot.clone(),
        hops,
        failed_hop,
        fixups,
        changes: compare(&root, &copy),
        tests,
        output: String::new(),
        detail: String::new(),
        duration_ms: output.duration_ms,
        next_steps: vec![],
    };
    if copied.symlinks > 0 || copied.secrets > 0 {
        report.detail = tr!(
            "Não copiados: {} link(s) simbólico(s) e {} arquivo(s) .env.",
            "Not copied: {} symbolic link(s) and {} .env file(s).",
            copied.symlinks,
            copied.secrets
        );
    }
    if succeeded {
        report.next_steps = next_steps(&plan, &report);
        return Ok(report);
    }
    report.output = tail(&text, 60);
    let detail = match output.status {
        ToolStatus::Timeout => tr!(
            "Prazo excedido. A cópia ficou como estava no momento; o projeto original não foi alterado.",
            "Time limit exceeded. The copy was left as it was; the original project was not modified."
        ),
        _ => match &report.failed_hop {
            Some(id) => tr!(
                "O upgrade parou no passo {id}. O projeto original não foi alterado; a cópia ficou em {} para você examinar.",
                "The upgrade stopped at step {id}. The original project was not modified; the copy was left in {} for you to examine.",
                plan.destination
            ),
            None => tr!(
                "O upgrade não terminou. O projeto original não foi alterado; a cópia ficou em {}.",
                "The upgrade did not finish. The original project was not modified; the copy was left in {}.",
                plan.destination
            ),
        },
    };
    report.detail = if report.detail.is_empty() {
        detail
    } else {
        format!("{detail} {}", report.detail)
    };
    report.next_steps = failure_steps(&plan, &report);
    Ok(report)
}

fn failure_report(
    plan: &UpgradePlan,
    output: &Output,
    text: String,
    fixups: Vec<FixupPlan>,
) -> UpgradeReport {
    UpgradeReport {
        schema_version: 1,
        succeeded: false,
        destination: plan.destination.clone(),
        from_java: plan.from_java,
        to_java: plan.to_java,
        from_spring_boot: plan.from_spring_boot.clone(),
        to_spring_boot: plan.to_spring_boot.clone(),
        hops: plan
            .hops
            .iter()
            .map(|h| HopResult {
                id: h.id.clone(),
                label: h.label.clone(),
                status: "skipped".into(),
                compiled: None,
            })
            .collect(),
        failed_hop: None,
        fixups,
        changes: Changes::default(),
        tests: None,
        output: text,
        detail: output.detail.clone(),
        duration_ms: output.duration_ms,
        next_steps: vec![],
    }
}

fn next_steps(plan: &UpgradePlan, report: &UpgradeReport) -> Vec<String> {
    let mut steps = vec![tr!(
        "Compare a cópia com o projeto original (git diff --no-index ou a sua IDE) e revise o que mudou.",
        "Compare the copy with the original project (git diff --no-index or your IDE) and review what changed."
    )];
    match &report.tests {
        Some(t) if !t.passed => steps.push(tr!(
            "{} teste(s) falham na cópia (falhas: {}, erros: {}): leia o relatório do Maven em target/surefire-reports depois de rodar mvn test.",
            "{} test(s) fail on the copy (failures: {}, errors: {}): read the Maven report in target/surefire-reports after running mvn test.",
            t.failures + t.errors,
            t.failures,
            t.errors
        )),
        None => steps.push(tr!(
            "Nenhum teste rodou: teste a cópia à mão antes de adotá-la.",
            "No test ran: test the copy by hand before adopting it."
        )),
        _ => {}
    }
    for api in plan.project.jdk_apis.iter().filter(|a| !a.fixable) {
        steps.push(tr!(
            "Reescreva o código que usa {} (sem substituto automático).",
            "Rewrite the code that uses {} (no automatic replacement).",
            api.name
        ));
    }
    if plan.to_spring_boot.is_some() {
        steps.push(tr!(
            "Leia o guia de migração do Spring Boot 4 e confira configurações (application.properties) que o AutoDock não migra.",
            "Read the Spring Boot 4 migration guide and check settings (application.properties) AutoDock does not migrate."
        ));
    }
    if Path::new(&plan.destination).join(".mvn/wrapper").exists() {
        steps.push(tr!(
            "O Maven Wrapper da cópia ainda aponta para o Maven antigo: rode mvn wrapper:wrapper para atualizá-lo.",
            "The copy's Maven Wrapper still points to the old Maven: run mvn wrapper:wrapper to update it."
        ));
    }
    steps.push(tr!(
        "Instale o JDK {} para compilar no seu computador. Quando estiver satisfeito, adote a cópia e apague a pasta antiga.",
        "Install JDK {} to build on your computer. When you are satisfied, adopt the copy and delete the old folder.",
        plan.to_java
    ));
    steps
}

fn failure_steps(plan: &UpgradePlan, report: &UpgradeReport) -> Vec<String> {
    let mut steps = vec![];
    if let Some(id) = &report.failed_hop {
        steps.push(tr!(
            "Leia os erros do passo {id} abaixo: costumam apontar a classe ou a dependência que precisa de ajuste manual.",
            "Read the errors of step {id} below: they usually point at the class or dependency that needs manual work."
        ));
    }
    steps.push(tr!(
        "Corrija na cópia ({}) ou no original e rode o upgrade de novo: cada tentativa cria uma cópia nova.",
        "Fix it in the copy ({}) or in the original and run the upgrade again: every attempt creates a new copy.",
        plan.destination
    ));
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOT15: &str = r#"<project>
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>legacy-shop</artifactId>
  <version>1</version>
  <parent>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-parent</artifactId>
    <version>1.5.22.RELEASE</version>
  </parent>
  <properties>
    <java.version>1.7</java.version>
  </properties>
  <dependencies>
    <dependency>
      <groupId>org.springframework.boot</groupId>
      <artifactId>spring-boot-starter-web</artifactId>
    </dependency>
  </dependencies>
</project>
"#;

    const PLAIN6: &str = r#"<project>
  <groupId>com.example</groupId>
  <artifactId>legacy-util</artifactId>
  <version>1.0</version>
  <properties><maven.compiler.source>1.6</maven.compiler.source></properties>
</project>
"#;

    fn workspace(files: &[(&str, &str)]) -> (tempfile::TempDir, String) {
        let base = tempfile::tempdir().unwrap();
        let project = base.path().join("shop");
        for (name, content) in files {
            let path = project.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        let path = project.to_string_lossy().into_owned();
        (base, path)
    }

    fn output(status: ToolStatus, exit: Option<i32>, stdout: &str) -> Output {
        Output {
            status,
            exit_code: exit,
            stdout: stdout.into(),
            stderr: String::new(),
            truncated: false,
            detail: String::new(),
            duration_ms: 5,
        }
    }

    fn request(path: &str, confirmed: bool) -> UpgradeRequest {
        UpgradeRequest {
            path: path.into(),
            destination: None,
            confirmed,
        }
    }

    /// The host folder a `docker run` command mounts as /work.
    fn mounted_copy(args: &[String]) -> PathBuf {
        let mount = args.iter().find(|a| a.ends_with("target=/work")).unwrap();
        PathBuf::from(
            mount
                .strip_prefix("type=bind,source=")
                .unwrap()
                .strip_suffix(",target=/work")
                .unwrap(),
        )
    }

    const SUCCESS: &str = "HOP spring-boot-2.7 ok\nHOP java-17 ok\nCHECK java-17 ok\nHOP spring-boot-3.5 ok\nCHECK spring-boot-3.5 ok\nHOP spring-boot-4.0 ok\nCHECK spring-boot-4.0 ok\nHOP java-25 ok\nCHECK java-25 ok\nTESTS\nTESTLINE [INFO] Tests run: 2, Failures: 0, Errors: 0, Skipped: 0\nTESTS_EXIT 0\n";

    #[test]
    fn the_plan_lists_the_steps_the_copy_and_the_dependencies_to_add() {
        let (_base, path) = workspace(&[
            ("pom.xml", BOOT15),
            ("src/main/java/A.java", "import javax.xml.bind.DatatypeConverter;\nclass A { Object o = new sun.misc.Unsafe(); }"),
            ("src/test/java/ATest.java", "class ATest {}"),
        ]);
        let plan = plan(&path).unwrap();
        let ids: Vec<&str> = plan.hops.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "spring-boot-2.7",
                "java-17",
                "spring-boot-3.5",
                "spring-boot-4.0",
                "java-25"
            ]
        );
        let checked: Vec<bool> = plan.hops.iter().map(|h| h.checked).collect();
        assert_eq!(checked, [false, true, true, true, true]);
        assert_eq!((plan.from_java, plan.to_java), (Some(7), 25));
        assert_eq!(plan.to_spring_boot.as_deref(), Some("4.0.x"));
        assert!(plan.destination.ends_with("shop-java25"));
        assert_eq!(plan.image, MAVEN_IMAGE);
        let dependencies: Vec<&str> = plan.fixups.iter().map(|f| f.dependency.as_str()).collect();
        assert_eq!(
            dependencies,
            [
                "jakarta.xml.bind:jakarta.xml.bind-api:2.3.3",
                "org.glassfish.jaxb:jaxb-runtime:2.3.9"
            ]
        );
        assert!(plan.warnings.iter().any(|w| w.contains("sun.")));
        assert!(plan.command.starts_with("docker run --rm"));
        assert!(plan.command.contains("shop-java25"));
        assert!(plan.command.contains(&format!("source={CACHE_VOLUME}")));
    }

    #[test]
    fn projects_without_spring_boot_only_move_the_java_level() {
        let (_base, path) = workspace(&[("pom.xml", PLAIN6)]);
        let plan = plan(&path).unwrap();
        let ids: Vec<&str> = plan.hops.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["java-17", "java-25"]);
        assert_eq!(plan.to_spring_boot, None);
        assert!(plan.warnings.iter().any(|w| w.contains("src/test")));
    }

    #[test]
    fn unsuitable_projects_are_refused_with_the_reason() {
        let (_base, gradle) = workspace(&[("build.gradle", "plugins {}")]);
        assert!(plan(&gradle).unwrap_err().contains("Gradle"));

        let (_base, nothing) = workspace(&[("README.md", "x")]);
        assert!(plan(&nothing).is_err());

        let current = BOOT15
            .replace("1.5.22.RELEASE", "4.0.9")
            .replace("1.7", "25");
        let (_base, up_to_date) = workspace(&[("pom.xml", &current)]);
        assert!(plan(&up_to_date).unwrap_err().contains("25"));

        let base = tempfile::tempdir().unwrap();
        let comma = base.path().join("a,b");
        fs::create_dir_all(&comma).unwrap();
        fs::write(comma.join("pom.xml"), BOOT15).unwrap();
        assert!(plan(comma.to_str().unwrap()).is_err());
    }

    #[test]
    fn the_copy_goes_next_to_the_project_and_never_over_something() {
        let (base, path) = workspace(&[("pom.xml", PLAIN6)]);
        fs::create_dir(base.path().join("shop-java25")).unwrap();
        fs::create_dir(base.path().join("shop-java25-2")).unwrap();
        assert!(plan(&path).unwrap().destination.ends_with("shop-java25-3"));
    }

    #[test]
    fn the_script_only_embeds_constants_and_integers() {
        let hops = hop_plans(Some(7), Some((1, 5)));
        let text = script(&hops, Some((1000, 1000)), false);
        assert!(text.contains("hop spring-boot-2.7 org.openrewrite.java.spring.boot2.UpgradeSpringBoot_2_7 0 || exit 1;"));
        assert!(
            text.contains("hop java-17 org.openrewrite.java.migrate.UpgradeToJava17 1 || exit 1;")
        );
        assert!(text.contains("chown -R 1000:1000 /work"));
        assert!(text.contains(REWRITE_PLUGIN));
        assert!(text.contains("failOnInvalidActiveRecipes=true"));
        assert!(!script(&hops, None, false).contains("chown"));
        assert!(!text.contains("maven.main.skip"));
        // A multi-module project registers its modules first (see the comment on `script`).
        let multi = script(&hops, None, true);
        assert!(multi.contains("$M -Dmaven.main.skip=true compile org.openrewrite.maven"));
    }

    #[test]
    fn copying_skips_secrets_build_output_links_and_vcs_folders_but_keeps_packages_called_target() {
        let (_base, path) = workspace(&[
            ("pom.xml", PLAIN6),
            (
                "src/main/java/com/x/target/Goal.java",
                "package com.x.target;",
            ),
            ("target/classes/A.class", "binary"),
            (".git/config", "x"),
            (".env", "SECRET=1"),
            (".env.production", "SECRET=2"),
            (".env.example", "SECRET="),
            ("node_modules/x/index.js", "x"),
            ("app.iml", "x"),
            ("src/main/resources/application.properties", "a=b"),
        ]);
        let root = Path::new(&path);
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/passwd", root.join("link")).unwrap();
        let destination = root.parent().unwrap().join("copy");
        let copied = copy_project(root, &destination).unwrap();
        assert_eq!(copied.secrets, 2);
        let exists = |p: &str| destination.join(p).exists();
        assert!(exists("pom.xml") && exists("src/main/java/com/x/target/Goal.java"));
        assert!(exists("src/main/resources/application.properties") && exists(".env.example"));
        assert!(
            !exists("target") && !exists(".git") && !exists(".env") && !exists(".env.production")
        );
        assert!(!exists("node_modules") && !exists("app.iml") && !exists("link"));
        // The destination is never reused.
        assert!(copy_project(root, &destination).is_err());
    }

    #[test]
    fn dependencies_are_added_to_an_existing_block_a_missing_one_or_an_empty_one() {
        let artifacts: [java::Artifact; 1] =
            [("jakarta.annotation", "jakarta.annotation-api", "1.3.5")];
        let (added, names) = add_dependencies(BOOT15, &artifacts).unwrap().unwrap();
        assert_eq!(names, artifacts);
        assert!(added.contains(
            "    </dependency>\n    <dependency>\n      <groupId>jakarta.annotation</groupId>\n      <artifactId>jakarta.annotation-api</artifactId>\n      <version>1.3.5</version>\n    </dependency>\n  </dependencies>"
        ));
        roxmltree::Document::parse(&added).unwrap();
        // Idempotent: it is declared now.
        assert!(add_dependencies(&added, &artifacts).unwrap().is_none());

        let (without, _) = add_dependencies(PLAIN6, &artifacts).unwrap().unwrap();
        assert!(without.contains("  <dependencies>\n    <dependency>"));
        assert!(without.trim_end().ends_with("</dependencies>\n</project>"));
        roxmltree::Document::parse(&without).unwrap();

        let empty = "<project>\n  <dependencies/>\n</project>\n";
        let (filled, _) = add_dependencies(empty, &artifacts).unwrap().unwrap();
        roxmltree::Document::parse(&filled).unwrap();
        assert!(filled.contains("<artifactId>jakarta.annotation-api</artifactId>"));

        let one_line = "<project><dependencies></dependencies></project>";
        roxmltree::Document::parse(&add_dependencies(one_line, &artifacts).unwrap().unwrap().0)
            .unwrap();
        assert!(add_dependencies("<project>", &artifacts).is_err());
    }

    #[test]
    fn a_dependency_that_sits_in_dependency_management_does_not_count_as_declared() {
        let pom = "<project><dependencyManagement><dependencies><dependency><artifactId>jakarta.annotation-api</artifactId></dependency></dependencies></dependencyManagement></project>";
        let artifacts: [java::Artifact; 1] =
            [("jakarta.annotation", "jakarta.annotation-api", "1.3.5")];
        let (added, _) = add_dependencies(pom, &artifacts).unwrap().unwrap();
        assert_eq!(
            added
                .matches("<artifactId>jakarta.annotation-api</artifactId>")
                .count(),
            2
        );
    }

    #[test]
    fn nothing_runs_without_confirmation() {
        let (base, path) = workspace(&[("pom.xml", PLAIN6)]);
        let error = run_with(request(&path, false), |_, _| panic!("must not run")).unwrap_err();
        assert!(error.contains("onfirm"));
        assert!(!base.path().join("shop-java25").exists());
    }

    #[test]
    fn a_successful_upgrade_reports_the_steps_the_changes_and_the_tests_and_leaves_the_original_alone(
    ) {
        let source = "import javax.xml.bind.DatatypeConverter;\nclass A {}";
        let (base, path) = workspace(&[
            ("pom.xml", BOOT15),
            ("src/main/java/A.java", source),
            ("src/test/java/ATest.java", "class ATest {}"),
            ("README.md", "hello"),
        ]);
        let original_pom = fs::read_to_string(Path::new(&path).join("pom.xml")).unwrap();
        let report = run_with(request(&path, true), |args, timeout| {
            assert_eq!(timeout, UPGRADE_TIMEOUT);
            let copy = mounted_copy(args);
            // Behave like the container: the pom and a source change, a file goes away, target appears.
            let pom = fs::read_to_string(copy.join("pom.xml")).unwrap();
            assert!(
                pom.contains("jakarta.xml.bind-api"),
                "the fixup ran before the container"
            );
            fs::write(
                copy.join("pom.xml"),
                pom.replace("1.5.22.RELEASE", "4.0.9").replace("1.7", "25"),
            )
            .unwrap();
            fs::write(
                copy.join("src/main/java/A.java"),
                "import jakarta.xml.bind.DatatypeConverter;\nclass A {}",
            )
            .unwrap();
            fs::write(copy.join("src/main/java/New.java"), "class New {}").unwrap();
            fs::remove_file(copy.join("README.md")).unwrap();
            fs::create_dir_all(copy.join("target/classes")).unwrap();
            output(ToolStatus::Available, Some(0), SUCCESS)
        })
        .unwrap();

        assert!(report.succeeded, "{report:?}");
        assert!(report.hops.iter().all(|h| h.status == "ok"));
        assert_eq!(report.hops[1].compiled, Some(true));
        assert_eq!(report.hops[0].compiled, None);
        assert_eq!(
            report.tests,
            Some(TestSummary {
                run: 2,
                failures: 0,
                errors: 0,
                skipped: 0,
                passed: true
            })
        );
        assert_eq!(report.changes.modified, ["pom.xml", "src/main/java/A.java"]);
        assert_eq!(report.changes.added, ["src/main/java/New.java"]);
        assert_eq!(report.changes.removed, ["README.md"]);
        assert_eq!(report.fixups.len(), 2);
        assert!(report
            .next_steps
            .iter()
            .any(|s| s.contains("Spring Boot 4")));
        assert!(
            !Path::new(&report.destination).join("target").exists(),
            "build output is removed"
        );
        // The original is byte for byte what it was.
        assert_eq!(
            fs::read_to_string(Path::new(&path).join("pom.xml")).unwrap(),
            original_pom
        );
        assert_eq!(
            fs::read_to_string(Path::new(&path).join("src/main/java/A.java")).unwrap(),
            source
        );
        assert!(Path::new(&path).join("README.md").exists());
        drop(base);
    }

    #[test]
    fn failing_tests_do_not_fail_the_upgrade_but_are_reported() {
        let (_base, path) =
            workspace(&[("pom.xml", PLAIN6), ("src/test/java/T.java", "class T {}")]);
        let out = "HOP java-17 ok\nCHECK java-17 ok\nHOP java-25 ok\nCHECK java-25 ok\nTESTS\nTESTLINE [ERROR] Tests run: 5, Failures: 1, Errors: 1, Skipped: 1\nTESTLINE Tests run: 3, Failures: 0, Errors: 0, Skipped: 0\nTESTS_EXIT 1\n";
        let report = run_with(request(&path, true), |_, _| {
            output(ToolStatus::Available, Some(0), out)
        })
        .unwrap();
        assert!(report.succeeded);
        let tests = report.tests.unwrap();
        assert_eq!(
            (
                tests.run,
                tests.failures,
                tests.errors,
                tests.skipped,
                tests.passed
            ),
            (8, 1, 1, 1, false)
        );
        assert!(report
            .next_steps
            .iter()
            .any(|s| s.contains("teste") || s.contains("test(s)")));
    }

    #[test]
    fn a_failed_step_keeps_the_copy_for_inspection_and_names_the_step() {
        let (_base, path) = workspace(&[("pom.xml", BOOT15)]);
        let out = "HOP spring-boot-2.7 ok\nHOP java-17 ok\nCHECK java-17 failed\n[ERROR] /work/src/A.java:[3,22] package javax.xml.bind does not exist\n";
        let report = run_with(request(&path, true), |_, _| {
            output(ToolStatus::Available, Some(1), out)
        })
        .unwrap();
        assert!(!report.succeeded);
        assert_eq!(report.failed_hop.as_deref(), Some("java-17"));
        let statuses: Vec<&str> = report.hops.iter().map(|h| h.status.as_str()).collect();
        assert_eq!(statuses, ["ok", "failed", "skipped", "skipped", "skipped"]);
        assert_eq!(report.hops[1].compiled, Some(false));
        assert!(report.output.contains("javax.xml.bind does not exist"));
        assert!(report.detail.contains("java-17"));
        assert!(
            Path::new(&report.destination).join("pom.xml").exists(),
            "the copy stays"
        );
        assert_eq!(
            fs::read_to_string(Path::new(&path).join("pom.xml")).unwrap(),
            BOOT15
        );
        assert!(report.tests.is_none());
    }

    #[test]
    fn a_zero_exit_without_the_test_marker_is_not_a_success() {
        let (_base, path) = workspace(&[("pom.xml", PLAIN6)]);
        let out = "HOP java-17 ok\nCHECK java-17 ok\nHOP java-25 ok\nCHECK java-25 ok\n";
        let report = run_with(request(&path, true), |_, _| {
            output(ToolStatus::Available, Some(0), out)
        })
        .unwrap();
        assert!(!report.succeeded);
    }

    #[test]
    fn missing_docker_removes_the_copy_and_a_timeout_keeps_it() {
        let (base, path) = workspace(&[("pom.xml", PLAIN6)]);
        let report = run_with(request(&path, true), |_, _| Output::missing()).unwrap();
        assert!(!report.succeeded && !report.detail.is_empty());
        assert!(
            !base.path().join("shop-java25").exists(),
            "nothing ran, so no copy is left"
        );

        let report = run_with(request(&path, true), |_, _| {
            output(ToolStatus::Timeout, None, "HOP java-17 ok\n")
        })
        .unwrap();
        assert!(!report.succeeded);
        assert!(base.path().join("shop-java25").exists());
        assert!(report.detail.contains("original"));
    }

    #[test]
    fn a_pom_of_a_module_gets_its_own_dependencies() {
        let parent = "<project><artifactId>p</artifactId><packaging>pom</packaging><modules><module>core</module></modules></project>";
        let module =
            "<project><artifactId>core</artifactId>\n<dependencies>\n</dependencies>\n</project>\n";
        let (_base, path) = workspace(&[
            ("pom.xml", parent),
            ("core/pom.xml", module),
            (
                "core/src/main/java/A.java",
                "import javax.annotation.PostConstruct;",
            ),
        ]);
        let plan = plan(&path).unwrap();
        assert_eq!(plan.fixups.len(), 1);
        assert_eq!(plan.fixups[0].module, "core");
        let report = run_with(request(&path, true), |args, _| {
            let copy = mounted_copy(args);
            assert!(!fs::read_to_string(copy.join("pom.xml")).unwrap().contains("jakarta"));
            assert!(fs::read_to_string(copy.join("core/pom.xml")).unwrap().contains("jakarta.annotation-api"));
            output(ToolStatus::Available, Some(0), "HOP java-17 ok\nCHECK java-17 ok\nHOP java-25 ok\nCHECK java-25 ok\nTESTS\nTESTS_EXIT 0\n")
        })
        .unwrap();
        assert!(report.succeeded);
        assert!(report.tests.is_none());
        assert!(report
            .next_steps
            .iter()
            .any(|s| s.contains("Nenhum teste") || s.contains("No test")));
    }

    #[test]
    fn container_output_is_parsed_into_steps_checks_and_test_totals() {
        let parsed = parse(SUCCESS);
        assert_eq!(parsed.hops.len(), 5);
        assert_eq!(parsed.checks.len(), 4);
        assert_eq!(parsed.tests_exit, Some(0));
        assert_eq!((parsed.tests.run, parsed.test_lines), (2, 1));
        assert!(parse("garbage\nHOP only\nTESTLINE nothing").hops.is_empty());
    }
    #[test]
    fn the_copy_can_go_into_a_folder_the_person_chooses() {
        let (_base, path) = workspace(&[("pom.xml", PLAIN6)]);
        let elsewhere = tempfile::tempdir().unwrap();
        let chosen = elsewhere.path().to_string_lossy().into_owned();
        let plan = plan_in(&path, Some(&chosen)).unwrap();
        let destination = Path::new(&plan.destination);
        assert!(destination.ends_with("shop-java25"));
        assert_eq!(
            destination.parent().unwrap().canonicalize().unwrap(),
            elsewhere.path().canonicalize().unwrap()
        );
        // the numbering works in the chosen folder too
        fs::create_dir(elsewhere.path().join("shop-java25")).unwrap();
        assert!(plan_in(&path, Some(&chosen))
            .unwrap()
            .destination
            .ends_with("shop-java25-2"));
        // an empty value means the default place
        assert!(plan_in(&path, Some("  "))
            .unwrap()
            .destination
            .ends_with("shop-java25"));
    }

    #[test]
    fn a_folder_inside_the_project_a_file_or_a_missing_folder_is_refused() {
        let (_base, path) =
            workspace(&[("pom.xml", PLAIN6), ("src/main/java/A.java", "class A {}")]);
        let inside = Path::new(&path).join("src");
        assert!(plan_in(&path, Some(&inside.to_string_lossy())).is_err());
        assert!(plan_in(&path, Some(&path)).is_err());
        let file = Path::new(&path).join("pom.xml");
        assert!(plan_in(&path, Some(&file.to_string_lossy())).is_err());
        let missing = Path::new(&path).parent().unwrap().join("nowhere");
        assert!(plan_in(&path, Some(&missing.to_string_lossy())).is_err());
    }

    #[test]
    fn the_upgrade_creates_the_copy_in_the_chosen_folder_and_leaves_the_original_alone() {
        let (base, path) = workspace(&[("pom.xml", PLAIN6)]);
        let elsewhere = tempfile::tempdir().unwrap();
        let mut asked = request(&path, true);
        asked.destination = Some(elsewhere.path().to_string_lossy().into_owned());
        let report = run_with(asked, |args, _| {
            let copy = mounted_copy(args);
            assert_eq!(
                copy.parent().unwrap().canonicalize().unwrap(),
                elsewhere.path().canonicalize().unwrap()
            );
            output(
                ToolStatus::Available,
                Some(0),
                "HOP java-17 ok
CHECK java-17 ok
HOP java-25 ok
CHECK java-25 ok
TESTS
TESTS_EXIT 0
",
            )
        })
        .unwrap();
        assert!(report.succeeded);
        assert!(elsewhere.path().join("shop-java25/pom.xml").exists());
        assert!(
            !base.path().join("shop-java25").exists(),
            "nothing next to the project"
        );
        assert_eq!(
            fs::read_to_string(Path::new(&path).join("pom.xml")).unwrap(),
            PLAIN6
        );
    }
}
