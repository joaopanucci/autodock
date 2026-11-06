use serde::Serialize;
use std::{fs, io::Read, path::Path};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub path: String,
    pub technology: String,
    pub evidence: Vec<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub schema_version: u32,
    pub project: String,
    pub components: Vec<Component>,
    pub warnings: Vec<String>,
    pub findings: Vec<crate::findings::Finding>,
}
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_DIRS: usize = 10_000;
const MAX_DEPTH: usize = 32;
const SKIP: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    ".next",
    ".venv",
    "vendor",
    ".local-backups",
    ".autodock-bootstrap",
    ".local-backup",
    ".backups",
    "backups",
    ".cache",
    ".turbo",
    ".output",
    "coverage",
    ".gradle",
    ".idea",
    ".vscode",
    "__pycache__",
];

pub fn scan(path: &Path) -> Result<ScanReport, String> {
    let root = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if !root.is_dir() {
        return Err(tr!(
            "Não é diretório: {}",
            "Not a directory: {}",
            root.display()
        ));
    }
    let mut report = ScanReport {
        schema_version: 1,
        project: root.display().to_string(),
        components: vec![],
        warnings: vec![],
        findings: vec![],
    };
    let mut pending = vec![(root.clone(), 0)];
    let mut visited = 0;
    while let Some((dir, depth)) = pending.pop() {
        if visited >= MAX_DIRS {
            report.warnings.push(tr!(
                "Limite de 10000 diretórios atingido; análise parcial.",
                "Limit of 10000 directories reached; partial analysis."
            ));
            break;
        }
        visited += 1;
        for marker in [
            "package.json",
            "pom.xml",
            "Dockerfile",
            "compose.yaml",
            "compose.yml",
            "docker-compose.yaml",
            "docker-compose.yml",
        ] {
            let file = dir.join(marker);
            let meta = match fs::symlink_metadata(&file) {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    report.warnings.push(format!("{}: {e}", file.display()));
                    continue;
                }
            };
            if meta.file_type().is_symlink() || !meta.is_file() {
                report.warnings.push(tr!(
                    "{}: marcador não regular ignorado",
                    "{}: non-regular marker ignored",
                    file.display()
                ));
                continue;
            }
            if marker != "package.json" && marker != "pom.xml" {
                let relative = file
                    .strip_prefix(&root)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .replace('\\', "/");
                let module = dir
                    .strip_prefix(&root)
                    .unwrap_or(&dir)
                    .to_string_lossy()
                    .replace('\\', "/");
                report.findings.push(crate::findings::docker_file(
                    if module.is_empty() { "." } else { &module },
                    &relative,
                    marker,
                ));
                continue;
            }
            let text = (|| -> Result<String, String> {
                if meta.len() > MAX_BYTES {
                    return Err("manifesto excede 1 MiB".into());
                }
                let mut bytes = Vec::new();
                fs::File::open(&file)
                    .map_err(|e| e.to_string())?
                    .take(MAX_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() as u64 > MAX_BYTES {
                    return Err("manifesto excede 1 MiB".into());
                }
                String::from_utf8(bytes).map_err(|e| e.to_string())
            })();
            let relative = file
                .strip_prefix(&root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            let text = match text {
                Ok(t) => t,
                Err(e) => {
                    report.warnings.push(format!("{relative}: {e}"));
                    continue;
                }
            };
            let component_path = dir
                .strip_prefix(&root)
                .unwrap_or(&dir)
                .to_string_lossy()
                .replace('\\', "/");
            let component_path = if component_path.is_empty() {
                ".".into()
            } else {
                component_path
            };
            let mut add = |technology: &str, evidence: String| {
                report.components.push(Component {
                    path: component_path.clone(),
                    technology: technology.into(),
                    evidence: vec![evidence],
                })
            };
            if marker == "package.json" {
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(value) if value.is_object() => {
                        report.findings.extend(crate::findings::node(
                            &value,
                            &component_path,
                            &relative,
                        ));
                        add("nodejs", relative);
                    }
                    Ok(_) => report.warnings.push(tr!(
                        "{relative}: esperado objeto JSON",
                        "{relative}: expected a JSON object"
                    )),
                    Err(e) => report.warnings.push(tr!(
                        "{relative}: JSON inválido: {e}",
                        "{relative}: invalid JSON: {e}"
                    )),
                }
            } else {
                match roxmltree::Document::parse(&text) {
                    Ok(doc) if doc.root_element().tag_name().name() == "project" => {
                        report.findings.extend(crate::findings::maven(
                            &doc,
                            &component_path,
                            &relative,
                        ));
                        add("maven", relative.clone());
                        let project = doc.root_element();
                        // Only effective direct declarations, never comments, profiles or dependencyManagement.
                        for declaration in project.children().filter(|n| n.is_element()) {
                            let candidates: Vec<_> = if declaration.has_tag_name("parent") {
                                vec![declaration]
                            } else if declaration.has_tag_name("dependencies") {
                                declaration
                                    .children()
                                    .filter(|n| n.has_tag_name("dependency"))
                                    .collect()
                            } else {
                                vec![]
                            };
                            for node in candidates {
                                let child_text = |name| {
                                    node.children()
                                        .find(|n| n.has_tag_name(name))
                                        .and_then(|n| n.text())
                                        .unwrap_or("")
                                        .trim()
                                };
                                let group = child_text("groupId");
                                let artifact = child_text("artifactId");
                                let is_parent = node.has_tag_name("parent")
                                    && artifact == "spring-boot-starter-parent";
                                let is_dependency = node.has_tag_name("dependency")
                                    && (artifact == "spring-boot"
                                        || artifact == "spring-boot-autoconfigure"
                                        || artifact == "spring-boot-starter"
                                        || artifact.starts_with("spring-boot-starter-"));
                                if group == "org.springframework.boot"
                                    && (is_parent || is_dependency)
                                {
                                    add("spring-boot", format!("{relative}: {group}:{artifact}"));
                                    break;
                                }
                            }
                        }
                    }
                    Ok(_) => report.warnings.push(tr!(
                        "{relative}: raiz XML deve ser project",
                        "{relative}: the XML root must be project"
                    )),
                    Err(e) => report.warnings.push(tr!(
                        "{relative}: XML inválido: {e}",
                        "{relative}: invalid XML: {e}"
                    )),
                }
            }
        }
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                report.warnings.push(format!("{}: {e}", dir.display()));
                continue;
            }
        };
        let mut children = vec![];
        for entry in entries {
            match entry.and_then(|e| Ok((e.file_type()?, e.path(), e.file_name()))) {
                Ok((kind, path, name))
                    if kind.is_dir()
                        && !kind.is_symlink()
                        && !SKIP.contains(&name.to_string_lossy().as_ref()) =>
                {
                    if depth < MAX_DEPTH {
                        children.push((path, depth + 1));
                    } else {
                        report.warnings.push(tr!(
                            "{}: profundidade máxima atingida",
                            "{}: maximum depth reached",
                            path.display()
                        ));
                    }
                }
                Ok(_) => {}
                Err(e) => report.warnings.push(format!("{}: {e}", dir.display())),
            }
        }
        children.sort();
        pending.extend(children.into_iter().rev());
    }
    report
        .components
        .sort_by(|a, b| (&a.path, &a.technology).cmp(&(&b.path, &b.technology)));
    report
        .components
        .dedup_by(|a, b| a.path == b.path && a.technology == b.technology);
    Ok(report)
}
