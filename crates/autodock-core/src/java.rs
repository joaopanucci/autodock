//! Legacy Java projects: which Java level and Spring Boot version a Maven project
//! declares, and which APIs the JDK stopped shipping that its sources still use.
//!
//! Inspecting only reads `pom.xml` files and the text of `.java` files as data.
//! Nothing is executed and nothing is written.
use crate::generate::{self, regular_file};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};

/// The JDK a finished upgrade targets: the current LTS release.
pub const TARGET_JAVA: u32 = 25;
/// The Spring Boot line a finished upgrade targets (`major`, `minor`).
pub const TARGET_BOOT: (u32, u32) = (4, 0);

const MAX_POM_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SOURCE_FILES: usize = 20_000;
const MAX_MODULES: usize = 100;
const MAX_DEPTH: usize = 8;

/// One Maven artifact: group, artifact id and version.
pub(crate) type Artifact = (&'static str, &'static str, &'static str);

/// An API the JDK shipped and no longer does (JEP 320 and friends).
pub(crate) struct Removed {
    pub id: &'static str,
    /// Dependencies that bring the same classes back (same `javax.*` names).
    pub artifacts: &'static [Artifact],
}

pub(crate) const JAXB: Removed = Removed {
    id: "javax.xml.bind",
    artifacts: &[
        ("jakarta.xml.bind", "jakarta.xml.bind-api", "2.3.3"),
        ("org.glassfish.jaxb", "jaxb-runtime", "2.3.9"),
    ],
};
const ACTIVATION: Removed = Removed {
    id: "javax.activation",
    artifacts: &[("com.sun.activation", "jakarta.activation", "1.2.2")],
};
const WS: Removed = Removed {
    id: "javax.xml.ws",
    artifacts: &[("jakarta.xml.ws", "jakarta.xml.ws-api", "2.3.3")],
};
const JWS: Removed = Removed {
    id: "javax.jws",
    artifacts: &[("jakarta.jws", "jakarta.jws-api", "2.1.0")],
};
const SOAP: Removed = Removed {
    id: "javax.xml.soap",
    artifacts: &[("jakarta.xml.soap", "jakarta.xml.soap-api", "1.4.2")],
};
const ANNOTATION: Removed = Removed {
    id: "javax.annotation",
    artifacts: &[("jakarta.annotation", "jakarta.annotation-api", "1.3.5")],
};
const TRANSACTION: Removed = Removed {
    id: "javax.transaction",
    artifacts: &[("jakarta.transaction", "jakarta.transaction-api", "1.3.3")],
};
/// Gone from the JDK with no drop-in replacement: reported, never fixed.
pub(crate) const CORBA: Removed = Removed {
    id: "org.omg (CORBA)",
    artifacts: &[],
};
pub(crate) const INTERNAL: Removed = Removed {
    id: "sun.* (JDK internals)",
    artifacts: &[],
};

const JSR250: [&str; 7] = [
    "Generated",
    "PostConstruct",
    "PreDestroy",
    "Resource",
    "Resources",
    "ManagedBean",
    "Priority",
];
const INTERNAL_PACKAGES: [&str; 6] = [
    "sun.misc.",
    "sun.security.",
    "sun.reflect.",
    "sun.nio.",
    "sun.net.",
    "sun.awt.",
];

/// What a piece of Java source text uses that the JDK no longer provides.
pub(crate) fn removed_apis(text: &str) -> Vec<&'static Removed> {
    let mut found: Vec<&'static Removed> = vec![];
    let mut add = |api: &'static Removed| {
        if !found.iter().any(|f| f.id == api.id) {
            found.push(api);
        }
    };
    for line in text.lines() {
        let line = line.trim();
        // Comments and string literals can mention these names; a false positive only adds a
        // harmless dependency, so a plain scan is enough.
        for (prefix, api) in [
            ("javax.xml.bind.", &JAXB),
            ("javax.activation.", &ACTIVATION),
            ("javax.xml.ws.", &WS),
            ("javax.jws.", &JWS),
            ("javax.xml.soap.", &SOAP),
            ("org.omg.", &CORBA),
        ] {
            if contains_name(line, prefix) {
                add(api);
            }
        }
        if let Some(index) = find_name(line, "javax.annotation.") {
            let name: String = line[index + "javax.annotation.".len()..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if JSR250.contains(&name.as_str()) {
                add(&ANNOTATION);
            }
        }
        if let Some(index) = find_name(line, "javax.transaction.") {
            let rest = &line[index + "javax.transaction.".len()..];
            if !rest.starts_with("xa.") {
                add(&TRANSACTION);
            }
        }
        if INTERNAL_PACKAGES.iter().any(|p| contains_name(line, p)) {
            add(&INTERNAL);
        }
    }
    found
}

/// Position of `name` in `line` when it starts a qualified name (not the tail of another).
fn find_name(line: &str, name: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(offset) = line[from..].find(name) {
        let index = from + offset;
        let before = line[..index].chars().next_back();
        if before.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')) {
            return Some(index);
        }
        from = index + name.len();
    }
    None
}

fn contains_name(line: &str, name: &str) -> bool {
    find_name(line, name).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JdkApi {
    pub name: String,
    pub files: usize,
    /// A dependency can bring it back (`false`: it needs manual work).
    pub fixable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaProject {
    pub schema_version: u32,
    pub path: String,
    pub name: String,
    pub build_tool: String,
    pub packaging: String,
    /// Java level the build declares, e.g. `7` for `1.7`.
    pub java_level: Option<u32>,
    pub java_declared: Option<String>,
    /// Spring Boot version as declared, e.g. `1.5.22.RELEASE`.
    pub spring_boot: Option<String>,
    /// The BOM that pins the Spring Boot version (`group:artifact`), when the recipes cannot move
    /// it: the upgrade then only changes the Java level.
    pub spring_boot_pinned_by: Option<String>,
    pub modules: Vec<String>,
    /// Removed JDK APIs the sources still use.
    pub jdk_apis: Vec<JdkApi>,
    /// There is something to upgrade (the Java level or the Spring Boot line is behind).
    pub upgrade_available: bool,
    pub upgrade_supported: bool,
    pub unsupported_reason: Option<String>,
}

/// One Maven module (the root included) with what its sources use.
pub(crate) struct ModuleScan {
    /// Folder relative to the project root, `""` for the root.
    pub dir: String,
    pub packaging: String,
    pub apis: BTreeMap<&'static str, (usize, &'static Removed)>,
}

/// `1.7` → 7, `8` → 8, `17` → 17.
pub(crate) fn level_of(text: &str) -> Option<u32> {
    let text = text.trim();
    let text = match text.strip_prefix("1.") {
        Some(rest) if rest.len() == 1 => rest,
        _ => text,
    };
    let level: u32 = text.parse().ok()?;
    (1..=99).contains(&level).then_some(level)
}

/// `1.5.22.RELEASE` → (1, 5); `4.0.9` → (4, 0).
pub(crate) fn boot_line(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.trim().split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: String = parts
        .next()?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    Some((major, minor.parse().ok()?))
}

/// The ordered steps that bring a project to the current Java and Spring Boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hop {
    Boot27,
    Java17,
    Boot35,
    Boot40,
    Java25,
}

pub(crate) fn hops_for(java: Option<u32>, boot: Option<(u32, u32)>) -> Vec<Hop> {
    let mut hops = vec![];
    let mut level = java.unwrap_or(0);
    match boot {
        Some(line) => {
            let mut line = line;
            if line < (2, 7) {
                hops.push(Hop::Boot27);
                line = (2, 7);
            }
            if level < 17 {
                hops.push(Hop::Java17);
                level = 17;
            }
            if line < (3, 5) {
                hops.push(Hop::Boot35);
                line = (3, 5);
            }
            if line < TARGET_BOOT {
                hops.push(Hop::Boot40);
            }
        }
        None => {
            if level < 17 {
                hops.push(Hop::Java17);
                level = 17;
            }
        }
    }
    if level < TARGET_JAVA {
        hops.push(Hop::Java25);
    }
    hops
}

/// Spring Boot pinned by a BOM the recipes cannot move: only the Java level can change, and only
/// as far as that Spring Boot line is meant for (before 3.5 that is Java 17).
pub(crate) fn hops_pinned(java: Option<u32>, boot: Option<(u32, u32)>) -> Vec<Hop> {
    let level = java.unwrap_or(0);
    let ceiling = if boot.is_some_and(|line| line >= (3, 5)) {
        TARGET_JAVA
    } else {
        17
    };
    let mut hops = vec![];
    if level < 17 {
        hops.push(Hop::Java17);
    }
    if ceiling >= TARGET_JAVA && level < TARGET_JAVA {
        hops.push(Hop::Java25);
    }
    hops
}

/// The steps for a project, depending on whether its Spring Boot can be moved.
pub(crate) fn plan_hops(
    java: Option<u32>,
    boot: Option<(u32, u32)>,
    boot_pinned: bool,
) -> Vec<Hop> {
    if boot_pinned {
        hops_pinned(java, boot)
    } else {
        hops_for(java, boot)
    }
}

// ───────────────────────────── pom reading ─────────────────────────────

type Node<'a> = roxmltree::Node<'a, 'a>;

fn child<'a>(node: Node<'a>, name: &str) -> Option<Node<'a>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}

fn children<'a>(node: Node<'a>, name: &'a str) -> impl Iterator<Item = Node<'a>> {
    node.children()
        .filter(move |n| n.is_element() && n.tag_name().name() == name)
}

fn text_of(node: Node, name: &str) -> Option<String> {
    child(node, name)
        .and_then(|n| n.text())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

fn properties(project: Node) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(props) = child(project, "properties") {
        for p in props.children().filter(Node::is_element) {
            if let Some(value) = p.text().map(str::trim).filter(|v| !v.is_empty()) {
                map.insert(p.tag_name().name().to_string(), value.to_string());
            }
        }
    }
    map
}

/// Replaces `${name}` with the project property, up to three levels deep.
fn expand(value: &str, props: &HashMap<String, String>) -> String {
    let mut text = value.trim().to_string();
    for _ in 0..3 {
        let Some(start) = text.find("${") else { break };
        let Some(end) = text[start..].find('}') else {
            break;
        };
        let key = text[start + 2..start + end].to_string();
        match props.get(&key) {
            Some(value) => {
                text = format!("{}{}{}", &text[..start], value, &text[start + end + 1..])
            }
            None => break,
        }
    }
    text
}

fn plugin_setting(project: Node, setting: &str, props: &HashMap<String, String>) -> Option<String> {
    let mut sections = vec![];
    if let Some(build) = child(project, "build") {
        sections.push(build);
        if let Some(management) = child(build, "pluginManagement") {
            sections.push(management);
        }
    }
    for build in sections {
        let Some(plugins) = child(build, "plugins") else {
            continue;
        };
        for plugin in children(plugins, "plugin") {
            if text_of(plugin, "artifactId").as_deref() != Some("maven-compiler-plugin") {
                continue;
            }
            let value = child(plugin, "configuration").and_then(|c| text_of(c, setting));
            if let Some(value) = value {
                return Some(expand(&value, props));
            }
        }
    }
    None
}

/// The Java level a pom declares: `release` wins over `source`, then `target`, then
/// `java.version` (Spring Boot's own property).
fn declared_java(project: Node, props: &HashMap<String, String>) -> Option<String> {
    let property = |name: &str| props.get(name).map(|v| expand(v, props));
    let candidates = [
        property("maven.compiler.release"),
        plugin_setting(project, "release", props),
        plugin_setting(project, "source", props),
        property("maven.compiler.source"),
        plugin_setting(project, "target", props),
        property("maven.compiler.target"),
        property("java.version"),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|value| level_of(value).is_some())
}

fn spring_boot_version(project: Node, props: &HashMap<String, String>) -> Option<String> {
    let is_boot =
        |node: Node| text_of(node, "groupId").as_deref() == Some("org.springframework.boot");
    if let Some(parent) = child(project, "parent") {
        if is_boot(parent)
            && matches!(
                text_of(parent, "artifactId").as_deref(),
                Some("spring-boot-starter-parent" | "spring-boot-dependencies")
            )
        {
            return text_of(parent, "version").map(|v| expand(&v, props));
        }
    }
    let management = child(project, "dependencyManagement").and_then(|m| child(m, "dependencies"));
    let imported = management.and_then(|management| {
        children(management, "dependency")
            .find(|d| {
                is_boot(*d)
                    && text_of(*d, "artifactId").as_deref() == Some("spring-boot-dependencies")
            })
            .and_then(|d| text_of(d, "version"))
    });
    if let Some(version) = imported {
        return Some(expand(&version, props));
    }
    // Projects that get Spring Boot through another BOM (JHipster, for one) or pin it by hand:
    // the plugin, an explicit starter version or the conventional property still tell the line.
    let mut sections = vec![];
    if let Some(build) = child(project, "build") {
        sections.push(build);
        if let Some(plugin_management) = child(build, "pluginManagement") {
            sections.push(plugin_management);
        }
    }
    let plugin_versions = sections
        .into_iter()
        .filter_map(|section| child(section, "plugins"))
        .flat_map(|plugins| children(plugins, "plugin"))
        .filter(|plugin| {
            text_of(*plugin, "artifactId").as_deref() == Some("spring-boot-maven-plugin")
                && text_of(*plugin, "groupId")
                    .is_none_or(|group| group == "org.springframework.boot")
        })
        .filter_map(|plugin| text_of(plugin, "version"));
    let dependency_versions = [child(project, "dependencies"), management]
        .into_iter()
        .flatten()
        .flat_map(|dependencies| children(dependencies, "dependency"))
        .filter(|d| is_boot(*d))
        .filter_map(|d| text_of(d, "version"));
    let property_versions = ["spring-boot.version", "spring.boot.version"]
        .into_iter()
        .filter_map(|name| props.get(name).cloned());
    plugin_versions
        .chain(dependency_versions)
        .chain(property_versions)
        .map(|v| expand(&v, props))
        .find(|v| boot_line(v).is_some())
}

/// BOMs that decide the Spring Boot version of a project on their own: the OpenRewrite recipes
/// do not know them, so they cannot move the Spring Boot that comes through them.
const PINNING_BOMS: [(&str, &str); 1] = [("tech.jhipster", "jhipster-dependencies")];

/// `group:artifact` of the BOM that pins Spring Boot, when the project gets it that way instead
/// of through the Spring Boot parent or the Spring Boot BOM.
fn spring_boot_pinned_by(project: Node) -> Option<String> {
    let boot_group =
        |node: Node| text_of(node, "groupId").as_deref() == Some("org.springframework.boot");
    if child(project, "parent").is_some_and(boot_group) {
        return None;
    }
    let management =
        child(project, "dependencyManagement").and_then(|m| child(m, "dependencies"))?;
    if children(management, "dependency").any(|d| {
        boot_group(d) && text_of(d, "artifactId").as_deref() == Some("spring-boot-dependencies")
    }) {
        return None;
    }
    children(management, "dependency").find_map(|d| {
        let group = text_of(d, "groupId")?;
        let artifact = text_of(d, "artifactId")?;
        PINNING_BOMS
            .iter()
            .any(|(g, a)| *g == group && *a == artifact)
            .then(|| format!("{group}:{artifact}"))
    })
}

fn read_pom(path: &Path) -> Result<String, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(tr!(
            "{} não é um arquivo comum.",
            "{} is not a regular file.",
            path.display()
        ));
    }
    if meta.len() > MAX_POM_BYTES {
        return Err(tr!(
            "{} é grande demais para ler.",
            "{} is too large to read.",
            path.display()
        ));
    }
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Module folders listed by `<modules>`, relative to the project root, nested ones included.
fn module_dirs(root: &Path, dir: &str, text: &str, depth: usize, out: &mut Vec<String>) {
    let Ok(doc) = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            ..Default::default()
        },
    ) else {
        return;
    };
    let Some(modules) = child(doc.root_element(), "modules") else {
        return;
    };
    for module in children(modules, "module") {
        let Some(name) = module.text().map(str::trim).filter(|m| !m.is_empty()) else {
            continue;
        };
        // Only folders inside the project: no absolute paths, no `..`.
        let safe = !name.starts_with('/')
            && !name.starts_with('\\')
            && !name.contains(':')
            && name
                .split(['/', '\\'])
                .all(|part| part != ".." && !part.is_empty());
        if !safe || out.len() >= MAX_MODULES || depth >= MAX_DEPTH {
            continue;
        }
        let name = name.replace('\\', "/");
        let relative = if dir.is_empty() {
            name.clone()
        } else {
            format!("{dir}/{name}")
        };
        let pom = root.join(&relative).join("pom.xml");
        let folder_is_real = fs::symlink_metadata(root.join(&relative))
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink());
        if !folder_is_real
            || !generate::regular_file(root, &format!("{relative}/pom.xml"))
            || out.contains(&relative)
        {
            continue;
        }
        out.push(relative.clone());
        if let Ok(text) = read_pom(&pom) {
            module_dirs(root, &relative, &text, depth + 1, out);
        }
    }
}

/// Folders that never hold the project's own sources.
const SKIPPED_SOURCE_DIRS: [&str; 4] = ["node_modules", ".git", ".idea", ".gradle"];

/// Reads every `.java` file under `dir`, wherever the project keeps its sources (legacy
/// projects often do not use `src/main/java`). Folders of other modules are left to their own
/// scan, hidden folders and the `target` of a Maven folder are skipped, symbolic links are not
/// followed.
fn scan_sources(
    dir: &Path,
    other_modules: &[std::path::PathBuf],
    files: &mut usize,
    found: &mut BTreeMap<&'static str, (usize, &'static Removed)>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let maven_folder = dir.join("pom.xml").is_file();
    for entry in entries.flatten() {
        let Ok(meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if meta.is_dir() {
            let skipped = SKIPPED_SOURCE_DIRS.contains(&name.as_str())
                || (maven_folder && name == "target")
                || other_modules.iter().any(|module| *module == entry.path());
            if !skipped {
                scan_sources(&entry.path(), other_modules, files, found);
            }
        } else if meta.is_file()
            && meta.len() <= MAX_SOURCE_BYTES
            && name.ends_with(".java")
            && *files < MAX_SOURCE_FILES
        {
            *files += 1;
            if let Ok(text) = fs::read_to_string(entry.path()) {
                for api in removed_apis(&text) {
                    found.entry(api.id).or_insert((0, api)).0 += 1;
                }
            }
        }
    }
}

/// The root and its modules, each with the removed APIs its own sources use.
pub(crate) fn scan_modules(root: &Path) -> Vec<ModuleScan> {
    let mut dirs = vec![String::new()];
    if let Ok(text) = read_pom(&root.join("pom.xml")) {
        module_dirs(root, "", &text, 0, &mut dirs);
    }
    let mut files = 0;
    let module_dirs_all = dirs.clone();
    dirs.into_iter()
        .map(|dir| {
            let base = if dir.is_empty() {
                root.to_path_buf()
            } else {
                root.join(&dir)
            };
            let packaging = read_pom(&base.join("pom.xml"))
                .ok()
                .and_then(|text| {
                    let doc = roxmltree::Document::parse(&text).ok()?;
                    text_of(doc.root_element(), "packaging")
                })
                .unwrap_or_else(|| "jar".into());
            let mut apis = BTreeMap::new();
            let other_modules: Vec<std::path::PathBuf> = module_dirs_all
                .iter()
                .filter(|other| !other.is_empty() && **other != dir)
                .map(|other| root.join(other))
                .collect();
            scan_sources(&base, &other_modules, &mut files, &mut apis);
            ModuleScan {
                dir,
                packaging,
                apis,
            }
        })
        .collect()
}

// ───────────────────────────── inspect ─────────────────────────────

fn unsupported(root: &Path, text: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(text).ok()?;
    let project = doc.root_element();
    let parent = child(project, "parent")?;
    if text_of(parent, "groupId").as_deref() == Some("org.springframework.boot") {
        return None;
    }
    // A module of a larger tree builds only together with its parent pom.
    let parent_pom = root.parent()?.join("pom.xml");
    let parent_text = read_pom(&parent_pom).ok()?;
    let parent_doc = roxmltree::Document::parse(&parent_text).ok()?;
    let sibling = text_of(parent_doc.root_element(), "artifactId");
    (sibling.is_some() && sibling == text_of(parent, "artifactId")).then(|| {
        tr!(
            "Esta pasta é um módulo de um projeto Maven maior (o pom pai está em {}). Escolha a pasta do projeto raiz.",
            "This folder is a module of a larger Maven project (the parent pom is in {}). Pick the root project folder.",
            parent_pom.display()
        )
    })
}

/// Reads a folder as a Maven project. `Ok(None)`: it is not a Java project.
pub fn inspect(path: &str) -> Result<Option<JavaProject>, String> {
    let root = generate::root_of(path)?;
    let pom_path = root.join("pom.xml");
    if !regular_file(&root, "pom.xml") {
        let gradle = [
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
        ]
        .iter()
        .any(|f| regular_file(&root, f));
        if !gradle {
            return Ok(None);
        }
        return Ok(Some(JavaProject {
            schema_version: 1,
            path: generate::plain_path(&root),
            name: root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            build_tool: "gradle".into(),
            packaging: String::new(),
            java_level: None,
            java_declared: None,
            spring_boot: None,
            spring_boot_pinned_by: None,
            modules: vec![],
            jdk_apis: vec![],
            upgrade_available: false,
            upgrade_supported: false,
            unsupported_reason: Some(tr!(
                "Projetos Gradle ainda não são suportados: o upgrade usa o Maven.",
                "Gradle projects are not supported yet: the upgrade uses Maven."
            )),
        }));
    }
    let text = read_pom(&pom_path)?;
    let doc = roxmltree::Document::parse_with_options(
        &text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            ..Default::default()
        },
    )
    .map_err(|e| tr!("pom.xml inválido: {e}", "Invalid pom.xml: {e}"))?;
    let project = doc.root_element();
    if project.tag_name().name() != "project" {
        return Err(tr!(
            "pom.xml sem o elemento <project>.",
            "pom.xml has no <project> element."
        ));
    }
    let props = properties(project);
    let java_declared = declared_java(project, &props);
    let java_level = java_declared.as_deref().and_then(level_of);
    let spring_boot = spring_boot_version(project, &props);

    let scans = scan_modules(&root);
    let modules: Vec<String> = scans
        .iter()
        .filter(|m| !m.dir.is_empty())
        .map(|m| m.dir.clone())
        .collect();
    let mut totals: BTreeMap<&'static str, (usize, &'static Removed)> = BTreeMap::new();
    for module in &scans {
        for (id, (count, api)) in &module.apis {
            totals.entry(*id).or_insert((0, *api)).0 += *count;
        }
    }
    let jdk_apis: Vec<JdkApi> = totals
        .into_iter()
        .map(|(name, (files, api))| JdkApi {
            name: name.to_string(),
            files,
            fixable: !api.artifacts.is_empty(),
        })
        .collect();

    let name = text_of(project, "artifactId")
        .or_else(|| root.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let boot = spring_boot.as_deref().and_then(boot_line);
    let pinned_by = spring_boot
        .as_ref()
        .and_then(|_| spring_boot_pinned_by(project));
    let unsupported_reason = unsupported(&root, &text);
    Ok(Some(JavaProject {
        schema_version: 1,
        path: generate::plain_path(&root),
        name,
        build_tool: "maven".into(),
        packaging: text_of(project, "packaging").unwrap_or_else(|| "jar".into()),
        java_level,
        java_declared,
        spring_boot,
        spring_boot_pinned_by: pinned_by.clone(),
        modules,
        jdk_apis,
        upgrade_available: !plan_hops(java_level, boot, pinned_by.is_some()).is_empty(),
        upgrade_supported: unsupported_reason.is_none(),
        unsupported_reason,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, content) in files {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        dir
    }

    fn inspect_dir(dir: &tempfile::TempDir) -> JavaProject {
        inspect(dir.path().to_str().unwrap()).unwrap().unwrap()
    }

    const BOOT15: &str = r#"<project>
  <groupId>com.example</groupId><artifactId>legacy-shop</artifactId><version>1</version>
  <parent><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-parent</artifactId><version>1.5.22.RELEASE</version></parent>
  <properties><java.version>1.7</java.version></properties>
</project>"#;

    #[test]
    fn levels_and_boot_lines_are_normalised() {
        assert_eq!(level_of("1.6"), Some(6));
        assert_eq!(level_of("1.8"), Some(8));
        assert_eq!(level_of("11"), Some(11));
        assert_eq!(level_of("${java.version}"), None);
        assert_eq!(level_of(""), None);
        assert_eq!(boot_line("1.5.22.RELEASE"), Some((1, 5)));
        assert_eq!(boot_line("2.7.18"), Some((2, 7)));
        assert_eq!(boot_line("3.0.0-M1"), Some((3, 0)));
        assert_eq!(boot_line("nope"), None);
    }

    #[test]
    fn the_hops_follow_the_order_that_keeps_the_build_working() {
        use Hop::*;
        assert_eq!(
            hops_for(Some(7), Some((1, 5))),
            [Boot27, Java17, Boot35, Boot40, Java25]
        );
        assert_eq!(
            hops_for(Some(8), Some((2, 7))),
            [Java17, Boot35, Boot40, Java25]
        );
        assert_eq!(hops_for(Some(17), Some((3, 2))), [Boot35, Boot40, Java25]);
        assert_eq!(hops_for(Some(21), Some((4, 0))), [Java25]);
        assert_eq!(hops_for(Some(25), Some((4, 0))), []);
        assert_eq!(hops_for(Some(6), None), [Java17, Java25]);
        assert_eq!(hops_for(None, None), [Java17, Java25]);
        assert_eq!(hops_for(Some(17), None), [Java25]);
        assert_eq!(hops_for(Some(25), None), []);
    }

    #[test]
    fn a_legacy_spring_boot_project_is_described() {
        let dir = project(&[("pom.xml", BOOT15)]);
        let p = inspect_dir(&dir);
        assert_eq!(p.build_tool, "maven");
        assert_eq!(p.name, "legacy-shop");
        assert_eq!(p.java_level, Some(7));
        assert_eq!(p.java_declared.as_deref(), Some("1.7"));
        assert_eq!(p.spring_boot.as_deref(), Some("1.5.22.RELEASE"));
        assert!(p.upgrade_available && p.upgrade_supported);
        assert!(p.modules.is_empty() && p.jdk_apis.is_empty());
    }

    #[test]
    fn the_compiler_level_comes_from_the_plugin_or_the_properties() {
        let plugin = r#"<project><artifactId>a</artifactId><build><plugins><plugin>
            <artifactId>maven-compiler-plugin</artifactId>
            <configuration><source>${jdk}</source><target>1.6</target></configuration>
            </plugin></plugins></build><properties><jdk>1.6</jdk></properties></project>"#;
        let dir = project(&[("pom.xml", plugin)]);
        assert_eq!(inspect_dir(&dir).java_level, Some(6));

        let release = r#"<project><artifactId>a</artifactId>
            <properties><maven.compiler.release>21</maven.compiler.release><maven.compiler.source>8</maven.compiler.source></properties></project>"#;
        let dir = project(&[("pom.xml", release)]);
        assert_eq!(inspect_dir(&dir).java_level, Some(21));

        let none = r#"<project><artifactId>a</artifactId></project>"#;
        let dir = project(&[("pom.xml", none)]);
        let p = inspect_dir(&dir);
        assert_eq!(p.java_level, None);
        assert!(p.upgrade_available);
    }

    #[test]
    fn a_bom_import_counts_as_spring_boot() {
        let pom = r#"<project><artifactId>a</artifactId>
            <properties><boot>2.3.12.RELEASE</boot></properties>
            <dependencyManagement><dependencies><dependency>
              <groupId>org.springframework.boot</groupId><artifactId>spring-boot-dependencies</artifactId>
              <version>${boot}</version><type>pom</type><scope>import</scope>
            </dependency></dependencies></dependencyManagement></project>"#;
        let dir = project(&[("pom.xml", pom)]);
        assert_eq!(
            inspect_dir(&dir).spring_boot.as_deref(),
            Some("2.3.12.RELEASE")
        );
    }

    #[test]
    fn a_project_that_is_current_has_nothing_to_upgrade() {
        let pom = r#"<project><artifactId>a</artifactId>
            <parent><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-parent</artifactId><version>4.0.9</version></parent>
            <properties><java.version>25</java.version></properties></project>"#;
        let dir = project(&[("pom.xml", pom)]);
        assert!(!inspect_dir(&dir).upgrade_available);
    }

    #[test]
    fn removed_jdk_apis_are_found_by_name() {
        let text = "import javax.xml.bind.DatatypeConverter;
import javax.annotation.PostConstruct;
\
                    import javax.annotation.Nullable;
import javax.transaction.xa.XAResource;
\
                    import javax.transaction.Transactional;
foo(new sun.misc.BASE64Encoder());
\
                    import org.omg.CORBA.ORB;
import myjavax.xml.bind.Thing;";
        let ids: Vec<&str> = removed_apis(text).iter().map(|a| a.id).collect();
        assert_eq!(
            ids,
            [
                "javax.xml.bind",
                "javax.annotation",
                "javax.transaction",
                "sun.* (JDK internals)",
                "org.omg (CORBA)"
            ]
        );
        assert!(removed_apis(
            "import javax.persistence.Entity;
import javax.xml.stream.XMLReader;"
        )
        .is_empty());
    }

    #[test]
    fn the_sources_of_every_module_are_scanned() {
        let parent = r#"<project><artifactId>parent</artifactId><packaging>pom</packaging>
            <modules><module>core</module><module>../outside</module><module>web</module><module>missing</module></modules></project>"#;
        let module = r#"<project><artifactId>m</artifactId></project>"#;
        let dir = project(&[
            ("pom.xml", parent),
            ("core/pom.xml", module),
            (
                "core/src/main/java/A.java",
                "import javax.xml.bind.JAXBContext;",
            ),
            ("web/pom.xml", module),
            ("web/src/test/java/B.java", "import sun.misc.Unsafe;"),
        ]);
        let p = inspect_dir(&dir);
        assert_eq!(p.modules, ["core", "web"]);
        assert_eq!(p.packaging, "pom");
        let apis: Vec<(&str, usize, bool)> = p
            .jdk_apis
            .iter()
            .map(|a| (a.name.as_str(), a.files, a.fixable))
            .collect();
        assert_eq!(
            apis,
            [
                ("javax.xml.bind", 1, true),
                ("sun.* (JDK internals)", 1, false)
            ]
        );
    }

    #[test]
    fn gradle_and_other_folders_are_reported_or_ignored() {
        let dir = project(&[("build.gradle", "plugins { id 'java' }")]);
        let p = inspect_dir(&dir);
        assert_eq!(p.build_tool, "gradle");
        assert!(!p.upgrade_supported && p.unsupported_reason.is_some());

        let dir = project(&[("README.md", "x")]);
        assert!(inspect(dir.path().to_str().unwrap()).unwrap().is_none());
    }

    #[test]
    fn broken_poms_are_refused_and_dtds_are_not_expanded() {
        let dir = project(&[("pom.xml", "<project>")]);
        assert!(inspect(dir.path().to_str().unwrap()).is_err());
        let bomb =
            "<!DOCTYPE project [<!ENTITY a \"x\">]><project><artifactId>&a;</artifactId></project>";
        let dir = project(&[("pom.xml", bomb)]);
        assert!(inspect(dir.path().to_str().unwrap()).is_err());
    }

    #[test]
    fn a_module_of_a_bigger_tree_is_refused() {
        let outer = tempfile::tempdir().unwrap();
        fs::write(
            outer.path().join("pom.xml"),
            "<project><artifactId>big</artifactId><packaging>pom</packaging></project>",
        )
        .unwrap();
        let inner = outer.path().join("child");
        fs::create_dir_all(&inner).unwrap();
        fs::write(
            inner.join("pom.xml"),
            "<project><parent><groupId>g</groupId><artifactId>big</artifactId><version>1</version></parent><artifactId>child</artifactId></project>",
        )
        .unwrap();
        let p = inspect(inner.to_str().unwrap()).unwrap().unwrap();
        assert!(!p.upgrade_supported);
        assert!(p.unsupported_reason.unwrap().contains("pom"));
    }
    #[test]
    fn sources_are_found_wherever_the_project_keeps_them() {
        // A legacy layout: sources straight in `src/` and in `java/`, none in src/main/java.
        let pom = "<project><artifactId>old</artifactId><packaging>pom</packaging><modules><module>web</module></modules></project>";
        let module = "<project><artifactId>web</artifactId></project>";
        let dir = project(&[
            ("pom.xml", pom),
            (
                "src/com/acme/Main.java",
                "import javax.xml.bind.JAXBContext;",
            ),
            (
                "java/com/acme/Util.java",
                "import javax.annotation.PostConstruct;",
            ),
            ("web/pom.xml", module),
            ("web/src/Servlet.java", "import javax.xml.bind.JAXBContext;"),
            ("target/generated/Gen.java", "import org.omg.CORBA.ORB;"),
            ("node_modules/x/Fake.java", "import sun.misc.Unsafe;"),
            (".git/Hidden.java", "import sun.misc.Unsafe;"),
        ]);
        let scans = scan_modules(dir.path());
        let root = &scans[0];
        let web = &scans[1];
        assert_eq!(root.dir, "");
        // the root sees its own folders, not the module's, `target`, `node_modules` or `.git`
        let names = |m: &ModuleScan| m.apis.keys().copied().collect::<Vec<_>>();
        assert_eq!(names(root), ["javax.annotation", "javax.xml.bind"]);
        assert_eq!(root.apis["javax.xml.bind"].0, 1);
        assert_eq!(web.dir, "web");
        assert_eq!(names(web), ["javax.xml.bind"]);
        assert_eq!(web.apis["javax.xml.bind"].0, 1);
    }
    #[test]
    fn spring_boot_is_found_when_a_third_party_bom_or_a_property_pins_it() {
        // Found with a real project: JHipster Online (Spring Boot 2.7.3) has no Spring Boot parent
        // and no Spring Boot BOM, only the property, the BOM of JHipster and bare starters.
        let jhipster = r#"<project><artifactId>jh</artifactId>
            <properties><java.version>11</java.version><spring-boot.version>2.7.3</spring-boot.version></properties>
            <dependencyManagement><dependencies><dependency>
              <groupId>tech.jhipster</groupId><artifactId>jhipster-dependencies</artifactId>
              <version>7.9.3</version><type>pom</type><scope>import</scope>
            </dependency></dependencies></dependencyManagement>
            <dependencies><dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-web</artifactId></dependency></dependencies>
            </project>"#;
        let dir = project(&[("pom.xml", jhipster)]);
        assert_eq!(inspect_dir(&dir).spring_boot.as_deref(), Some("2.7.3"));

        // the plugin version, also through a property
        let plugin = r#"<project><artifactId>a</artifactId><properties><boot>2.1.9.RELEASE</boot></properties>
            <build><plugins><plugin><groupId>org.springframework.boot</groupId>
              <artifactId>spring-boot-maven-plugin</artifactId><version>${boot}</version></plugin></plugins></build></project>"#;
        let dir = project(&[("pom.xml", plugin)]);
        assert_eq!(
            inspect_dir(&dir).spring_boot.as_deref(),
            Some("2.1.9.RELEASE")
        );

        // an explicit starter version
        let starter = r#"<project><artifactId>a</artifactId><dependencies><dependency>
            <groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter</artifactId><version>1.5.4.RELEASE</version>
            </dependency></dependencies></project>"#;
        let dir = project(&[("pom.xml", starter)]);
        assert_eq!(
            inspect_dir(&dir).spring_boot.as_deref(),
            Some("1.5.4.RELEASE")
        );

        // starters without any version, and a property that is not a version, say nothing
        let bare = r#"<project><artifactId>a</artifactId><properties><spring-boot.version>${x}</spring-boot.version></properties>
            <dependencies><dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter</artifactId></dependency></dependencies></project>"#;
        let dir = project(&[("pom.xml", bare)]);
        assert_eq!(inspect_dir(&dir).spring_boot, None);
        // and a project that has nothing to do with Spring Boot stays that way
        let dir = project(&[("pom.xml", "<project><artifactId>a</artifactId></project>")]);
        assert_eq!(inspect_dir(&dir).spring_boot, None);
    }
    #[test]
    fn a_spring_boot_pinned_by_a_third_party_bom_only_moves_the_java_level() {
        use Hop::*;
        // before 3.5 the ceiling is Java 17; from 3.5 on Java 25 is allowed
        assert_eq!(hops_pinned(Some(11), Some((2, 7))), [Java17]);
        assert_eq!(hops_pinned(Some(8), Some((2, 7))), [Java17]);
        assert_eq!(hops_pinned(Some(17), Some((2, 7))), []);
        assert_eq!(hops_pinned(Some(11), Some((3, 5))), [Java17, Java25]);
        assert_eq!(hops_pinned(Some(17), Some((3, 5))), [Java25]);
        assert_eq!(plan_hops(Some(11), Some((2, 7)), true), [Java17]);
        assert_eq!(
            plan_hops(Some(11), Some((2, 7)), false),
            hops_for(Some(11), Some((2, 7)))
        );
    }

    #[test]
    fn the_jhipster_bom_is_reported_as_the_one_that_pins_spring_boot() {
        // Found with a real project: JHipster Online. The Spring Boot recipes cannot move this
        // BOM, so the Spring Boot hops failed halfway and left the copy broken.
        let jhipster = r#"<project><artifactId>jh</artifactId>
            <properties><java.version>11</java.version><spring-boot.version>2.7.3</spring-boot.version></properties>
            <dependencyManagement><dependencies><dependency>
              <groupId>tech.jhipster</groupId><artifactId>jhipster-dependencies</artifactId>
              <version>7.9.3</version><type>pom</type><scope>import</scope>
            </dependency></dependencies></dependencyManagement></project>"#;
        let dir = project(&[("pom.xml", jhipster)]);
        let p = inspect_dir(&dir);
        assert_eq!(p.spring_boot.as_deref(), Some("2.7.3"));
        assert_eq!(
            p.spring_boot_pinned_by.as_deref(),
            Some("tech.jhipster:jhipster-dependencies")
        );
        assert!(p.upgrade_available, "Java 11 can still go to 17");

        // the Spring Boot BOM or parent next to it means Spring Boot is not pinned
        let with_boot_bom = jhipster.replace("</dependencies></dependencyManagement>", "<dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-dependencies</artifactId><version>2.7.3</version><type>pom</type><scope>import</scope></dependency></dependencies></dependencyManagement>");
        let dir = project(&[("pom.xml", &with_boot_bom)]);
        assert_eq!(inspect_dir(&dir).spring_boot_pinned_by, None);
        let dir = project(&[("pom.xml", BOOT15)]);
        assert_eq!(inspect_dir(&dir).spring_boot_pinned_by, None);
        // no Spring Boot, nothing to pin
        let dir = project(&[("pom.xml", "<project><artifactId>a</artifactId></project>")]);
        assert_eq!(inspect_dir(&dir).spring_boot_pinned_by, None);
    }
}
