use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingOperation {
    pub schema_version: u32,
    pub operation_id: String,
    pub docker_context: String,
    pub container_id: String,
    pub action: String,
}

fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|b| b.is_ascii_hexdigit())
}

impl PendingOperation {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Versão de registro não suportada.".into());
        }

        if !valid_hex(&self.operation_id, 32) || !valid_hex(&self.container_id, 64) {
            return Err("Identificador persistente inválido.".into());
        }

        if self.docker_context.is_empty()
            || self.docker_context.len() > 128
            || !self
                .docker_context
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err("Contexto Docker inválido.".into());
        }

        if !matches!(self.action.as_str(), "start" | "stop" | "restart") {
            return Err("Operação não permitida.".into());
        }

        Ok(())
    }
}

const MAX_PENDING_RECORD_BYTES: u64 = 4096;

fn validate_journal_directory(directory: &Path) -> Result<(), String> {
    use std::path::Component;

    if directory.as_os_str().is_empty() {
        return Err("Caminho do journal vazio.".into());
    }

    let absolute = if directory.is_absolute() {
        directory.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("Diretorio atual indisponivel: {e}"))?
            .join(directory)
    };

    let mut current = PathBuf::new();

    for component in absolute.components() {
        if matches!(component, Component::ParentDir) {
            return Err("Caminho do journal contem navegacao relativa.".into());
        }

        current.push(component.as_os_str());

        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if !metadata.file_type().is_dir() {
                    return Err(format!(
                        "Componente do journal nao e diretorio: {}",
                        current.display()
                    ));
                }

                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;

                    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

                    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                        return Err(format!(
                            "Reparse point proibido no journal: {}",
                            current.display()
                        ));
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Componentes inexistentes podem ser criados posteriormente.
            }
            Err(e) => {
                return Err(format!(
                    "Falha ao validar componente do journal {}: {e}",
                    current.display()
                ));
            }
        }
    }

    Ok(())
}
pub struct LifecycleJournal {
    directory: PathBuf,
}

impl LifecycleJournal {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn record(&self, pending: &PendingOperation) -> Result<(), String> {
        pending.validate()?;

        validate_journal_directory(&self.directory)?;

        fs::create_dir_all(&self.directory)
            .map_err(|e| format!("Diretório de pendências indisponível: {e}"))?;

        validate_journal_directory(&self.directory)?;

        // Prechecagem. Exclusao mutua entre processos sera implementada
        // antes de conectar este journal ao executor Docker.
        if self.list()?.iter().any(|existing| {
            existing.docker_context == pending.docker_context
                && existing
                    .container_id
                    .eq_ignore_ascii_case(&pending.container_id)
        }) {
            return Err("Pendencia ja existente para este container e contexto.".into());
        }

        let path = self.path(&pending.operation_id);

        let data = serde_json::to_vec_pretty(pending)
            .map_err(|e| format!("Falha ao serializar pendência: {e}"))?;

        if data.len() as u64 > MAX_PENDING_RECORD_BYTES {
            return Err("Registro excede o tamanho permitido.".into());
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("Não foi possível criar pendência exclusiva: {e}"))?;

        file.write_all(&data)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("Falha ao persistir pendência: {e}"))?;

        Ok(())
    }

    pub fn list(&self) -> Result<Vec<PendingOperation>, String> {
        validate_journal_directory(&self.directory)?;

        if !self.directory.exists() {
            return Ok(Vec::new());
        }

        let mut records = Vec::new();
        let mut identities = std::collections::HashSet::new();

        for entry in fs::read_dir(&self.directory)
            .map_err(|e| format!("Não foi possível ler pendências: {e}"))?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            let metadata =
                fs::symlink_metadata(&path).map_err(|e| format!("Metadados indisponiveis: {e}"))?;

            if !metadata.file_type().is_file()
                || path.extension().and_then(|e| e.to_str()) != Some("json")
            {
                return Err("Arquivo inesperado no diretório de pendências.".into());
            }

            if metadata.len() > MAX_PENDING_RECORD_BYTES {
                return Err("Registro persistente excede o limite permitido.".into());
            }

            let bytes =
                fs::read(&path).map_err(|e| format!("Não foi possível ler registro: {e}"))?;

            let pending: PendingOperation = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Registro persistente inválido: {e}"))?;

            pending.validate()?;

            let expected = self.path(&pending.operation_id);
            if path != expected {
                return Err("Nome de registro inconsistente.".into());
            }

            if bytes.len() as u64 > MAX_PENDING_RECORD_BYTES {
                return Err("Registro persistente excede o limite permitido.".into());
            }

            let identity = (
                pending.docker_context.clone(),
                pending.container_id.to_ascii_lowercase(),
            );

            if !identities.insert(identity) {
                return Err("Pendencias duplicadas para o mesmo container e contexto.".into());
            }

            records.push(pending);
        }

        records.sort_by(|a, b| a.operation_id.cmp(&b.operation_id));

        Ok(records)
    }

    fn path(&self, operation_id: &str) -> PathBuf {
        self.directory.join(format!("{operation_id}.json"))
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PendingOperation {
        PendingOperation {
            schema_version: 1,
            operation_id: "a".repeat(32),
            docker_context: "default".into(),
            container_id: "b".repeat(64),
            action: "restart".into(),
        }
    }

    #[test]
    fn rejects_file_as_journal_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-directory");

        fs::write(&path, b"file").unwrap();

        let journal = LifecycleJournal::new(path);

        assert!(journal.list().is_err());
        assert!(journal.record(&sample()).is_err());
    }

    #[test]
    fn rejects_parent_navigation_in_journal_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir").join("..").join("pending");

        let journal = LifecycleJournal::new(path);

        assert!(journal.list().is_err());
        assert!(journal.record(&sample()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_directory_on_unix() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        let link = dir.path().join("link");

        fs::create_dir(&real).unwrap();
        symlink(&real, &link).unwrap();

        let journal = LifecycleJournal::new(link);

        assert!(journal.list().is_err());
        assert!(journal.record(&sample()).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_junction_directory_on_windows_when_supported() {
        use std::process::Command;

        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        let junction = dir.path().join("junction");

        fs::create_dir(&real).unwrap();

        let output = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                real.to_str().unwrap(),
            ])
            .output()
            .unwrap();

        if !output.status.success() {
            eprintln!("Teste de junction indisponivel neste ambiente.");
            return;
        }

        let journal = LifecycleJournal::new(junction);

        assert!(journal.list().is_err());
        assert!(journal.record(&sample()).is_err());
    }
    #[test]
    fn persists_and_loads_after_recreating_journal() {
        let dir = tempfile::tempdir().unwrap();
        let journal = LifecycleJournal::new(dir.path().join("pending"));
        let record = sample();

        journal.record(&record).unwrap();

        let reopened = LifecycleJournal::new(dir.path().join("pending"));
        assert_eq!(reopened.list().unwrap(), vec![record]);
    }

    #[test]
    fn refuses_to_overwrite_existing_record() {
        let dir = tempfile::tempdir().unwrap();
        let journal = LifecycleJournal::new(dir.path().to_path_buf());
        let record = sample();

        journal.record(&record).unwrap();
        assert!(journal.record(&record).is_err());
        assert_eq!(journal.list().unwrap(), vec![record]);
    }

    #[test]
    fn rejects_invalid_identifiers_and_actions() {
        let dir = tempfile::tempdir().unwrap();
        let journal = LifecycleJournal::new(dir.path().to_path_buf());

        let mut record = sample();
        record.container_id = "../invalid".into();
        assert!(journal.record(&record).is_err());

        let mut record = sample();
        record.action = "rm".into();
        assert!(journal.record(&record).is_err());

        let mut record = sample();
        record.docker_context = "../outside".into();
        assert!(journal.record(&record).is_err());
    }

    #[test]
    fn corrupted_records_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let journal = LifecycleJournal::new(dir.path().to_path_buf());

        fs::write(dir.path().join(format!("{}.json", "a".repeat(32))), b"{bad").unwrap();

        assert!(journal.list().is_err());
    }

    #[test]
    fn empty_directory_has_no_pending_records() {
        let dir = tempfile::tempdir().unwrap();
        let journal = LifecycleJournal::new(dir.path().join("missing"));

        assert!(journal.list().unwrap().is_empty());
    }

    #[test]
    fn refuses_duplicate_container_in_same_context() {
        let dir = tempfile::tempdir().unwrap();
        let journal = LifecycleJournal::new(dir.path().to_path_buf());

        let first = sample();
        journal.record(&first).unwrap();

        let mut second = first.clone();
        second.operation_id = "c".repeat(32);
        second.container_id = first.container_id.to_uppercase();

        assert!(journal.record(&second).is_err());
        assert_eq!(journal.list().unwrap().len(), 1);
    }

    #[test]
    fn allows_same_container_in_different_contexts() {
        let dir = tempfile::tempdir().unwrap();
        let journal = LifecycleJournal::new(dir.path().to_path_buf());

        let first = sample();
        journal.record(&first).unwrap();

        let mut second = first.clone();
        second.operation_id = "d".repeat(32);
        second.docker_context = "remote".into();

        journal.record(&second).unwrap();
        assert_eq!(journal.list().unwrap().len(), 2);
    }

    #[test]
    fn oversized_record_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let journal = LifecycleJournal::new(dir.path().to_path_buf());

        fs::write(
            dir.path().join(format!("{}.json", "e".repeat(32))),
            vec![b'x'; 4097],
        )
        .unwrap();

        assert!(journal.list().is_err());
    }

    #[test]
    fn unexpected_directory_entry_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let journal = LifecycleJournal::new(dir.path().to_path_buf());

        fs::create_dir(dir.path().join("unexpected")).unwrap();
        assert!(journal.list().is_err());
    }

    #[test]
    fn duplicate_records_on_disk_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let journal = LifecycleJournal::new(dir.path().to_path_buf());

        let first = sample();
        journal.record(&first).unwrap();

        let mut second = first.clone();
        second.operation_id = "f".repeat(32);

        fs::write(
            dir.path().join(format!("{}.json", second.operation_id)),
            serde_json::to_vec(&second).unwrap(),
        )
        .unwrap();

        assert!(journal.list().is_err());
    }
}
