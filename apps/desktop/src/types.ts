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
  | "expo"
  | "java"
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

export type AppRole = "backend" | "frontend" | "mobile";

export type AppCandidate = {
  path: string;
  name: string;
  stack: StackId;
  role: AppRole;
  framework: string | null;
  evidence: string;
  port: number;
  startScript: string | null;
  packageManager: PackageManager | null;
  pythonFramework: PythonFramework | null;
  hasDockerfile: boolean;
  selected: boolean;
  note: string | null;
};

export type SkippedFolder = { path: string; reason: string };

export type AppsReport = {
  schemaVersion: number;
  project: string;
  projectName: string;
  apps: AppCandidate[];
  skipped: SkippedFolder[];
  existingCompose: string | null;
  warnings: string[];
};

export type MultiAppRequest = {
  path: string;
  name: string;
  stack: StackId;
  role: AppRole;
  framework?: string | null;
  port: number;
  startScript?: string | null;
  packageManager?: PackageManager | null;
  pythonFramework?: PythonFramework | null;
};

export type MultiRequest = {
  path: string;
  projectName?: string;
  apps: MultiAppRequest[];
  services: ServiceId[];
  seed?: string;
  ports?: PortAssignment[];
};

export type ComposeAction = "up" | "stop" | "down";

export type ComposeReport = {
  schemaVersion: number;
  action: ComposeAction;
  command: string;
  status: "available" | "missing" | "timeout" | "execution_error";
  exitCode: number | null;
  succeeded: boolean;
  output: string;
  detail: string;
  durationMs: number;
};

export type ServiceState = {
  service: string;
  container: string;
  state: string;
  status: string;
  health: string | null;
  ports: string[];
};

export type ComposeStatus = {
  schemaVersion: number;
  services: ServiceState[];
  status: "available" | "missing" | "timeout" | "execution_error";
  detail: string;
};

export type ExpoNativeFolder = { name: string; ignoredByGit: boolean };

export type ExpoProject = {
  schemaVersion: number;
  path: string;
  name: string;
  declaredVersion: string;
  installedVersion: string | null;
  sdk: number | null;
  reactNative: string | null;
  usesRouter: boolean;
  packageManager: "npm" | "yarn" | "pnpm" | "bun" | null;
  lockFile: string | null;
  nativeFolders: ExpoNativeFolder[];
  upgradeSupported: boolean;
  unsupportedReason: string | null;
};

export type ExpoLatest = {
  /** Seconds since the Unix epoch. */
  checkedAt: number;
  latestVersion: string;
  latestSdk: number;
  availableSdks: number[];
};

export type ExpoUpdate = {
  currentSdk: number;
  latestSdk: number;
  nextSdk: number;
  stepsBehind: number;
  dismissed: boolean;
};

export type ExpoWatched = {
  path: string;
  name: string;
  /** Null when the folder is gone or is no longer an Expo project. */
  project: ExpoProject | null;
  update: ExpoUpdate | null;
};

export type ExpoWatchState = {
  schemaVersion: number;
  checkOnStart: boolean;
  latest: ExpoLatest | null;
  projects: ExpoWatched[];
  /** Projects behind the latest SDK whose alert was not dismissed. */
  alerts: number;
  checkError: string | null;
};

export type ExpoPlan = {
  schemaVersion: number;
  project: ExpoProject;
  fromSdk: number;
  toSdk: number;
  packageManager: string;
  image: string;
  steps: string[];
  command: string;
  filesTouched: string[];
  notes: string[];
  warnings: string[];
  links: { label: string; url: string }[];
};

export type ExpoChange = {
  name: string;
  section: "dependencies" | "devDependencies";
  from: string | null;
  to: string | null;
};

export type ExpoUpgradeReport = {
  schemaVersion: number;
  succeeded: boolean;
  /** The upgrade failed and the files were put back as they were. */
  restored: boolean;
  fromSdk: number;
  toSdk: number;
  changes: ExpoChange[];
  doctor: string;
  doctorPassed: boolean | null;
  usedLegacyPeerDeps: boolean;
  output: string;
  detail: string;
  durationMs: number;
  backupId: string;
  nextSteps: string[];
};

export type ExpoBackup = {
  id: string;
  fromSdk: number;
  toSdk: number;
  /** Seconds since the Unix epoch. */
  createdAt: number;
  status: "running" | "done" | "failed";
  files: string[];
  modifiedSinceUpgrade: boolean;
};

export type ExpoRestoreReport = {
  schemaVersion: number;
  restored: string[];
  removed: string[];
  detail: string;
};

export type UpdateOffer = {
  currentVersion: string;
  version: string;
  /** Release notes, plain text. */
  notes: string | null;
  /** Seconds since the Unix epoch. */
  date: number | null;
};

export type UpdateCheck = {
  /** A request was made (false when the automatic check is off or not due). */
  checked: boolean;
  offer: UpdateOffer | null;
  error: string | null;
};

export type UpdatePrefs = {
  schemaVersion: number;
  checkOnStart: boolean;
  /** Seconds since the Unix epoch. */
  lastCheck: number | null;
  skippedVersion: string | null;
};

export type DownloadEvent =
  | { event: "started"; contentLength: number | null }
  | { event: "progress"; chunkLength: number }
  | { event: "finished" };

export type JavaApi = { name: string; files: number; fixable: boolean };

export type JavaProject = {
  schemaVersion: number;
  path: string;
  name: string;
  buildTool: string;
  packaging: string;
  javaLevel: number | null;
  javaDeclared: string | null;
  springBoot: string | null;
  /** BOM that pins the Spring Boot version: the upgrade then only changes Java. */
  springBootPinnedBy: string | null;
  modules: string[];
  jdkApis: JavaApi[];
  upgradeAvailable: boolean;
  upgradeSupported: boolean;
  unsupportedReason: string | null;
};

export type JavaHopPlan = { id: string; label: string; recipe: string; checked: boolean };
export type JavaFixup = { module: string; dependency: string; because: string };

export type JavaPlan = {
  schemaVersion: number;
  project: JavaProject;
  fromJava: number | null;
  toJava: number;
  fromSpringBoot: string | null;
  toSpringBoot: string | null;
  image: string;
  hops: JavaHopPlan[];
  fixups: JavaFixup[];
  destination: string;
  command: string;
  notes: string[];
  warnings: string[];
  links: { label: string; url: string }[];
};

export type JavaHopResult = {
  id: string;
  label: string;
  status: "ok" | "failed" | "skipped";
  compiled: boolean | null;
};

export type JavaTests = {
  run: number;
  failures: number;
  errors: number;
  skipped: number;
  passed: boolean;
};

export type JavaChanges = {
  modified: string[];
  added: string[];
  removed: string[];
  truncated: boolean;
  total: number;
};

export type JavaUpgradeReport = {
  schemaVersion: number;
  succeeded: boolean;
  destination: string;
  fromJava: number | null;
  toJava: number;
  fromSpringBoot: string | null;
  toSpringBoot: string | null;
  hops: JavaHopResult[];
  failedHop: string | null;
  /** A step failed and was undone: the copy is at the last step that worked. */
  rolledBack: boolean;
  reached: string | null;
  fixups: JavaFixup[];
  changes: JavaChanges;
  tests: JavaTests | null;
  /** Some tests need Docker (Testcontainers): their errors do not come from the upgrade. */
  testsNeedDocker: boolean;
  output: string;
  detail: string;
  durationMs: number;
  nextSteps: string[];
};
