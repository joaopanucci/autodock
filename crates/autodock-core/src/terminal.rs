//! The integrated terminal of the code editor.
//!
//! The shell does **not** run on the computer: it runs in a throwaway Docker container that sees
//! the project folder, like everything else AutoDock executes. This module decides which
//! container is opened and with which arguments (only images from a short list, only a folder
//! that passed the usual checks), and connects it to the interface through a pseudo terminal,
//! which `docker run -it` needs.
use crate::{doctor::resolve, generate};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

/// Bytes the interface may send to the terminal in one call (a paste, say).
pub const MAX_WRITE_BYTES: usize = 64 * 1024;
const MIN_COLS: u16 = 10;
const MAX_COLS: u16 = 500;
const MIN_ROWS: u16 = 3;
const MAX_ROWS: u16 = 200;

struct Preset {
    id: &'static str,
    label: &'static str,
    image: &'static str,
    /// Shares the Maven download cache of the Java upgrade (volume `autodock-maven-cache`); only
    /// where the shell runs as root (Windows), because on Unix it runs as the user.
    #[cfg_attr(unix, allow(dead_code))]
    maven: bool,
}

/// The images a terminal can open. Nothing else is ever started, whatever the interface asks.
const PRESETS: [Preset; 4] = [
    Preset {
        id: "maven-jdk25",
        label: "Maven + JDK 25 (Java)",
        image: "maven:3.9-eclipse-temurin-25",
        maven: true,
    },
    Preset {
        id: "node22",
        label: "Node.js 22",
        image: "node:22-bookworm-slim",
        maven: false,
    },
    Preset {
        id: "python313",
        label: "Python 3.13",
        image: "python:3.13-slim",
        maven: false,
    },
    Preset {
        id: "debian",
        label: "Debian (generic shell)",
        image: "debian:bookworm-slim",
        maven: false,
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub id: String,
    pub label: String,
    /// The Docker image, for display.
    pub image: String,
}

pub fn images() -> Vec<ImageInfo> {
    PRESETS
        .iter()
        .map(|p| ImageInfo {
            id: p.id.into(),
            label: p.label.into(),
            image: p.image.into(),
        })
        .collect()
}

/// The image that fits a project: Maven for a `pom.xml`, Node for a `package.json`, Python for
/// its usual manifests, otherwise a plain Debian.
pub fn default_image(root: &str) -> Result<String, String> {
    let root = generate::root_of(root)?;
    let has = |name: &str| generate::regular_file(&root, name);
    let id = if has("pom.xml") {
        "maven-jdk25"
    } else if has("package.json") {
        "node22"
    } else if has("requirements.txt") || has("pyproject.toml") {
        "python313"
    } else {
        "debian"
    };
    Ok(id.into())
}

#[derive(Debug, Clone)]
pub struct TerminalPlan {
    /// The preset id.
    pub image_id: String,
    pub container: String,
    /// Arguments of `docker`.
    pub args: Vec<String>,
}

/// The `docker run` of a terminal for a project folder. `session` keeps the container names of
/// several terminals apart.
pub fn plan(root: &str, image_id: &str, session: u32) -> Result<TerminalPlan, String> {
    let preset = PRESETS.iter().find(|p| p.id == image_id).ok_or_else(|| {
        tr!(
            "Imagem de terminal desconhecida: {image_id}.",
            "Unknown terminal image: {image_id}."
        )
    })?;
    let root = generate::root_of(root)?;
    // The folder goes into a comma separated `--mount` value.
    generate::check_mount_path(&root)?;
    let container = format!("autodock-terminal-{}-{session}", std::process::id());
    let mut args: Vec<String> = ["run", "-it", "--rm", "--name"].map(String::from).to_vec();
    args.push(container.clone());
    args.extend(["--security-opt", "no-new-privileges"].map(String::from));
    args.push("--mount".into());
    args.push(format!(
        "type=bind,source={},target=/work",
        generate::plain_path(&root)
    ));
    #[cfg(unix)]
    {
        // Files made in the terminal belong to the user, not to root.
        // SAFETY: geteuid/getegid have no preconditions.
        let (uid, gid) = unsafe { (libc::geteuid(), libc::getegid()) };
        args.extend(["--user".into(), format!("{uid}:{gid}")]);
        args.extend(["-e", "HOME=/tmp"].map(String::from));
    }
    #[cfg(not(unix))]
    if preset.maven {
        // As root the Maven cache of the Java upgrade can be shared; a user without a home cannot.
        args.push("--mount".into());
        args.push("type=volume,source=autodock-maven-cache,target=/root/.m2/repository".into());
    }
    args.extend(
        [
            "-w",
            "/work",
            "-e",
            "TERM=xterm-256color",
            "-e",
            "COLORTERM=truecolor",
        ]
        .map(String::from),
    );
    args.push(preset.image.into());
    args.push("bash".into());
    Ok(TerminalPlan {
        image_id: preset.id.into(),
        container,
        args,
    })
}

fn clamp_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.clamp(MIN_ROWS, MAX_ROWS),
        cols: cols.clamp(MIN_COLS, MAX_COLS),
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// A running terminal: a pseudo terminal with a command on the other end.
pub struct Session {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Dropped on stop: on Windows that is what ends the pseudo console and lets the reader finish.
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    /// The container to remove when the terminal is stopped (a killed `docker run` can leave it).
    container: Option<String>,
    docker: Option<PathBuf>,
}

/// How a terminal is launched, apart from the command itself.
struct Launch {
    cols: u16,
    rows: u16,
    /// The container to remove when the terminal is stopped.
    container: Option<String>,
    docker: Option<PathBuf>,
    /// The Windows pseudo console asks where the cursor is (ESC [ 6 n) and waits for the terminal
    /// emulator to answer; the interface has one (xterm.js), the tests do not.
    answer_cursor_queries: bool,
}

impl Session {
    /// Opens the terminal described by `plan`. `on_data` gets everything the terminal prints;
    /// `on_exit` is called once, with the exit code, when the shell ends.
    pub fn start(
        plan: &TerminalPlan,
        cols: u16,
        rows: u16,
        on_data: impl Fn(Vec<u8>) + Send + 'static,
        on_exit: impl FnOnce(Option<u32>) + Send + 'static,
    ) -> Result<Session, String> {
        let docker = resolve("docker").ok_or_else(|| {
            tr!(
                "Docker não foi encontrado no PATH. Instale e abra o Docker para usar o terminal.",
                "Docker was not found in PATH. Install and open Docker to use the terminal."
            )
        })?;
        let mut command = CommandBuilder::new(&docker);
        command.args(&plan.args);
        command.env("TERM", "xterm-256color");
        Self::spawn(
            command,
            Launch {
                cols,
                rows,
                container: Some(plan.container.clone()),
                docker: Some(docker),
                answer_cursor_queries: false,
            },
            on_data,
            on_exit,
        )
    }

    fn spawn(
        command: CommandBuilder,
        launch: Launch,
        on_data: impl Fn(Vec<u8>) + Send + 'static,
        on_exit: impl FnOnce(Option<u32>) + Send + 'static,
    ) -> Result<Session, String> {
        let Launch {
            cols,
            rows,
            container,
            docker,
            answer_cursor_queries,
        } = launch;
        let pair = native_pty_system()
            .openpty(clamp_size(cols, rows))
            .map_err(|e| e.to_string())?;
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|e| e.to_string())?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = Arc::new(Mutex::new(
            pair.master.take_writer().map_err(|e| e.to_string())?,
        ));
        let killer = child.clone_killer();

        let answering = Arc::clone(&writer);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                let chunk = &buffer[..read];
                if answer_cursor_queries && chunk.windows(4).any(|w| w == b"[6n") {
                    let mut writer = answering.lock().unwrap_or_else(|e| e.into_inner());
                    let _ = writer.write_all(b"[1;1R").and_then(|_| writer.flush());
                }
                on_data(chunk.to_vec());
            }
        });
        // The shell ending is what the interface hears about, so it comes from the child itself
        // (on Windows the reader only finishes once the pseudo console is closed).
        std::thread::spawn(move || {
            let code = child.wait().ok().map(|status| status.exit_code());
            // Let the reader hand over what was still being printed.
            std::thread::sleep(Duration::from_millis(150));
            on_exit(code);
        });
        Ok(Session {
            writer,
            master: Mutex::new(Some(pair.master)),
            killer: Mutex::new(killer),
            container,
            docker,
        })
    }

    /// Sends what was typed (or pasted) to the shell.
    pub fn write(&self, data: &[u8]) -> Result<(), String> {
        if data.len() > MAX_WRITE_BYTES {
            return Err(tr!(
                "Entrada grande demais para o terminal.",
                "Input too large for the terminal."
            ));
        }
        let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        writer
            .write_all(data)
            .and_then(|_| writer.flush())
            .map_err(|e| e.to_string())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        let master = self.master.lock().unwrap_or_else(|e| e.into_inner());
        match master.as_ref() {
            Some(master) => master
                .resize(clamp_size(cols, rows))
                .map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }

    /// Ends the shell and removes its container. Safe to call more than once.
    pub fn stop(&self) {
        let _ = self.killer.lock().unwrap_or_else(|e| e.into_inner()).kill();
        // Closing the pseudo terminal is what unblocks the reader on Windows.
        self.master.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let (Some(container), Some(docker)) = (&self.container, &self.docker) {
            remove_container(docker, container);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

/// `docker rm -f <container>`, without waiting and without showing a window.
fn remove_container(docker: &Path, container: &str) {
    let mut command = std::process::Command::new(docker);
    command
        .args(["rm", "-f", container])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let _ = command.spawn();
}

/// Whether a process with this id is running.
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: signal 0 sends nothing; it only reports whether the process exists.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    // EPERM: it exists but belongs to someone else.
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Whether a process with this id is running.
#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ACCESS_DENIED, STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: the handle is checked before use and closed once.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            // Not allowed to look at it: it exists, it is just not ours.
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0u32;
        let known = GetExitCodeProcess(handle, &mut code) != 0;
        CloseHandle(handle);
        !known || code == STILL_ACTIVE as u32
    }
}

/// Which of the container names (one per line) belong to terminals of an AutoDock that is gone.
fn orphaned(names: &str, own_pid: u32, alive: impl Fn(u32) -> bool) -> Vec<String> {
    names
        .lines()
        .map(str::trim)
        .filter_map(|name| {
            let (pid, session) = name.strip_prefix("autodock-terminal-")?.split_once('-')?;
            let pid: u32 = pid.parse().ok()?;
            session.parse::<u32>().ok()?;
            (pid != own_pid && !alive(pid)).then(|| name.to_string())
        })
        .collect()
}

/// Removes the containers that terminals of a previous AutoDock left behind. A terminal's
/// container is removed when it closes, but not when AutoDock is killed or crashes: Docker keeps
/// a container running after the program that started it is gone. Containers of an AutoDock that
/// still runs (another window or installation) are not touched.
pub fn remove_orphans() {
    let Some(docker) = resolve("docker") else {
        return;
    };
    let mut command = std::process::Command::new(&docker);
    command
        .args([
            "ps",
            "-a",
            "--filter",
            "name=autodock-terminal-",
            "--format",
            "{{.Names}}",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let Ok(output) = command.output() else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let names = String::from_utf8_lossy(&output.stdout);
    for name in orphaned(&names, std::process::id(), process_alive) {
        remove_container(&docker, &name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};

    #[test]
    fn only_containers_of_terminals_of_a_dead_autodock_are_orphans() {
        let names = "autodock-terminal-100-1\nautodock-terminal-100-2\nautodock-terminal-200-1\n\
                     autodock-terminal-300-7\nmy-autodock-terminal-400-1\nautodock-terminal-x-1\n\
                     autodock-terminal-500-\nautodock-terminal-600-1-extra\n";
        // 100 is this process, 200 is another AutoDock that still runs, 300 is gone
        let found = orphaned(names, 100, |pid| pid == 200);
        assert_eq!(found, ["autodock-terminal-300-7"]);
        assert!(orphaned("", 1, |_| false).is_empty());
    }

    #[test]
    fn a_process_that_ended_is_not_alive_and_this_one_is() {
        assert!(process_alive(std::process::id()));
        #[cfg(windows)]
        let mut command = {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", "exit"]);
            c
        };
        #[cfg(unix)]
        let mut command = std::process::Command::new("true");
        let mut child = command.spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert!(!process_alive(pid));
    }

    fn project(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for name in files {
            std::fs::write(dir.path().join(name), "x").unwrap();
        }
        dir
    }

    fn path(dir: &tempfile::TempDir) -> String {
        dir.path().to_string_lossy().into_owned()
    }

    #[test]
    fn the_image_follows_the_project_and_only_listed_images_exist() {
        assert_eq!(
            default_image(&path(&project(&["pom.xml", "package.json"]))).unwrap(),
            "maven-jdk25"
        );
        assert_eq!(
            default_image(&path(&project(&["package.json"]))).unwrap(),
            "node22"
        );
        assert_eq!(
            default_image(&path(&project(&["pyproject.toml"]))).unwrap(),
            "python313"
        );
        assert_eq!(
            default_image(&path(&project(&["requirements.txt"]))).unwrap(),
            "python313"
        );
        assert_eq!(
            default_image(&path(&project(&["README.md"]))).unwrap(),
            "debian"
        );
        assert!(default_image("/nowhere/at/all").is_err());
        let ids: Vec<String> = images().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, ["maven-jdk25", "node22", "python313", "debian"]);
        for info in images() {
            assert!(
                !info.label.is_empty() && info.image.contains(':'),
                "{info:?}"
            );
        }
    }

    #[test]
    fn the_docker_arguments_are_fixed_and_only_embed_a_listed_image_and_the_folder() {
        let dir = project(&["pom.xml"]);
        let plan = plan(&path(&dir), "maven-jdk25", 7).unwrap();
        assert_eq!(plan.image_id, "maven-jdk25");
        assert!(plan.container.starts_with("autodock-terminal-") && plan.container.ends_with("-7"));
        let args = &plan.args;
        assert_eq!(&args[..4], ["run", "-it", "--rm", "--name"]);
        assert_eq!(args[4], plan.container);
        assert!(args
            .windows(2)
            .any(|w| w == ["--security-opt", "no-new-privileges"]));
        assert!(args.windows(2).any(|w| w == ["-w", "/work"]));
        let mount = args.iter().find(|a| a.starts_with("type=bind")).unwrap();
        assert!(mount.ends_with(",target=/work"), "{mount}");
        assert!(mount.contains(&*generate::plain_path(&dir.path().canonicalize().unwrap())));
        assert_eq!(
            &args[args.len() - 2..],
            ["maven:3.9-eclipse-temurin-25", "bash"]
        );
        // nothing that could give the container more power
        for forbidden in [
            "--privileged",
            "--cap-add",
            "docker.sock",
            "--pid",
            "--network",
        ] {
            assert!(!args.iter().any(|a| a.contains(forbidden)), "{forbidden}");
        }
        // two terminals never share a container name
        assert_ne!(
            plan.container,
            super::plan(&path(&dir), "debian", 8).unwrap().container
        );
    }

    #[cfg(unix)]
    #[test]
    fn on_unix_the_terminal_runs_as_the_user_so_files_stay_theirs() {
        let plan = plan(&path(&project(&[])), "debian", 1).unwrap();
        let at = plan.args.iter().position(|a| a == "--user").unwrap();
        let (uid, gid) = unsafe { (libc::geteuid(), libc::getegid()) };
        assert_eq!(plan.args[at + 1], format!("{uid}:{gid}"));
        assert!(plan.args.iter().any(|a| a == "HOME=/tmp"));
        assert!(!plan.args.iter().any(|a| a.contains("autodock-maven-cache")));
    }

    #[cfg(not(unix))]
    #[test]
    fn the_maven_terminal_shares_the_cache_of_the_java_upgrade() {
        let maven = plan(&path(&project(&[])), "maven-jdk25", 1).unwrap();
        assert!(maven
            .args
            .iter()
            .any(|a| a == "type=volume,source=autodock-maven-cache,target=/root/.m2/repository"));
        let node = plan(&path(&project(&[])), "node22", 1).unwrap();
        assert!(!node.args.iter().any(|a| a.contains("autodock-maven-cache")));
    }

    #[test]
    fn an_unknown_image_or_an_unsafe_folder_never_reaches_docker() {
        let dir = project(&[]);
        for bad in ["", "ubuntu", "maven-jdk25 --privileged", "../x", "alpine:3"] {
            assert!(plan(&path(&dir), bad, 1).is_err(), "{bad:?}");
        }
        let base = tempfile::tempdir().unwrap();
        let comma = base.path().join("a,b");
        std::fs::create_dir(&comma).unwrap();
        assert!(
            plan(&comma.to_string_lossy(), "debian", 1).is_err(),
            "a comma would add mount options"
        );
        assert!(plan("/nowhere/at/all", "debian", 1).is_err());
    }

    #[test]
    fn the_size_is_kept_inside_sane_limits() {
        assert_eq!(
            (clamp_size(0, 0).cols, clamp_size(0, 0).rows),
            (MIN_COLS, MIN_ROWS)
        );
        assert_eq!(
            (clamp_size(60000, 60000).cols, clamp_size(60000, 60000).rows),
            (MAX_COLS, MAX_ROWS)
        );
        assert_eq!(
            (clamp_size(120, 30).cols, clamp_size(120, 30).rows),
            (120, 30)
        );
    }

    /// A command that prints a marker and ends with code 3 (the same on every system).
    fn marker_command() -> CommandBuilder {
        #[cfg(windows)]
        {
            let mut c = CommandBuilder::new("cmd");
            c.args(["/C", "echo hello-from-pty& exit /b 3"]);
            c
        }
        #[cfg(not(windows))]
        {
            let mut c = CommandBuilder::new("sh");
            c.args(["-c", "printf hello-from-pty; exit 3"]);
            c
        }
    }

    type Sink = Arc<Mutex<Vec<u8>>>;

    impl Launch {
        fn test() -> Launch {
            Launch {
                cols: 80,
                rows: 24,
                container: None,
                docker: None,
                answer_cursor_queries: true,
            }
        }
    }

    fn collect() -> (Sink, impl Fn(Vec<u8>) + Send + 'static) {
        let sink = Arc::new(Mutex::new(vec![]));
        let writer = Arc::clone(&sink);
        (sink, move |data: Vec<u8>| {
            writer.lock().unwrap().extend(data)
        })
    }

    #[test]
    fn what_the_command_prints_and_its_exit_code_reach_the_interface() {
        let (sink, on_data) = collect();
        let (sender, receiver) = mpsc::channel();
        let _session = Session::spawn(marker_command(), Launch::test(), on_data, move |code| {
            let _ = sender.send(code);
        })
        .unwrap();
        let code = receiver
            .recv_timeout(Duration::from_secs(20))
            .expect("the command ends");
        assert_eq!(code, Some(3));
        let text = String::from_utf8_lossy(&sink.lock().unwrap()).into_owned();
        assert!(text.contains("hello-from-pty"), "{text:?}");
    }

    #[test]
    fn stopping_ends_a_command_that_would_run_on() {
        let command = {
            #[cfg(windows)]
            {
                let mut c = CommandBuilder::new("ping");
                c.args(["-n", "60", "127.0.0.1"]);
                c
            }
            #[cfg(not(windows))]
            {
                let mut c = CommandBuilder::new("sleep");
                c.arg("60");
                c
            }
        };
        let (_sink, on_data) = collect();
        let (sender, receiver) = mpsc::channel();
        let session = Session::spawn(command, Launch::test(), on_data, move |code| {
            let _ = sender.send(code);
        })
        .unwrap();
        assert!(
            receiver.recv_timeout(Duration::from_millis(500)).is_err(),
            "still running"
        );
        session.stop();
        assert!(
            receiver.recv_timeout(Duration::from_secs(20)).is_ok(),
            "stop ended it"
        );
        session.stop(); // twice is harmless
        assert!(session.resize(100, 30).is_ok());
    }

    #[test]
    fn too_much_input_is_refused_and_a_closed_terminal_reports_it() {
        let (_sink, on_data) = collect();
        let (sender, receiver) = mpsc::channel();
        let session = Session::spawn(marker_command(), Launch::test(), on_data, move |code| {
            let _ = sender.send(code);
        })
        .unwrap();
        assert!(session.write(&vec![b'a'; MAX_WRITE_BYTES + 1]).is_err());
        receiver.recv_timeout(Duration::from_secs(20)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn what_is_typed_reaches_the_shell() {
        let mut command = CommandBuilder::new("cat");
        command.args(["-"]);
        let (sink, on_data) = collect();
        let session = Session::spawn(command, Launch::test(), on_data, |_| {}).unwrap();
        session.write(b"typed-text\n").unwrap();
        let end = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if String::from_utf8_lossy(&sink.lock().unwrap()).contains("typed-text") {
                break;
            }
            assert!(
                std::time::Instant::now() < end,
                "the typed text never came back"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        session.stop();
    }
}
