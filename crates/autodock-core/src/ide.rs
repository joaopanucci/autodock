//! Detects installed editors and opens a project folder in one of them.
//!
//! Safety: the editor is chosen from a fixed allowlist by id; the UI never
//! supplies a command. Processes are spawned without a shell, with the canonical
//! directory path as a single argument, detached from the app.
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

struct Ide {
    id: &'static str,
    label: &'static str,
    /// Command line launchers found on PATH.
    commands: &'static [&'static str],
    /// macOS application bundles.
    mac_apps: &'static [&'static str],
    /// Windows: executable relative to %LOCALAPPDATA%\Programs.
    windows_exe: Option<&'static str>,
}

const IDES: &[Ide] = &[
    Ide {
        id: "vscode",
        label: "VS Code",
        commands: &["code"],
        mac_apps: &["Visual Studio Code.app"],
        windows_exe: Some("Microsoft VS Code\\Code.exe"),
    },
    Ide {
        id: "cursor",
        label: "Cursor",
        commands: &["cursor"],
        mac_apps: &["Cursor.app"],
        windows_exe: Some("cursor\\Cursor.exe"),
    },
    Ide {
        id: "windsurf",
        label: "Windsurf",
        commands: &["windsurf"],
        mac_apps: &["Windsurf.app"],
        windows_exe: Some("Windsurf\\Windsurf.exe"),
    },
    Ide {
        id: "intellij",
        label: "IntelliJ IDEA",
        commands: &["idea"],
        mac_apps: &[
            "IntelliJ IDEA.app",
            "IntelliJ IDEA CE.app",
            "IntelliJ IDEA Ultimate.app",
        ],
        windows_exe: None,
    },
    Ide {
        id: "webstorm",
        label: "WebStorm",
        commands: &["webstorm"],
        mac_apps: &["WebStorm.app"],
        windows_exe: None,
    },
    Ide {
        id: "pycharm",
        label: "PyCharm",
        commands: &["pycharm", "charm"],
        mac_apps: &[
            "PyCharm.app",
            "PyCharm CE.app",
            "PyCharm Professional Edition.app",
        ],
        windows_exe: None,
    },
    Ide {
        id: "phpstorm",
        label: "PhpStorm",
        commands: &["phpstorm"],
        mac_apps: &["PhpStorm.app"],
        windows_exe: None,
    },
    Ide {
        id: "zed",
        label: "Zed",
        commands: &["zed"],
        mac_apps: &["Zed.app"],
        windows_exe: None,
    },
    Ide {
        id: "sublime",
        label: "Sublime Text",
        commands: &["subl"],
        mac_apps: &["Sublime Text.app"],
        windows_exe: None,
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeInfo {
    pub id: String,
    pub label: String,
}

enum Launcher {
    MacApp(String),
    Program(PathBuf),
}

fn mac_app_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join("Applications"));
    }
    dirs
}

fn launcher(ide: &Ide) -> Option<Launcher> {
    if cfg!(target_os = "macos") {
        // A GUI app does not inherit the shell PATH, so prefer the app bundle.
        for app in ide.mac_apps {
            if mac_app_dirs().iter().any(|dir| dir.join(app).is_dir()) {
                return Some(Launcher::MacApp(app.trim_end_matches(".app").to_string()));
            }
        }
    }
    if cfg!(windows) {
        if let (Some(exe), Some(base)) = (ide.windows_exe, std::env::var_os("LOCALAPPDATA")) {
            let path = PathBuf::from(base).join("Programs").join(exe);
            if path.is_file() {
                return Some(Launcher::Program(path));
            }
        }
    }
    ide.commands
        .iter()
        .find_map(|command| which::which(command).ok())
        .map(Launcher::Program)
}

/// Editors found on this machine, in a stable order.
pub fn detect_ides() -> Vec<IdeInfo> {
    IDES.iter()
        .filter(|ide| launcher(ide).is_some())
        .map(|ide| IdeInfo {
            id: ide.id.into(),
            label: ide.label.into(),
        })
        .collect()
}

fn project_dir(path: &str) -> Result<PathBuf, String> {
    let canonical = Path::new(path)
        .canonicalize()
        .map_err(|e| format!("{path}: {e}"))?;
    if !canonical.is_dir() {
        return Err(tr!(
            "Não é diretório: {}",
            "Not a directory: {}",
            canonical.display()
        ));
    }
    Ok(strip_verbatim(canonical))
}

/// Editors and Explorer do not understand the `\\?\` prefix produced by canonicalize.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy().into_owned();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

fn spawn_detached(command: &mut Command) -> Result<(), String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| {
        tr!(
            "Não foi possível abrir o programa: {e}",
            "Could not open the program: {e}"
        )
    })?;
    // Reap the child without blocking the caller.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Opens the project folder in the editor with the given allowlisted id.
pub fn open_in_ide(id: &str, path: &str) -> Result<(), String> {
    let ide = IDES
        .iter()
        .find(|ide| ide.id == id)
        .ok_or_else(|| format!("Editor desconhecido: {id}"))?;
    let directory = project_dir(path)?;
    match launcher(ide).ok_or_else(|| {
        tr!(
            "{} não foi encontrado neste computador.",
            "{} was not found on this computer.",
            ide.label
        )
    })? {
        Launcher::MacApp(app) => {
            spawn_detached(Command::new("open").arg("-a").arg(app).arg(&directory))
        }
        Launcher::Program(program) => spawn_detached(Command::new(program).arg(&directory)),
    }
}

/// Shows the folder in the system file manager.
pub fn reveal_folder(path: &str) -> Result<(), String> {
    let directory = project_dir(path)?;
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    spawn_detached(Command::new(program).arg(&directory))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_detection_only_reports_allowlisted_editors() {
        let mut ids: Vec<_> = IDES.iter().map(|i| i.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), IDES.len());
        for found in detect_ides() {
            assert!(IDES.iter().any(|i| i.id == found.id));
        }
    }

    #[test]
    fn rejects_unknown_editors_and_invalid_directories_before_spawning() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        for id in ["", "code", "vscode; rm -rf /", "../vscode", "--help"] {
            assert!(open_in_ide(id, &path).is_err(), "{id:?}");
        }
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(open_in_ide("vscode", &file.to_string_lossy()).is_err());
        assert!(open_in_ide("vscode", "/definitely/not/here").is_err());
        assert!(reveal_folder(&file.to_string_lossy()).is_err());
    }

    #[test]
    fn strips_windows_verbatim_prefixes() {
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\C:\a\b")),
            PathBuf::from(r"C:\a\b")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\UNC\srv\share\a")),
            PathBuf::from(r"\\srv\share\a")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from("/tmp/x")),
            PathBuf::from("/tmp/x")
        );
    }
}
