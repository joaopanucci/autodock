use crate::{
    doctor::{probe, resolve, ToolStatus},
    lifecycle_journal::{LifecycleJournal, PendingOperation},
    memory_capture::{self, Output},
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleAction {
    Start,
    Stop,
    Restart,
}

impl LifecycleAction {
    fn argument(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }

    fn expected_state(self) -> &'static str {
        match self {
            Self::Start | Self::Restart => "running",
            Self::Stop => "exited",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifecycleRequest {
    pub container_id: String,
    pub action: LifecycleAction,
    pub confirmed: bool,
}

impl LifecycleRequest {
    fn arguments(&self) -> Result<Vec<String>, String> {
        if !self.confirmed {
            return Err("Confirmação explícita obrigatória.".into());
        }

        if !(12..=64).contains(&self.container_id.len())
            || !self.container_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("ID Docker inválido: esperado hexadecimal de 12 a 64 caracteres.".into());
        }

        let mut args = vec![self.action.argument().to_owned()];

        if matches!(
            self.action,
            LifecycleAction::Stop | LifecycleAction::Restart
        ) {
            args.extend(["--time".into(), "10".into()]);
        }

        args.push(self.container_id.clone());

        Ok(args)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleReport {
    pub schema_version: u32,
    pub container_id: String,
    pub action: LifecycleAction,
    pub status: ToolStatus,
    pub exit_code: Option<i32>,
    pub detail: String,
    pub duration_ms: u128,
    pub outcome_confirmed: bool,
    pub state_verified: bool,
    pub observed_state: Option<String>,
    pub reconciliation_required: bool,
}

/// Public executor contract: implementations must scope every call to `context`.
/// Inspection must return the full immutable Docker ID, not the requested alias.
pub trait LifecycleExecutor {
    fn inspect(&mut self, context: &str, id: &str) -> Result<ContainerObservation, String>;
    /// `Missing` guarantees no command was spawned. Every other unverified
    /// result retains the reservation, including spawn errors and timeouts.
    fn execute(&mut self, context: &str, arguments: &[String]) -> LifecycleOutput;
}

#[derive(Debug, Clone)]
pub struct LifecycleOutput {
    pub status: ToolStatus,
    pub exit_code: Option<i32>,
    pub truncated: bool,
    pub duration_ms: u128,
}

impl From<Output> for LifecycleOutput {
    fn from(output: Output) -> Self {
        Self {
            status: output.status,
            exit_code: output.exit_code,
            truncated: output.truncated,
            duration_ms: output.duration_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContainerObservation {
    pub container_id: String,
    pub state: String,
}

fn validate_observation(observation: &ContainerObservation, requested: &str) -> Result<(), String> {
    if observation.container_id.len() != 64
        || !observation
            .container_id
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || !observation
            .container_id
            .to_ascii_lowercase()
            .starts_with(&requested.to_ascii_lowercase())
        || !matches!(
            observation.state.as_str(),
            "created" | "running" | "paused" | "restarting" | "removing" | "exited" | "dead"
        )
    {
        return Err("Identidade ou estado observado invalido.".into());
    }
    Ok(())
}

pub struct LifecycleService {
    journal: LifecycleJournal,
    context: String,
}

impl LifecycleService {
    pub fn new(directory: PathBuf, context: String) -> Result<Self, String> {
        PendingOperation {
            schema_version: 1,
            operation_id: "0".repeat(32),
            docker_context: context.clone(),
            container_id: "0".repeat(64),
            action: "start".into(),
        }
        .validate()?;
        Ok(Self {
            journal: LifecycleJournal::new(directory),
            context,
        })
    }

    pub fn execute_with_docker(
        &self,
        request: LifecycleRequest,
    ) -> Result<LifecycleReport, String> {
        self.execute(request, &mut DockerExecutor)
    }

    pub fn reconcile_with_docker(
        &self,
        request: ReconciliationRequest,
    ) -> Result<ReconciliationReport, String> {
        self.reconcile(request, &mut DockerExecutor)
    }

    /// Listing after restart never replays Docker operations. Any unreadable
    /// record or abandoned temporary blocks the entire journal.
    pub fn pending(&self) -> Result<Vec<PendingOperation>, String> {
        self.journal.list()
    }

    pub fn execute(
        &self,
        mut request: LifecycleRequest,
        executor: &mut impl LifecycleExecutor,
    ) -> Result<LifecycleReport, String> {
        request.arguments()?;
        let transaction = self.journal.transaction()?;
        let records = transaction.list()?;
        if records.iter().any(|p| {
            p.docker_context == self.context
                && p.container_id
                    .to_ascii_lowercase()
                    .starts_with(&request.container_id.to_ascii_lowercase())
        }) {
            return Err("Container bloqueado: reconciliacao explicita necessaria.".into());
        }
        let before = executor.inspect(&self.context, &request.container_id)?;
        validate_observation(&before, &request.container_id)?;
        request.container_id = before.container_id.to_ascii_lowercase();
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        // IDs are collision-resistant labels, not credentials. Any collision is
        // refused by exclusive publication rather than overwriting old evidence.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let operation_id = format!(
            "{:032x}",
            stamp
                ^ ((std::process::id() as u128) << 64)
                ^ SEQUENCE.fetch_add(1, Ordering::Relaxed) as u128
        );
        let pending = PendingOperation {
            schema_version: 1,
            operation_id,
            docker_context: self.context.clone(),
            container_id: request.container_id.clone(),
            action: request.action.argument().into(),
        };
        transaction.record(&pending)?;
        // No mutating command can run before the reservation is durable/readable.
        // Retain the lock through verification: a persistent reservation alone
        // cannot distinguish an interrupted executor from one still running.
        let output = executor.execute(&self.context, &request.arguments()?);
        let observed_state = if output.status != ToolStatus::Missing {
            executor
                .inspect(&self.context, &request.container_id)
                .ok()
                .filter(|o| validate_observation(o, &request.container_id).is_ok())
                .map(|o| o.state)
        } else {
            None
        };
        let report = lifecycle_report(request, output, observed_state);
        if report.state_verified || report.status == ToolStatus::Missing {
            transaction.complete(&pending)?;
        }
        Ok(report)
    }

    pub fn reconcile(
        &self,
        request: ReconciliationRequest,
        executor: &mut impl LifecycleExecutor,
    ) -> Result<ReconciliationReport, String> {
        if !request.acknowledged {
            return Err("Reconciliacao exige confirmacao explicita.".into());
        }
        if request.container_id.len() != 64
            || !request.container_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Reconciliacao exige o ID completo persistido.".into());
        }
        let transaction = self.journal.transaction()?;
        let pending = transaction
            .list()?
            .into_iter()
            .find(|p| {
                p.docker_context == self.context
                    && p.container_id.eq_ignore_ascii_case(&request.container_id)
            })
            .ok_or("Nao existe operacao pendente para reconciliar.")?;
        let observed = executor.inspect(&self.context, &pending.container_id)?;
        validate_observation(&observed, &pending.container_id)?;
        if !matches!(
            observed.state.as_str(),
            "running" | "exited" | "created" | "paused" | "dead"
        ) {
            return Err("Estado transitorio: reconciliacao mantida.".into());
        }
        transaction.complete(&pending)?;
        Ok(ReconciliationReport { schema_version: 1, container_id: pending.container_id, observed_state: observed.state, released: true, detail: "Estado observado e pendencia liberada mediante confirmacao. O resultado anterior permanece indeterminado.".into() })
    }
}

struct DockerExecutor;
impl LifecycleExecutor for DockerExecutor {
    fn inspect(&mut self, context: &str, id: &str) -> Result<ContainerObservation, String> {
        let result = probe(
            "autodock-lifecycle-inspect",
            "docker",
            &[
                "--context",
                context,
                "inspect",
                "--type",
                "container",
                "--format",
                "{{.Id}} {{.State.Status}}",
                id,
            ],
            Duration::from_secs(10),
        );
        if result.status != ToolStatus::Available || result.exit_code != Some(0) || result.truncated
        {
            return Err("Nao foi possivel verificar identidade e estado no Docker Engine.".into());
        }
        let parts: Vec<_> = result.stdout.split_whitespace().collect();
        if parts.len() != 2 {
            return Err("Inspecao Docker ambigua.".into());
        }
        Ok(ContainerObservation {
            container_id: parts[0].into(),
            state: parts[1].into(),
        })
    }

    fn execute(&mut self, context: &str, arguments: &[String]) -> LifecycleOutput {
        let Some(docker) = resolve("docker") else {
            return Output::missing().into();
        };
        memory_capture::run(
            Command::new(docker)
                .args(["--context", context])
                .args(arguments),
            Duration::from_secs(35),
            100,
        )
        .into()
    }
}

fn default_service() -> Result<LifecycleService, String> {
    let context = probe(
        "autodock-lifecycle-context",
        "docker",
        &["context", "show"],
        Duration::from_secs(10),
    );
    if context.status != ToolStatus::Available || context.exit_code != Some(0) || context.truncated
    {
        return Err("Contexto Docker indisponivel.".into());
    }
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or("LOCALAPPDATA indisponivel")?;
    #[cfg(not(windows))]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME indisponivel")?;
    if !base.is_absolute() {
        return Err("Diretorio de dados deve ser absoluto.".into());
    }
    let base = base.canonicalize().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    let directory = base.join("Library/Application Support/AutoDock/lifecycle/pending");
    #[cfg(target_os = "linux")]
    let directory = match std::env::var_os("XDG_DATA_HOME") {
        Some(value) if PathBuf::from(&value).is_absolute() => {
            PathBuf::from(value).join("autodock/lifecycle/pending")
        }
        Some(_) => return Err("XDG_DATA_HOME deve ser absoluto.".into()),
        None => base.join(".local/share/autodock/lifecycle/pending"),
    };
    #[cfg(windows)]
    let directory = base.join("AutoDock/lifecycle/pending");
    LifecycleService::new(directory, context.stdout.trim().into())
}

pub fn execute_container_lifecycle(request: LifecycleRequest) -> Result<LifecycleReport, String> {
    request.arguments()?;
    default_service()?.execute_with_docker(request)
}

fn lifecycle_report(
    request: LifecycleRequest,
    output: LifecycleOutput,
    observed_state: Option<String>,
) -> LifecycleReport {
    let cli_succeeded =
        output.status == ToolStatus::Available && output.exit_code == Some(0) && !output.truncated;

    let state_verified =
        cli_succeeded && observed_state.as_deref() == Some(request.action.expected_state());

    let detail = if state_verified {
        "Docker CLI concluiu a operação e o estado esperado foi observado.".into()
    } else if output.status == ToolStatus::Timeout {
        "Prazo excedido. Resultado indeterminado; não repita automaticamente. Verifique o estado do container.".into()
    } else if output.status == ToolStatus::Missing {
        "Docker CLI não encontrado.".into()
    } else if cli_succeeded {
        format!(
            "Docker CLI concluiu, mas o estado esperado ({}) não foi confirmado. Estado observado: {}.",
            request.action.expected_state(),
            observed_state.as_deref().unwrap_or("indisponível")
        )
    } else {
        "Operação não confirmada. Verifique o estado do container antes de repetir.".into()
    };

    let reconciliation_required = output.status != ToolStatus::Missing && !state_verified;

    LifecycleReport {
        schema_version: 1,
        container_id: request.container_id,
        action: request.action,
        status: output.status,
        exit_code: output.exit_code,
        detail,
        duration_ms: output.duration_ms,
        outcome_confirmed: cli_succeeded,
        state_verified,
        observed_state,
        reconciliation_required,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReconciliationRequest {
    pub container_id: String,
    pub acknowledged: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationReport {
    pub schema_version: u32,
    pub container_id: String,
    pub observed_state: String,
    pub released: bool,
    pub detail: String,
}

pub fn reconcile_container_lifecycle(
    request: ReconciliationRequest,
) -> Result<ReconciliationReport, String> {
    if !request.acknowledged {
        return Err("Reconciliacao exige confirmacao explicita.".into());
    }
    default_service()?.reconcile_with_docker(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, fs};

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap()
    }
    fn id() -> String {
        "abcdef1234567890".repeat(4)
    }
    fn request(action: LifecycleAction, id: &str) -> LifecycleRequest {
        LifecycleRequest {
            container_id: id.into(),
            action,
            confirmed: true,
        }
    }
    struct Fake {
        state: String,
        status: ToolStatus,
        exit: Option<i32>,
        truncated: bool,
        fail_inspection: bool,
        inspections: usize,
        calls: usize,
        wrong_identity: bool,
    }
    impl Default for Fake {
        fn default() -> Self {
            Self {
                state: "running".into(),
                status: ToolStatus::Available,
                exit: Some(0),
                truncated: false,
                fail_inspection: false,
                inspections: 0,
                calls: 0,
                wrong_identity: false,
            }
        }
    }
    impl LifecycleExecutor for Fake {
        fn inspect(&mut self, context: &str, _: &str) -> Result<ContainerObservation, String> {
            assert!(context == "default" || context == "remote");
            self.inspections += 1;
            if self.fail_inspection && self.inspections > 1 {
                return Err("offline".into());
            }
            Ok(ContainerObservation {
                container_id: if self.wrong_identity && self.inspections > 1 {
                    "f".repeat(64)
                } else {
                    id()
                },
                state: self.state.clone(),
            })
        }
        fn execute(&mut self, _: &str, arguments: &[String]) -> LifecycleOutput {
            self.calls += 1;
            assert_eq!(arguments.last(), Some(&id()));
            LifecycleOutput {
                status: self.status,
                exit_code: self.exit,
                truncated: self.truncated,
                duration_ms: 1,
            }
        }
    }
    fn service(dir: &tempfile::TempDir) -> LifecycleService {
        LifecycleService::new(dir.path().join("pending"), "default".into()).unwrap()
    }
    fn acknowledgement() -> ReconciliationRequest {
        ReconciliationRequest {
            container_id: id(),
            acknowledged: true,
        }
    }

    #[test]
    fn only_permitted_operations_and_fixed_arguments() {
        for (action, args) in [
            (LifecycleAction::Start, vec!["start", "abcdef123456"]),
            (
                LifecycleAction::Stop,
                vec!["stop", "--time", "10", "abcdef123456"],
            ),
            (
                LifecycleAction::Restart,
                vec!["restart", "--time", "10", "abcdef123456"],
            ),
        ] {
            assert_eq!(request(action, "abcdef123456").arguments().unwrap(), args);
        }
    }

    #[test]
    fn rejects_invalid_ids_before_execution() {
        let dir = tempdir();
        let service = service(&dir);
        let mut fake = Fake::default();
        for value in [
            "",
            "short",
            "--help",
            "abcdef012345;rm",
            "../abcdef012345",
            "abcdef012345\n",
            "gggggggggggg",
            &"a".repeat(65),
        ] {
            assert!(service
                .execute(request(LifecycleAction::Start, value), &mut fake)
                .is_err());
        }
        assert_eq!((fake.calls, fake.inspections), (0, 0));
    }

    #[test]
    fn requires_explicit_confirmation() {
        let dir = tempdir();
        let service = service(&dir);
        let mut fake = Fake::default();
        let mut r = request(LifecycleAction::Start, &id());
        r.confirmed = false;
        assert!(service.execute(r, &mut fake).is_err());
        assert_eq!((fake.calls, fake.inspections), (0, 0));
    }

    #[test]
    fn rejects_unknown_actions_and_fields() {
        for action in ["rm", "prune", "exec", "kill"] {
            assert!(serde_json::from_value::<LifecycleRequest>(
                serde_json::json!({"containerId": id(), "action": action, "confirmed": true})
            )
            .is_err());
        }
        assert!(serde_json::from_value::<LifecycleRequest>(serde_json::json!({"containerId": id(), "action": "start", "confirmed": true, "command": "rm"})).is_err());
    }

    #[test]
    fn verifies_expected_state_after_success_and_clears_reservation() {
        for (action, state) in [
            (LifecycleAction::Start, "running"),
            (LifecycleAction::Stop, "exited"),
            (LifecycleAction::Restart, "running"),
        ] {
            let dir = tempdir();
            let service = service(&dir);
            let mut fake = Fake {
                state: state.into(),
                ..Fake::default()
            };
            let report = service
                .execute(request(action, "ABCDEF123456"), &mut fake)
                .unwrap();
            assert!(
                report.outcome_confirmed
                    && report.state_verified
                    && !report.reconciliation_required
            );
            assert_eq!(report.container_id, id());
            assert!(service.pending().unwrap().is_empty());
            assert!(service.execute(request(action, &id()), &mut fake).is_ok());
        }
    }

    #[test]
    fn uncertain_results_survive_restart_and_block_aliases() {
        for (status, exit, state, fail, truncated, wrong) in [
            (ToolStatus::Timeout, None, "running", false, false, false),
            (
                ToolStatus::Available,
                Some(0),
                "exited",
                false,
                false,
                false,
            ),
            (
                ToolStatus::Available,
                Some(0),
                "running",
                true,
                false,
                false,
            ),
            (
                ToolStatus::ExecutionError,
                Some(1),
                "running",
                false,
                false,
                false,
            ),
            (
                ToolStatus::Available,
                Some(0),
                "running",
                false,
                true,
                false,
            ),
            (
                ToolStatus::Available,
                Some(0),
                "running",
                false,
                false,
                true,
            ),
        ] {
            let dir = tempdir();
            let first = service(&dir);
            let mut fake = Fake {
                status,
                exit,
                state: state.into(),
                fail_inspection: fail,
                truncated,
                wrong_identity: wrong,
                ..Fake::default()
            };
            let report = first
                .execute(request(LifecycleAction::Restart, &id()), &mut fake)
                .unwrap();
            assert!(!report.state_verified && report.reconciliation_required);
            drop(first);
            let reopened = service(&dir);
            assert_eq!(reopened.pending().unwrap().len(), 1);
            for alias in [&id(), &id().to_uppercase(), &id()[..12]] {
                assert!(reopened
                    .execute(request(LifecycleAction::Restart, alias), &mut fake)
                    .is_err());
            }
            assert_eq!(fake.calls, 1);
        }
    }

    #[test]
    fn missing_docker_skips_post_inspection_and_releases_reservation() {
        let dir = tempdir();
        let service = service(&dir);
        let mut fake = Fake {
            status: ToolStatus::Missing,
            exit: None,
            ..Fake::default()
        };
        let report = service
            .execute(request(LifecycleAction::Start, &id()), &mut fake)
            .unwrap();
        assert!(!report.state_verified && !report.reconciliation_required);
        assert_eq!(fake.inspections, 1);
        assert!(service.pending().unwrap().is_empty());
    }

    #[test]
    fn no_execution_when_preinspection_identity_is_invalid() {
        struct Invalid;
        impl LifecycleExecutor for Invalid {
            fn inspect(&mut self, _: &str, _: &str) -> Result<ContainerObservation, String> {
                Ok(ContainerObservation {
                    container_id: "f".repeat(64),
                    state: "running".into(),
                })
            }
            fn execute(&mut self, _: &str, _: &[String]) -> LifecycleOutput {
                panic!("must not execute")
            }
        }
        let dir = tempdir();
        assert!(service(&dir)
            .execute(request(LifecycleAction::Start, &id()), &mut Invalid)
            .is_err());
        assert!(service(&dir).pending().unwrap().is_empty());
    }

    #[test]
    fn reconciliation_preserves_pending_until_explicit_stable_identical_inspection() {
        let dir = tempdir();
        let service = service(&dir);
        let mut fake = Fake {
            status: ToolStatus::Timeout,
            ..Fake::default()
        };
        service
            .execute(request(LifecycleAction::Restart, &id()), &mut fake)
            .unwrap();
        let mut r = acknowledgement();
        r.acknowledged = false;
        assert!(service.reconcile(r, &mut fake).is_err());
        let mut r = acknowledgement();
        r.container_id.truncate(12);
        assert!(service.reconcile(r, &mut fake).is_err());
        for state in ["restarting", "removing", "unknown"] {
            fake.state = state.into();
            assert!(service.reconcile(acknowledgement(), &mut fake).is_err());
            assert_eq!(service.pending().unwrap().len(), 1);
        }
        fake.state = "running".into();
        fake.fail_inspection = true;
        assert!(service.reconcile(acknowledgement(), &mut fake).is_err());
        fake.fail_inspection = false;
        fake.wrong_identity = true;
        assert!(service.reconcile(acknowledgement(), &mut fake).is_err());
        assert_eq!(service.pending().unwrap().len(), 1);
        fake.wrong_identity = false;
        let mut r = acknowledgement();
        r.container_id = id().to_uppercase();
        assert!(service.reconcile(r, &mut fake).unwrap().released);
        assert!(service.pending().unwrap().is_empty());
        assert_eq!(fake.calls, 1); // reconciliation never invokes mutations
    }

    #[test]
    fn reconciliation_without_pending_never_inspects() {
        let dir = tempdir();
        let mut fake = Fake::default();
        assert!(service(&dir)
            .reconcile(acknowledgement(), &mut fake)
            .is_err());
        assert_eq!(fake.inspections, 0);
    }

    #[test]
    fn corrupt_or_abandoned_records_prevent_any_docker_call() {
        for (name, contents) in [
            ("abandoned.tmp", b"partial".as_slice()),
            ("invalid.json", b"{".as_slice()),
        ] {
            let dir = tempdir();
            let service = service(&dir);
            fs::create_dir(dir.path().join("pending")).unwrap();
            fs::write(dir.path().join("pending").join(name), contents).unwrap();
            let mut fake = Fake::default();
            assert!(service
                .execute(request(LifecycleAction::Start, &id()), &mut fake)
                .is_err());
            assert_eq!((fake.calls, fake.inspections), (0, 0));
        }
    }

    #[test]
    fn lock_covers_inspection_execution_and_reconciliation() {
        struct Nested<'a> {
            service: &'a LifecycleService,
            calls: Cell<usize>,
        }
        impl LifecycleExecutor for Nested<'_> {
            fn inspect(&mut self, _: &str, _: &str) -> Result<ContainerObservation, String> {
                assert!(self.service.pending().is_err());
                Ok(ContainerObservation {
                    container_id: id(),
                    state: "running".into(),
                })
            }
            fn execute(&mut self, _: &str, _: &[String]) -> LifecycleOutput {
                assert!(self
                    .service
                    .execute(request(LifecycleAction::Start, &id()), &mut Fake::default())
                    .is_err());
                assert!(self
                    .service
                    .reconcile(acknowledgement(), &mut Fake::default())
                    .is_err());
                self.calls.set(self.calls.get() + 1);
                LifecycleOutput {
                    status: ToolStatus::Available,
                    exit_code: Some(0),
                    truncated: false,
                    duration_ms: 0,
                }
            }
        }
        let dir = tempdir();
        let service = service(&dir);
        let mut nested = Nested {
            service: &service,
            calls: Cell::new(0),
        };
        assert!(
            service
                .execute(request(LifecycleAction::Start, &id()), &mut nested)
                .unwrap()
                .state_verified
        );
        assert_eq!(nested.calls.get(), 1);
    }

    #[test]
    fn panic_after_reservation_is_recovered_without_replay() {
        struct Interrupted;
        impl LifecycleExecutor for Interrupted {
            fn inspect(&mut self, _: &str, _: &str) -> Result<ContainerObservation, String> {
                Ok(ContainerObservation {
                    container_id: id(),
                    state: "running".into(),
                })
            }
            fn execute(&mut self, _: &str, _: &[String]) -> LifecycleOutput {
                panic!("simulated interruption")
            }
        }
        let dir = tempdir();
        assert!(std::panic::catch_unwind(
            || service(&dir).execute(request(LifecycleAction::Start, &id()), &mut Interrupted)
        )
        .is_err());
        let reopened = service(&dir);
        assert_eq!(reopened.pending().unwrap().len(), 1);
        let mut fake = Fake::default();
        assert!(reopened
            .execute(request(LifecycleAction::Start, &id()), &mut fake)
            .is_err());
        assert_eq!(fake.calls, 0);
        assert!(
            reopened
                .reconcile(acknowledgement(), &mut fake)
                .unwrap()
                .released
        );
    }

    #[test]
    fn context_is_part_of_identity_and_invalid_contexts_are_rejected() {
        let dir = tempdir();
        let mut fake = Fake {
            status: ToolStatus::Timeout,
            ..Fake::default()
        };
        service(&dir)
            .execute(request(LifecycleAction::Start, &id()), &mut fake)
            .unwrap();
        let remote = LifecycleService::new(dir.path().join("pending"), "remote".into()).unwrap();
        remote
            .execute(request(LifecycleAction::Start, &id()), &mut fake)
            .unwrap();
        assert_eq!(remote.pending().unwrap().len(), 2);
        for context in ["", "--help", "../bad", "a b", &"x".repeat(129)] {
            // Leading hyphens must never become CLI option values.
            assert!(LifecycleService::new(dir.path().join("pending"), context.into()).is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn directory_sync_failure_prevents_execution_and_retains_ambiguous_publication() {
        let dir = tempdir();
        let service = service(&dir);
        assert!(service.pending().unwrap().is_empty());
        let failure = crate::lifecycle_fs::fail_next_sync(&dir.path().join("pending"));
        let mut fake = Fake::default();
        let error = service
            .execute(request(LifecycleAction::Start, &id()), &mut fake)
            .unwrap_err();
        assert!(error.contains("simulated directory sync failure"));
        assert_eq!(fake.calls, 0);
        drop(failure);
        let files: Vec<_> = fs::read_dir(dir.path().join("pending"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert!(files
            .iter()
            .any(|path| path.extension().is_some_and(|ext| ext == "json")));
        assert!(files
            .iter()
            .any(|path| path.extension().is_some_and(|ext| ext == "tmp")));
        assert!(service.pending().is_err());
        assert!(service
            .execute(request(LifecycleAction::Start, &id()), &mut fake)
            .is_err());
        assert_eq!(fake.calls, 0);
    }
}
