/** Monaco language ids by file extension (only languages Monaco ships a tokenizer for). */
const BY_EXTENSION: Record<string, string> = {
  java: "java",
  kt: "kotlin",
  kts: "kotlin",
  scala: "scala",
  // Gradle scripts are Groovy; Java's colours are the closest Monaco has.
  gradle: "java",
  groovy: "java",
  xml: "xml",
  pom: "xml",
  xsd: "xml",
  xsl: "xml",
  xslt: "xml",
  wsdl: "xml",
  svg: "xml",
  html: "html",
  htm: "html",
  jsp: "html",
  xhtml: "html",
  css: "css",
  scss: "scss",
  less: "less",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "javascript",
  ts: "typescript",
  mts: "typescript",
  cts: "typescript",
  tsx: "typescript",
  json: "json",
  jsonc: "json",
  md: "markdown",
  markdown: "markdown",
  yml: "yaml",
  yaml: "yaml",
  properties: "ini",
  ini: "ini",
  toml: "ini",
  conf: "ini",
  env: "ini",
  sh: "shell",
  bash: "shell",
  zsh: "shell",
  bat: "bat",
  cmd: "bat",
  ps1: "powershell",
  py: "python",
  rb: "ruby",
  php: "php",
  go: "go",
  rs: "rust",
  cs: "csharp",
  c: "cpp",
  h: "cpp",
  cc: "cpp",
  cpp: "cpp",
  hpp: "cpp",
  swift: "swift",
  lua: "lua",
  dart: "dart",
  sql: "sql",
  graphql: "graphql",
  proto: "protobuf",
  dockerfile: "dockerfile",
};

const BY_NAME: Record<string, string> = {
  dockerfile: "dockerfile",
  jenkinsfile: "java",
};

/** The language Monaco should use for a file, from its name. */
export function languageFor(path: string): string {
  const name = path.split("/").pop()?.toLowerCase() ?? "";
  if (BY_NAME[name]) return BY_NAME[name];
  if (name === ".env" || name.startsWith(".env.")) return "ini";
  const dot = name.lastIndexOf(".");
  if (dot < 0) return "plaintext";
  return BY_EXTENSION[name.slice(dot + 1)] ?? "plaintext";
}

/** The name of a path (its last part). */
export function baseName(path: string): string {
  return path.split("/").pop() || path;
}

/** The folder a path lives in (`""` for the project folder itself). */
export function parentOf(path: string): string {
  const index = path.lastIndexOf("/");
  return index < 0 ? "" : path.slice(0, index);
}

/** `parent` + `name` as a path inside the project. */
export function joinPath(parent: string, name: string): string {
  return parent ? `${parent}/${name}` : name;
}
