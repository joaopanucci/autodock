//! Turning a Java folder that has no `pom.xml` (Ant, Eclipse, loose jars) into a Maven project,
//! in a copy.
//!
//! The original folder is never touched. The copy gets the Maven layout (`src/main/java`,
//! `src/test/java`, resources, the web application), a generated `pom.xml` and its libraries as
//! dependencies: the ones whose file name is well known become real coordinates, the others stay
//! as local jars. Nothing is compiled or run here, and nothing leaves the computer; once the copy
//! is a Maven project, the Java upgrade can run on it.
use crate::{
    editor_fs::is_link,
    generate,
    java_map::{self, JavaMap, Walk},
    java_upgrade,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const MAX_COPY_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_LISTED: usize = 200;
/// What a generated pom assumes when the project does not say which Java it was written for.
const ASSUMED_JAVA: u32 = 8;

/// What the plan and the report let the person choose.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertOptions {
    /// Folder that will hold the copy (default: next to the project).
    pub into: Option<String>,
    pub group_id: Option<String>,
    pub artifact_id: Option<String>,
    pub version: Option<String>,
    pub java_level: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Move {
    /// Relative to the project folder.
    pub from: String,
    /// Relative to the copy.
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
    /// `compile`, `provided`, `test` or `system` (a jar kept in `lib/`).
    pub scope: String,
    /// The jar this comes from, relative to the project folder.
    pub jar: String,
    /// The coordinates are known from the file name (`false`: kept as a local jar).
    pub known: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertCounts {
    pub main_sources: usize,
    pub test_sources: usize,
    pub main_resources: usize,
    pub test_resources: usize,
    pub webapp_files: usize,
    /// Sources that were in a folder that does not match their package and are put where it says.
    pub moved_to_their_package: usize,
    pub libraries_known: usize,
    pub libraries_local: usize,
    pub not_copied: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertPlan {
    pub schema_version: u32,
    pub path: String,
    pub name: String,
    pub destination: String,
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
    /// `jar` or `war`.
    pub packaging: String,
    pub java_level: u32,
    /// The level was declared by a project file (`false`: AutoDock had to assume one).
    pub java_level_declared: bool,
    pub encoding: String,
    pub counts: ConvertCounts,
    /// The first moves (the counts are complete).
    pub moves: Vec<Move>,
    pub dependencies: Vec<Dependency>,
    /// A few of the files that are not part of the copy.
    pub left_out: Vec<String>,
    /// The generated `pom.xml`, as it will be written.
    pub pom: String,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    pub path: String,
    #[serde(default)]
    pub options: ConvertOptions,
    /// The person confirmed that the copy is made.
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertReport {
    pub schema_version: u32,
    pub succeeded: bool,
    pub destination: String,
    pub files_written: usize,
    pub counts: ConvertCounts,
    pub packaging: String,
    pub next_steps: Vec<String>,
}

// ───────────────────────────── known libraries ─────────────────────────────

/// A library whose coordinates the file name gives away: name (lower case), group, artifact, scope
/// and the major versions it applies to (the same name moved group at some version).
type Known = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    u64,
    u64,
);

const ANY: (u64, u64) = (0, u64::MAX);

const fn lib(name: &'static str, group: &'static str, artifact: &'static str) -> Known {
    (name, group, artifact, "compile", ANY.0, ANY.1)
}
const fn scoped(
    name: &'static str,
    group: &'static str,
    artifact: &'static str,
    scope: &'static str,
) -> Known {
    (name, group, artifact, scope, ANY.0, ANY.1)
}
const fn from_to(
    name: &'static str,
    group: &'static str,
    artifact: &'static str,
    min: u64,
    max: u64,
) -> Known {
    (name, group, artifact, "compile", min, max)
}

const KNOWN: &[Known] = &[
    // Apache Commons
    lib("commons-lang", "commons-lang", "commons-lang"),
    lib("commons-lang3", "org.apache.commons", "commons-lang3"),
    lib("commons-io", "commons-io", "commons-io"),
    lib("commons-codec", "commons-codec", "commons-codec"),
    lib(
        "commons-collections",
        "commons-collections",
        "commons-collections",
    ),
    lib(
        "commons-collections4",
        "org.apache.commons",
        "commons-collections4",
    ),
    lib(
        "commons-beanutils",
        "commons-beanutils",
        "commons-beanutils",
    ),
    lib("commons-logging", "commons-logging", "commons-logging"),
    lib(
        "commons-fileupload",
        "commons-fileupload",
        "commons-fileupload",
    ),
    lib("commons-dbcp", "commons-dbcp", "commons-dbcp"),
    lib("commons-dbcp2", "org.apache.commons", "commons-dbcp2"),
    lib("commons-pool", "commons-pool", "commons-pool"),
    lib("commons-pool2", "org.apache.commons", "commons-pool2"),
    lib("commons-digester", "commons-digester", "commons-digester"),
    lib(
        "commons-httpclient",
        "commons-httpclient",
        "commons-httpclient",
    ),
    lib("commons-cli", "commons-cli", "commons-cli"),
    lib("commons-net", "commons-net", "commons-net"),
    lib("commons-email", "org.apache.commons", "commons-email"),
    lib("commons-text", "org.apache.commons", "commons-text"),
    lib(
        "commons-validator",
        "commons-validator",
        "commons-validator",
    ),
    lib("commons-math3", "org.apache.commons", "commons-math3"),
    lib("commons-compress", "org.apache.commons", "commons-compress"),
    lib("commons-csv", "org.apache.commons", "commons-csv"),
    lib("commons-exec", "org.apache.commons", "commons-exec"),
    lib(
        "commons-configuration",
        "commons-configuration",
        "commons-configuration",
    ),
    // Logging
    lib("log4j", "log4j", "log4j"),
    lib("log4j-api", "org.apache.logging.log4j", "log4j-api"),
    lib("log4j-core", "org.apache.logging.log4j", "log4j-core"),
    lib("slf4j-api", "org.slf4j", "slf4j-api"),
    lib("slf4j-log4j12", "org.slf4j", "slf4j-log4j12"),
    lib("slf4j-simple", "org.slf4j", "slf4j-simple"),
    lib("slf4j-jdk14", "org.slf4j", "slf4j-jdk14"),
    lib("jcl-over-slf4j", "org.slf4j", "jcl-over-slf4j"),
    lib("jul-to-slf4j", "org.slf4j", "jul-to-slf4j"),
    lib("log4j-over-slf4j", "org.slf4j", "log4j-over-slf4j"),
    lib("logback-classic", "ch.qos.logback", "logback-classic"),
    lib("logback-core", "ch.qos.logback", "logback-core"),
    // Tests
    scoped("junit", "junit", "junit", "test"),
    scoped("hamcrest-core", "org.hamcrest", "hamcrest-core", "test"),
    scoped(
        "hamcrest-library",
        "org.hamcrest",
        "hamcrest-library",
        "test",
    ),
    scoped("hamcrest-all", "org.hamcrest", "hamcrest-all", "test"),
    scoped("mockito-core", "org.mockito", "mockito-core", "test"),
    scoped("mockito-all", "org.mockito", "mockito-all", "test"),
    scoped("easymock", "org.easymock", "easymock", "test"),
    scoped("testng", "org.testng", "testng", "test"),
    scoped("assertj-core", "org.assertj", "assertj-core", "test"),
    scoped("spring-test", "org.springframework", "spring-test", "test"),
    // Spring
    lib("spring-core", "org.springframework", "spring-core"),
    lib("spring-beans", "org.springframework", "spring-beans"),
    lib("spring-context", "org.springframework", "spring-context"),
    lib(
        "spring-context-support",
        "org.springframework",
        "spring-context-support",
    ),
    lib("spring-aop", "org.springframework", "spring-aop"),
    lib("spring-aspects", "org.springframework", "spring-aspects"),
    lib(
        "spring-expression",
        "org.springframework",
        "spring-expression",
    ),
    lib("spring-web", "org.springframework", "spring-web"),
    lib("spring-webmvc", "org.springframework", "spring-webmvc"),
    lib("spring-jdbc", "org.springframework", "spring-jdbc"),
    lib("spring-tx", "org.springframework", "spring-tx"),
    lib("spring-orm", "org.springframework", "spring-orm"),
    lib("spring-jms", "org.springframework", "spring-jms"),
    lib("spring-oxm", "org.springframework", "spring-oxm"),
    lib(
        "spring-security-core",
        "org.springframework.security",
        "spring-security-core",
    ),
    lib(
        "spring-security-web",
        "org.springframework.security",
        "spring-security-web",
    ),
    lib(
        "spring-security-config",
        "org.springframework.security",
        "spring-security-config",
    ),
    // JSON and XML
    lib("jackson-core", "com.fasterxml.jackson.core", "jackson-core"),
    lib(
        "jackson-databind",
        "com.fasterxml.jackson.core",
        "jackson-databind",
    ),
    lib(
        "jackson-annotations",
        "com.fasterxml.jackson.core",
        "jackson-annotations",
    ),
    lib(
        "jackson-core-asl",
        "org.codehaus.jackson",
        "jackson-core-asl",
    ),
    lib(
        "jackson-mapper-asl",
        "org.codehaus.jackson",
        "jackson-mapper-asl",
    ),
    lib("gson", "com.google.code.gson", "gson"),
    lib("json", "org.json", "json"),
    lib("jdom", "org.jdom", "jdom"),
    lib("jdom2", "org.jdom", "jdom2"),
    from_to("dom4j", "dom4j", "dom4j", 0, 1),
    from_to("dom4j", "org.dom4j", "dom4j", 2, u64::MAX),
    lib("xstream", "com.thoughtworks.xstream", "xstream"),
    lib("xercesimpl", "xerces", "xercesImpl"),
    lib("xml-apis", "xml-apis", "xml-apis"),
    lib("jaxb-api", "javax.xml.bind", "jaxb-api"),
    lib("jaxb-impl", "com.sun.xml.bind", "jaxb-impl"),
    lib("jaxb-core", "com.sun.xml.bind", "jaxb-core"),
    // Web
    scoped("servlet-api", "javax.servlet", "servlet-api", "provided"),
    scoped(
        "javax.servlet-api",
        "javax.servlet",
        "javax.servlet-api",
        "provided",
    ),
    scoped("jsp-api", "javax.servlet.jsp", "jsp-api", "provided"),
    lib("jstl", "javax.servlet", "jstl"),
    lib(
        "taglibs-standard-impl",
        "org.apache.taglibs",
        "taglibs-standard-impl",
    ),
    lib(
        "taglibs-standard-spec",
        "org.apache.taglibs",
        "taglibs-standard-spec",
    ),
    lib("struts2-core", "org.apache.struts", "struts2-core"),
    lib("freemarker", "org.freemarker", "freemarker"),
    // Data
    from_to("hibernate-core", "org.hibernate", "hibernate-core", 0, 5),
    from_to(
        "hibernate-core",
        "org.hibernate.orm",
        "hibernate-core",
        6,
        u64::MAX,
    ),
    lib(
        "hibernate-entitymanager",
        "org.hibernate",
        "hibernate-entitymanager",
    ),
    lib("mybatis", "org.mybatis", "mybatis"),
    from_to("ehcache", "net.sf.ehcache", "ehcache", 0, 2),
    from_to("ehcache", "org.ehcache", "ehcache", 3, u64::MAX),
    lib("c3p0", "c3p0", "c3p0"),
    lib("hikaricp", "com.zaxxer", "HikariCP"),
    lib("mysql-connector-java", "mysql", "mysql-connector-java"),
    lib("postgresql", "org.postgresql", "postgresql"),
    lib("h2", "com.h2database", "h2"),
    lib("hsqldb", "org.hsqldb", "hsqldb"),
    lib("sqlite-jdbc", "org.xerial", "sqlite-jdbc"),
    lib("derby", "org.apache.derby", "derby"),
    // Other well-known ones
    lib("guava", "com.google.guava", "guava"),
    lib("joda-time", "joda-time", "joda-time"),
    lib("httpclient", "org.apache.httpcomponents", "httpclient"),
    lib("httpcore", "org.apache.httpcomponents", "httpcore"),
    lib("httpmime", "org.apache.httpcomponents", "httpmime"),
    lib("aspectjweaver", "org.aspectj", "aspectjweaver"),
    lib("aspectjrt", "org.aspectj", "aspectjrt"),
    lib("cglib", "cglib", "cglib"),
    lib("cglib-nodep", "cglib", "cglib-nodep"),
    lib("poi", "org.apache.poi", "poi"),
    lib("poi-ooxml", "org.apache.poi", "poi-ooxml"),
    lib("jsoup", "org.jsoup", "jsoup"),
    lib("javax.inject", "javax.inject", "javax.inject"),
    lib("jsr305", "com.google.code.findbugs", "jsr305"),
    lib("mail", "javax.mail", "mail"),
    lib("javax.mail", "com.sun.mail", "javax.mail"),
    lib("activation", "javax.activation", "activation"),
    from_to("quartz", "org.quartz-scheduler", "quartz", 2, u64::MAX),
    scoped("lombok", "org.projectlombok", "lombok", "provided"),
];

/// The major version a version string starts with (`4.3.25.RELEASE` → 4).
fn major_of(version: &str) -> Option<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|digits| digits.parse().ok())
}

/// The coordinates a jar file name gives away, when it is a well-known library.
fn known_library(
    name: &str,
    version: Option<&str>,
) -> Option<(&'static str, &'static str, &'static str)> {
    let lower = name.to_ascii_lowercase();
    let major = version.and_then(major_of);
    KNOWN
        .iter()
        .find(|(known, _, _, _, min, max)| {
            *known == lower && major.map_or(*min == 0, |m| m >= *min && m <= *max)
        })
        .map(|(_, group, artifact, scope, _, _)| (*group, *artifact, *scope))
}

// ───────────────────────────── coordinates and the pom ─────────────────────────────

/// A name that is safe as a Maven group, artifact or version part.
fn valid_coordinate(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 100
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn sanitize(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches(|c| c == '-' || c == '.').to_string()
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn level_text(level: u32, declared: Option<&str>) -> String {
    match declared {
        Some(text) => text.to_string(),
        None if level <= 8 => format!("1.{level}"),
        None => level.to_string(),
    }
}

struct PomInput<'a> {
    group_id: &'a str,
    artifact_id: &'a str,
    version: &'a str,
    packaging: &'a str,
    name: &'a str,
    level: String,
    encoding: &'a str,
    dependencies: &'a [Dependency],
    main_class: Option<&'a str>,
}

fn pom_text(input: &PomInput) -> String {
    let mut text = String::new();
    text.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    text.push_str("<project xmlns=\"http://maven.apache.org/POM/4.0.0\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n         xsi:schemaLocation=\"http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd\">\n");
    text.push_str("  <modelVersion>4.0.0</modelVersion>\n\n");
    text.push_str(
        "  <!-- Generated by AutoDock from a folder that had no pom.xml. Review it. -->\n",
    );
    text.push_str(&format!(
        "  <groupId>{}</groupId>\n  <artifactId>{}</artifactId>\n  <version>{}</version>\n  <packaging>{}</packaging>\n  <name>{}</name>\n\n",
        xml(input.group_id),
        xml(input.artifact_id),
        xml(input.version),
        xml(input.packaging),
        xml(input.name)
    ));
    text.push_str(&format!(
        "  <properties>\n    <project.build.sourceEncoding>{}</project.build.sourceEncoding>\n    <maven.compiler.source>{}</maven.compiler.source>\n    <maven.compiler.target>{}</maven.compiler.target>\n  </properties>\n\n",
        xml(input.encoding),
        xml(&input.level),
        xml(&input.level)
    ));
    if !input.dependencies.is_empty() {
        text.push_str("  <dependencies>\n");
        for dependency in input.dependencies {
            if !dependency.known {
                text.push_str(&format!(
                    "    <!-- {}: no known coordinates, kept as a local jar -->\n",
                    xml(&dependency.jar)
                ));
            }
            text.push_str(&format!(
                "    <dependency>\n      <groupId>{}</groupId>\n      <artifactId>{}</artifactId>\n      <version>{}</version>\n",
                xml(&dependency.group_id),
                xml(&dependency.artifact_id),
                xml(&dependency.version)
            ));
            if dependency.scope != "compile" {
                text.push_str(&format!("      <scope>{}</scope>\n", dependency.scope));
            }
            if dependency.scope == "system" {
                let file = dependency.jar.rsplit('/').next().unwrap_or(&dependency.jar);
                text.push_str(&format!(
                    "      <systemPath>${{project.basedir}}/lib/{}</systemPath>\n",
                    xml(file)
                ));
            }
            text.push_str("    </dependency>\n");
        }
        text.push_str("  </dependencies>\n\n");
    }
    let war = input.packaging == "war";
    if war || input.main_class.is_some() {
        text.push_str("  <build>\n    <plugins>\n");
        if war {
            text.push_str("      <plugin>\n        <artifactId>maven-war-plugin</artifactId>\n        <version>3.3.2</version>\n        <configuration>\n          <failOnMissingWebXml>false</failOnMissingWebXml>\n        </configuration>\n      </plugin>\n");
        }
        if let Some(main) = input.main_class {
            text.push_str(&format!(
                "      <plugin>\n        <artifactId>maven-jar-plugin</artifactId>\n        <version>3.3.0</version>\n        <configuration>\n          <archive>\n            <manifest>\n              <mainClass>{}</mainClass>\n            </manifest>\n          </archive>\n        </configuration>\n      </plugin>\n",
                xml(main)
            ));
        }
        text.push_str("    </plugins>\n  </build>\n");
    }
    text.push_str("</project>\n");
    text
}

// ───────────────────────────── the layout ─────────────────────────────

/// One file that goes into the copy.
struct Copy {
    from: String,
    to: String,
}

struct Layout {
    copies: Vec<Copy>,
    counts: ConvertCounts,
    dependencies: Vec<Dependency>,
    left_out: Vec<String>,
    warnings: Vec<String>,
    main_class: Option<String>,
}

fn is_test_path(dir: &str) -> bool {
    dir.split('/')
        .any(|part| matches!(part.to_ascii_lowercase().as_str(), "test" | "tests"))
}

/// Files that are build scripts or editor settings: they never go into the copy.
fn is_build_file(name: &str) -> bool {
    matches!(
        name,
        "build.xml"
            | ".classpath"
            | ".project"
            | "Makefile"
            | "makefile"
            | "pom.xml"
            | "ivy.xml"
            | ".DS_Store"
            | "Thumbs.db"
    ) || name.ends_with(".iml")
        || name.ends_with(".class")
        || name.ends_with(".orig")
}

fn layout(walked: &Walk, map: &JavaMap) -> Layout {
    let mut out = Layout {
        copies: vec![],
        counts: ConvertCounts::default(),
        dependencies: vec![],
        left_out: vec![],
        warnings: vec![],
        main_class: None,
    };
    let kinds: BTreeMap<&str, &str> = map
        .source_roots
        .iter()
        .map(|r| (r.dir.as_str(), r.kind.as_str()))
        .collect();
    let mut taken: BTreeMap<String, String> = BTreeMap::new();
    let mut used: BTreeSet<String> = BTreeSet::new();

    // Sources go where their package says, under main or test.
    let mut mains: Vec<String> = vec![];
    for source in &walked.sources {
        let from = if source.dir.is_empty() {
            source.file.clone()
        } else {
            format!("{}/{}", source.dir, source.file)
        };
        let root = java_map::source_root(&source.dir, &source.package);
        let test = match root.as_deref().and_then(|dir| kinds.get(dir)) {
            Some(&"test") => true,
            Some(&"main") => false,
            _ => source.is_test || is_test_path(&source.dir),
        };
        let package_dir = source.package.replace('.', "/");
        let tail = if package_dir.is_empty() {
            source.file.clone()
        } else {
            format!("{package_dir}/{}", source.file)
        };
        let to = format!("src/{}/java/{tail}", if test { "test" } else { "main" });
        if root.is_none() {
            out.counts.moved_to_their_package += 1;
        }
        if let Some(first) = taken.get(&to) {
            out.warnings.push(tr!(
                "{from} vai para o mesmo lugar que {first}: ficou só o primeiro.",
                "{from} would land where {first} already goes: only the first was kept."
            ));
            out.left_out.push(from);
            continue;
        }
        taken.insert(to.clone(), from.clone());
        if test {
            out.counts.test_sources += 1;
        } else {
            out.counts.main_sources += 1;
            if source.has_main {
                let stem = source.file.strip_suffix(".java").unwrap_or(&source.file);
                mains.push(if source.package.is_empty() {
                    stem.to_string()
                } else {
                    format!("{}.{stem}", source.package)
                });
            }
        }
        used.insert(from.clone());
        out.copies.push(Copy { from, to });
    }
    if mains.len() == 1 {
        out.main_class = mains.pop();
    }

    // The web application: everything but the compiled classes and the jars, which are libraries.
    let app = map.web_apps.first().cloned();
    if map.web_apps.len() > 1 {
        out.warnings.push(tr!(
            "A pasta tem {} aplicações web; só a primeira ({}) foi para a cópia.",
            "The folder has {} web applications; only the first ({}) went into the copy.",
            map.web_apps.len(),
            map.web_apps[0]
        ));
    }
    let mut roots_by_depth: Vec<&str> = map
        .source_roots
        .iter()
        .map(|r| r.dir.as_str())
        .filter(|d| !d.is_empty())
        .collect();
    roots_by_depth.sort_by_key(|d| std::cmp::Reverse(d.len()));

    for file in &walked.files {
        if used.contains(file) {
            continue;
        }
        let name = file.rsplit('/').next().unwrap_or(file);
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".jar") || lower.ends_with(".java") {
            continue; // libraries are dealt with below; the sources were all used
        }
        if java_upgrade::skipped_file(name) || is_build_file(name) {
            out.left_out.push(file.clone());
            continue;
        }
        // 1. the web application
        let in_app = app.as_deref().and_then(|dir| {
            if dir.is_empty() {
                file.strip_prefix("WEB-INF/")
                    .map(|rest| format!("WEB-INF/{rest}"))
            } else {
                file.strip_prefix(&format!("{dir}/")).map(str::to_string)
            }
        });
        let destination = if let Some(inside) = in_app {
            if inside.starts_with("WEB-INF/classes/") || inside.starts_with("WEB-INF/lib/") {
                out.left_out.push(file.clone());
                continue;
            }
            out.counts.webapp_files += 1;
            Some(format!("src/main/webapp/{inside}"))
        } else if let Some(root) = roots_by_depth
            .iter()
            .find(|root| file.starts_with(&format!("{root}/")))
        {
            // 2. a resource that sits among the sources
            let inside = &file[root.len() + 1..];
            let test = kinds.get(root) == Some(&"test");
            if test {
                out.counts.test_resources += 1;
            } else {
                out.counts.main_resources += 1;
            }
            Some(format!(
                "src/{}/resources/{inside}",
                if test { "test" } else { "main" }
            ))
        } else if [
            "src/main/resources/",
            "src/test/resources/",
            "src/main/webapp/",
        ]
        .iter()
        .any(|dir| file.starts_with(dir))
        {
            // 3. a standard folder that is already where Maven wants it
            if file.starts_with("src/test/resources/") {
                out.counts.test_resources += 1;
            } else if file.starts_with("src/main/resources/") {
                out.counts.main_resources += 1;
            } else {
                out.counts.webapp_files += 1;
            }
            Some(file.clone())
        } else {
            None
        };
        match destination {
            Some(to) => {
                if let Some(first) = taken.get(&to) {
                    out.warnings.push(tr!(
                        "{file} vai para o mesmo lugar que {first}: ficou só o primeiro.",
                        "{file} would land where {first} already goes: only the first was kept."
                    ));
                    out.left_out.push(file.clone());
                } else {
                    taken.insert(to.clone(), file.clone());
                    out.copies.push(Copy {
                        from: file.clone(),
                        to,
                    });
                }
            }
            None => out.left_out.push(file.clone()),
        }
    }

    // Libraries: known ones become coordinates, the others stay as local jars in lib/.
    let mut by_key: BTreeMap<String, Dependency> = BTreeMap::new();
    let mut lib_names: BTreeSet<String> = BTreeSet::new();
    for (relative, _, _) in &walked.jars {
        if relative.contains(".mvn/wrapper/") || relative.contains("gradle/wrapper/") {
            out.left_out.push(relative.clone());
            continue;
        }
        let file_name = relative.rsplit('/').next().unwrap_or(relative);
        let (name, version) = java_map::artifact_of(file_name);
        let dependency = match (known_library(&name, version.as_deref()), &version) {
            (Some((group, artifact, scope)), Some(version)) => Dependency {
                group_id: group.into(),
                artifact_id: artifact.into(),
                version: version.clone(),
                scope: scope.into(),
                jar: relative.clone(),
                known: true,
            },
            _ => {
                let mut copy_name = file_name.to_string();
                let mut attempt = 1;
                while !lib_names.insert(copy_name.to_ascii_lowercase()) {
                    attempt += 1;
                    copy_name = format!("{attempt}-{file_name}");
                }
                out.copies.push(Copy {
                    from: relative.clone(),
                    to: format!("lib/{copy_name}"),
                });
                Dependency {
                    group_id: "local".into(),
                    artifact_id: sanitize(&name),
                    version: version.clone().unwrap_or_else(|| "1".into()),
                    scope: "system".into(),
                    jar: if copy_name == file_name {
                        relative.clone()
                    } else {
                        format!("lib/{copy_name}")
                    },
                    known: false,
                }
            }
        };
        let key = format!("{}:{}", dependency.group_id, dependency.artifact_id);
        match by_key.get(&key) {
            Some(existing) if existing.known && dependency.known => {
                // two versions of the same library: Maven takes one, the newer
                let newer = version_key(&dependency.version) > version_key(&existing.version);
                let (kept, dropped) = if newer {
                    (dependency.clone(), existing.clone())
                } else {
                    (existing.clone(), dependency.clone())
                };
                out.warnings.push(tr!(
                    "A pasta traz duas versões de {}: {} e {}. Ficou a mais nova ({}).",
                    "The folder carries two versions of {}: {} and {}. The newer one ({}) was kept.",
                    key,
                    existing.version,
                    dependency.version,
                    kept.version
                ));
                out.left_out.push(dropped.jar.clone());
                by_key.insert(key, kept);
            }
            Some(_) => {
                // a local jar with the same artifact name: keep both, told apart by the version
                let unique = format!("{key}:{}", dependency.jar);
                by_key.insert(unique, dependency);
            }
            None => {
                by_key.insert(key, dependency);
            }
        }
    }
    let mut dependencies: Vec<Dependency> = by_key.into_values().collect();
    let order = |scope: &str| match scope {
        "compile" => 0,
        "provided" => 1,
        "test" => 2,
        _ => 3,
    };
    dependencies.sort_by(|a, b| {
        order(&a.scope)
            .cmp(&order(&b.scope))
            .then_with(|| a.group_id.cmp(&b.group_id))
            .then_with(|| a.artifact_id.cmp(&b.artifact_id))
    });
    // A jar whose version lost to a newer one is not copied to lib/.
    let kept_local: BTreeSet<&str> = dependencies
        .iter()
        .filter(|d| !d.known)
        .map(|d| d.jar.as_str())
        .collect();
    out.copies.retain(|c| {
        !c.to.starts_with("lib/")
            || kept_local.contains(c.from.as_str())
            || kept_local.contains(c.to.as_str())
    });
    out.counts.libraries_known = dependencies.iter().filter(|d| d.known).count();
    out.counts.libraries_local = dependencies.iter().filter(|d| !d.known).count();
    out.dependencies = dependencies;
    out.counts.not_copied = out.left_out.len();
    out
}

/// A comparable form of a version: its numbers, in order.
fn version_key(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

// ───────────────────────────── plan and run ─────────────────────────────

/// Where the copy goes: `<name>-maven` in the holding folder, never over something that exists.
fn destination_for(root: &Path, into: Option<&str>) -> Result<PathBuf, String> {
    let parent = java_upgrade::holding_folder(root, into)?;
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
        let candidate = parent.join(format!("{name}-maven{suffix}"));
        if fs::symlink_metadata(&candidate).is_err() {
            return Ok(candidate);
        }
    }
    Err(tr!(
        "Já existem 20 cópias Maven nessa pasta: apague as que não precisa mais.",
        "There are already 20 Maven copies in that folder: delete the ones you no longer need."
    ))
}

fn check_coordinate(label: &str, value: &str) -> Result<(), String> {
    if valid_coordinate(value) {
        Ok(())
    } else {
        Err(tr!(
            "{label} inválido ({value}): use letras, números, ponto, hífen e sublinhado.",
            "Invalid {label} ({value}): use letters, digits, dot, dash and underscore."
        ))
    }
}

/// What converting the folder would do. Reads the folder and writes nothing.
pub fn plan(path: &str, options: &ConvertOptions) -> Result<ConvertPlan, String> {
    let root = generate::root_of(path)?;
    let walked = java_map::walk(&root);
    let map = java_map::build(&root, &walked);
    if map.counts.java == 0 {
        return Err(tr!(
            "Nenhum arquivo .java foi encontrado nesta pasta.",
            "No .java file was found in this folder."
        ));
    }
    if map.layout == "maven" {
        return Err(tr!(
            "Esta pasta já tem um pom.xml: use o upgrade, que já trabalha com projetos Maven.",
            "This folder already has a pom.xml: use the upgrade, which works with Maven projects."
        ));
    }
    let destination = destination_for(&root, options.into.as_deref())?;
    let name = map.name.clone();

    let main_roots: Vec<&java_map::SourceRoot> = map
        .source_roots
        .iter()
        .filter(|r| r.kind != "test" && !r.top_package.is_empty())
        .collect();
    let top_package = main_roots
        .iter()
        .max_by_key(|r| r.files)
        .map(|r| r.top_package.clone())
        .unwrap_or_default();
    let parts: Vec<&str> = top_package.split('.').collect();
    let default_group = if top_package.is_empty() {
        "local".to_string()
    } else if parts.len() >= 3 {
        parts[..parts.len() - 1].join(".")
    } else {
        top_package.clone()
    };
    let group_id = options
        .group_id
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(default_group);
    let artifact_id = options
        .artifact_id
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            let cleaned = sanitize(&name);
            if cleaned.is_empty() {
                "app".into()
            } else {
                cleaned
            }
        });
    let version = options
        .version
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "1.0.0-SNAPSHOT".into());
    check_coordinate("groupId", &group_id)?;
    check_coordinate("artifactId", &artifact_id)?;
    check_coordinate("version", &version)?;

    let declared = map.java_level.as_ref();
    let java_level = options
        .java_level
        .or_else(|| declared.map(|h| h.level))
        .unwrap_or(ASSUMED_JAVA);
    if !(1..=99).contains(&java_level) {
        return Err(tr!(
            "Nível de Java inválido: {java_level}.",
            "Invalid Java level: {java_level}."
        ));
    }
    let java_level_declared = options.java_level.is_some() || declared.is_some();
    let encoding = if map.non_utf8_files > 0 {
        "ISO-8859-1"
    } else {
        "UTF-8"
    };

    let laid = layout(&walked, &map);
    let packaging = if map.web_apps.is_empty() {
        "jar"
    } else {
        "war"
    };
    let pom = pom_text(&PomInput {
        group_id: &group_id,
        artifact_id: &artifact_id,
        version: &version,
        packaging,
        name: &name,
        level: level_text(
            java_level,
            options
                .java_level
                .is_none()
                .then(|| declared.map(|h| h.declared.as_str()))
                .flatten(),
        ),
        encoding,
        dependencies: &laid.dependencies,
        main_class: laid.main_class.as_deref(),
    });

    let mut notes = vec![tr!(
        "O projeto original não é alterado: o AutoDock cria uma cópia com o layout do Maven e um pom.xml gerado. Nada é compilado nem executado nesta etapa.",
        "The original project is not modified: AutoDock creates a copy with the Maven layout and a generated pom.xml. Nothing is compiled or run in this step."
    )];
    if laid.counts.libraries_local > 0 {
        notes.push(tr!(
            "{} biblioteca(s) não têm coordenadas conhecidas e ficam na pasta lib/ da cópia, declaradas como jar local. O upgrade e os testes mostram se alguma faz falta.",
            "{} librar(ies) have no known coordinates and stay in the copy's lib/ folder, declared as local jars. The upgrade and the tests show whether any is missing.",
            laid.counts.libraries_local
        ));
    }
    if laid.counts.libraries_known > 0 {
        notes.push(tr!(
            "{} biblioteca(s) viraram dependências Maven pelo nome do arquivo (nome e versão); o Maven baixa o jar de verdade na hora de compilar.",
            "{} librar(ies) became Maven dependencies from the file name (name and version); Maven downloads the real jar when it builds.",
            laid.counts.libraries_known
        ));
    }
    let mut warnings = laid.warnings.clone();
    if !java_level_declared {
        warnings.push(tr!(
            "Nenhum arquivo do projeto diz para qual Java ele foi escrito: o pom assume Java {ASSUMED_JAVA}. Ajuste se for outro.",
            "No project file says which Java it was written for: the pom assumes Java {ASSUMED_JAVA}. Change it if it is another."
        ));
    }
    if encoding == "ISO-8859-1" {
        warnings.push(tr!(
            "Há fontes que não estão em UTF-8: o pom declara ISO-8859-1 para o build, e os arquivos são copiados sem mudar.",
            "Some sources are not UTF-8: the pom declares ISO-8859-1 for the build, and the files are copied unchanged."
        ));
    }

    let mut left_out = laid.left_out.clone();
    left_out.sort();
    left_out.truncate(MAX_LISTED);
    Ok(ConvertPlan {
        schema_version: 1,
        path: map.path.clone(),
        name,
        destination: generate::plain_path(&destination),
        group_id,
        artifact_id,
        version,
        packaging: packaging.into(),
        java_level,
        java_level_declared,
        encoding: encoding.into(),
        counts: laid.counts.clone(),
        moves: laid
            .copies
            .iter()
            .filter(|c| !c.to.starts_with("lib/"))
            .take(MAX_LISTED)
            .map(|c| Move {
                from: c.from.clone(),
                to: c.to.clone(),
            })
            .collect(),
        dependencies: laid.dependencies.clone(),
        left_out,
        pom,
        notes,
        warnings,
    })
}

/// Makes the copy. The original folder is never touched, and the copy goes into a folder that does
/// not exist yet.
pub fn run(request: ConvertRequest) -> Result<ConvertReport, String> {
    if !request.confirmed {
        return Err(tr!(
            "A conversão precisa de confirmação.",
            "The conversion needs a confirmation."
        ));
    }
    let plan = plan(&request.path, &request.options)?;
    let root = generate::root_of(&request.path)?;
    let walked = java_map::walk(&root);
    let map = java_map::build(&root, &walked);
    let laid = layout(&walked, &map);

    // Every file is checked again right before it is copied: a link that appeared since the walk
    // is refused, and the total size is bounded.
    let mut total = 0u64;
    for copy in &laid.copies {
        let meta = fs::symlink_metadata(root.join(&copy.from))
            .map_err(|e| format!("{}: {e}", copy.from))?;
        if is_link(&meta) || !meta.is_file() {
            return Err(tr!(
                "{} mudou desde a leitura da pasta (agora é um link ou não é um arquivo): nada foi copiado.",
                "{} changed since the folder was read (now a link or not a file): nothing was copied.",
                copy.from
            ));
        }
        total += meta.len();
    }
    if total > MAX_COPY_BYTES {
        return Err(tr!(
            "O projeto é grande demais para copiar ({} MB, o limite é {} MB).",
            "The project is too large to copy ({} MB, the limit is {} MB).",
            total / 1_000_000,
            MAX_COPY_BYTES / 1_000_000
        ));
    }

    let destination = destination_for(&root, request.options.into.as_deref())?;
    // `create_dir` fails when the folder exists: nothing is ever copied over something.
    fs::create_dir(&destination).map_err(|e| format!("{}: {e}", destination.display()))?;
    let written = (|| -> Result<usize, String> {
        let mut written = 0usize;
        for copy in &laid.copies {
            let target = destination.join(&copy.to);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(root.join(&copy.from), &target).map_err(|e| format!("{}: {e}", copy.from))?;
            written += 1;
        }
        fs::write(destination.join("pom.xml"), &plan.pom).map_err(|e| format!("pom.xml: {e}"))?;
        Ok(written + 1)
    })();
    let files_written = match written {
        Ok(count) => count,
        Err(error) => {
            let _ = fs::remove_dir_all(&destination);
            return Err(error);
        }
    };
    let next_steps = vec![
        tr!(
            "Abra o pom.xml da cópia e confira as coordenadas, o nível do Java e as dependências.",
            "Open the copy's pom.xml and check the coordinates, the Java level and the dependencies."
        ),
        tr!(
            "Com a cópia pronta, ela já é um projeto Maven: faça o upgrade para o Java 25 e o Spring Boot 4 a partir dela.",
            "With the copy done, it is a Maven project: upgrade it to Java 25 and Spring Boot 4 from there."
        ),
    ];
    Ok(ConvertReport {
        schema_version: 1,
        succeeded: true,
        destination: generate::plain_path(&destination),
        files_written,
        counts: plan.counts,
        packaging: plan.packaging,
        next_steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java;

    fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        // the project lives in a folder of its own so the copy has somewhere to go
        let root = dir.path().join("loja antiga");
        for (name, text) in files {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let root = root.to_string_lossy().into_owned();
        (dir, root)
    }

    const APP: &str = "package com.acme.shop;\n\npublic class App {\n    public static void main(String[] args) {}\n}\n";
    const ORDER: &str = "package com.acme.shop.model;\n\npublic class Order {}\n";
    const ORDER_TEST: &str =
        "package com.acme.shop.model;\n\nimport org.junit.Test;\n\npublic class OrderTest {}\n";

    fn messy() -> (tempfile::TempDir, String) {
        project(&[
            (".project", "<projectDescription/>"),
            (
                ".classpath",
                "<classpath><classpathentry kind=\"con\" path=\"org.eclipse.jdt.launching.JRE_CONTAINER/org.eclipse.jdt.internal.debug.ui.launcher.StandardVMType/JavaSE-1.6\"/></classpath>",
            ),
            ("build.xml", "<project name=\"shop\"/>"),
            ("src/com/acme/shop/App.java", APP),
            ("src/com/acme/shop/model/Order.java", ORDER),
            ("src/log4j.properties", "log4j.rootLogger=INFO\n"),
            ("src/com/acme/shop/messages.properties", "hello=Hello\n"),
            ("test/com/acme/shop/model/OrderTest.java", ORDER_TEST),
            ("test/fixtures.properties", "a=b\n"),
            ("lib/commons-lang-2.6.jar", "commons"),
            ("lib/junit-4.12.jar", "junit"),
            ("lib/servlet-api-2.5.jar", "servlet"),
            ("lib/acme-legacy-1.0.jar", "proprietary"),
            ("WebContent/WEB-INF/web.xml", "<web-app/>"),
            ("WebContent/WEB-INF/classes/Old.class", "bytes"),
            ("WebContent/WEB-INF/lib/jstl-1.2.jar", "jstl"),
            ("WebContent/index.jsp", "<html/>"),
            ("docs/manual.txt", "read me"),
            ("bin/com/acme/shop/App.class", "bytes"),
        ])
    }

    /// Every file under a folder with its bytes, for "the original was not touched".
    fn snapshot(root: &str) -> BTreeMap<String, Vec<u8>> {
        fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for entry in fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, base, out);
                } else {
                    let relative = path
                        .strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.insert(relative, fs::read(&path).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(Path::new(root), Path::new(root), &mut out);
        out
    }

    fn moves(plan: &ConvertPlan) -> BTreeMap<&str, &str> {
        plan.moves
            .iter()
            .map(|m| (m.from.as_str(), m.to.as_str()))
            .collect()
    }

    #[test]
    fn an_ant_and_eclipse_project_is_planned_as_a_maven_project() {
        let (_dir, root) = messy();
        let plan = plan(&root, &ConvertOptions::default()).unwrap();
        // coordinates: the package gives the group, the folder gives the artifact
        assert_eq!(plan.group_id, "com.acme");
        assert_eq!(plan.artifact_id, "loja-antiga");
        assert_eq!(plan.version, "1.0.0-SNAPSHOT");
        assert_eq!(plan.packaging, "war");
        assert_eq!((plan.java_level, plan.java_level_declared), (6, true));
        assert_eq!(plan.encoding, "UTF-8");
        assert!(plan.destination.ends_with("loja antiga-maven"));
        let moved = moves(&plan);
        assert_eq!(
            moved["src/com/acme/shop/App.java"],
            "src/main/java/com/acme/shop/App.java"
        );
        assert_eq!(
            moved["test/com/acme/shop/model/OrderTest.java"],
            "src/test/java/com/acme/shop/model/OrderTest.java"
        );
        // resources among the sources keep their place inside the source folder
        assert_eq!(
            moved["src/log4j.properties"],
            "src/main/resources/log4j.properties"
        );
        assert_eq!(
            moved["src/com/acme/shop/messages.properties"],
            "src/main/resources/com/acme/shop/messages.properties"
        );
        assert_eq!(
            moved["test/fixtures.properties"],
            "src/test/resources/fixtures.properties"
        );
        // the web application, without its compiled classes and jars
        assert_eq!(moved["WebContent/index.jsp"], "src/main/webapp/index.jsp");
        assert_eq!(
            moved["WebContent/WEB-INF/web.xml"],
            "src/main/webapp/WEB-INF/web.xml"
        );
        assert!(!moved.contains_key("WebContent/WEB-INF/classes/Old.class"));
        // libraries: known names become coordinates (with the right scope), the rest stay local
        let deps: Vec<(&str, &str, &str, &str, bool)> = plan
            .dependencies
            .iter()
            .map(|d| {
                (
                    d.group_id.as_str(),
                    d.artifact_id.as_str(),
                    d.version.as_str(),
                    d.scope.as_str(),
                    d.known,
                )
            })
            .collect();
        assert_eq!(
            deps,
            [
                ("commons-lang", "commons-lang", "2.6", "compile", true),
                ("javax.servlet", "jstl", "1.2", "compile", true),
                ("javax.servlet", "servlet-api", "2.5", "provided", true),
                ("junit", "junit", "4.12", "test", true),
                ("local", "acme-legacy", "1.0", "system", false),
            ]
        );
        assert_eq!(plan.counts.main_sources, 2);
        assert_eq!(plan.counts.test_sources, 1);
        assert_eq!(plan.counts.main_resources, 2);
        assert_eq!(plan.counts.test_resources, 1);
        assert_eq!(plan.counts.webapp_files, 2);
        assert_eq!(
            (plan.counts.libraries_known, plan.counts.libraries_local),
            (4, 1)
        );
        // nothing was written by planning
        assert!(!Path::new(&plan.destination).exists());
        // the build scripts and the docs stay behind and are listed
        assert!(plan.left_out.iter().any(|f| f == "build.xml"));
        assert!(plan.left_out.iter().any(|f| f == "docs/manual.txt"));
        assert!(plan.notes.iter().any(|n| n.contains("local")));
    }

    #[test]
    fn the_generated_pom_is_valid_xml_with_the_dependencies_and_the_plugins_that_are_needed() {
        let (_dir, root) = messy();
        let plan = plan(&root, &ConvertOptions::default()).unwrap();
        let doc = roxmltree::Document::parse(&plan.pom).unwrap();
        let text_of = |parent: roxmltree::Node, name: &str| -> String {
            parent
                .children()
                .find(|n| n.has_tag_name(name))
                .and_then(|n| n.text())
                .unwrap_or("")
                .to_string()
        };
        let project = doc.root_element();
        assert_eq!(text_of(project, "groupId"), "com.acme");
        assert_eq!(text_of(project, "packaging"), "war");
        let properties = project
            .children()
            .find(|n| n.has_tag_name("properties"))
            .unwrap();
        assert_eq!(text_of(properties, "maven.compiler.source"), "1.6");
        assert_eq!(text_of(properties, "project.build.sourceEncoding"), "UTF-8");
        let dependencies = project
            .children()
            .find(|n| n.has_tag_name("dependencies"))
            .unwrap();
        let system = dependencies
            .children()
            .filter(|n| n.has_tag_name("dependency"))
            .find(|d| text_of(*d, "artifactId") == "acme-legacy")
            .unwrap();
        assert_eq!(text_of(system, "scope"), "system");
        assert_eq!(
            text_of(system, "systemPath"),
            "${project.basedir}/lib/acme-legacy-1.0.jar"
        );
        // a war without a web.xml in a standard place must not fail the build; one main class
        // makes the jar runnable
        assert!(plan.pom.contains("maven-war-plugin"));
        assert!(plan
            .pom
            .contains("<mainClass>com.acme.shop.App</mainClass>"));
    }

    #[test]
    fn the_copy_is_made_with_the_files_where_the_plan_said_and_the_original_is_untouched() {
        let (_dir, root) = messy();
        let before = snapshot(&root);
        let report = run(ConvertRequest {
            path: root.clone(),
            options: ConvertOptions::default(),
            confirmed: true,
        })
        .unwrap();
        assert!(report.succeeded);
        let copy = Path::new(&report.destination);
        assert_eq!(
            fs::read_to_string(copy.join("src/main/java/com/acme/shop/App.java")).unwrap(),
            APP
        );
        assert_eq!(
            fs::read_to_string(copy.join("src/test/java/com/acme/shop/model/OrderTest.java"))
                .unwrap(),
            ORDER_TEST
        );
        assert_eq!(
            fs::read_to_string(copy.join("src/main/resources/log4j.properties")).unwrap(),
            "log4j.rootLogger=INFO\n"
        );
        assert!(copy.join("src/main/webapp/WEB-INF/web.xml").is_file());
        assert_eq!(
            fs::read_to_string(copy.join("lib/acme-legacy-1.0.jar")).unwrap(),
            "proprietary"
        );
        // the jar of a known library is not copied: Maven downloads the real one
        assert!(!copy.join("lib/commons-lang-2.6.jar").exists());
        assert!(copy.join("pom.xml").is_file());
        assert!(!copy.join("build.xml").exists());
        assert!(!copy.join("docs").exists());
        // the original is exactly as it was, and the copy is where the person expects it
        assert_eq!(snapshot(&root), before);
        assert!(report.destination.ends_with("loja antiga-maven"));
        // it is a Maven project now: the upgrade can read it
        let project = java::inspect(&report.destination).unwrap().unwrap();
        assert_eq!(project.build_tool, "maven");
        assert_eq!(project.java_level, Some(6));
        assert_eq!(project.packaging, "war");
        assert_eq!(java_map::map(&report.destination).unwrap().layout, "maven");
    }

    #[test]
    fn nothing_is_written_without_a_confirmation_and_a_second_copy_gets_its_own_folder() {
        let (_dir, root) = messy();
        let refused = run(ConvertRequest {
            path: root.clone(),
            options: ConvertOptions::default(),
            confirmed: false,
        });
        assert!(refused.is_err());
        let parent = Path::new(&root).parent().unwrap().to_path_buf();
        assert!(!parent.join("loja antiga-maven").exists());
        let first = run(ConvertRequest {
            path: root.clone(),
            options: ConvertOptions::default(),
            confirmed: true,
        })
        .unwrap();
        let second = run(ConvertRequest {
            path: root.clone(),
            options: ConvertOptions::default(),
            confirmed: true,
        })
        .unwrap();
        assert_ne!(first.destination, second.destination);
        assert!(second.destination.ends_with("loja antiga-maven-2"));
        // the first copy was not touched by the second
        assert!(Path::new(&first.destination).join("pom.xml").is_file());
    }

    #[test]
    fn the_copy_can_go_into_another_folder_but_not_inside_the_project() {
        let (_dir, root) = messy();
        let elsewhere = tempfile::tempdir().unwrap();
        let plan = plan(
            &root,
            &ConvertOptions {
                into: Some(elsewhere.path().to_string_lossy().into_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            Path::new(&plan.destination).starts_with(generate::plain_path(
                &elsewhere.path().canonicalize().unwrap()
            ))
        );
        let inside = Path::new(&root).join("lib");
        assert!(self::plan(
            &root,
            &ConvertOptions {
                into: Some(inside.to_string_lossy().into_owned()),
                ..Default::default()
            }
        )
        .is_err());
    }

    #[test]
    fn sources_in_the_wrong_folder_go_where_their_package_says_and_duplicates_keep_the_first() {
        let (_dir, root) = project(&[
            (
                "src/old/Place.java",
                "package com.acme.shop;\nclass Place {}\n",
            ),
            (
                "src/com/acme/shop/A.java",
                "package com.acme.shop;\nclass A {}\n",
            ),
            (
                "src2/com/acme/shop/A.java",
                "package com.acme.shop;\nclass AOther {}\n",
            ),
            ("src/Main.java", "public class Main {}\n"),
        ]);
        let plan = plan(&root, &ConvertOptions::default()).unwrap();
        let moved = moves(&plan);
        assert_eq!(
            moved["src/old/Place.java"],
            "src/main/java/com/acme/shop/Place.java"
        );
        assert_eq!(moved["src/Main.java"], "src/main/java/Main.java");
        assert_eq!(plan.counts.moved_to_their_package, 1);
        // two files for the same class: the first (in name order) stays, and the plan says so
        assert_eq!(
            moved["src/com/acme/shop/A.java"],
            "src/main/java/com/acme/shop/A.java"
        );
        assert!(!moved.contains_key("src2/com/acme/shop/A.java"));
        assert!(plan
            .warnings
            .iter()
            .any(|w| w.contains("src2/com/acme/shop/A.java")));
        assert!(plan
            .left_out
            .iter()
            .any(|f| f == "src2/com/acme/shop/A.java"));
    }

    #[test]
    fn a_standard_layout_without_a_pom_keeps_its_paths_and_assumes_the_java_level() {
        let (_dir, root) = project(&[
            (
                "src/main/java/com/acme/App.java",
                "package com.acme;\npublic class App {}\n",
            ),
            ("src/main/resources/app.properties", "a=1\n"),
            (
                "src/test/java/com/acme/AppTest.java",
                "package com.acme;\nimport org.junit.Test;\npublic class AppTest {}\n",
            ),
            ("src/test/resources/t.properties", "t=1\n"),
        ]);
        let plan = plan(&root, &ConvertOptions::default()).unwrap();
        let moved = moves(&plan);
        for (from, to) in [
            (
                "src/main/java/com/acme/App.java",
                "src/main/java/com/acme/App.java",
            ),
            (
                "src/main/resources/app.properties",
                "src/main/resources/app.properties",
            ),
            (
                "src/test/java/com/acme/AppTest.java",
                "src/test/java/com/acme/AppTest.java",
            ),
            (
                "src/test/resources/t.properties",
                "src/test/resources/t.properties",
            ),
        ] {
            assert_eq!(moved[from], to);
        }
        assert_eq!(plan.packaging, "jar");
        assert_eq!((plan.java_level, plan.java_level_declared), (8, false));
        assert!(plan.warnings.iter().any(|w| w.contains("Java 8")));
        // `com.acme` has two parts: it is the group itself
        assert_eq!(plan.group_id, "com.acme");
    }

    #[test]
    fn two_versions_of_a_library_keep_the_newer_and_say_so() {
        let (_dir, root) = project(&[
            ("src/A.java", "class A {}\n"),
            ("lib/log4j-1.2.15.jar", "old"),
            ("lib/log4j-1.2.17.jar", "new"),
        ]);
        let plan = plan(&root, &ConvertOptions::default()).unwrap();
        assert_eq!(plan.dependencies.len(), 1);
        assert_eq!(plan.dependencies[0].version, "1.2.17");
        assert!(plan
            .warnings
            .iter()
            .any(|w| w.contains("1.2.15") && w.contains("1.2.17")));
    }

    #[test]
    fn the_coordinates_can_be_chosen_and_are_checked() {
        let (_dir, root) = messy();
        let chosen = plan(
            &root,
            &ConvertOptions {
                group_id: Some("org.example".into()),
                artifact_id: Some("shop".into()),
                version: Some("2.0".into()),
                java_level: Some(11),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            (
                chosen.group_id.as_str(),
                chosen.artifact_id.as_str(),
                chosen.version.as_str()
            ),
            ("org.example", "shop", "2.0")
        );
        assert_eq!(chosen.java_level, 11);
        assert!(chosen
            .pom
            .contains("<maven.compiler.source>11</maven.compiler.source>"));
        for bad in ["a b", "x/y", "<z>", "", "a;b"] {
            let result = plan(
                &root,
                &ConvertOptions {
                    artifact_id: Some(bad.into()),
                    version: Some("1".into()),
                    ..Default::default()
                },
            );
            // an empty value falls back to the default; the others are refused
            assert_eq!(result.is_err(), !bad.is_empty(), "{bad:?}");
        }
    }

    #[test]
    fn a_maven_folder_and_a_folder_without_java_are_refused() {
        let (_dir, root) = project(&[("pom.xml", "<project/>"), ("src/A.java", "class A {}\n")]);
        assert!(plan(&root, &ConvertOptions::default()).is_err());
        let (_dir, root) = project(&[("README.md", "x")]);
        assert!(plan(&root, &ConvertOptions::default()).is_err());
    }

    #[test]
    fn sources_that_are_not_utf8_make_the_pom_say_so() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("old");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/A.java"),
            b"package a;\n// caf\xE9\nclass A {}\n",
        )
        .unwrap();
        let plan = plan(&root.to_string_lossy(), &ConvertOptions::default()).unwrap();
        assert_eq!(plan.encoding, "ISO-8859-1");
        assert!(plan
            .pom
            .contains("<project.build.sourceEncoding>ISO-8859-1<"));
        // the file is copied byte for byte
        let report = run(ConvertRequest {
            path: root.to_string_lossy().into_owned(),
            options: ConvertOptions::default(),
            confirmed: true,
        })
        .unwrap();
        assert_eq!(
            fs::read(Path::new(&report.destination).join("src/main/java/a/A.java")).unwrap(),
            b"package a;\n// caf\xE9\nclass A {}\n"
        );
    }

    #[test]
    fn the_names_that_reach_the_pom_are_escaped() {
        let pom = pom_text(&PomInput {
            group_id: "g",
            artifact_id: "a",
            version: "1",
            packaging: "jar",
            name: "Shop & <Co> \"x\"",
            level: "1.8".into(),
            encoding: "UTF-8",
            dependencies: &[],
            main_class: None,
        });
        let doc = roxmltree::Document::parse(&pom).unwrap();
        let name = doc
            .root_element()
            .children()
            .find(|n| n.has_tag_name("name"))
            .and_then(|n| n.text())
            .unwrap();
        assert_eq!(name, "Shop & <Co> \"x\"");
    }

    #[test]
    fn known_libraries_follow_the_version_they_moved_group_at() {
        assert_eq!(
            known_library("dom4j", Some("1.6.1")),
            Some(("dom4j", "dom4j", "compile"))
        );
        assert_eq!(
            known_library("dom4j", Some("2.1.3")),
            Some(("org.dom4j", "dom4j", "compile"))
        );
        assert_eq!(
            known_library("hibernate-core", Some("5.4.33.Final")),
            Some(("org.hibernate", "hibernate-core", "compile"))
        );
        assert_eq!(
            known_library("hibernate-core", Some("6.2.0")),
            Some(("org.hibernate.orm", "hibernate-core", "compile"))
        );
        assert_eq!(
            known_library("JUnit", Some("4.12")),
            Some(("junit", "junit", "test"))
        );
        assert_eq!(
            known_library("servlet-api", Some("2.5")),
            Some(("javax.servlet", "servlet-api", "provided"))
        );
        // a quartz of the old group is not guessed, and unknown names say nothing
        assert_eq!(known_library("quartz", Some("1.8.6")), None);
        assert_eq!(known_library("acme-legacy", Some("1.0")), None);
        // every entry of the table has a name in lower case and sane coordinates
        for (name, group, artifact, scope, min, max) in KNOWN {
            assert_eq!(*name, name.to_ascii_lowercase());
            assert!(
                valid_coordinate(group) && valid_coordinate(artifact),
                "{name}"
            );
            assert!(["compile", "provided", "test"].contains(scope), "{name}");
            assert!(min <= max, "{name}");
        }
    }

    #[test]
    fn a_link_in_the_folder_is_not_copied() {
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            fs::write(outside.path().join("Secret.java"), "class Secret {}\n").unwrap();
            let (_dir, root) = project(&[("src/A.java", "class A {}\n")]);
            std::os::unix::fs::symlink(
                outside.path().join("Secret.java"),
                Path::new(&root).join("src/S.java"),
            )
            .unwrap();
            std::os::unix::fs::symlink(outside.path(), Path::new(&root).join("out2")).unwrap();
            let report = run(ConvertRequest {
                path: root,
                options: ConvertOptions::default(),
                confirmed: true,
            })
            .unwrap();
            let copy = Path::new(&report.destination);
            assert!(copy.join("src/main/java/A.java").is_file());
            assert!(!copy.join("src/main/java/S.java").exists());
            assert!(!copy.join("out2").exists());
        }
    }
}
