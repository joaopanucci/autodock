//! Descriptor-relative journal IO on Unix; pinned, non-reparse directories on Windows.
//! The storage root must belong to the application user. Locks are advisory and do
//! not defend against a malicious process running as that same user.
#[cfg(windows)]
use std::fs::{self, OpenOptions};
use std::{
    fs::File,
    io,
    path::{Component, Path, PathBuf},
};

pub(crate) struct Directory {
    path: PathBuf,
    file: File,
    #[cfg(windows)]
    _parents: Vec<File>,
}

/// True when any component navigates relatively. `Path::components` only emits
/// `ParentDir` for non-verbatim paths: after a `\\?\` prefix (which
/// `canonicalize` produces on Windows) `..` and `.` surface as `Normal`
/// components, so inspecting `Component::ParentDir` alone lets them through.
pub(crate) fn has_relative_navigation(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::ParentDir | Component::CurDir => true,
        Component::Normal(part) => part == ".." || part == ".",
        _ => false,
    })
}

fn name(name: &std::ffi::OsStr) -> io::Result<()> {
    let mut parts = Path::new(name).components();
    if !matches!(parts.next(), Some(Component::Normal(_))) || parts.next().is_some() {
        return Err(io::Error::other("Nome de arquivo inseguro"));
    }
    Ok(())
}

#[cfg(unix)]
fn c_name(value: &std::ffi::OsStr) -> io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(value.as_bytes()).map_err(|_| io::Error::other("NUL no caminho"))
}

#[cfg(windows)]
fn verify_directory_name(file: &File, requested: &std::ffi::OsStr) -> io::Result<()> {
    use std::{
        ffi::OsString,
        os::windows::{
            ffi::{OsStrExt, OsStringExt},
            io::AsRawHandle,
        },
    };
    use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
    // FILE_NAME_NORMALIZED | VOLUME_NAME_DOS resolves short names and Win32
    // trimming. Otherwise two names could select one journal but different locks.
    // SAFETY: the first call only queries the buffer length of a live handle.
    let required =
        unsafe { GetFinalPathNameByHandleW(file.as_raw_handle(), std::ptr::null_mut(), 0, 0) };
    if required == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0u16; required as usize];
    // SAFETY: the buffer has exactly the advertised number of writable WCHARs.
    let written = unsafe {
        GetFinalPathNameByHandleW(file.as_raw_handle(), buffer.as_mut_ptr(), required, 0)
    };
    if written == 0 {
        return Err(io::Error::last_os_error());
    }
    if written >= required {
        return Err(io::Error::other(
            "Nome de diretorio mudou durante validacao",
        ));
    }
    let final_path = PathBuf::from(OsString::from_wide(&buffer[..written as usize]));
    let actual = final_path
        .file_name()
        .ok_or_else(|| io::Error::other("Diretorio sem nome final"))?;
    // ASCII case aliases also alias the sibling .lock. Other name changes are
    // rejected conservatively, without lossy conversion of Windows UTF-16 names.
    let fold = |unit: u16| {
        if (b'A' as u16..=b'Z' as u16).contains(&unit) {
            unit + 32
        } else {
            unit
        }
    };
    if !actual
        .encode_wide()
        .map(fold)
        .eq(requested.encode_wide().map(fold))
    {
        return Err(io::Error::other("Alias de diretorio proibido no journal"));
    }
    Ok(())
}

#[cfg(all(test, unix))]
std::thread_local! {
    static SYNC_FAILURE: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(all(test, unix))]
pub(crate) struct SyncFailureReset;
#[cfg(all(test, unix))]
impl Drop for SyncFailureReset {
    fn drop(&mut self) {
        SYNC_FAILURE.with(|failure| {
            failure.borrow_mut().take();
        });
    }
}
#[cfg(all(test, unix))]
pub(crate) fn fail_next_sync(path: &Path) -> SyncFailureReset {
    SYNC_FAILURE.with(|failure| {
        assert!(failure.borrow_mut().replace(path.to_path_buf()).is_none());
    });
    SyncFailureReset
}

impl Directory {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        if path.as_os_str().is_empty() || has_relative_navigation(&absolute) {
            return Err(io::Error::other("Caminho de diretorio inseguro"));
        }
        #[cfg(unix)]
        {
            let mut current = Self {
                path: PathBuf::from("/"),
                file: File::open("/")?,
            };
            for component in absolute.components() {
                if let Component::Normal(part) = component {
                    current = current.child(part)?;
                }
            }
            Ok(current)
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
            let mut current = PathBuf::new();
            let mut handles = Vec::new();
            for component in absolute.components() {
                current.push(component.as_os_str());
                if matches!(component, Component::Prefix(_)) {
                    continue;
                }
                // A drive/UNC root already exists and must never be created.
                if matches!(component, Component::Normal(_)) {
                    match fs::create_dir(&current) {
                        Ok(()) => {}
                        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                        Err(e) => return Err(e),
                    }
                }
                let file = OpenOptions::new()
                    .read(true)
                    .share_mode(1 | 2) // Deny FILE_SHARE_DELETE: pin each ancestor.
                    .custom_flags(0x0200_0000 | 0x0020_0000)
                    .open(&current)?;
                let metadata = file.metadata()?;
                if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
                    return Err(io::Error::other("Diretorio ou reparse point inseguro"));
                }
                if let Component::Normal(part) = component {
                    verify_directory_name(&file, part)?;
                }
                handles.push(file);
            }
            let file = handles
                .pop()
                .ok_or_else(|| io::Error::other("Diretorio ausente"))?;
            Ok(Self {
                path: absolute,
                file,
                _parents: handles,
            })
        }
    }

    pub(crate) fn child(&self, child: &std::ffi::OsStr) -> io::Result<Self> {
        name(child)?;
        #[cfg(unix)]
        {
            use std::os::{
                fd::{AsRawFd, FromRawFd},
                unix::fs::MetadataExt,
            };
            let metadata = self.file.metadata()?;
            // An untrusted writable ancestor could rename the application's
            // private directory. Sticky directories such as /tmp protect owned
            // entries and are allowed; application storage itself must be private.
            let owner = metadata.uid();
            if (owner != 0 && owner != unsafe { libc::geteuid() })
                || (metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0)
            {
                return Err(io::Error::other(
                    "Ancestral do journal permite substituicao por terceiros",
                ));
            }
            let part = c_name(child)?;
            // mkdirat never follows the final component. openat rejects links and
            // pins the directory even if its pathname is subsequently replaced.
            let made = unsafe { libc::mkdirat(self.file.as_raw_fd(), part.as_ptr(), 0o700) };
            if made != 0 {
                let e = io::Error::last_os_error();
                if e.kind() != io::ErrorKind::AlreadyExists {
                    return Err(e);
                }
            }
            let fd = unsafe {
                libc::openat(
                    self.file.as_raw_fd(),
                    part.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // Own the descriptor before the fallible sync so errors cannot leak it.
            let file = unsafe { File::from_raw_fd(fd) };
            // An existing directory may have been created by a process interrupted
            // before syncing its parent. Confirm that entry on *every* acquisition.
            self.sync()?;
            Ok(Self {
                path: self.path.join(child),
                file,
            })
        }
        #[cfg(windows)]
        {
            Self::open(&self.path.join(child))
        }
    }

    pub(crate) fn ensure_private(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = self.file.metadata()?;
            if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o022 != 0 {
                return Err(io::Error::other(
                    "Journal exige diretorio privado do usuario",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn open_file(
        &self,
        child: &std::ffi::OsStr,
        create: bool,
        exclusive: bool,
    ) -> io::Result<File> {
        name(child)?;
        #[cfg(unix)]
        let file = {
            use std::os::fd::{AsRawFd, FromRawFd};
            let child = c_name(child)?;
            let mut flags = libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK;
            flags |= if create {
                libc::O_RDWR | libc::O_CREAT
            } else {
                libc::O_RDONLY
            };
            if exclusive {
                flags |= libc::O_EXCL;
            }
            let fd = unsafe { libc::openat(self.file.as_raw_fd(), child.as_ptr(), flags, 0o600) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            unsafe { File::from_raw_fd(fd) }
        };
        #[cfg(windows)]
        let file = {
            use std::os::windows::fs::OpenOptionsExt;
            OpenOptions::new()
                .read(true)
                .write(create)
                .create(create && !exclusive)
                .create_new(exclusive)
                .share_mode(1 | 2)
                .custom_flags(0x0020_0000)
                .open(self.path.join(child))?
        };
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::other("Registro nao regular"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o022 != 0 {
                return Err(io::Error::other(
                    "Arquivo com proprietario ou permissoes inseguros",
                ));
            }
            if metadata.nlink() != 1 {
                return Err(io::Error::other("Hardlink inesperado"));
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(io::Error::other("Reparse point proibido"));
            }
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                FileStandardInfo, GetFileInformationByHandleEx, FILE_STANDARD_INFO,
            };
            let mut info = std::mem::MaybeUninit::<FILE_STANDARD_INFO>::zeroed();
            // SAFETY: a valid file handle and correctly sized output buffer.
            if unsafe {
                GetFileInformationByHandleEx(
                    file.as_raw_handle(),
                    FileStandardInfo,
                    info.as_mut_ptr().cast(),
                    std::mem::size_of::<FILE_STANDARD_INFO>() as u32,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if unsafe { info.assume_init() }.NumberOfLinks != 1 {
                return Err(io::Error::other("Hardlink inesperado"));
            }
        }
        Ok(file)
    }

    pub(crate) fn entries(&self) -> io::Result<Vec<std::ffi::OsString>> {
        #[cfg(unix)]
        {
            use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
            // Open a fresh description: dup would share the enumeration offset.
            let fd = unsafe {
                libc::openat(
                    self.file.as_raw_fd(),
                    c".".as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let stream = unsafe { libc::fdopendir(fd) };
            if stream.is_null() {
                unsafe {
                    libc::close(fd);
                }
                return Err(io::Error::last_os_error());
            }
            struct Stream(*mut libc::DIR);
            impl Drop for Stream {
                fn drop(&mut self) {
                    unsafe {
                        libc::closedir(self.0);
                    }
                }
            }
            let stream = Stream(stream);
            let mut entries = Vec::new();
            loop {
                // readdir uses errno to distinguish EOF from an error.
                #[cfg(target_os = "linux")]
                unsafe {
                    *libc::__errno_location() = 0;
                }
                #[cfg(target_os = "macos")]
                unsafe {
                    *libc::__error() = 0;
                }
                let entry = unsafe { libc::readdir(stream.0) };
                if entry.is_null() {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error().unwrap_or(0) != 0 {
                        return Err(error);
                    }
                    break;
                }
                let bytes =
                    unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
                if bytes != b"." && bytes != b".." {
                    entries.push(std::ffi::OsStr::from_bytes(bytes).to_owned());
                }
            }
            Ok(entries)
        }
        #[cfg(windows)]
        {
            fs::read_dir(&self.path)?
                .map(|entry| entry.map(|e| e.file_name()))
                .collect()
        }
    }

    pub(crate) fn publish(
        &self,
        source: &std::ffi::OsStr,
        target: &std::ffi::OsStr,
    ) -> io::Result<()> {
        name(source)?;
        name(target)?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let source = c_name(source)?;
            let target = c_name(target)?;
            if unsafe {
                libc::linkat(
                    self.file.as_raw_fd(),
                    source.as_ptr(),
                    self.file.as_raw_fd(),
                    target.as_ptr(),
                    0,
                )
            } != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            fs::hard_link(self.path.join(source), self.path.join(target))
        }
    }

    pub(crate) fn remove(&self, child: &std::ffi::OsStr) -> io::Result<()> {
        name(child)?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let child = c_name(child)?;
            if unsafe { libc::unlinkat(self.file.as_raw_fd(), child.as_ptr(), 0) } != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            fs::remove_file(self.path.join(child))
        }
    }

    pub(crate) fn sync(&self) -> io::Result<()> {
        #[cfg(all(test, unix))]
        if SYNC_FAILURE.with(|failure| {
            let mut failure = failure.borrow_mut();
            if failure.as_deref() == Some(self.path.as_path()) {
                failure.take();
                true
            } else {
                false
            }
        }) {
            return Err(io::Error::other("simulated directory sync failure"));
        }
        #[cfg(unix)]
        {
            self.file.sync_all()
        }
        // Windows has no portable directory FlushFileBuffers guarantee. Files
        // are flushed before publication; power-loss durability is filesystem dependent.
        #[cfg(windows)]
        {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap()
    }
    #[test]
    fn exclusive_publication_preserves_existing_destination() {
        let dir = tempdir();
        let directory = Directory::open(dir.path()).unwrap();
        for (name, data) in [("source", b"new"), ("target", b"old")] {
            let mut file = directory.open_file(name.as_ref(), true, true).unwrap();
            file.write_all(data).unwrap();
            file.sync_all().unwrap();
        }
        assert!(directory
            .publish("source".as_ref(), "target".as_ref())
            .is_err());
        assert_eq!(std::fs::read(dir.path().join("target")).unwrap(), b"old");
    }
    #[test]
    fn detects_relative_navigation_in_every_component_form() {
        assert!(has_relative_navigation(Path::new("a/../b")));
        assert!(has_relative_navigation(Path::new("../b")));
        assert!(!has_relative_navigation(Path::new("a/b/c")));
        assert!(!has_relative_navigation(Path::new("a/..b/c.")));
        // Verbatim paths do not normalize `..`; it must still be rejected.
        #[cfg(windows)]
        {
            assert!(has_relative_navigation(Path::new(r"\\?\C:\a\..\b")));
            assert!(has_relative_navigation(Path::new(r"\\?\C:\a\.\b")));
            assert!(!has_relative_navigation(Path::new(r"\\?\C:\a\..b\c")));
            let dir = tempdir();
            assert!(Directory::open(&dir.path().join("sub").join("..").join("p")).is_err());
        }
    }
    #[test]
    fn repeated_enumeration_has_independent_offsets() {
        let dir = tempdir();
        let directory = Directory::open(dir.path()).unwrap();
        drop(directory.open_file("record".as_ref(), true, true).unwrap());
        assert_eq!(directory.entries().unwrap(), directory.entries().unwrap());
        assert_eq!(directory.entries().unwrap().len(), 1);
    }
    #[test]
    fn rejects_escape_names_in_every_io_operation() {
        let dir = tempdir();
        let directory = Directory::open(dir.path()).unwrap();
        for value in ["../escape", "sub/file", "..", ".", ""] {
            assert!(directory.open_file(value.as_ref(), true, true).is_err());
            assert!(directory.remove(value.as_ref()).is_err());
            assert!(directory
                .publish("source".as_ref(), value.as_ref())
                .is_err());
            assert!(directory.child(value.as_ref()).is_err());
        }
    }
    #[test]
    fn publication_is_readable_only_after_temporary_name_is_retired() {
        let dir = tempdir();
        let directory = Directory::open(dir.path()).unwrap();
        let mut file = directory
            .open_file("temporary".as_ref(), true, true)
            .unwrap();
        file.write_all(b"complete").unwrap();
        file.sync_all().unwrap();
        drop(file);
        directory
            .publish("temporary".as_ref(), "definitive".as_ref())
            .unwrap();
        assert!(directory
            .open_file("definitive".as_ref(), false, false)
            .is_err());
        directory.remove("temporary".as_ref()).unwrap();
        directory.sync().unwrap();
        let mut bytes = String::new();
        directory
            .open_file("definitive".as_ref(), false, false)
            .unwrap()
            .read_to_string(&mut bytes)
            .unwrap();
        assert_eq!(bytes, "complete");
    }
    #[cfg(unix)]
    #[test]
    fn existing_directory_entry_must_be_synced_after_interrupted_creation() {
        let dir = tempdir();
        let directory = Directory::open(dir.path()).unwrap();
        // First attempt creates the entry but cannot confirm parent durability.
        let failure = fail_next_sync(dir.path());
        assert!(directory.child("pending".as_ref()).is_err());
        assert!(dir.path().join("pending").is_dir());
        drop(failure);
        // Retrying must not bypass sync just because mkdir now reports EEXIST.
        let failure = fail_next_sync(dir.path());
        assert!(directory.child("pending".as_ref()).is_err());
        drop(failure);
        assert!(directory.child("pending".as_ref()).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_win32_trimmed_directory_aliases_but_accepts_canonical_name() {
        use std::path::Prefix;
        let dir = tempdir();
        let actual = dir.path().join("pending");
        fs::create_dir(&actual).unwrap();
        let opened = Directory::open(&actual).unwrap();
        // Validate the handle comparison even on systems where Rust's path
        // handling rejects a trimmed alias before it reaches CreateFileW.
        assert!(verify_directory_name(&opened.file, "pending".as_ref()).is_ok());
        assert!(verify_directory_name(&opened.file, "PENDING".as_ref()).is_ok());
        for alias in ["pending.", "pending "] {
            assert!(verify_directory_name(&opened.file, alias.as_ref()).is_err());
        }
        // tempdir's canonical path is verbatim. Use a DOS path to exercise the
        // trailing-dot/space normalization that used to select a second lock.
        let mut components = dir.path().components();
        let drive = match components.next().unwrap() {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::VerbatimDisk(drive) | Prefix::Disk(drive) => drive,
                _ => panic!("Test requires a local drive"),
            },
            _ => panic!("Test requires an absolute Windows path"),
        };
        let mut dos = PathBuf::from(format!("{}:\\", drive as char));
        for component in components {
            if let Component::Normal(part) = component {
                dos.push(part);
            }
        }
        for alias in ["pending.", "pending "] {
            let path = dos.join(alias);
            if path.is_dir() {
                // proves the OS resolves this alias to the journal
                assert!(Directory::open(&path).is_err());
            }
        }
        assert!(Directory::open(&actual).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_short_name_of_existing_directory_when_filesystem_exposes_one() {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
        let dir = tempdir();
        let path = dir.path().join("journal-with-a-long-name");
        fs::create_dir(&path).unwrap();
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut short = vec![0u16; 32768];
        // SAFETY: both UTF-16 buffers are valid and the input is NUL terminated.
        let length =
            unsafe { GetShortPathNameW(wide.as_ptr(), short.as_mut_ptr(), short.len() as u32) };
        assert!(length > 0 && (length as usize) < short.len());
        let alias = PathBuf::from(std::ffi::OsString::from_wide(&short[..length as usize]));
        if alias.file_name() == path.file_name() {
            eprintln!(
                "8.3 names are disabled on this filesystem; normalized-name validation still runs"
            );
        } else {
            assert!(Directory::open(&alias).is_err());
        }
        assert!(Directory::open(&path).is_ok());
    }
}
