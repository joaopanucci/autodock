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
  | "environment"
  | "containers"
  | "monitoring"
  | "console"
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
