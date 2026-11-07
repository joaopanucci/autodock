import type { Translate } from "./i18n/core.ts";
import type { Check, Doctor, Scan, SessionProject } from "./types.ts";

/** Short status label of a check (the Engine gets its own wording when reachable). */
export function checkLabel(check: Check, t: Translate) {
  if (check.name === "docker-engine" && check.status === "available")
    return t("check.engineReachable");
  return t(`check.${check.status}`);
}
export const technologyNames: Record<string, string> = {
  nodejs: "Node.js",
  maven: "Maven",
  "spring-boot": "Spring Boot",
  python: "Python",
  laravel: "PHP / Laravel",
  go: "Go",
  dotnet: ".NET",
  rails: "Ruby on Rails",
};
export const toolNames: Record<string, string> = {
  node: "Node.js",
  npm: "npm",
  rustc: "Rust",
  cargo: "Cargo",
  git: "Git",
  docker: "Docker CLI",
  compose: "Docker Compose",
  "docker-engine": "Docker Engine",
  msbuild: "MSBuild",
  "xcode-tools": "Xcode Tools",
};

export function displayPath(path: string) {
  return path.startsWith("\\\\?\\UNC\\")
    ? "\\\\" + path.slice(8)
    : path.replace(/^\\\\\?\\/, "");
}
export function projectName(path: string) {
  return (
    displayPath(path)
      .replace(/[\\/]+$/, "")
      .split(/[\\/]/)
      .pop() || path
  );
}
export function groupModules(scan: Scan) {
  const modules = new Map<
    string,
    { path: string; technologies: string[]; evidence: string[] }
  >();
  for (const component of scan.components) {
    const module = modules.get(component.path) ?? {
      path: component.path,
      technologies: [],
      evidence: [],
    };
    module.technologies = [
      ...new Set([...module.technologies, component.technology]),
    ];
    module.evidence = [...new Set([...module.evidence, ...component.evidence])];
    modules.set(component.path, module);
  }
  for (const finding of scan.findings ?? []) {
    if (!modules.has(finding.module))
      modules.set(finding.module, {
        path: finding.module,
        technologies: [],
        evidence: [],
      });
  }
  return [...modules.values()].sort((a, b) => a.path.localeCompare(b.path));
}
export function rememberProject(
  projects: SessionProject[],
  entry: SessionProject,
  os?: string,
) {
  const key = (path: string) => {
    const normalized = displayPath(path).replace(/\\/g, "/").replace(/\/$/, "");
    return os === "windows" ||
      /^[A-Za-z]:\//.test(normalized) ||
      normalized.startsWith("//")
      ? normalized.toLowerCase()
      : normalized;
  };
  return [
    entry,
    ...projects.filter((p) => key(p.scan.project) !== key(entry.scan.project)),
  ];
}
export function checkSummary(check: Check, t: Translate) {
  // xcode-select prints a directory, not a version. Preserve the raw evidence.
  return (
    (check.stdout || check.stderr).split(/\r?\n/)[0] || t("check.noVersion")
  );
}
export function recommendation(check: Check, t: Translate) {
  if (check.status === "missing")
    return t("rec.missing");
  if (check.status === "timeout")
    return t("rec.timeout");
  if (check.status === "execution_error")
    return check.name === "docker-engine"
      ? t("rec.engineError")
      : t("rec.error");
  return check.name === "docker-engine" ? t("rec.engineOk") : t("rec.ok");
}

export type Compatibility = {
  source: string;
  name: string;
  declared: string;
  installed: string | null;
  status: "unverified" | "matches" | "incompatible";
  reason: string;
};
export function compatibility(
  scan: Scan,
  doctor: Doctor | null,
  t: Translate,
): Compatibility[] {
  return (scan.findings ?? [])
    .filter((f) => f.kind === "runtime")
    .map((f) => {
      const check =
        f.name === "node"
          ? doctor?.checks.find(
              (c) => c.name === "node" && c.status === "available",
            )
          : undefined;
      const installed =
        check?.stdout
          .trim()
          .match(/^v?((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))$/)?.[1] ??
        null;
      const declared = f.value ?? "";
      const exact = declared
        .trim()
        .match(/^((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))$/)?.[1];
      const status =
        exact && installed
          ? exact === installed
            ? "matches"
            : "incompatible"
          : "unverified";
      return {
        source: f.source,
        name: f.name,
        declared,
        installed,
        status,
        reason:
          status === "incompatible"
            ? t("compat.reason.incompatible")
            : status === "matches"
              ? t("compat.reason.matches")
              : t("compat.reason.unverified"),
      };
    });
}
