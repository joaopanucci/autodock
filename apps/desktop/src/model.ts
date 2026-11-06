import type { Check, Doctor, Scan, SessionProject } from "./types.ts";

export const checkLabels = {
  available: "Instalado no sistema",
  missing: "Não encontrado",
  timeout: "Tempo esgotado",
  execution_error: "Erro na verificação",
};
export const technologyNames: Record<string, string> = {
  nodejs: "Node.js",
  maven: "Maven",
  "spring-boot": "Spring Boot",
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
export function checkSummary(check: Check) {
  // xcode-select prints a directory, not a version. Preserve the raw evidence.
  return (
    (check.stdout || check.stderr).split(/\r?\n/)[0] || "Sem versão informada"
  );
}
export function recommendation(check: Check) {
  if (check.status === "missing")
    return "Confira o PATH e a instalação. A busca não confirma que a ferramenta esteja ausente da máquina.";
  if (check.status === "timeout")
    return "A ferramenta não respondeu no prazo. Verifique seu estado e repita o diagnóstico; timeout não confirma ausência.";
  if (check.status === "execution_error")
    return check.name === "docker-engine"
      ? "O Engine não pôde ser consultado. Confira o contexto, a conexão e as permissões do Docker; a causa não foi confirmada."
      : "Confira a saída da verificação. Um erro de execução não confirma incompatibilidade ou ausência.";
  return check.name === "docker-engine"
    ? "O Engine respondeu à consulta. Nenhum container foi alterado."
    : "A verificação respondeu com sucesso. Compatibilidade depende dos requisitos de cada projeto.";
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
            ? "A versão exata declarada difere da versão instalada verificada."
            : status === "matches"
              ? "A versão instalada corresponde à declaração exata."
              : "Comparação não realizada. Nesta etapa, apenas engines.node com versão exata x.y.z e resposta Node válida são comparados; ranges e propriedades permanecem literais.",
      };
    });
}
