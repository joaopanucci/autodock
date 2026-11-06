use autodock_core::scan;
use std::{fs, path::PathBuf};
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}
#[test]
fn detects_node_and_does_not_execute_scripts() {
    let result = scan(&fixture("node-basic")).unwrap();
    assert_eq!(result.components.len(), 1);
    assert_eq!(result.components[0].technology, "nodejs");
    assert_eq!(result.components[0].evidence, ["package.json"]);
}
#[test]
fn detects_maven_without_inventing_spring() {
    let result = scan(&fixture("maven-basic")).unwrap();
    assert_eq!(result.components.len(), 1);
    assert_eq!(result.components[0].technology, "maven");
}
#[test]
fn detects_spring_parent() {
    let result = scan(&fixture("spring-boot-basic")).unwrap();
    assert!(result
        .components
        .iter()
        .any(|c| c.technology == "spring-boot"));
    assert!(result.components.iter().any(|c| c.technology == "maven"));
}
#[test]
fn detects_spring_dependency_with_evidence() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("pom.xml"), "<project><dependencies><dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-web</artifactId></dependency></dependencies></project>").unwrap();
    let result = scan(dir.path()).unwrap();
    assert!(result.components.iter().any(
        |c| c.technology == "spring-boot" && c.evidence[0].contains("spring-boot-starter-web")
    ));
}
#[test]
fn ignores_comments_unrelated_groups_and_dependency_management() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("pom.xml"), "<project><!-- org.springframework.boot spring-boot-starter-parent --><dependencies><dependency><groupId>example</groupId><artifactId>spring-boot-starter-web</artifactId></dependency></dependencies><dependencyManagement><dependencies><dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter</artifactId></dependency></dependencies></dependencyManagement></project>").unwrap();
    assert_eq!(scan(dir.path()).unwrap().components.len(), 1);
}
#[test]
fn empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let report = scan(dir.path()).unwrap();
    assert!(report.components.is_empty());
    assert!(report.warnings.is_empty());
}
#[test]
fn missing_path_and_file_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    assert!(scan(&dir.path().join("missing")).is_err());
    assert!(scan(&fixture("node-basic").join("package.json")).is_err());
}
#[test]
fn serializes_report_contract() {
    let value = serde_json::to_value(scan(&fixture("node-basic")).unwrap()).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["components"][0]["technology"], "nodejs");
    assert!(value["warnings"].is_array());
}
#[test]
fn nested_projects_and_exclusions() {
    let dir = tempfile::tempdir().unwrap();
    for path in ["apps/web", "node_modules/ignored", "target/ignored"] {
        fs::create_dir_all(dir.path().join(path)).unwrap();
        fs::write(dir.path().join(path).join("package.json"), "{}").unwrap();
    }
    let result = scan(dir.path()).unwrap();
    assert_eq!(result.components.len(), 1);
    assert_eq!(result.components[0].path, "apps/web");
}
#[test]
fn ignores_backups_and_generated_directories() {
    let dir = tempfile::tempdir().unwrap();

    let excluded = [
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

    for name in excluded {
        let nested = dir.path().join(name).join("ignored");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("package.json"), "{}").unwrap();
    }

    let actual = dir.path().join("apps").join("web");
    fs::create_dir_all(&actual).unwrap();
    fs::write(actual.join("package.json"), "{}").unwrap();

    let report = scan(dir.path()).unwrap();

    assert_eq!(report.components.len(), 1);
    assert_eq!(report.components[0].path, "apps/web");
    assert_eq!(report.components[0].technology, "nodejs");
}
#[test]
fn malformed_and_oversized_manifests_warn() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("package.json"), "not json").unwrap();
    fs::write(dir.path().join("pom.xml"), "<project>").unwrap();
    assert_eq!(scan(dir.path()).unwrap().warnings.len(), 2);
    fs::write(dir.path().join("package.json"), vec![b' '; 1024 * 1024 + 1]).unwrap();
    assert!(scan(dir.path())
        .unwrap()
        .warnings
        .iter()
        .any(|w| w.contains("1 MiB")));
}
#[cfg(unix)]
#[test]
fn does_not_follow_symlink_markers_or_directories() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    symlink(
        fixture("node-basic").join("package.json"),
        dir.path().join("package.json"),
    )
    .unwrap();
    symlink(fixture("node-basic"), dir.path().join("linked")).unwrap();
    assert!(scan(dir.path()).unwrap().components.is_empty());
}
