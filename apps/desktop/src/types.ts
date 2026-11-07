export type Component = {
  path: string;
  technology: string;
  evidence: string[];
};
export type Finding = {
  module: string;
  kind: string;
  name: string;
  value: string | null;
  source: string;
  status: "declared" | "detected";
};
export type Scan = {
  schemaVersion: number;
  project: string;
  components: Component[];
  warnings: string[];
  findings?: Finding[];
};
export type Check = {
  name: string;
  status: "available" | "missing" | "timeout" | "execution_error";
  executable?: string | null;
  exitCode?: number | null;
  durationMs?: number;
  stdout: string;
  stderr: string;
  detail: string;
};
export type Doctor = {
  schemaVersion: number;
  os: string;
  checks: Check[];
  notes: string[];
};
export type SessionProject = { scan: Scan; analyzedAt: string };
export type Page =
  | "overview"
  | "projects"
  | "setup"
  | "contribute"
  | "environment"
  | "containers"
  | "monitoring"
  | "console"
  | "learn"
  | "settings";

export type DockerContainer = {
  id: string;
  name: string;
  image: string;
  state: string;
  status: string;
  ports: string;
  project: string | null;
  service: string | null;
  health: string | null;
};

export type DockerContainersReport = {
  schemaVersion: number;
  containers: DockerContainer[];
};

export type ContainerMetrics = {
  id: string;
  name: string;
  cpuPercent: number | null;
  memoryUsageBytes: number | null;
  memoryLimitBytes: number | null;
  memoryPercent: number | null;
  networkRxBytes: number | null;
  networkTxBytes: number | null;
};

export type MonitoringReport = {
  schemaVersion: number;
  collectedAtUnixMs: number;
  containers: ContainerMetrics[];
};
export type ConsoleOperation = "containers" | "engine_info" | "docker_version";

export type ConsoleStatus =
  "available" | "missing" | "timeout" | "execution_error";

export type ConsoleReport = {
  schemaVersion: number;
  operation: ConsoleOperation;
  label: string;
  command: string;
  status: ConsoleStatus;
  exitCode: number | null;
  stdout: string;
  stderr: string;
  detail: string;
  durationMs: number;
  truncated?: boolean;
};

export type LogsReport = {
  schemaVersion: number;
  containerId: string;
  tail: 100 | 300 | 500;
  status: ConsoleStatus;
  exitCode: number | null;
  stdout: string;
  stderr: string;
  truncated: boolean;
  detail: string;
  durationMs: number;
};

export type StackId =
  | "nodejs"
  | "spring-boot"
  | "python"
  | "laravel"
  | "go"
  | "dotnet"
  | "rails";
export type ServiceId =
  | "postgres"
  | "mysql"
  | "mongodb"
  | "redis"
  | "rabbitmq"
  | "adminer"
  | "mailhog";
export type PackageManager = "npm" | "pnpm" | "yarn";
export type PythonFramework = "fastapi" | "django" | "flask";

export type StackSuggestion = {
  stack: StackId;
  evidence: string;
  scripts: string[];
  packageManager: PackageManager | null;
  pythonFramework: PythonFramework | null;
  vite: boolean;
  nodeFramework?: "nextjs" | "nestjs" | null;
};

export type GenerateRequest = {
  path: string;
  stack: StackId;
  port: number;
  runtimeVersion?: string;
  packageManager?: PackageManager;
  startScript?: string;
  pythonFramework?: PythonFramework;
  pythonModule?: string;
  services: ServiceId[];
  newProject?: boolean;
  projectName?: string;
  seed?: string;
  ports?: PortAssignment[];
};

export type PortAssignment = {
  name: string;
  containerPort: number;
  hostPort: number;
  preferredPort: number;
};

export type GeneratedFile = {
  name: string;
  description: string;
  content: string;
  /** Holds secrets: must not be versioned. */
  sensitive?: boolean;
  exists: boolean;
};

export type GeneratePlan = {
  schemaVersion: number;
  project: string;
  files: GeneratedFile[];
  notes: string[];
  /** Sent back on apply so the written files match the preview. */
  seed?: string;
  ports?: PortAssignment[];
};

export type ApplyReport = {
  written: string[];
  skipped: { name: string; reason: string }[];
};

export type IdeInfo = { id: string; label: string };

export type FrameworkId =
  | "laravel"
  | "django"
  | "vite-react"
  | "vite-vue"
  | "nextjs"
  | "nestjs";
export type ScaffoldPlan = {
  title: string;
  image: string;
  command: string;
  notes: string[];
};
export type ScaffoldReport = {
  success: boolean;
  durationMs: number;
  output: string;
  detail: string;
};
