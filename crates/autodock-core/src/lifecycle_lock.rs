use fs4::TryLockError;
#[cfg(unix)]
use std::fs;
#[cfg(windows)]
use std::fs::OpenOptions;
use std::{
    fs::File,
    path::{Path, PathBuf},
};

#[cfg(windows)]
fn windows_file_identity(file: &File) -> Result<(u64, [u8; 16]), String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileIdInfo, GetFileInformationByHandleEx, FILE_ID_INFO,
    };

    let mut info = std::mem::MaybeUninit::<FILE_ID_INFO>::zeroed();

    let result = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            info.as_mut_ptr().cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    };

    if result == 0 {
        return Err(format!(
            "Falha ao consultar identidade do arquivo: {}",
            std::io::Error::last_os_error()
        ));
    }

    let info = unsafe { info.assume_init() };

    Ok((info.VolumeSerialNumber, info.FileId.Identifier))
}

#[cfg(windows)]
fn verify_lock_identity(file: &File, path: &Path) -> Result<(), String> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

    let mut options = OpenOptions::new();
    options.read(true);
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);

    let current = options
        .open(path)
        .map_err(|e| format!("Caminho de bloqueio indisponivel: {e}"))?;

    let metadata = current
        .metadata()
        .map_err(|e| format!("Metadados do caminho indisponiveis: {e}"))?;

    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err("Caminho de bloqueio nao corresponde a arquivo regular.".into());
    }

    let opened_id = windows_file_identity(file)?;
    let current_id = windows_file_identity(&current)?;

    if opened_id != current_id {
        return Err("Arquivo de bloqueio foi substituido durante a aquisicao.".into());
    }

    Ok(())
}

#[cfg(unix)]
fn verify_lock_identity(file: &File, path: &Path) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;

    let opened = file
        .metadata()
        .map_err(|e| format!("Metadados do handle indisponiveis: {e}"))?;

    let current =
        fs::symlink_metadata(path).map_err(|e| format!("Caminho de bloqueio indisponivel: {e}"))?;

    if !current.file_type().is_file() {
        return Err("Caminho de bloqueio deixou de ser arquivo regular.".into());
    }

    if opened.dev() != current.dev() || opened.ino() != current.ino() {
        return Err("Arquivo de bloqueio foi substituido durante a aquisicao.".into());
    }

    Ok(())
}
pub struct LifecycleFileLock {
    _file: File,
    pub(crate) parent: crate::lifecycle_fs::Directory,
}

impl LifecycleFileLock {
    pub fn try_acquire(path: &Path) -> Result<Self, String> {
        let parent = path
            .parent()
            .ok_or_else(|| "Caminho de bloqueio sem diretorio pai.".to_string())?;

        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        let parent = crate::lifecycle_fs::Directory::open(parent)
            .map_err(|e| format!("Diretorio de bloqueio inseguro: {e}"))?;
        parent.ensure_private().map_err(|e| e.to_string())?;
        let name = path.file_name().ok_or("Nome de bloqueio ausente")?;
        let file = parent
            .open_file(name, true, false)
            .map_err(|e| format!("Bloqueio inseguro: {e}"))?;

        fs4::FileExt::try_lock(&file).map_err(|e| match e {
            TryLockError::WouldBlock => "Journal ocupado por outro processo.".to_string(),
            other => format!("Falha ao adquirir bloqueio exclusivo: {other}"),
        })?;

        verify_lock_identity(&file, path)?;

        Ok(Self {
            _file: file,
            parent,
        })
    }
}

impl Drop for LifecycleFileLock {
    fn drop(&mut self) {
        // Explicit unlock also releases transient fork-inherited descriptions
        // before another thread's subprocess reaches exec/close-on-exec.
        let _ = fs4::FileExt::unlock(&self._file);
    }
}

pub fn lock_path_for(journal_directory: &Path) -> Result<PathBuf, String> {
    let parent = journal_directory
        .parent()
        .ok_or_else(|| "Journal sem diretorio pai.".to_string())?;

    let name = journal_directory
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Nome do journal invalido.".to_string())?;

    if name.is_empty() || name == "." || name == ".." {
        return Err("Nome do journal invalido.".into());
    }

    Ok(parent.join(format!("{name}.lock")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn secure_test_tempdir() -> tempfile::TempDir {
        tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap()
    }

    #[test]
    fn exclusive_lock_blocks_second_acquisition() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("pending.lock");

        let first = LifecycleFileLock::try_acquire(&path).unwrap();

        assert!(LifecycleFileLock::try_acquire(&path).is_err());

        drop(first);
    }

    #[test]
    fn lock_can_be_reacquired_after_release() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("pending.lock");

        {
            let _first = LifecycleFileLock::try_acquire(&path).unwrap();
        }

        assert!(LifecycleFileLock::try_acquire(&path).is_ok());
    }

    #[test]
    fn rejects_directory_as_lock_file() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("pending.lock");

        fs::create_dir(&path).unwrap();

        assert!(LifecycleFileLock::try_acquire(&path).is_err());
        assert!(path.is_dir());
    }

    #[test]
    fn lock_file_persists_after_guard_drop() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("pending.lock");

        {
            let _guard = LifecycleFileLock::try_acquire(&path).unwrap();
            assert!(path.is_file());
        }

        assert!(path.is_file());

        let _second = LifecycleFileLock::try_acquire(&path).unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn rejects_missing_parent_filename() {
        let path = Path::new("/");

        assert!(lock_path_for(path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_as_lock_file_unix() {
        use std::os::unix::fs::symlink;

        let dir = secure_test_tempdir();
        let target = dir.path().join("target");
        let link = dir.path().join("pending.lock");

        fs::write(&target, b"original").unwrap();
        symlink(&target, &link).unwrap();

        assert!(LifecycleFileLock::try_acquire(&link).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
    }

    #[cfg(windows)]
    #[test]
    fn rejects_symlink_as_lock_file_windows_when_supported() {
        use std::os::windows::fs::symlink_file;

        let dir = secure_test_tempdir();
        let target = dir.path().join("target");
        let link = dir.path().join("pending.lock");

        fs::write(&target, b"original").unwrap();

        if symlink_file(&target, &link).is_err() {
            eprintln!("Teste de symlink nao executado: permissao indisponivel.");
            return;
        }

        assert!(LifecycleFileLock::try_acquire(&link).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
    }
    #[cfg(windows)]
    #[test]
    fn same_file_handles_have_same_identity() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("pending.lock");

        fs::write(&path, b"original").unwrap();

        let first = OpenOptions::new().read(true).open(&path).unwrap();
        let second = OpenOptions::new().read(true).open(&path).unwrap();

        assert_eq!(
            windows_file_identity(&first).unwrap(),
            windows_file_identity(&second).unwrap()
        );
    }

    #[cfg(windows)]
    #[test]
    fn hardlink_preserves_identity_but_distinct_file_does_not() {
        let dir = secure_test_tempdir();
        let original = dir.path().join("original.lock");
        let hardlink = dir.path().join("alias.lock");
        let distinct = dir.path().join("distinct.lock");

        fs::write(&original, b"original").unwrap();
        fs::write(&distinct, b"distinct").unwrap();
        fs::hard_link(&original, &hardlink).unwrap();

        let original_handle = File::open(&original).unwrap();
        let alias_handle = File::open(&hardlink).unwrap();
        let distinct_handle = File::open(&distinct).unwrap();

        assert_eq!(
            windows_file_identity(&original_handle).unwrap(),
            windows_file_identity(&alias_handle).unwrap()
        );

        assert_ne!(
            windows_file_identity(&original_handle).unwrap(),
            windows_file_identity(&distinct_handle).unwrap()
        );
    }

    #[cfg(windows)]
    #[test]
    fn detects_replaced_lock_path() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("pending.lock");
        let old_path = dir.path().join("old.lock");

        fs::write(&path, b"original").unwrap();

        let original_handle = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();

        fs::rename(&path, &old_path).unwrap();
        fs::write(&path, b"replacement").unwrap();

        let result = verify_lock_identity(&original_handle, &path);

        assert!(
            result.is_err(),
            "Substituicao do caminho deveria ser rejeitada"
        );

        assert_eq!(fs::read(&old_path).unwrap(), b"original");
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
    }

    #[cfg(windows)]
    #[test]
    fn active_lock_replacement_is_rejected_or_blocked_by_windows() {
        let dir = secure_test_tempdir();
        let path = dir.path().join("pending.lock");
        let moved = dir.path().join("moved.lock");

        let guard = LifecycleFileLock::try_acquire(&path).unwrap();

        match fs::rename(&path, &moved) {
            Ok(()) => {
                fs::write(&path, b"replacement").unwrap();

                assert!(
                    verify_lock_identity(&guard._file, &path).is_err(),
                    "Lock ativo nao detectou substituicao"
                );
            }
            Err(error) => {
                eprintln!("Windows impediu renomeacao do lock ativo: {error}");

                assert!(path.is_file());
                assert!(verify_lock_identity(&guard._file, &path).is_ok());
            }
        }

        drop(guard);
    }
    #[test]
    fn lock_file_is_outside_pending_directory() {
        let dir = secure_test_tempdir();
        let pending = dir.path().join("pending");
        let lock_path = lock_path_for(&pending).unwrap();

        assert_eq!(lock_path, dir.path().join("pending.lock"));
        assert_ne!(lock_path.parent().unwrap(), pending.as_path());
    }
}
