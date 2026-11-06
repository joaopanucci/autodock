use crate::{
    doctor::{probe, resolve, ToolStatus},
    memory_capture::{self, Output},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    process::Command,
    sync::{Mutex, OnceLock},
    time::Duration,
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

#[derive(Default)]
struct LifecycleRegistry {
    active: HashSet<String>,
    pending: HashSet<String>,
}

static REGISTRY: OnceLock<Mutex<LifecycleRegistry>> = OnceLock::new();

fn registry() -> &'static Mutex<LifecycleRegistry> {
    REGISTRY.get_or_init(|| Mutex::new(LifecycleRegistry::default()))
}

fn normalize_identity(id: &str) -> String {
    id.to_ascii_lowercase()
}

// Dois identificadores podem representar o mesmo container
// quando um deles e prefixo hexadecimal do outro.
// Em caso de duvida, bloqueamos conservadoramente.
fn identities_overlap(left: &str, right: &str) -> bool {
    let left = normalize_identity(left);
    let right = normalize_identity(right);

    left.starts_with(&right) || right.starts_with(&left)
}

struct OperationGuard {
    id: String,
}

impl OperationGuard {
    fn acquire(id: &str) -> Result<Self, String> {
        let mut state = registry()
            .lock()
            .map_err(|_| "Controle de concorrência indisponível.")?;

        let canonical = normalize_identity(id);

        if state
            .pending
            .iter()
            .any(|pending| identities_overlap(pending, &canonical))
        {
            return Err("Container bloqueado: reconciliação explícita necessária.".into());
        }

        if state
            .active
            .iter()
            .any(|active| identities_overlap(active, &canonical))
        {
            return Err("Já existe uma operação em andamento neste container.".into());
        }

        state.active.insert(canonical.clone());

        Ok(Self { id: canonical })
    }

    fn acquire_reconciliation(id: &str) -> Result<Self, String> {
        let mut state = registry()
            .lock()
            .map_err(|_| "Controle de concorrência indisponível.")?;

        let canonical = normalize_identity(id);

        // A reconciliacao exige a identidade original exata,
        // normalizada apenas quanto a letras maiusculas.
        if !state.pending.contains(&canonical) {
            return Err("Não existe operação pendente para reconciliar.".into());
        }

        if state
            .active
            .iter()
            .any(|active| identities_overlap(active, &canonical))
        {
            return Err("Já existe uma operação em andamento neste container.".into());
        }

        state.active.insert(canonical.clone());

        Ok(Self { id: canonical })
    }

    fn mark_pending(&self) -> Result<(), String> {
        let mut state = registry()
            .lock()
            .map_err(|_| "Controle de concorrência indisponível.")?;

        state.pending.insert(self.id.clone());
        Ok(())
    }

    fn clear_pending(&self) -> Result<(), String> {
        let mut state = registry()
            .lock()
            .map_err(|_| "Controle de concorrência indisponível.")?;

        state.pending.remove(&self.id);
        Ok(())
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = registry().lock() {
            state.active.remove(&self.id);
        }
    }
}

fn inspect_container_state(id: &str) -> Result<String, String> {
    let result = probe(
        "autodock-lifecycle-inspect",
        "docker",
        &["inspect", "--format", "{{.State.Status}}", id],
        Duration::from_secs(10),
    );

    if result.status != ToolStatus::Available || result.exit_code != Some(0) || result.truncated {
        return Err("Não foi possível verificar o estado no Docker Engine.".into());
    }

    let state = result.stdout.trim();

    if !matches!(
        state,
        "created" | "running" | "paused" | "restarting" | "removing" | "exited" | "dead"
    ) {
        return Err("Docker Engine retornou um estado não reconhecido.".into());
    }

    Ok(state.to_owned())
}

pub fn execute_container_lifecycle(request: LifecycleRequest) -> Result<LifecycleReport, String> {
    execute_with_verified(
        request,
        |args| {
            let Some(docker) = resolve("docker") else {
                return Output::missing();
            };

            memory_capture::run(
                Command::new(docker).args(args),
                Duration::from_secs(35),
                100,
            )
        },
        inspect_container_state,
    )
}

fn execute_with_verified(
    request: LifecycleRequest,
    execute: impl FnOnce(&[String]) -> Output,
    inspect: impl FnOnce(&str) -> Result<String, String>,
) -> Result<LifecycleReport, String> {
    let args = request.arguments()?;

    // O bloqueio cobre a execução e a inspeção posterior.
    let guard = OperationGuard::acquire(&request.container_id)?;

    let output = execute(&args);

    let cli_succeeded =
        output.status == ToolStatus::Available && output.exit_code == Some(0) && !output.truncated;

    // A inspeção é somente leitura e não repete a operação.
    let observed_state = if output.status != ToolStatus::Missing {
        inspect(&request.container_id).ok()
    } else {
        None
    };

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

    // A pendência é registrada antes de liberar o bloqueio ativo.
    // Falhas ou resultados não verificados exigem reconciliação.
    let reconciliation_required = output.status != ToolStatus::Missing && !state_verified;

    if reconciliation_required {
        guard.mark_pending()?;
    }

    Ok(LifecycleReport {
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
    })
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
    reconcile_with(request, inspect_container_state)
}

fn reconcile_with(
    request: ReconciliationRequest,
    inspect: impl FnOnce(&str) -> Result<String, String>,
) -> Result<ReconciliationReport, String> {
    if !request.acknowledged {
        return Err("Reconciliação exige confirmação explícita.".into());
    }

    if !(12..=64).contains(&request.container_id.len())
        || !request.container_id.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("ID Docker inválido para reconciliação.".into());
    }

    let guard = OperationGuard::acquire_reconciliation(&request.container_id)?;

    let observed_state = inspect(&request.container_id)?;

    // Estado transitório não autoriza liberação.
    if !matches!(
        observed_state.as_str(),
        "running" | "exited" | "created" | "paused" | "dead"
    ) {
        return Err("Estado transitório ou desconhecido: reconciliação mantida.".into());
    }

    guard.clear_pending()?;

    Ok(ReconciliationReport {
        schema_version: 1,
        container_id: request.container_id,
        observed_state,
        released: true,
        detail: "Estado observado e pendência liberada mediante confirmação. O resultado da operação anterior permanece indeterminado.".into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    // IDs exclusivos evitam interferência entre testes executados em paralelo.
    fn request(action: LifecycleAction, id: &str) -> LifecycleRequest {
        LifecycleRequest {
            container_id: id.into(),
            action,
            confirmed: true,
        }
    }

    fn output(status: ToolStatus, exit_code: Option<i32>) -> Output {
        let mut result = Output::missing();
        result.status = status;
        result.exit_code = exit_code;
        result
    }

    #[test]
    fn only_permitted_operations_and_fixed_arguments() {
        assert_eq!(
            request(LifecycleAction::Start, "aaaaaaaaaaaa")
                .arguments()
                .unwrap(),
            ["start", "aaaaaaaaaaaa"]
        );

        assert_eq!(
            request(LifecycleAction::Stop, "bbbbbbbbbbbb")
                .arguments()
                .unwrap(),
            ["stop", "--time", "10", "bbbbbbbbbbbb"]
        );

        assert_eq!(
            request(LifecycleAction::Restart, "cccccccccccc")
                .arguments()
                .unwrap(),
            ["restart", "--time", "10", "cccccccccccc"]
        );
    }

    #[test]
    fn rejects_invalid_ids_before_execution() {
        for id in [
            "",
            "short",
            "--help",
            "abcdef012345;rm",
            "../abcdef012345",
            "abcdef012345\n",
            "gggggggggggg",
            &"a".repeat(65),
        ] {
            let r = request(LifecycleAction::Stop, id);

            assert!(execute_with_verified(
                r,
                |_| panic!("must not execute"),
                |_| panic!("must not inspect"),
            )
            .is_err());
        }
    }

    #[test]
    fn requires_explicit_confirmation() {
        let mut r = request(LifecycleAction::Start, "dddddddddddd");
        r.confirmed = false;

        assert!(execute_with_verified(
            r,
            |_| panic!("must not execute"),
            |_| panic!("must not inspect"),
        )
        .is_err());
    }

    #[test]
    fn rejects_unknown_actions_and_fields() {
        for action in ["rm", "prune", "exec", "kill"] {
            let value = serde_json::json!({
                "containerId": "eeeeeeeeeeee",
                "action": action,
                "confirmed": true
            });

            assert!(serde_json::from_value::<LifecycleRequest>(value).is_err());
        }

        let extra = serde_json::json!({
            "containerId": "eeeeeeeeeeee",
            "action": "start",
            "confirmed": true,
            "command": "rm"
        });

        assert!(serde_json::from_value::<LifecycleRequest>(extra).is_err());
    }

    #[test]
    fn verifies_expected_state_after_success() {
        for (action, id, state) in [
            (LifecycleAction::Start, "111111111111", "running"),
            (LifecycleAction::Stop, "222222222222", "exited"),
            (LifecycleAction::Restart, "333333333333", "running"),
        ] {
            let report = execute_with_verified(
                request(action, id),
                |_| output(ToolStatus::Available, Some(0)),
                |_| Ok(state.into()),
            )
            .unwrap();

            assert!(report.outcome_confirmed);
            assert!(report.state_verified);
            assert_eq!(report.observed_state.as_deref(), Some(state));
        }
    }

    #[test]
    fn does_not_verify_unexpected_state() {
        let report = execute_with_verified(
            request(LifecycleAction::Stop, "444444444444"),
            |_| output(ToolStatus::Available, Some(0)),
            |_| Ok("running".into()),
        )
        .unwrap();

        assert!(report.outcome_confirmed);
        assert!(!report.state_verified);
        assert_eq!(report.observed_state.as_deref(), Some("running"));
    }

    #[test]
    fn timeout_remains_unconfirmed_even_if_state_matches() {
        let report = execute_with_verified(
            request(LifecycleAction::Start, "555555555555"),
            |_| output(ToolStatus::Timeout, None),
            |_| Ok("running".into()),
        )
        .unwrap();

        assert!(!report.outcome_confirmed);
        assert!(!report.state_verified);
        assert_eq!(report.observed_state.as_deref(), Some("running"));
    }

    #[test]
    fn inspection_failure_does_not_claim_verified_state() {
        let report = execute_with_verified(
            request(LifecycleAction::Restart, "666666666666"),
            |_| output(ToolStatus::Available, Some(0)),
            |_| Err("Engine unavailable".into()),
        )
        .unwrap();

        assert!(report.outcome_confirmed);
        assert!(!report.state_verified);
        assert!(report.observed_state.is_none());
    }

    #[test]
    fn concurrent_operation_on_same_id_is_rejected() {
        let id = "777777777777";
        let _guard = OperationGuard::acquire(id).unwrap();

        let result = execute_with_verified(
            request(LifecycleAction::Stop, id),
            |_| panic!("must not execute"),
            |_| panic!("must not inspect"),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("em andamento"));
    }

    #[test]
    fn guard_releases_id_after_completion() {
        let id = "888888888888";

        let first = execute_with_verified(
            request(LifecycleAction::Start, id),
            |_| output(ToolStatus::Available, Some(0)),
            |_| Ok("running".into()),
        );

        let second = execute_with_verified(
            request(LifecycleAction::Stop, id),
            |_| output(ToolStatus::Available, Some(0)),
            |_| Ok("exited".into()),
        );

        assert!(first.is_ok());
        assert!(second.is_ok());
    }

    #[test]
    fn failed_cli_result_never_verifies_state() {
        let report = execute_with_verified(
            request(LifecycleAction::Restart, "999999999999"),
            |_| output(ToolStatus::ExecutionError, Some(1)),
            |_| Ok("running".into()),
        )
        .unwrap();

        assert!(!report.outcome_confirmed);
        assert!(!report.state_verified);
    }

    #[test]
    fn missing_docker_skips_inspection() {
        let report = execute_with_verified(
            request(LifecycleAction::Start, "abababababab"),
            |_| output(ToolStatus::Missing, None),
            |_| panic!("must not inspect"),
        )
        .unwrap();

        assert!(!report.outcome_confirmed);
        assert!(!report.state_verified);
        assert!(report.observed_state.is_none());
    }

    #[test]
    fn accepts_full_hexadecimal_id() {
        let id = "abcdef0123456789".repeat(4);

        let report = execute_with_verified(
            request(LifecycleAction::Start, &id),
            |_| output(ToolStatus::Available, Some(0)),
            |_| Ok("running".into()),
        )
        .unwrap();

        assert_eq!(report.container_id, id);
        assert!(report.state_verified);
    }

    #[test]
    fn timeout_blocks_repeated_operation() {
        let id = "a1a1a1a1a1a1";

        let report = execute_with_verified(
            request(LifecycleAction::Restart, id),
            |_| output(ToolStatus::Timeout, None),
            |_| Ok("running".into()),
        )
        .unwrap();

        assert!(report.reconciliation_required);

        let retry = execute_with_verified(
            request(LifecycleAction::Restart, id),
            |_| panic!("must not repeat"),
            |_| panic!("must not inspect"),
        );

        assert!(retry.is_err());
        assert!(retry.unwrap_err().contains("reconciliação"));
    }

    #[test]
    fn inspection_failure_keeps_container_blocked() {
        let id = "b2b2b2b2b2b2";

        let report = execute_with_verified(
            request(LifecycleAction::Start, id),
            |_| output(ToolStatus::Available, Some(0)),
            |_| Err("inspect failed".into()),
        )
        .unwrap();

        assert!(report.reconciliation_required);

        let result = reconcile_with(
            ReconciliationRequest {
                container_id: id.into(),
                acknowledged: true,
            },
            |_| Err("Engine unavailable".into()),
        );

        assert!(result.is_err());

        let retry = OperationGuard::acquire(id);
        assert!(retry.is_err());
    }

    #[test]
    fn explicit_reconciliation_releases_pending_container() {
        let id = "c3c3c3c3c3c3";

        let report = execute_with_verified(
            request(LifecycleAction::Stop, id),
            |_| output(ToolStatus::Timeout, None),
            |_| Ok("exited".into()),
        )
        .unwrap();

        assert!(report.reconciliation_required);

        let result = reconcile_with(
            ReconciliationRequest {
                container_id: id.into(),
                acknowledged: true,
            },
            |_| Ok("exited".into()),
        )
        .unwrap();

        assert!(result.released);
        assert_eq!(result.observed_state, "exited");

        assert!(OperationGuard::acquire(id).is_ok());
    }

    #[test]
    fn reconciliation_requires_acknowledgement() {
        let id = "d4d4d4d4d4d4";

        let report = execute_with_verified(
            request(LifecycleAction::Start, id),
            |_| output(ToolStatus::ExecutionError, Some(1)),
            |_| Ok("exited".into()),
        )
        .unwrap();

        assert!(report.reconciliation_required);

        let result = reconcile_with(
            ReconciliationRequest {
                container_id: id.into(),
                acknowledged: false,
            },
            |_| panic!("must not inspect"),
        );

        assert!(result.is_err());
        assert!(OperationGuard::acquire(id).is_err());
    }

    #[test]
    fn transitional_state_does_not_release_pending() {
        let id = "e5e5e5e5e5e5";

        execute_with_verified(
            request(LifecycleAction::Restart, id),
            |_| output(ToolStatus::Timeout, None),
            |_| Ok("restarting".into()),
        )
        .unwrap();

        let result = reconcile_with(
            ReconciliationRequest {
                container_id: id.into(),
                acknowledged: true,
            },
            |_| Ok("restarting".into()),
        );

        assert!(result.is_err());
        assert!(OperationGuard::acquire(id).is_err());
    }

    #[test]
    fn successful_verified_operation_does_not_create_pending() {
        let id = "f6f6f6f6f6f6";

        let report = execute_with_verified(
            request(LifecycleAction::Start, id),
            |_| output(ToolStatus::Available, Some(0)),
            |_| Ok("running".into()),
        )
        .unwrap();

        assert!(report.state_verified);
        assert!(!report.reconciliation_required);
        assert!(OperationGuard::acquire(id).is_ok());
    }

    #[test]
    fn reconciliation_without_pending_is_rejected() {
        let result = reconcile_with(
            ReconciliationRequest {
                container_id: "123456789abc".into(),
                acknowledged: true,
            },
            |_| panic!("must not inspect"),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Não existe operação pendente"));
    }

    #[test]
    fn identity_comparison_is_case_insensitive() {
        assert!(identities_overlap("ABCDEF123456", "abcdef123456"));
        assert!(!identities_overlap("abcdef123456", "123456abcdef"));
    }

    #[test]
    fn short_and_full_identity_cannot_bypass_active_guard() {
        let short = "abcdef987654";
        let full = format!("{}{}", short, "1".repeat(52));

        let _guard = OperationGuard::acquire(short).unwrap();

        assert!(OperationGuard::acquire(&full).is_err());
        assert!(OperationGuard::acquire(&short.to_uppercase()).is_err());
    }

    #[test]
    fn short_and_full_identity_cannot_bypass_pending_guard() {
        let short = "1234abcdef56";
        let full = format!("{}{}", short, "2".repeat(52));

        let report = execute_with_verified(
            request(LifecycleAction::Restart, short),
            |_| output(ToolStatus::Timeout, None),
            |_| Ok("running".into()),
        )
        .unwrap();

        assert!(report.reconciliation_required);
        assert!(OperationGuard::acquire(&full).is_err());
        assert!(OperationGuard::acquire(&short.to_uppercase()).is_err());
    }

    #[test]
    fn reconciliation_accepts_case_change_but_not_alias() {
        let short = "fedcba123456";
        let full = format!("{}{}", short, "3".repeat(52));

        execute_with_verified(
            request(LifecycleAction::Start, short),
            |_| output(ToolStatus::Timeout, None),
            |_| Ok("running".into()),
        )
        .unwrap();

        let alias = reconcile_with(
            ReconciliationRequest {
                container_id: full.clone(),
                acknowledged: true,
            },
            |_| panic!("alias must not inspect"),
        );

        assert!(alias.is_err());

        let result = reconcile_with(
            ReconciliationRequest {
                container_id: short.to_uppercase(),
                acknowledged: true,
            },
            |_| Ok("running".into()),
        )
        .unwrap();

        assert!(result.released);
        assert!(OperationGuard::acquire(&full).is_ok());
    }

    #[test]
    fn unrelated_identities_remain_independent() {
        let _guard = OperationGuard::acquire("aaaaaaaa1234").unwrap();

        assert!(OperationGuard::acquire("bbbbbbbb5678").is_ok());
    }
}
