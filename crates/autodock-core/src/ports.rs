//! Host-port allocation for generated compose files.
//!
//! A port is considered free when it can be bound on this machine and is not
//! reserved by another project that AutoDock created earlier. Reservations live
//! in a small local registry (`~/.autodock/ports.json`) so that several projects
//! that are not running yet still get different ports.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    net::{Ipv4Addr, TcpListener},
    path::{Path, PathBuf},
    sync::Mutex,
};

const MAX_REGISTRY_BYTES: u64 = 1024 * 1024;
const MAX_STEPS: u32 = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortAssignment {
    /// "app", a service name, or "rabbitmq-management".
    pub name: String,
    /// Port inside the container (never changes).
    pub container_port: u16,
    /// Port on this computer that the compose file publishes.
    pub host_port: u16,
    /// Port AutoDock would have liked to use.
    pub preferred_port: u16,
}

/// What a caller asks for: `(name, container port, preferred host port)`.
pub(crate) type Wanted = (String, u16, u16);

#[derive(Default, Serialize, Deserialize)]
struct Registry {
    /// Canonical project path -> reserved host ports.
    #[serde(default)]
    projects: BTreeMap<String, Vec<u16>>,
}

pub(crate) fn autodock_home() -> PathBuf {
    #[cfg(test)]
    {
        // Each test thread gets its own registry so tests never touch the real one
        // or each other.
        thread_local! {
            static HOME: tempfile::TempDir = tempfile::tempdir().expect("test home");
        }
        HOME.with(|dir| dir.path().to_path_buf())
    }
    #[cfg(not(test))]
    {
        let base = std::env::var_os("AUTODOCK_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                    .map(|home| PathBuf::from(home).join(".autodock"))
            });
        base.unwrap_or_else(|| std::env::temp_dir().join("autodock"))
    }
}

fn registry_path(home: &Path) -> PathBuf {
    home.join("ports.json")
}

fn load(home: &Path) -> Registry {
    let path = registry_path(home);
    let Ok(meta) = fs::metadata(&path) else {
        return Registry::default();
    };
    if !meta.is_file() || meta.len() > MAX_REGISTRY_BYTES {
        return Registry::default();
    }
    fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Ports reserved by *other* existing projects.
fn reserved_by_others(home: &Path, project: &str) -> HashSet<u16> {
    load(home)
        .projects
        .into_iter()
        .filter(|(path, _)| path != project && Path::new(path).exists())
        .flat_map(|(_, ports)| ports)
        .collect()
}

/// Serializes probing so two concurrent probes of the same port cannot make each
/// other look busy.
static PROBE: Mutex<()> = Mutex::new(());

/// True when the port can be bound on this computer right now. Each address is
/// bound and released on its own: two live listeners on overlapping addresses
/// would conflict on some systems and make every port look busy.
pub(crate) fn is_free(port: u16) -> bool {
    let _guard = PROBE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for address in [Ipv4Addr::LOCALHOST, Ipv4Addr::UNSPECIFIED] {
        let listener = TcpListener::bind((address, port));
        let free = listener.is_ok();
        drop(listener);
        if !free {
            return false;
        }
    }
    true
}

fn next_free(
    preferred: u16,
    taken: &HashSet<u16>,
    free: &dyn Fn(u16) -> bool,
) -> Result<u16, String> {
    let mut candidate = preferred.max(1);
    for _ in 0..MAX_STEPS {
        if !taken.contains(&candidate) && free(candidate) {
            return Ok(candidate);
        }
        candidate = if candidate == u16::MAX {
            1024
        } else {
            candidate + 1
        };
    }
    Err(format!(
        "Não foi encontrada uma porta livre a partir da {preferred}."
    ))
}

fn allocate_with(
    home: &Path,
    project: &str,
    wanted: &[Wanted],
    free: &dyn Fn(u16) -> bool,
) -> Result<Vec<PortAssignment>, String> {
    let mut taken = reserved_by_others(home, project);
    let mut out = Vec::with_capacity(wanted.len());
    for (name, container, preferred) in wanted {
        let host = next_free(*preferred, &taken, free)?;
        taken.insert(host);
        out.push(PortAssignment {
            name: name.clone(),
            container_port: *container,
            host_port: host,
            preferred_port: *preferred,
        });
    }
    Ok(out)
}

/// Picks a free host port for each wanted entry.
pub(crate) fn allocate(project: &Path, wanted: &[Wanted]) -> Result<Vec<PortAssignment>, String> {
    allocate_with(
        &autodock_home(),
        &project.to_string_lossy(),
        wanted,
        &is_free,
    )
}

/// Validates assignments that came back from the interface: they must describe
/// exactly the wanted entries and still be usable.
pub(crate) fn validate_given(
    project: &Path,
    wanted: &[Wanted],
    given: &[PortAssignment],
) -> Result<Vec<PortAssignment>, String> {
    validate_given_with(
        &autodock_home(),
        &project.to_string_lossy(),
        wanted,
        given,
        &is_free,
    )
}

fn validate_given_with(
    home: &Path,
    project: &str,
    wanted: &[Wanted],
    given: &[PortAssignment],
    free: &dyn Fn(u16) -> bool,
) -> Result<Vec<PortAssignment>, String> {
    let mismatch = || "As portas não correspondem ao plano. Gere o preview novamente.".to_string();
    if given.len() != wanted.len() {
        return Err(mismatch());
    }
    let taken = reserved_by_others(home, project);
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(wanted.len());
    for (name, container, preferred) in wanted {
        let item = given
            .iter()
            .find(|a| &a.name == name && a.container_port == *container)
            .ok_or_else(mismatch)?;
        if item.preferred_port != *preferred || item.host_port == 0 || !seen.insert(item.host_port)
        {
            return Err(mismatch());
        }
        if taken.contains(&item.host_port) || !free(item.host_port) {
            return Err(format!(
                "A porta {} deixou de estar livre. Gere o preview novamente.",
                item.host_port
            ));
        }
        out.push(item.clone());
    }
    Ok(out)
}

/// Records the ports of `project`, replacing its previous reservation.
/// Best effort: a failure here must never block writing the project files.
pub(crate) fn register(project: &Path, ports: &[PortAssignment]) -> Result<(), String> {
    register_in(&autodock_home(), &project.to_string_lossy(), ports)
}

fn register_in(home: &Path, project: &str, ports: &[PortAssignment]) -> Result<(), String> {
    fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let mut registry = load(home);
    registry.projects.retain(|path, _| Path::new(path).exists());
    registry.projects.insert(
        project.to_string(),
        ports.iter().map(|p| p.host_port).collect(),
    );
    let json = serde_json::to_vec_pretty(&registry).map_err(|e| e.to_string())?;
    let temporary = home.join(format!("ports.json.{}.tmp", std::process::id()));
    fs::write(&temporary, json).map_err(|e| e.to_string())?;
    fs::rename(&temporary, registry_path(home)).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wanted() -> Vec<Wanted> {
        vec![("app".into(), 3000, 3000), ("postgres".into(), 5432, 5432)]
    }

    #[test]
    fn keeps_preferred_ports_when_everything_is_free() {
        let home = tempfile::tempdir().unwrap();
        let out = allocate_with(home.path(), "/p/a", &wanted(), &|_| true).unwrap();
        assert_eq!(out[0].host_port, 3000);
        assert_eq!(out[1].host_port, 5432);
        assert_eq!(out[1].container_port, 5432);
    }

    #[test]
    fn skips_busy_ports_and_never_repeats_a_port_inside_one_plan() {
        let home = tempfile::tempdir().unwrap();
        let busy = |p: u16| p != 3000 && p != 3001;
        let both = vec![
            ("app".to_string(), 3000, 3000),
            ("api".to_string(), 8000, 3000),
        ];
        let out = allocate_with(home.path(), "/p/a", &both, &busy).unwrap();
        assert_eq!(out[0].host_port, 3002);
        assert_eq!(out[1].host_port, 3003);
        assert_eq!(out[0].preferred_port, 3000);
    }

    #[test]
    fn a_really_bound_port_is_detected() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(!is_free(port));
        let home = tempfile::tempdir().unwrap();
        let out =
            allocate_with(home.path(), "/p/a", &[("app".into(), 80, port)], &is_free).unwrap();
        assert_ne!(out[0].host_port, port);
        drop(listener);
    }

    #[test]
    fn registered_projects_reserve_their_ports_for_other_projects() {
        let home = tempfile::tempdir().unwrap();
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let first_path = first.path().to_string_lossy().into_owned();
        let second_path = second.path().to_string_lossy().into_owned();
        let a = allocate_with(home.path(), &first_path, &wanted(), &|_| true).unwrap();
        register_in(home.path(), &first_path, &a).unwrap();
        // another project gets different ports even though nothing is running
        let b = allocate_with(home.path(), &second_path, &wanted(), &|_| true).unwrap();
        assert_eq!((b[0].host_port, b[1].host_port), (3001, 5433));
        register_in(home.path(), &second_path, &b).unwrap();
        // the first project keeps its own ports when it is generated again
        let again = allocate_with(home.path(), &first_path, &wanted(), &|_| true).unwrap();
        assert_eq!(again, a);
        // ten projects never collide
        let mut seen = HashSet::new();
        for _ in 0..10 {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().to_string_lossy().into_owned();
            let ports = allocate_with(home.path(), &path, &wanted(), &|_| true).unwrap();
            for p in &ports {
                assert!(seen.insert((p.name.clone(), p.host_port)), "{p:?}");
            }
            register_in(home.path(), &path, &ports).unwrap();
            std::mem::forget(dir); // keep the directory so its reservation stays valid
        }
    }

    #[test]
    fn deleted_projects_release_their_ports() {
        let home = tempfile::tempdir().unwrap();
        let gone = tempfile::tempdir().unwrap();
        let gone_path = gone.path().to_string_lossy().into_owned();
        let a = allocate_with(home.path(), &gone_path, &wanted(), &|_| true).unwrap();
        register_in(home.path(), &gone_path, &a).unwrap();
        drop(gone);
        let b = allocate_with(home.path(), "/p/other", &wanted(), &|_| true).unwrap();
        assert_eq!(b[0].host_port, 3000);
    }

    #[test]
    fn corrupt_or_oversized_registry_is_ignored() {
        let home = tempfile::tempdir().unwrap();
        fs::write(registry_path(home.path()), b"{not json").unwrap();
        assert!(allocate_with(home.path(), "/p/a", &wanted(), &|_| true).is_ok());
        fs::write(registry_path(home.path()), vec![b' '; 2 * 1024 * 1024]).unwrap();
        assert!(allocate_with(home.path(), "/p/a", &wanted(), &|_| true).is_ok());
    }

    #[test]
    fn given_assignments_must_match_the_plan_and_still_be_free() {
        let home = tempfile::tempdir().unwrap();
        let good = allocate_with(home.path(), "/p/a", &wanted(), &|_| true).unwrap();
        assert!(validate_given_with(home.path(), "/p/a", &wanted(), &good, &|_| true).is_ok());
        // a port that stopped being free is reported
        let busy = |p: u16| p != good[1].host_port;
        let error = validate_given_with(home.path(), "/p/a", &wanted(), &good, &busy).unwrap_err();
        assert!(error.contains("deixou de estar livre"), "{error}");
        // tampering: wrong count, wrong container port, duplicate host ports, port 0
        assert!(
            validate_given_with(home.path(), "/p/a", &wanted(), &good[..1], &|_| true).is_err()
        );
        let mut wrong = good.clone();
        wrong[1].container_port = 1;
        assert!(validate_given_with(home.path(), "/p/a", &wanted(), &wrong, &|_| true).is_err());
        let mut same = good.clone();
        same[1].host_port = same[0].host_port;
        assert!(validate_given_with(home.path(), "/p/a", &wanted(), &same, &|_| true).is_err());
        let mut zero = good.clone();
        zero[0].host_port = 0;
        assert!(validate_given_with(home.path(), "/p/a", &wanted(), &zero, &|_| true).is_err());
    }
}
