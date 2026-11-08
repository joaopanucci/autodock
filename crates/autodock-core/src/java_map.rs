//! A map of a Java project folder, whatever its layout: which build files it has (Maven,
//! Gradle, Ant, Eclipse, IntelliJ, NetBeans), where the sources really are (found from the
//! `package` line of each file, not from folder names), which libraries it carries as `.jar`
//! files, which classes have a `main`, where the web application is and which Java level the
//! project files declare.
//!
//! This is the base for organizing a project that has no `pom.xml` (and for anything else that
//! needs to know what is in a folder). It only reads: nothing is executed or written, links and
//! Windows junctions are never followed, and every step is bounded.
use crate::{editor_fs::is_link, generate, java};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_FILES: usize = 100_000;
const MAX_DEPTH: usize = 40;
/// The head of a source file is enough for its package, imports and `main`.
const MAX_HEAD_BYTES: u64 = 256 * 1024;
/// A project file that declares the Java level is small.
const MAX_CONFIG_BYTES: u64 = 256 * 1024;
const MAX_JARS_HASHED: usize = 400;
const MAX_JAR_BYTES: u64 = 128 * 1024 * 1024;
/// How many entries of a list the report carries (the counts are always complete).
const MAX_LISTED: usize = 200;
const MAX_BUILD_FILES: usize = 20;

/// Folders of build output, dependencies and version control: nothing in them is the project.
const SKIPPED_DIRS: [&str; 14] = [
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "target",
    "out",
    "bin",
    "classes",
    "build",
    "dist",
    ".gradle",
    ".metadata",
    "__MACOSX",
    ".recommenders",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildSystem {
    /// `maven`, `gradle`, `ant`, `ivy`, `eclipse`, `intellij`, `netbeans` or `make`.
    pub kind: String,
    /// The files that tell, relative to the project folder.
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRoot {
    /// Relative to the project folder (`""`: the folder itself).
    pub dir: String,
    /// `main`, `test` or `mixed`.
    pub kind: String,
    pub files: usize,
    pub packages: usize,
    /// The package all the files share, e.g. `com.acme.shop` (empty: none).
    pub top_package: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainClass {
    pub file: String,
    /// Fully qualified, e.g. `com.acme.shop.Application`.
    pub class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub path: String,
    pub size: u64,
    pub sha256: Option<String>,
    /// What the file name says, e.g. `commons-lang` and `2.6` for `commons-lang-2.6.jar`.
    pub artifact: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaHint {
    pub level: u32,
    /// As written, e.g. `1.6`.
    pub declared: String,
    /// The file that says it.
    pub source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub java: usize,
    pub jars: usize,
    pub xml: usize,
    pub properties: usize,
    pub other: usize,
    pub folders: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaMap {
    pub schema_version: u32,
    pub path: String,
    pub name: String,
    /// `maven` (a pom.xml at the root), `standard` (src/main/java without a pom), `custom` (the
    /// sources are somewhere else) or `none` (no Java sources).
    pub layout: String,
    pub build_systems: Vec<BuildSystem>,
    pub source_roots: Vec<SourceRoot>,
    /// Java files in a folder that does not match their `package` line (a few listed).
    pub scattered_files: Vec<String>,
    pub scattered_count: usize,
    pub main_classes: Vec<MainClass>,
    pub libraries: Vec<Library>,
    pub library_count: usize,
    /// Folders that hold a `WEB-INF/web.xml`, relative to the project folder.
    pub web_apps: Vec<String>,
    pub java_level: Option<JavaHint>,
    pub counts: Counts,
    /// Java files that are not valid UTF-8 (usually ISO-8859-1 from an old Windows machine).
    pub non_utf8_files: usize,
    /// Cut by the limits on files or depth.
    pub truncated: bool,
    /// Things worth knowing before organizing the project.
    pub notes: Vec<String>,
}

/// What one `.java` file tells from its first bytes.
struct SourceInfo {
    /// Folder relative to the project folder.
    dir: String,
    file: String,
    package: String,
    is_test: bool,
    has_main: bool,
}

/// The `.java` file after the comments: its `package` (empty when it has none).
fn package_of(text: &str) -> String {
    let mut rest = text.trim_start_matches('\u{feff}');
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("//") {
            rest = after.split_once('\n').map_or("", |(_, tail)| tail);
        } else if let Some(after) = rest.strip_prefix("/*") {
            rest = after.split_once("*/").map_or("", |(_, tail)| tail);
        } else {
            break;
        }
    }
    let Some(after) = rest.strip_prefix("package") else {
        return String::new();
    };
    if !after.starts_with(char::is_whitespace) {
        return String::new();
    }
    let name: String = after
        .split(';')
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let valid = !name.is_empty()
        && name.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
                && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        });
    if valid {
        name
    } else {
        String::new()
    }
}

fn uses_test_framework(text: &str) -> bool {
    [
        "import org.junit",
        "import junit.",
        "import static org.junit",
        "import org.testng",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn has_main(text: &str) -> bool {
    text.contains("static") && text.contains("void main(")
}

/// `commons-lang-2.6.jar` → (`commons-lang`, `2.6`); `activation.jar` → (`activation`, none).
fn artifact_of(file_name: &str) -> (String, Option<String>) {
    let stem = file_name.strip_suffix(".jar").unwrap_or(file_name);
    let split = stem
        .char_indices()
        .filter(|(_, c)| *c == '-')
        .map(|(at, _)| at)
        .find(|at| {
            stem[at + 1..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        });
    match split {
        Some(at) => (stem[..at].to_string(), Some(stem[at + 1..].to_string())),
        None => (stem.to_string(), None),
    }
}

/// The value of `key` (`key=value`, or `key="value"`) in a text, the first time it appears.
fn value_after(text: &str, key: &str) -> Option<String> {
    let at = text.find(key)?;
    let rest = &text[at + key.len()..];
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let value: String = rest
        .chars()
        .take_while(|c| !matches!(c, '"' | '\n' | '\r' | ' ' | '>' | '\'' | '<'))
        .collect();
    (!value.is_empty()).then_some(value)
}

/// The Java level a project file declares, by kind of file.
fn level_in(file_name: &str, text: &str) -> Option<String> {
    let lower = file_name.to_ascii_lowercase();
    let declared = if lower == "org.eclipse.jdt.core.prefs" {
        value_after(text, "org.eclipse.jdt.core.compiler.source=")
    } else if lower == "project.properties" {
        value_after(text, "javac.source=")
    } else if lower.ends_with(".iml") {
        value_after(text, "languageLevel=\"JDK_").map(|v| v.replace('_', "."))
    } else if lower == ".classpath" {
        ["JavaSE-", "J2SE-"]
            .iter()
            .find_map(|marker| value_after(text, marker))
            .map(|v| v.trim_end_matches('/').to_string())
    } else if lower == "build.xml" {
        value_after(text, "source=\"")
    } else {
        None
    }?;
    java::level_of(&declared).map(|_| declared)
}

/// The build system a file name belongs to.
fn build_kind(file_name: &str, parent: &str) -> Option<&'static str> {
    match file_name {
        "pom.xml" => Some("maven"),
        "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" => {
            Some("gradle")
        }
        "build.xml" => Some("ant"),
        "ivy.xml" => Some("ivy"),
        ".classpath" | ".project" => Some("eclipse"),
        "Makefile" | "makefile" => Some("make"),
        "project.xml" if parent == "nbproject" => Some("netbeans"),
        name if name.ends_with(".iml") => Some("intellij"),
        _ => None,
    }
}

fn read_head(path: &Path, limit: u64) -> Option<(Vec<u8>, bool)> {
    let mut bytes = Vec::new();
    let file = fs::File::open(path).ok()?;
    file.take(limit + 1).read_to_end(&mut bytes).ok()?;
    let truncated = bytes.len() as u64 > limit;
    bytes.truncate(limit as usize);
    Some((bytes, truncated))
}

fn sha256_of(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

/// The folder a source file's `package` says it belongs to, `None` when the folder does not
/// match the package.
fn source_root(dir: &str, package: &str) -> Option<String> {
    let folders: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    let parts: Vec<&str> = package.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() > folders.len() || folders[folders.len() - parts.len()..] != parts[..] {
        return None;
    }
    Some(folders[..folders.len() - parts.len()].join("/"))
}

fn common_prefix(packages: &BTreeSet<String>) -> String {
    let mut iter = packages.iter().filter(|p| !p.is_empty());
    let Some(first) = iter.next() else {
        return String::new();
    };
    let mut prefix: Vec<&str> = first.split('.').collect();
    for package in iter {
        let parts: Vec<&str> = package.split('.').collect();
        let same = prefix
            .iter()
            .zip(&parts)
            .take_while(|(a, b)| a == b)
            .count();
        prefix.truncate(same);
    }
    prefix.join(".")
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

/// Reads a folder and describes what is in it.
pub fn map(path: &str) -> Result<JavaMap, String> {
    let root: PathBuf = generate::root_of(path)?;
    let mut counts = Counts::default();
    let mut systems: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut sources: Vec<SourceInfo> = vec![];
    let mut jars: Vec<(String, PathBuf, u64)> = vec![];
    let mut web_apps: BTreeSet<String> = BTreeSet::new();
    let mut hints: Vec<(u8, JavaHint)> = vec![];
    let mut non_utf8 = 0usize;
    let mut truncated = false;
    let mut visited = 0usize;

    let mut pending: Vec<(PathBuf, String, usize)> = vec![(root.clone(), String::new(), 0)];
    while let Some((dir, prefix, depth)) = pending.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = read.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        let parent_name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut folders = vec![];
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if is_link(&meta) {
                continue;
            }
            let relative = join(&prefix, &name);
            if meta.is_dir() {
                if SKIPPED_DIRS.contains(&name.as_str()) {
                    continue;
                }
                if depth + 1 > MAX_DEPTH {
                    truncated = true;
                    continue;
                }
                counts.folders += 1;
                folders.push((entry.path(), relative, depth + 1));
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            visited += 1;
            if visited > MAX_FILES {
                truncated = true;
                break;
            }
            counts.bytes += meta.len();
            if let Some(kind) = build_kind(&name, &parent_name) {
                let files = systems.entry(kind).or_default();
                if files.len() < MAX_BUILD_FILES {
                    files.push(relative.clone());
                }
            }
            if name == "web.xml" && parent_name == "WEB-INF" {
                let app = prefix.rsplit_once('/').map_or("", |(app, _)| app);
                web_apps.insert(app.to_string());
            }
            let lower = name.to_ascii_lowercase();
            let ext = lower.rsplit_once('.').map_or("", |(_, ext)| ext);
            match ext {
                "java" => {
                    counts.java += 1;
                    let Some((bytes, cut)) = read_head(&entry.path(), MAX_HEAD_BYTES) else {
                        continue;
                    };
                    let text = match std::str::from_utf8(&bytes) {
                        Ok(text) => text.to_string(),
                        Err(error) => {
                            // A character cut by the limit is not a bad file.
                            if !(cut && error.error_len().is_none()) {
                                non_utf8 += 1;
                            }
                            String::from_utf8_lossy(&bytes).into_owned()
                        }
                    };
                    sources.push(SourceInfo {
                        dir: prefix.clone(),
                        file: name.clone(),
                        package: package_of(&text),
                        is_test: uses_test_framework(&text),
                        has_main: has_main(&text),
                    });
                }
                "jar" => {
                    counts.jars += 1;
                    jars.push((relative.clone(), entry.path(), meta.len()));
                }
                "xml" => counts.xml += 1,
                "properties" => counts.properties += 1,
                _ => counts.other += 1,
            }
            // Project files that declare the Java level (the first kind found wins).
            let priority = match lower.as_str() {
                "org.eclipse.jdt.core.prefs" => Some(0u8),
                "project.properties" if parent_name == "nbproject" => Some(1),
                ".classpath" => Some(2),
                "build.xml" => Some(4),
                _ if lower.ends_with(".iml") => Some(3),
                _ => None,
            };
            if let (Some(priority), true) = (priority, meta.len() <= MAX_CONFIG_BYTES) {
                if let Some((bytes, _)) = read_head(&entry.path(), MAX_CONFIG_BYTES) {
                    let text = String::from_utf8_lossy(&bytes);
                    if let Some(declared) = level_in(&name, &text) {
                        if let Some(level) = java::level_of(&declared) {
                            hints.push((
                                priority,
                                JavaHint {
                                    level,
                                    declared,
                                    source: relative.clone(),
                                },
                            ));
                        }
                    }
                }
            }
        }
        if visited > MAX_FILES {
            break;
        }
        // Sorted, and popped from the end: reversed so the folders are visited in name order.
        folders.reverse();
        pending.extend(folders);
    }

    // Where the sources are: the folder above the package path of each file.
    struct Root {
        files: usize,
        tests: usize,
        packages: BTreeSet<String>,
    }
    let mut roots: BTreeMap<String, Root> = BTreeMap::new();
    let mut scattered: Vec<String> = vec![];
    let mut scattered_count = 0usize;
    let mut main_classes: Vec<MainClass> = vec![];
    for source in &sources {
        let relative = join(&source.dir, &source.file);
        if source.has_main && main_classes.len() < MAX_LISTED {
            let stem = source.file.strip_suffix(".java").unwrap_or(&source.file);
            main_classes.push(MainClass {
                file: relative.clone(),
                class: if source.package.is_empty() {
                    stem.to_string()
                } else {
                    format!("{}.{stem}", source.package)
                },
            });
        }
        let Some(dir) = source_root(&source.dir, &source.package) else {
            scattered_count += 1;
            if scattered.len() < MAX_LISTED {
                scattered.push(relative);
            }
            continue;
        };
        let entry = roots.entry(dir).or_insert_with(|| Root {
            files: 0,
            tests: 0,
            packages: BTreeSet::new(),
        });
        entry.files += 1;
        entry.tests += usize::from(source.is_test);
        entry.packages.insert(source.package.clone());
    }
    let mut source_roots: Vec<SourceRoot> = roots
        .into_iter()
        .map(|(dir, root)| {
            let named_test = dir
                .split('/')
                .any(|part| matches!(part.to_ascii_lowercase().as_str(), "test" | "tests"));
            let kind = if named_test || root.tests == root.files {
                "test"
            } else if root.tests == 0 {
                "main"
            } else {
                "mixed"
            };
            SourceRoot {
                dir,
                kind: kind.into(),
                files: root.files,
                packages: root.packages.len(),
                top_package: common_prefix(&root.packages),
            }
        })
        .collect();
    source_roots.sort_by(|a, b| b.files.cmp(&a.files).then_with(|| a.dir.cmp(&b.dir)));
    source_roots.truncate(MAX_LISTED);

    // Libraries: the jars the project carries, hashed so they can be matched to Maven coordinates.
    jars.sort();
    let library_count = jars.len();
    let libraries: Vec<Library> = jars
        .iter()
        .take(MAX_LISTED)
        .enumerate()
        .map(|(index, (relative, path, size))| {
            let file_name = relative.rsplit('/').next().unwrap_or(relative);
            let (artifact, version) = artifact_of(file_name);
            Library {
                path: relative.clone(),
                size: *size,
                sha256: (index < MAX_JARS_HASHED && *size <= MAX_JAR_BYTES)
                    .then(|| sha256_of(path))
                    .flatten(),
                artifact,
                version,
            }
        })
        .collect();

    let build_systems: Vec<BuildSystem> = systems
        .into_iter()
        .map(|(kind, files)| BuildSystem {
            kind: kind.into(),
            files,
        })
        .collect();
    hints.sort_by_key(|(priority, hint)| (*priority, hint.source.clone()));
    let java_level = hints.into_iter().next().map(|(_, hint)| hint);

    let has_pom = build_systems
        .iter()
        .any(|s| s.kind == "maven" && s.files.iter().any(|f| f == "pom.xml"));
    let layout = if counts.java == 0 {
        "none"
    } else if has_pom {
        "maven"
    } else if source_roots.iter().any(|r| r.dir == "src/main/java") {
        "standard"
    } else {
        "custom"
    };

    let mut notes = vec![];
    if counts.java == 0 {
        notes.push(tr!(
            "Nenhum arquivo .java foi encontrado nesta pasta.",
            "No .java file was found in this folder."
        ));
    }
    if counts.java > 0 && !has_pom {
        notes.push(if build_systems.is_empty() {
            tr!(
                "A pasta não tem arquivo de build (pom.xml, build.xml, build.gradle…): um projeto Maven teria de ser criado do zero.",
                "The folder has no build file (pom.xml, build.xml, build.gradle…): a Maven project would have to be created from scratch."
            )
        } else {
            tr!(
                "A pasta não tem pom.xml: o upgrade para Java 25 e Spring Boot 4 precisa de um projeto Maven.",
                "The folder has no pom.xml: the upgrade to Java 25 and Spring Boot 4 needs a Maven project."
            )
        });
    }
    if counts.jars > 0 && !has_pom {
        notes.push(tr!(
            "{} biblioteca(s) .jar vêm junto com o projeto: num projeto Maven elas viram dependências, achadas pelo hash do arquivo.",
            "{} library .jar file(s) come with the project: in a Maven project they become dependencies, found by the file's hash.",
            counts.jars
        ));
    }
    if scattered_count > 0 {
        notes.push(tr!(
            "{} arquivo(s) .java estão numa pasta que não bate com o seu package.",
            "{} .java file(s) are in a folder that does not match their package.",
            scattered_count
        ));
    }
    if non_utf8 > 0 {
        notes.push(tr!(
            "{} arquivo(s) .java não estão em UTF-8 (provavelmente ISO-8859-1): o build precisa declarar a codificação dos fontes.",
            "{} .java file(s) are not UTF-8 (probably ISO-8859-1): the build has to declare the source encoding.",
            non_utf8
        ));
    }
    if truncated {
        notes.push(tr!(
            "A pasta é grande demais para ser lida inteira: o mapa é parcial.",
            "The folder is too big to be read in full: the map is partial."
        ));
    }

    Ok(JavaMap {
        schema_version: 1,
        path: generate::plain_path(&root),
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        layout: layout.into(),
        build_systems,
        source_roots,
        scattered_files: scattered,
        scattered_count,
        main_classes,
        libraries,
        library_count,
        web_apps: web_apps.into_iter().take(MAX_LISTED).collect(),
        java_level,
        counts,
        non_utf8_files: non_utf8,
        truncated,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder with the given files (text) in it.
    fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in files {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let root = dir.path().to_string_lossy().into_owned();
        (dir, root)
    }

    const APP: &str = "package com.acme.shop;\n\npublic class App {\n    public static void main(String[] args) {}\n}\n";
    const ORDER: &str = "package com.acme.shop.model;\n\npublic class Order {}\n";
    const ORDER_TEST: &str =
        "package com.acme.shop.model;\n\nimport org.junit.Test;\n\npublic class OrderTest {}\n";

    /// What an old Eclipse + Ant project looks like: no pom, sources in `src`, tests in `test`,
    /// jars in `lib`, compiled classes in `bin`, a web folder.
    fn messy() -> (tempfile::TempDir, String) {
        project(&[
            (".project", "<projectDescription/>"),
            (
                ".classpath",
                "<classpath><classpathentry kind=\"con\" path=\"org.eclipse.jdt.launching.JRE_CONTAINER/org.eclipse.jdt.internal.debug.ui.launcher.StandardVMType/JavaSE-1.6\"/></classpath>",
            ),
            ("build.xml", "<project name=\"shop\" default=\"dist\"/>"),
            ("src/com/acme/shop/App.java", APP),
            ("src/com/acme/shop/model/Order.java", ORDER),
            ("test/com/acme/shop/model/OrderTest.java", ORDER_TEST),
            ("lib/commons-lang-2.6.jar", "not really a jar"),
            ("lib/log4j-1.2.17.jar", "another one"),
            ("lib/activation.jar", "third"),
            ("WebContent/WEB-INF/web.xml", "<web-app/>"),
            ("WebContent/index.jsp", "<html/>"),
            ("bin/com/acme/shop/App.class", "bytes"),
            ("bin/Generated.java", "class Generated {}"),
        ])
    }

    #[test]
    fn an_ant_and_eclipse_project_without_a_pom_is_mapped_from_what_is_in_it() {
        let (_dir, root) = messy();
        let map = map(&root).unwrap();
        assert_eq!(map.layout, "custom");
        assert_eq!(
            map.build_systems
                .iter()
                .map(|s| s.kind.as_str())
                .collect::<Vec<_>>(),
            ["ant", "eclipse"]
        );
        // the sources are found from their package lines; `bin` is build output and is skipped
        assert_eq!(map.counts.java, 3);
        assert_eq!(
            map.source_roots
                .iter()
                .map(|r| (r.dir.as_str(), r.kind.as_str(), r.files))
                .collect::<Vec<_>>(),
            [("src", "main", 2), ("test", "test", 1)]
        );
        assert_eq!(map.source_roots[0].top_package, "com.acme.shop");
        assert_eq!(map.source_roots[0].packages, 2);
        assert_eq!(
            map.main_classes,
            [MainClass {
                file: "src/com/acme/shop/App.java".into(),
                class: "com.acme.shop.App".into()
            }]
        );
        assert_eq!(map.web_apps, ["WebContent"]);
        assert_eq!(map.scattered_count, 0);
        let hint = map.java_level.unwrap();
        assert_eq!((hint.level, hint.declared.as_str()), (6, "1.6"));
        assert_eq!(hint.source, ".classpath");
        // there is no pom, so the notes say what that means
        assert!(map.notes.iter().any(|n| n.contains("pom.xml")));
        assert!(map.notes.iter().any(|n| n.contains('3')));
    }

    #[test]
    fn libraries_are_listed_with_what_their_name_says_and_a_hash_of_the_file() {
        let (_dir, root) = messy();
        let map = map(&root).unwrap();
        assert_eq!(map.library_count, 3);
        assert_eq!(
            map.libraries
                .iter()
                .map(|l| (l.path.as_str(), l.artifact.as_str(), l.version.as_deref()))
                .collect::<Vec<_>>(),
            [
                ("lib/activation.jar", "activation", None),
                ("lib/commons-lang-2.6.jar", "commons-lang", Some("2.6")),
                ("lib/log4j-1.2.17.jar", "log4j", Some("1.2.17")),
            ]
        );
        // sha256("another one"), computed from the bytes of the file
        let log4j = &map.libraries[2];
        assert_eq!(log4j.size, 11);
        let expected: String = Sha256::digest(b"another one")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(log4j.sha256.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn the_names_of_jars_are_split_at_the_version() {
        assert_eq!(
            artifact_of("commons-lang3-3.12.0.jar"),
            ("commons-lang3".into(), Some("3.12.0".into()))
        );
        assert_eq!(
            artifact_of("spring-core-5.3.30.RELEASE.jar"),
            ("spring-core".into(), Some("5.3.30.RELEASE".into()))
        );
        assert_eq!(artifact_of("activation.jar"), ("activation".into(), None));
        assert_eq!(artifact_of("jaxb-api.jar"), ("jaxb-api".into(), None));
    }

    #[test]
    fn files_in_the_wrong_folder_and_in_the_default_package_are_told_apart() {
        let (_dir, root) = project(&[
            ("src/Main.java", "public class Main {}\n"),
            (
                "src/wrong/Place.java",
                "package com.acme.shop;\nclass Place {}\n",
            ),
            (
                "src/com/acme/Right.java",
                "package com.acme;\nclass Right {}\n",
            ),
        ]);
        let map = map(&root).unwrap();
        assert_eq!(map.scattered_files, ["src/wrong/Place.java"]);
        assert_eq!(map.scattered_count, 1);
        // the default package lives in `src` itself, next to the package folders
        assert_eq!(map.source_roots.len(), 1);
        assert_eq!(map.source_roots[0].dir, "src");
        assert_eq!(map.source_roots[0].files, 2);
        assert!(map.notes.iter().any(|n| n.contains("package")));
    }

    #[test]
    fn a_standard_layout_without_a_pom_and_a_maven_project_are_told_apart() {
        let files = [
            (
                "src/main/java/com/acme/App.java",
                "package com.acme;\npublic class App {}\n",
            ),
            (
                "src/test/java/com/acme/AppTest.java",
                "package com.acme;\nimport org.junit.Test;\npublic class AppTest {}\n",
            ),
        ];
        let (_dir, root) = project(&files);
        let standard = map(&root).unwrap();
        assert_eq!(standard.layout, "standard");
        assert_eq!(
            standard
                .source_roots
                .iter()
                .map(|r| (r.dir.as_str(), r.kind.as_str()))
                .collect::<Vec<_>>(),
            [("src/main/java", "main"), ("src/test/java", "test")]
        );

        let (_dir, root) = project(&[files[0], files[1], ("pom.xml", "<project/>")]);
        let maven = map(&root).unwrap();
        assert_eq!(maven.layout, "maven");
        // a Maven project needs no notes about a missing pom
        assert!(!maven.notes.iter().any(|n| n.contains("pom.xml")));
    }

    #[test]
    fn a_source_with_comments_a_byte_order_mark_or_no_package_is_read_right() {
        assert_eq!(package_of("package a.b.c;\nclass X {}"), "a.b.c");
        assert_eq!(
            package_of("\u{feff}/* license\n * text */\n// note\npackage  org.x ;\n"),
            "org.x"
        );
        assert_eq!(package_of("package\ncom.acme\n  .shop;"), "com.acme.shop");
        assert_eq!(package_of("public class X {}"), "");
        assert_eq!(package_of("packages.Thing x;"), "");
        assert_eq!(package_of("package 1bad;"), "");
        assert_eq!(package_of(""), "");
    }

    #[test]
    fn sources_that_are_not_utf8_are_counted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("src/Cafe.java");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // "// café" in ISO-8859-1: the é is the single byte 0xE9
        fs::write(&path, b"package a;\n// caf\xE9\nclass Cafe {}\n").unwrap();
        fs::write(
            dir.path().join("src/Ok.java"),
            "package a;\n// café\nclass Ok {}\n",
        )
        .unwrap();
        let map = map(&dir.path().to_string_lossy()).unwrap();
        assert_eq!(map.counts.java, 2);
        assert_eq!(map.non_utf8_files, 1);
        assert!(map.notes.iter().any(|n| n.contains("UTF-8")));
    }

    #[test]
    fn each_kind_of_project_file_can_declare_the_java_level() {
        for (file, text, declared, level) in [
            (
                ".settings/org.eclipse.jdt.core.prefs",
                "eclipse.preferences.version=1\norg.eclipse.jdt.core.compiler.source=1.5\n",
                "1.5",
                5,
            ),
            (
                "nbproject/project.properties",
                "javac.source=1.7\njavac.target=1.7\n",
                "1.7",
                7,
            ),
            (
                "shop.iml",
                "<module><component name=\"NewModuleRootManager\" languageLevel=\"JDK_1_8\"/></module>",
                "1.8",
                8,
            ),
            (
                "build.xml",
                "<project><target name=\"c\"><javac srcdir=\"src\" source=\"1.4\" target=\"1.4\"/></target></project>",
                "1.4",
                4,
            ),
            (
                "shop.iml",
                "<module><component languageLevel=\"JDK_11\"/></module>",
                "11",
                11,
            ),
        ] {
            let (_dir, root) = project(&[(file, text), ("src/A.java", "class A {}\n")]);
            let hint = map(&root).unwrap().java_level.unwrap();
            assert_eq!(
                (hint.declared.as_str(), hint.level),
                (declared, level),
                "{file}"
            );
            assert_eq!(hint.source, file);
        }
        // a value that is not a version (a property reference) says nothing
        let (_dir, root) = project(&[("build.xml", "<javac source=\"${java.version}\"/>")]);
        assert!(map(&root).unwrap().java_level.is_none());
    }

    #[test]
    fn the_most_specific_project_file_decides_the_level() {
        let (_dir, root) = project(&[
            (
                ".settings/org.eclipse.jdt.core.prefs",
                "org.eclipse.jdt.core.compiler.source=1.6\n",
            ),
            ("build.xml", "<javac source=\"1.8\"/>"),
        ]);
        assert_eq!(map(&root).unwrap().java_level.unwrap().level, 6);
    }

    #[test]
    fn build_output_dependencies_and_version_control_are_not_part_of_the_project() {
        let (_dir, root) = project(&[
            ("src/A.java", "class A {}\n"),
            ("target/classes/B.java", "class B {}\n"),
            ("node_modules/x/C.java", "class C {}\n"),
            (".git/D.java", "class D {}\n"),
            ("build/E.java", "class E {}\n"),
            ("out/F.java", "class F {}\n"),
            ("target/lib.jar", "x"),
        ]);
        let map = map(&root).unwrap();
        assert_eq!(map.counts.java, 1);
        assert_eq!(map.library_count, 0);
    }

    #[test]
    fn a_folder_without_java_says_so_and_a_file_is_not_a_folder() {
        let (dir, root) = project(&[("README.md", "hello")]);
        let empty = map(&root).unwrap();
        assert_eq!(empty.layout, "none");
        assert!(empty.source_roots.is_empty());
        assert!(empty.notes.iter().any(|n| n.contains(".java")));
        assert!(map(&dir.path().join("README.md").to_string_lossy()).is_err());
        assert!(map("/nowhere/at/all").is_err());
    }

    #[test]
    fn several_build_systems_and_modules_are_all_listed() {
        let (_dir, root) = project(&[
            ("pom.xml", "<project/>"),
            ("api/pom.xml", "<project/>"),
            ("web/build.gradle", ""),
            ("settings.gradle", ""),
            ("Makefile", "all:\n"),
            ("nbproject/project.xml", "<project/>"),
            ("a.java", ""),
        ]);
        let map = map(&root).unwrap();
        let kinds: Vec<(&str, usize)> = map
            .build_systems
            .iter()
            .map(|s| (s.kind.as_str(), s.files.len()))
            .collect();
        assert_eq!(
            kinds,
            [("gradle", 2), ("make", 1), ("maven", 2), ("netbeans", 1)]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_is_never_followed_out_of_the_folder() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("Secret.java"), "class Secret {}\n").unwrap();
        fs::write(outside.path().join("secret.jar"), "x").unwrap();
        let (dir, root) = project(&[("src/A.java", "class A {}\n")]);
        std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("Secret.java"),
            dir.path().join("src/S.java"),
        )
        .unwrap();
        let map = map(&root).unwrap();
        assert_eq!((map.counts.java, map.library_count), (1, 0));
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_is_never_followed_out_of_the_folder() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("Secret.java"), "class Secret {}\n").unwrap();
        let (dir, root) = project(&[("src/A.java", "class A {}\n")]);
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(dir.path().join("out"))
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(made.status.success(), "mklink failed: {made:?}");
        assert_eq!(map(&root).unwrap().counts.java, 1);
        fs::remove_dir(dir.path().join("out")).unwrap();
        assert!(outside.path().join("Secret.java").exists());
    }

    #[test]
    fn the_same_folder_always_gives_the_same_map_and_the_lists_are_bounded() {
        let (_dir, root) = messy();
        assert_eq!(map(&root).unwrap(), map(&root).unwrap());
        let dir = tempfile::tempdir().unwrap();
        for index in 0..(MAX_LISTED + 20) {
            let path = dir.path().join(format!("lib/a{index:04}-1.0.jar"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        let big = map(&dir.path().to_string_lossy()).unwrap();
        assert_eq!(big.library_count, MAX_LISTED + 20);
        assert_eq!(big.libraries.len(), MAX_LISTED);
    }
}
