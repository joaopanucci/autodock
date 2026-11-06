use autodock_core::scan;
use std::fs;

#[test]
fn node_declarations_preserve_ranges_and_provenance() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("package.json"), r#"{"engines":{"node":">=22"},"packageManager":"pnpm@10.0.0","dependencies":{"next":"^15.0.0","db-client":"workspace:*"},"devDependencies":{"vite":"^6"}}"#).unwrap();
    let report = scan(dir.path()).unwrap();
    assert_eq!(report.components.len(), 1);
    assert!(report
        .findings
        .iter()
        .any(|f| f.name == "node" && f.value.as_deref() == Some(">=22") && f.status == "declared"));
    assert!(report.findings.iter().any(|f| f.name == "next"
        && f.kind == "framework"
        && f.source.contains("dependencies.next")));
    assert!(report
        .findings
        .iter()
        .any(|f| f.name == "vite" && f.kind == "build_tool"));
    assert!(report
        .findings
        .iter()
        .any(|f| f.name == "db-client" && f.value.as_deref() == Some("workspace:*")));
    assert!(!report.findings.iter().any(|f| f.status == "installed"));
}

#[test]
fn scripts_and_non_string_dependencies_are_not_framework_evidence() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("package.json"), r#"{"description":"next react","scripts":{"start":"next dev"},"dependencies":{"react":false},"engines":{"node":22}}"#).unwrap();
    assert!(scan(dir.path()).unwrap().findings.is_empty());
}

#[test]
fn invalid_manifests_do_not_create_declarations() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("package.json"), "{broken").unwrap();
    fs::write(dir.path().join("pom.xml"), "<project>").unwrap();
    let report = scan(dir.path()).unwrap();
    assert!(report.findings.is_empty());
    assert_eq!(report.warnings.len(), 2);
}

#[test]
fn maven_properties_are_literal_not_resolved_or_guessed() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("pom.xml"), "<project><properties><java.version>21</java.version><maven.compiler.release>${java.version}</maven.compiler.release></properties><dependencies><dependency><groupId>org.example</groupId><artifactId>library</artifactId></dependency></dependencies><profiles><profile><properties><java.version>99</java.version></properties></profile></profiles></project>").unwrap();
    let report = scan(dir.path()).unwrap();
    assert_eq!(report.findings.len(), 3);
    assert!(report
        .findings
        .iter()
        .any(|f| f.name == "java.version" && f.value.as_deref() == Some("21")));
    assert!(report
        .findings
        .iter()
        .any(|f| f.value.as_deref() == Some("${java.version}")));
    assert!(report
        .findings
        .iter()
        .any(|f| f.name == "org.example:library" && f.value.is_none()));
}

#[test]
fn docker_presence_is_not_a_validity_or_runtime_claim() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("compose.yaml"), "not: [valid yaml").unwrap();
    fs::write(dir.path().join("Dockerfile"), "not a valid Dockerfile").unwrap();
    fs::write(dir.path().join("Dockerfile.example"), "FROM anything").unwrap();
    let report = scan(dir.path()).unwrap();
    assert!(report.components.is_empty());
    assert_eq!(report.findings.len(), 2);
    assert!(report
        .findings
        .iter()
        .all(|f| f.status == "detected" && f.value.is_none()));
}

#[test]
fn multi_module_findings_respect_ignored_directories() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "apps/web",
        "services/api",
        ".local-backups/old",
        "node_modules/dep",
        ".cache/old",
    ] {
        let path = dir.path().join(name);
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("package.json"),
            r#"{"engines":{"node":"22.0.0"}}"#,
        )
        .unwrap();
        fs::write(path.join("compose.yml"), "").unwrap();
    }
    let report = scan(dir.path()).unwrap();
    assert_eq!(report.components.len(), 2);
    assert_eq!(report.findings.len(), 4);
    assert!(report
        .findings
        .iter()
        .all(|f| ["apps/web", "services/api"].contains(&f.module.as_str())));
    let json = serde_json::to_value(report).unwrap();
    assert_eq!(json["schemaVersion"], 1);
    assert!(
        json["components"].is_array() && json["warnings"].is_array() && json["findings"].is_array()
    );
}

#[cfg(unix)]
#[test]
fn symlinked_docker_files_are_not_detected() {
    let outside = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("compose.yml"), "services: {}").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("compose.yml"),
        project.path().join("compose.yml"),
    )
    .unwrap();
    assert!(scan(project.path()).unwrap().findings.is_empty());
}
