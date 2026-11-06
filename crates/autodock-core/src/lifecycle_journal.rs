use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
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
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Versão de registro não suportada.".into());
        }

        if !valid_hex(&self.operation_id, 32) || !valid_hex(&self.container_id, 64) {
            return Err("Identificador persistente inválido.".into());
        }

        if self.docker_context.is_empty()
            || self.docker_context.starts_with('-')
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

    if crate::lifecycle_fs::has_relative_navigation(&absolute) {
        return Err("Caminho do journal contem navegacao relativa.".into());
    }

    let mut current = PathBuf::new();

    for component in absolute.components() {
        current.push(component.as_os_str());
        // A Windows drive/UNC prefix alone is not an absolute directory yet.
        if matches!(component, std::path::Component::Prefix(_)) {
            continue;
        }

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

    /// Every transaction holds the interprocess lock until dropped. This also
    /// prevents reconciliation from releasing an operation that is still running.
    pub(crate) fn transaction(&self) -> Result<JournalTransaction, String> {
        validate_journal_directory(&self.directory)?;
        let path = crate::lifecycle_lock::lock_path_for(&self.directory)?;
        let lock = crate::lifecycle_lock::LifecycleFileLock::try_acquire(&path)?;
        let directory = lock
            .parent
            .child(
                self.directory
                    .file_name()
                    .ok_or("Nome do journal ausente")?,
            )
            .map_err(|e| format!("Journal inseguro: {e}"))?;
        directory.ensure_private().map_err(|e| e.to_string())?;
        Ok(JournalTransaction {
            directory,
            _lock: lock,
        })
    }

    pub fn record(&self, pending: &PendingOperation) -> Result<(), String> {
        self.transaction()?.record(pending)
    }

    pub fn list(&self) -> Result<Vec<PendingOperation>, String> {
        self.transaction()?.list()
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

/// Anchored IO and a single lock cover validation, publication, execution and
/// retirement. Never expose an unlocked remove operation.
pub(crate) struct JournalTransaction {
    directory: crate::lifecycle_fs::Directory,
    _lock: crate::lifecycle_lock::LifecycleFileLock,
}

impl JournalTransaction {
    pub(crate) fn record(&self, pending: &PendingOperation) -> Result<(), String> {
        self.record_with(pending, |file, data| {
            file.write_all(data).and_then(|_| file.sync_all())
        })
    }

    fn record_with(
        &self,
        pending: &PendingOperation,
        write: impl FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
    ) -> Result<(), String> {
        pending.validate()?;
        if self.list()?.iter().any(|record| {
            record.docker_context == pending.docker_context
                && record
                    .container_id
                    .eq_ignore_ascii_case(&pending.container_id)
        }) {
            return Err("Pendencia ja existente para este container e contexto.".into());
        }
        let data = serde_json::to_vec_pretty(pending).map_err(|e| e.to_string())?;
        if data.len() as u64 > MAX_PENDING_RECORD_BYTES {
            return Err("Registro muito grande".into());
        }
        let temporary = format!("{}.tmp", pending.operation_id);
        let definitive = format!("{}.json", pending.operation_id);
        // Keep abandoned files on *any* error. Their presence blocks subsequent
        // operations instead of silently discarding evidence of an interrupted write.
        let mut file = self
            .directory
            .open_file(temporary.as_ref(), true, true)
            .map_err(|e| e.to_string())?;
        write(&mut file, &data).map_err(|e| format!("Falha ao persistir pendencia: {e}"))?;
        drop(file);
        // linkat/CreateHardLink provide exclusive publication on all supported
        // platforms. Unlike rename, an existing destination is never overwritten.
        self.directory
            .publish(temporary.as_ref(), definitive.as_ref())
            .map_err(|e| format!("Falha na publicacao exclusiva: {e}"))?;
        self.directory.sync().map_err(|e| e.to_string())?;
        self.directory
            .remove(temporary.as_ref())
            .map_err(|e| e.to_string())?;
        self.directory.sync().map_err(|e| e.to_string())?;
        // Confirm the published record can be read before allowing execution.
        if !self.list()?.contains(pending) {
            return Err("Publicacao nao confirmada".into());
        }
        Ok(())
    }

    pub(crate) fn list(&self) -> Result<Vec<PendingOperation>, String> {
        let mut records = Vec::new();
        let mut identities = std::collections::HashSet::new();
        for name in self.directory.entries().map_err(|e| e.to_string())? {
            let path = Path::new(&name);
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                return Err("Arquivo inesperado ou temporario abandonado no journal.".into());
            }
            let file = self
                .directory
                .open_file(&name, false, false)
                .map_err(|e| format!("Registro inseguro: {e}"))?;
            if file.metadata().map_err(|e| e.to_string())?.len() > MAX_PENDING_RECORD_BYTES {
                return Err("Registro persistente excede o limite permitido.".into());
            }
            let mut bytes = Vec::new();
            file.take(MAX_PENDING_RECORD_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_PENDING_RECORD_BYTES {
                return Err("Registro muito grande".into());
            }
            let pending: PendingOperation = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Registro persistente invalido: {e}"))?;
            pending.validate()?;
            if name != std::ffi::OsStr::new(&format!("{}.json", pending.operation_id)) {
                return Err("Nome de registro inconsistente.".into());
            }
            if !identities.insert((
                pending.docker_context.clone(),
                pending.container_id.to_ascii_lowercase(),
            )) {
                return Err("Pendencias duplicadas para o mesmo container e contexto.".into());
            }
            records.push(pending);
        }
        records.sort_by(|a, b| a.operation_id.cmp(&b.operation_id));
        Ok(records)
    }

    pub(crate) fn complete(&self, pending: &PendingOperation) -> Result<(), String> {
        if !self.list()?.contains(pending) {
            return Err("Pendencia ausente ou alterada".into());
        }
        self.directory
            .remove(format!("{}.json", pending.operation_id).as_ref())
            .map_err(|e| e.to_string())?;
        self.directory.sync().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secure_test_tempdir() -> tempfile::TempDir {
        let base = std::env::temp_dir()
            .canonicalize()
            .expect("Diretorio temporario indisponivel");

        let mut builder = tempfile::Builder::new();
        builder.prefix("autodock-journal-test-");
        // Default tempdir mode follows the umask (0775 on Ubuntu runners).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(fs::Permissions::from_mode(0o700));
        }
        builder
            .tempdir_in(base)
            .expect("Falha ao criar diretorio temporario")
    }

    /// A journal must not sit directly in a shared parent such as /tmp (the
    /// sibling lock would live there). Use a private subdirectory of the
    /// private fixture root, as the application's own storage does.
    fn journal_dir(dir: &tempfile::TempDir) -> PathBuf {
        let path = dir.path().join("pending");
        fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        path
    }

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
        let dir = secure_test_tempdir();
        let path = dir.path().join("not-a-directory");

        fs::write(&path, b"file").unwrap();

        let journal = LifecycleJournal::new(path);

        assert!(journal.list().is_err());
        assert!(journal.record(&sample()).is_err());
    }

    #[test]
    fn rejects_parent_navigation_in_journal_path() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("subdir").join("..").join("pending");

        let journal = LifecycleJournal::new(path);

        assert!(journal.list().is_err());
        assert!(journal.record(&sample()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_directory_on_unix() {
        use std::os::unix::fs::symlink;

        let dir = secure_test_tempdir();
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

        let dir = secure_test_tempdir();
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
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(dir.path().join("pending"));
        let record = sample();

        journal.record(&record).unwrap();

        let reopened = LifecycleJournal::new(dir.path().join("pending"));
        assert_eq!(reopened.list().unwrap(), vec![record]);
    }

    #[test]
    fn refuses_to_overwrite_existing_record() {
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(journal_dir(&dir));
        let record = sample();

        journal.record(&record).unwrap();
        assert!(journal.record(&record).is_err());
        assert_eq!(journal.list().unwrap(), vec![record]);
    }

    #[test]
    fn rejects_invalid_identifiers_and_actions() {
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(journal_dir(&dir));

        // Positive control: the fixture is accepted, so rejections below are
        // caused by the invalid fields and not by the journal location.
        assert!(journal.list().unwrap().is_empty());

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
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(journal_dir(&dir));

        fs::write(
            journal.directory().join(format!("{}.json", "a".repeat(32))),
            b"{bad",
        )
        .unwrap();

        assert!(journal.list().is_err());
    }

    #[test]
    fn empty_directory_has_no_pending_records() {
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(dir.path().join("missing"));

        assert!(journal.list().unwrap().is_empty());
    }

    #[test]
    fn refuses_duplicate_container_in_same_context() {
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(journal_dir(&dir));

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
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(journal_dir(&dir));

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
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(journal_dir(&dir));

        fs::write(
            journal.directory().join(format!("{}.json", "e".repeat(32))),
            vec![b'x'; 4097],
        )
        .unwrap();

        assert!(journal.list().is_err());
    }

    #[test]
    fn unexpected_directory_entry_fails_closed() {
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(journal_dir(&dir));

        fs::create_dir(journal.directory().join("unexpected")).unwrap();
        assert!(journal.list().is_err());
    }

    #[test]
    fn duplicate_records_on_disk_fail_closed() {
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(journal_dir(&dir));

        let first = sample();
        journal.record(&first).unwrap();

        let mut second = first.clone();
        second.operation_id = "f".repeat(32);

        fs::write(
            journal
                .directory()
                .join(format!("{}.json", second.operation_id)),
            serde_json::to_vec(&second).unwrap(),
        )
        .unwrap();

        assert!(journal.list().is_err());
    }
    #[test]
    fn failed_write_leaves_only_abandoned_temporary_and_blocks_reopen() {
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(dir.path().join("pending"));
        let transaction = journal.transaction().unwrap();
        assert!(transaction
            .record_with(&sample(), |file, _| {
                file.write_all(b"partial")?;
                Err(std::io::Error::other("simulated disk failure"))
            })
            .is_err());
        assert!(!journal
            .directory()
            .join(format!("{}.json", sample().operation_id))
            .exists());
        assert!(journal
            .directory()
            .join(format!("{}.tmp", sample().operation_id))
            .exists());
        drop(transaction);
        assert!(LifecycleJournal::new(journal.directory().into())
            .list()
            .is_err());
    }

    #[test]
    fn publication_failure_never_overwrites_existing_operation_id() {
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(dir.path().join("pending"));
        let first = sample();
        journal.record(&first).unwrap();
        let original = fs::read(
            journal
                .directory()
                .join(format!("{}.json", first.operation_id)),
        )
        .unwrap();
        let mut conflicting = first.clone();
        conflicting.docker_context = "remote".into();
        assert!(journal.record(&conflicting).is_err());
        assert_eq!(
            fs::read(
                journal
                    .directory()
                    .join(format!("{}.json", first.operation_id))
            )
            .unwrap(),
            original
        );
        assert!(journal.list().is_err()); // unpublished temporary is retained
    }

    #[test]
    fn interrupted_publication_with_both_names_fails_closed() {
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(dir.path().join("pending"));
        journal.record(&sample()).unwrap();
        fs::hard_link(
            journal
                .directory()
                .join(format!("{}.json", sample().operation_id)),
            journal
                .directory()
                .join(format!("{}.tmp", sample().operation_id)),
        )
        .unwrap();
        assert!(journal.list().is_err());
        assert!(journal.record(&sample()).is_err());
    }

    #[test]
    fn concurrent_threads_cannot_reserve_equivalent_identity() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("pending");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let workers: Vec<_> = (0..8)
            .map(|n| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut pending = sample();
                    pending.operation_id = format!("{n:032x}");
                    if n % 2 == 0 {
                        pending.container_id = pending.container_id.to_uppercase();
                    }
                    barrier.wait();
                    LifecycleJournal::new(path).record(&pending).is_ok()
                })
            })
            .collect();
        assert_eq!(
            workers
                .into_iter()
                .filter_map(|worker| worker.join().unwrap().then_some(()))
                .count(),
            1
        );
        assert_eq!(LifecycleJournal::new(path).list().unwrap().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_hardlink_and_fifo_records_without_following_or_blocking() {
        use std::os::unix::{ffi::OsStrExt, fs::symlink};
        for kind in ["symlink", "hardlink", "fifo"] {
            let dir = secure_test_tempdir();
            let journal = LifecycleJournal::new(dir.path().join("pending"));
            fs::create_dir(journal.directory()).unwrap();
            let target = dir.path().join("outside");
            fs::write(&target, serde_json::to_vec(&sample()).unwrap()).unwrap();
            let path = journal
                .directory()
                .join(format!("{}.json", sample().operation_id));
            match kind {
                "symlink" => symlink(&target, &path).unwrap(),
                "hardlink" => fs::hard_link(&target, &path).unwrap(),
                _ => {
                    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
                    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
                }
            }
            assert!(journal.list().is_err());
            assert_eq!(
                fs::read(&target).unwrap(),
                serde_json::to_vec(&sample()).unwrap()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn pinned_transaction_never_writes_through_replaced_directory() {
        use std::os::unix::fs::symlink;
        let dir = secure_test_tempdir();
        let journal = LifecycleJournal::new(dir.path().join("pending"));
        let transaction = journal.transaction().unwrap();
        let moved = dir.path().join("original");
        let outside = dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::rename(journal.directory(), &moved).unwrap();
        symlink(&outside, journal.directory()).unwrap();
        transaction.record(&sample()).unwrap();
        assert!(fs::read_dir(&outside).unwrap().next().is_none());
        assert!(moved
            .join(format!("{}.json", sample().operation_id))
            .is_file());
        drop(transaction);
        assert!(journal.list().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_journal_placed_directly_in_shared_parent() {
        use std::os::unix::fs::PermissionsExt;
        // Shared parents (like /tmp: not ours, group/world writable) must be
        // refused: the sibling lock file would live there.
        let dir = secure_test_tempdir();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o775)).unwrap();
        let journal = LifecycleJournal::new(dir.path().join("pending"));
        assert!(journal.record(&sample()).is_err());
        assert!(journal.list().is_err());
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        journal.record(&sample()).unwrap();
    }

    #[test]
    fn rejects_parent_navigation_with_existing_target_and_verbatim_prefix() {
        let dir = secure_test_tempdir();
        let real = journal_dir(&dir);
        // `pending/../pending` resolves to a valid, existing journal; it must
        // still be refused. canonicalize() yields a verbatim path on Windows.
        let path = real.join("..").join("pending");
        let journal = LifecycleJournal::new(path);
        assert!(journal.list().is_err());
        assert!(journal.record(&sample()).is_err());
        assert!(LifecycleJournal::new(real).list().unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_shared_writable_journal_and_parent() {
        use std::os::unix::fs::PermissionsExt;
        let dir = secure_test_tempdir();
        let path = dir.path().join("pending");
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(LifecycleJournal::new(path.clone())
            .record(&sample())
            .is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o777)).unwrap();
        assert!(LifecycleJournal::new(path).record(&sample()).is_err());
    }
}
