use serde::Serialize;
use serde_json::Value;

/// Additive evidence; declarations are not proof of installation or compatibility.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub module: String,
    pub kind: String,
    pub name: String,
    pub value: Option<String>,
    pub source: String,
    pub status: String,
}

fn declared(module: &str, kind: &str, name: &str, value: &str, source: String) -> Finding {
    Finding {
        module: module.into(),
        kind: kind.into(),
        name: name.into(),
        value: Some(value.into()),
        source,
        status: "declared".into(),
    }
}

pub fn node(value: &Value, module: &str, source: &str) -> Vec<Finding> {
    let mut findings = vec![];
    if let Some(engines) = value.get("engines").and_then(Value::as_object) {
        for (name, version) in engines {
            if let Some(version) = version.as_str().filter(|v| !v.trim().is_empty()) {
                findings.push(declared(
                    module,
                    "runtime",
                    name,
                    version,
                    format!("{source} → engines.{name}"),
                ));
            }
        }
    }
    if let Some(manager) = value
        .get("packageManager")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        findings.push(declared(
            module,
            "package_manager",
            "packageManager",
            manager,
            format!("{source} → packageManager"),
        ));
    }
    for section in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(deps) = value.get(section).and_then(Value::as_object) {
            for (name, version) in deps {
                if let Some(version) = version.as_str().filter(|v| !v.trim().is_empty()) {
                    let kind = match name.as_str() {
                        "react" | "next" | "vue" | "nuxt" | "@angular/core" | "svelte"
                        | "express" | "@nestjs/core" => "framework",
                        "vite" | "webpack" | "typescript" | "rollup" | "esbuild" => "build_tool",
                        _ => "dependency",
                    };
                    findings.push(declared(
                        module,
                        kind,
                        name,
                        version,
                        format!("{source} → {section}.{name}"),
                    ));
                }
            }
        }
    }
    findings
}

pub fn maven(doc: &roxmltree::Document<'_>, module: &str, source: &str) -> Vec<Finding> {
    let mut findings = vec![];
    let project = doc.root_element();
    for properties in project.children().filter(|n| n.has_tag_name("properties")) {
        for property in properties.children().filter(|n| n.is_element()) {
            let name = property.tag_name().name();
            if [
                "java.version",
                "maven.compiler.release",
                "maven.compiler.source",
                "maven.compiler.target",
            ]
            .contains(&name)
            {
                if let Some(version) = property.text().map(str::trim).filter(|v| !v.is_empty()) {
                    findings.push(declared(
                        module,
                        "runtime",
                        name,
                        version,
                        format!("{source} → properties.{name}"),
                    ));
                }
            }
        }
    }
    for node in project
        .children()
        .filter(|n| n.has_tag_name("parent"))
        .chain(
            project
                .children()
                .filter(|n| n.has_tag_name("dependencies"))
                .flat_map(|n| n.children().filter(|c| c.has_tag_name("dependency"))),
        )
    {
        let text = |name| {
            node.children()
                .find(|n| n.has_tag_name(name))
                .and_then(|n| n.text())
                .unwrap_or("")
                .trim()
        };
        let group = text("groupId");
        let artifact = text("artifactId");
        if group.is_empty() || artifact.is_empty() {
            continue;
        }
        let kind = if node.has_tag_name("parent") {
            "parent"
        } else {
            "dependency"
        };
        let mut finding = declared(
            module,
            kind,
            &format!("{group}:{artifact}"),
            text("version"),
            format!("{source} → {kind} {group}:{artifact}"),
        );
        if text("version").is_empty() {
            finding.value = None;
        }
        findings.push(finding);
    }
    findings
}

pub fn docker_file(module: &str, source: &str, name: &str) -> Finding {
    Finding {
        module: module.into(),
        kind: "container_config".into(),
        name: name.into(),
        value: None,
        source: source.into(),
        status: "detected".into(),
    }
}
