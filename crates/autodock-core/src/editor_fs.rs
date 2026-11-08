//! Files of a project folder for the code editor: list, read, write and create, always inside
//! the folder the person chose.
//!
//! Every path comes from the interface, so it is treated as untrusted: it must be relative,
//! made of plain names (no `..`, no drive, no control characters), and every part of it that
//! exists must be a real folder or file, never a link (symbolic link, junction or other
//! reparse point), so nothing can lead outside the chosen folder. Nothing is executed.
use crate::generate;
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// Largest file the editor opens (bigger ones are refused, not cut: saving a cut file would lose data).
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Entries listed for one folder.
const MAX_ENTRIES: usize = 5000;
const MAX_NAME_BYTES: usize = 255;
const MAX_PATH_PARTS: usize = 64;
const BINARY_SNIFF: usize = 8000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorRoot {
    /// The folder, as people write it.
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorEntry {
    pub name: String,
    /// `file`, `dir`, `link` (never followed) or `other`.
    pub kind: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorListing {
    pub entries: Vec<EditorEntry>,
    /// The folder has more entries than the list holds.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorFile {
    /// Path inside the folder, with `/`.
    pub path: String,
    pub text: String,
    pub size: u64,
    /// Last change on disk, in milliseconds since the Unix epoch: it is sent back on save so a
    /// change made elsewhere in the meantime is noticed.
    pub modified_ms: u64,
    /// The file started with a UTF-8 byte order mark (kept on save).
    pub bom: bool,
    /// The name suggests secrets (`.env`, keys, certificates).
    pub sensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReport {
    pub modified_ms: u64,
    pub size: u64,
}

/// A recognisable first word, so the interface can offer "overwrite" or "reload" on a conflict.
pub const CONFLICT: &str = "conflict";

// ───────────────────────────── paths ─────────────────────────────

/// A folder or file name that is safe to create or to follow: one plain component.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= MAX_NAME_BYTES
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.chars().any(char::is_control)
        && !name.ends_with(' ')
        && !name.ends_with('.')
}

fn parts_of(relative: &str) -> Result<Vec<&str>, String> {
    if relative.is_empty() {
        return Ok(vec![]);
    }
    if relative.starts_with('/') || relative.starts_with('\\') {
        return Err(tr!(
            "O caminho precisa ser relativo à pasta do projeto.",
            "The path must be relative to the project folder."
        ));
    }
    let parts: Vec<&str> = relative.split(['/', '\\']).collect();
    if parts.len() > MAX_PATH_PARTS || !parts.iter().all(|p| valid_name(p)) {
        return Err(tr!(
            "Caminho inválido: {relative}",
            "Invalid path: {relative}"
        ));
    }
    Ok(parts)
}

/// True for symbolic links and, on Windows, for junctions and any other reparse point.
pub(crate) fn is_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

/// The canonical project folder.
fn root_dir(root: &str) -> Result<PathBuf, String> {
    generate::root_of(root)
}

/// The existing path `relative` inside `root`, with every part checked not to be a link.
fn resolve(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let mut current = root.to_path_buf();
    for part in parts_of(relative)? {
        current.push(part);
        let meta = fs::symlink_metadata(&current).map_err(|e| format!("{relative}: {e}"))?;
        if is_link(&meta) {
            return Err(tr!(
                "{relative} passa por um link, que o editor não segue.",
                "{relative} goes through a link, which the editor does not follow."
            ));
        }
    }
    Ok(current)
}

fn relative_text(parts: &[&str]) -> String {
    parts.join("/")
}

pub fn open_root(path: &str) -> Result<EditorRoot, String> {
    let root = root_dir(path)?;
    Ok(EditorRoot {
        path: generate::plain_path(&root),
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| generate::plain_path(&root)),
    })
}

// ───────────────────────────── list ─────────────────────────────

fn kind_of(meta: &fs::Metadata) -> &'static str {
    if is_link(meta) {
        "link"
    } else if meta.is_dir() {
        "dir"
    } else if meta.is_file() {
        "file"
    } else {
        "other"
    }
}

/// One level of a folder: folders first, then files, each sorted without regard to case.
pub fn list(root: &str, relative: &str) -> Result<EditorListing, String> {
    let root = root_dir(root)?;
    let dir = resolve(&root, relative)?;
    let meta = fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
    if !meta.is_dir() {
        return Err(tr!(
            "{relative} não é uma pasta.",
            "{relative} is not a folder."
        ));
    }
    let mut entries = vec![];
    let mut truncated = false;
    for entry in fs::read_dir(&dir).map_err(|e| format!("{relative}: {e}"))? {
        let Ok(entry) = entry else { continue };
        if entries.len() >= MAX_ENTRIES {
            truncated = true;
            break;
        }
        let Ok(meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        entries.push(EditorEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            kind: kind_of(&meta).into(),
            size: if meta.is_file() { meta.len() } else { 0 },
        });
    }
    entries.sort_by(|a, b| {
        let folder = |e: &EditorEntry| e.kind != "dir";
        folder(a)
            .cmp(&folder(b))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(EditorListing { entries, truncated })
}

// ───────────────────────────── read ─────────────────────────────

/// Names that usually hold secrets: the interface warns before they are shown.
pub fn sensitive_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    let example = ["example", "sample", "template", "dist"]
        .iter()
        .any(|s| lower.ends_with(&format!(".{s}")));
    (lower == ".env" || lower.starts_with(".env.")) && !example
        || [".pem", ".key", ".p12", ".pfx", ".jks", ".keystore"]
            .iter()
            .any(|s| lower.ends_with(s))
        || lower.starts_with("id_rsa")
        || lower.starts_with("id_ed25519")
        || lower == ".netrc"
        || lower == "credentials"
}

fn modified_ms(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn read(root: &str, relative: &str) -> Result<EditorFile, String> {
    let root = root_dir(root)?;
    let parts = parts_of(relative)?;
    if parts.is_empty() {
        return Err(tr!("Escolha um arquivo.", "Choose a file."));
    }
    let path = resolve(&root, relative)?;
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("{relative}: {e}"))?;
    if !meta.is_file() {
        return Err(tr!(
            "{relative} não é um arquivo comum.",
            "{relative} is not a regular file."
        ));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(tr!(
            "{relative} é grande demais para o editor ({} MB; o limite é {} MB).",
            "{relative} is too large for the editor ({} MB; the limit is {} MB).",
            meta.len() / 1_000_000,
            MAX_FILE_BYTES / 1_000_000
        ));
    }
    let bytes = fs::read(&path).map_err(|e| format!("{relative}: {e}"))?;
    if bytes.iter().take(BINARY_SNIFF).any(|b| *b == 0) {
        return Err(tr!(
            "{relative} parece um arquivo binário, que o editor não abre.",
            "{relative} looks like a binary file, which the editor does not open."
        ));
    }
    let (bom, body) = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => (true, rest),
        None => (false, &bytes[..]),
    };
    let text = String::from_utf8(body.to_vec()).map_err(|_| {
        tr!(
            "{relative} não está em UTF-8: o editor não o abre para não corromper o texto ao salvar.",
            "{relative} is not UTF-8: the editor does not open it so saving cannot corrupt the text."
        )
    })?;
    Ok(EditorFile {
        path: relative_text(&parts),
        text,
        size: meta.len(),
        modified_ms: modified_ms(&meta),
        bom,
        sensitive: parts.last().is_some_and(|name| sensitive_name(name)),
    })
}

// ───────────────────────────── write ─────────────────────────────

/// Saves `text` over an existing file. `expected_ms` is the `modified_ms` the editor read: when
/// the file changed on disk since, nothing is written unless `overwrite` is set.
pub fn write(
    root: &str,
    relative: &str,
    text: &str,
    bom: bool,
    expected_ms: Option<u64>,
    overwrite: bool,
) -> Result<SaveReport, String> {
    let root = root_dir(root)?;
    if parts_of(relative)?.is_empty() {
        return Err(tr!("Escolha um arquivo.", "Choose a file."));
    }
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err(tr!(
            "O texto é grande demais para salvar ({} MB; o limite é {} MB).",
            "The text is too large to save ({} MB; the limit is {} MB).",
            text.len() / 1_000_000,
            MAX_FILE_BYTES / 1_000_000
        ));
    }
    let path = resolve(&root, relative)?;
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("{relative}: {e}"))?;
    if !meta.is_file() {
        return Err(tr!(
            "{relative} não é um arquivo comum.",
            "{relative} is not a regular file."
        ));
    }
    if !overwrite {
        if let Some(expected) = expected_ms {
            if modified_ms(&meta) != expected {
                return Err(format!(
                    "{CONFLICT}: {}",
                    tr!(
                        "{relative} mudou no disco depois que foi aberto.",
                        "{relative} changed on disk after it was opened."
                    )
                ));
            }
        }
    }
    // Written next to the file and renamed over it, so a crash never leaves half a file.
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = path.with_file_name(format!(".{name}.autodock-save"));
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        if bom {
            file.write_all(&[0xEF, 0xBB, 0xBF])?;
        }
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::set_permissions(&temporary, meta.permissions())?;
        fs::rename(&temporary, &path)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(format!("{relative}: {error}"));
    }
    let after = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    Ok(SaveReport {
        modified_ms: modified_ms(&after),
        size: after.len(),
    })
}

// ───────────────────────────── search ─────────────────────────────

/// Matches returned for one search (more are reported as `truncated`).
pub const MAX_SEARCH_RESULTS: usize = 500;
const MAX_SEARCH_FILES: usize = 20_000;
const MAX_SEARCH_FILE_BYTES: u64 = 1024 * 1024;
const MAX_QUERY_CHARS: usize = 200;
/// Characters of the matching line that are returned.
const PREVIEW_CHARS: usize = 240;
/// Characters of context kept before a match when its line is cut.
const PREVIEW_BEFORE: usize = 60;
/// Folders that hold dependencies, build output or tool files: skipped unless asked for.
const SEARCH_SKIPPED_DIRS: [&str; 10] = [
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "build",
    "dist",
    ".gradle",
    ".idea",
    ".vscode",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    /// Path inside the folder, with `/`.
    pub path: String,
    /// 1-based, like the editor.
    pub line: u32,
    /// 1-based, in UTF-16 units, like the editor.
    pub column: u32,
    /// The line (cut around the match when it is long).
    pub preview: String,
    /// Where the match starts inside `preview`, and how long it is, in characters.
    pub preview_start: u32,
    pub length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchReport {
    pub matches: Vec<SearchMatch>,
    pub files_scanned: usize,
    pub files_with_matches: usize,
    /// Files that were not searched: too large, binary or not UTF-8.
    pub files_skipped: usize,
    /// There are more matches (or files) than were returned.
    pub truncated: bool,
}

/// One character, folded the way a case-insensitive search compares it. It never changes the
/// number of characters, so a column found in the folded line is the column of the real line.
fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Starts (in characters) of the non-overlapping occurrences of `needle` in `line`.
fn find_all(line: &[char], needle: &[char], case_sensitive: bool) -> Vec<usize> {
    let mut found = vec![];
    if needle.is_empty() || line.len() < needle.len() {
        return found;
    }
    let same = |a: char, b: char| {
        if case_sensitive {
            a == b
        } else {
            fold(a) == fold(b)
        }
    };
    let mut start = 0;
    while start + needle.len() <= line.len() {
        if needle
            .iter()
            .enumerate()
            .all(|(i, n)| same(line[start + i], *n))
        {
            found.push(start);
            start += needle.len();
        } else {
            start += 1;
        }
    }
    found
}

fn preview_of(line: &[char], start: usize, length: usize) -> (String, u32) {
    // Leading indentation is noise in a list of results.
    let indent = line
        .iter()
        .take_while(|c| c.is_whitespace())
        .count()
        .min(start);
    let from = if line.len() > PREVIEW_CHARS + indent {
        start.saturating_sub(PREVIEW_BEFORE).max(indent)
    } else {
        indent
    };
    let to = (from + PREVIEW_CHARS)
        .min(line.len())
        .max((start + length).min(line.len()));
    (line[from..to].iter().collect(), (start - from) as u32)
}

/// Looks for `query` as plain text in every text file under the folder. Folders of dependencies
/// and build output are skipped unless `include_ignored`; links are never followed.
pub fn search(
    root: &str,
    query: &str,
    case_sensitive: bool,
    include_ignored: bool,
) -> Result<SearchReport, String> {
    let root = root_dir(root)?;
    let needle: Vec<char> = query.chars().collect();
    if needle.is_empty() || needle.len() > MAX_QUERY_CHARS || query.contains(['\n', '\r', '\0']) {
        return Err(tr!(
            "Digite de 1 a {MAX_QUERY_CHARS} caracteres, em uma só linha.",
            "Type 1 to {MAX_QUERY_CHARS} characters, on a single line."
        ));
    }
    let mut report = SearchReport {
        matches: vec![],
        files_scanned: 0,
        files_with_matches: 0,
        files_skipped: 0,
        truncated: false,
    };
    // Depth first, in name order, so the same search always gives the same answer.
    let mut pending: Vec<(PathBuf, String)> = vec![(root.clone(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = read.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        let mut subfolders = vec![];
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if is_link(&meta) {
                continue;
            }
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if meta.is_dir() {
                if include_ignored || !SEARCH_SKIPPED_DIRS.contains(&name.as_str()) {
                    subfolders.push((entry.path(), relative));
                }
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            if report.files_scanned >= MAX_SEARCH_FILES {
                report.truncated = true;
                break;
            }
            report.files_scanned += 1;
            if meta.len() > MAX_SEARCH_FILE_BYTES {
                report.files_skipped += 1;
                continue;
            }
            let Ok(bytes) = fs::read(entry.path()) else {
                report.files_skipped += 1;
                continue;
            };
            if bytes.iter().take(BINARY_SNIFF).any(|b| *b == 0) {
                report.files_skipped += 1;
                continue;
            }
            let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
            let Ok(text) = std::str::from_utf8(body) else {
                report.files_skipped += 1;
                continue;
            };
            let mut found_here = false;
            for (index, raw) in text.split('\n').enumerate() {
                let line: Vec<char> = raw.trim_end_matches('\r').chars().collect();
                for start in find_all(&line, &needle, case_sensitive) {
                    if report.matches.len() >= MAX_SEARCH_RESULTS {
                        report.truncated = true;
                        break;
                    }
                    found_here = true;
                    let column = line[..start].iter().map(|c| c.len_utf16()).sum::<usize>() + 1;
                    let (preview, preview_start) = preview_of(&line, start, needle.len());
                    report.matches.push(SearchMatch {
                        path: relative.clone(),
                        line: index as u32 + 1,
                        column: column as u32,
                        preview,
                        preview_start,
                        length: needle.len() as u32,
                    });
                }
                if report.truncated && report.matches.len() >= MAX_SEARCH_RESULTS {
                    break;
                }
            }
            if found_here {
                report.files_with_matches += 1;
            }
            if report.matches.len() >= MAX_SEARCH_RESULTS {
                report.truncated = true;
                return Ok(report);
            }
        }
        // Folders are visited in name order too (the stack gives the last pushed first).
        pending.extend(subfolders.into_iter().rev());
    }
    Ok(report)
}

// ───────────────────────────── create ─────────────────────────────

/// Creates an empty file or a folder named `name` inside the folder `parent`. Never overwrites.
pub fn create(root: &str, parent: &str, name: &str, kind: &str) -> Result<EditorEntry, String> {
    let root = root_dir(root)?;
    if !valid_name(name) {
        return Err(tr!("Nome inválido: {name}", "Invalid name: {name}"));
    }
    let folder = resolve(&root, parent)?;
    if !fs::symlink_metadata(&folder).is_ok_and(|m| m.is_dir()) {
        return Err(tr!(
            "{parent} não é uma pasta.",
            "{parent} is not a folder."
        ));
    }
    let target = folder.join(name);
    match kind {
        "file" => {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| format!("{name}: {e}"))?;
        }
        "dir" => fs::create_dir(&target).map_err(|e| format!("{name}: {e}"))?,
        _ => return Err(tr!("Escolha arquivo ou pasta.", "Choose file or folder.")),
    }
    Ok(EditorEntry {
        name: name.to_string(),
        kind: if kind == "dir" { "dir" } else { "file" }.into(),
        size: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        for (name, content) in files {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        let root = dir.path().to_string_lossy().into_owned();
        (dir, root)
    }

    #[test]
    fn a_folder_is_listed_one_level_at_a_time_folders_first_and_sorted() {
        let (_dir, root) = project(&[
            ("b.txt", "1"),
            ("A.txt", "22"),
            ("src/main/App.java", "x"),
            ("lib/x.jar", "y"),
            (".hidden", "z"),
        ]);
        let top = list(&root, "").unwrap();
        let names: Vec<(&str, &str)> = top
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e.kind.as_str()))
            .collect();
        assert_eq!(
            names,
            [
                ("lib", "dir"),
                ("src", "dir"),
                (".hidden", "file"),
                ("A.txt", "file"),
                ("b.txt", "file")
            ]
        );
        assert_eq!(top.entries[3].size, 2);
        assert!(!top.truncated);
        let inner = list(&root, "src/main").unwrap();
        assert_eq!(inner.entries.len(), 1);
        assert!(list(&root, "b.txt").is_err(), "a file is not a folder");
        assert!(list(&root, "nowhere").is_err());
    }

    #[test]
    fn paths_that_could_leave_the_folder_are_refused() {
        let (_dir, root) = project(&[("a.txt", "x"), ("src/b.txt", "y")]);
        for bad in [
            "../a.txt",
            "src/../../a.txt",
            "/etc/passwd",
            "\\\\server\\share\\x",
            "C:/Windows/win.ini",
            "src/..",
            "./a.txt",
            "a.txt/",
            "src//b.txt",
            "a\0b",
            "src/b.txt\n",
        ] {
            assert!(read(&root, bad).is_err(), "{bad:?} must be refused");
            assert!(list(&root, bad).is_err(), "{bad:?} must be refused");
            assert!(
                write(&root, bad, "x", false, None, true).is_err(),
                "{bad:?}"
            );
        }
        assert_eq!(read(&root, "src/b.txt").unwrap().text, "y");
        // a backslash is a separator on every system, never part of a name
        assert_eq!(read(&root, "src\\b.txt").unwrap().path, "src/b.txt");
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_is_never_followed_for_reading_listing_writing_or_creating() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        let (dir, root) = project(&[("a.txt", "x")]);
        std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), dir.path().join("s.txt"))
            .unwrap();
        assert!(read(&root, "s.txt").is_err());
        assert!(read(&root, "out/secret.txt").is_err());
        assert!(list(&root, "out").is_err());
        assert!(write(&root, "s.txt", "pwn", false, None, true).is_err());
        assert!(create(&root, "out", "new.txt", "file").is_err());
        assert_eq!(
            fs::read_to_string(outside.path().join("secret.txt")).unwrap(),
            "secret"
        );
        assert!(!outside.path().join("new.txt").exists());
        // the link itself is listed as a link
        let listing = list(&root, "").unwrap();
        let kind = |name: &str| {
            listing
                .entries
                .iter()
                .find(|e| e.name == name)
                .map(|e| e.kind.clone())
        };
        assert_eq!(kind("out").as_deref(), Some("link"));
        assert_eq!(kind("s.txt").as_deref(), Some("link"));
        assert_eq!(kind("a.txt").as_deref(), Some("file"));
    }

    #[test]
    fn text_files_are_read_with_their_size_time_and_bom() {
        let (dir, root) = project(&[("plain.txt", "héllo\r\nworld\n")]);
        let file = read(&root, "plain.txt").unwrap();
        assert_eq!(file.text, "héllo\r\nworld\n");
        assert!(!file.bom && !file.sensitive);
        assert_eq!(file.size, "héllo\r\nworld\n".len() as u64);
        assert!(file.modified_ms > 0);
        fs::write(
            dir.path().join("bom.txt"),
            [&[0xEF, 0xBB, 0xBF][..], b"abc"].concat(),
        )
        .unwrap();
        let bom = read(&root, "bom.txt").unwrap();
        assert_eq!((bom.text.as_str(), bom.bom), ("abc", true));
    }

    #[test]
    fn binary_non_utf8_and_huge_files_are_refused_with_the_reason() {
        let (dir, root) = project(&[("ok.txt", "x")]);
        fs::write(
            dir.path().join("image.png"),
            [0x89, b'P', b'N', b'G', 0, 1, 2],
        )
        .unwrap();
        assert!(read(&root, "image.png")
            .unwrap_err()
            .to_lowercase()
            .contains("bin"));
        fs::write(dir.path().join("latin1.txt"), [b'c', b'a', b'f', 0xE9]).unwrap();
        assert!(read(&root, "latin1.txt").unwrap_err().contains("UTF-8"));
        let big = vec![b'a'; (MAX_FILE_BYTES + 1) as usize];
        fs::write(dir.path().join("big.txt"), big).unwrap();
        assert!(read(&root, "big.txt").unwrap_err().contains("MB"));
        assert!(read(&root, "").is_err(), "the folder is not a file");
        assert!(read(&root, "ok.txt").is_ok());
    }

    #[test]
    fn names_that_usually_hold_secrets_are_flagged() {
        for name in [
            ".env",
            ".env.production",
            "server.pem",
            "id_rsa",
            "id_ed25519.pub",
            "app.keystore",
            ".netrc",
        ] {
            assert!(sensitive_name(name), "{name}");
        }
        for name in [
            ".env.example",
            ".env.sample",
            "readme.md",
            "environment.ts",
            "monkey.txt",
        ] {
            assert!(!sensitive_name(name), "{name}");
        }
        let (_dir, root) = project(&[(".env", "A=1")]);
        assert!(read(&root, ".env").unwrap().sensitive);
    }

    #[test]
    fn saving_replaces_the_file_keeps_the_bom_and_leaves_nothing_behind() {
        let (dir, root) = project(&[("a.txt", "old")]);
        let before = read(&root, "a.txt").unwrap();
        let report = write(
            &root,
            "a.txt",
            "new text\n",
            false,
            Some(before.modified_ms),
            false,
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "new text\n"
        );
        assert_eq!(report.size, 9);
        let entries: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, ["a.txt"], "no temporary file is left");
        // the BOM comes back when asked
        write(&root, "a.txt", "abc", true, None, true).unwrap();
        assert_eq!(
            fs::read(dir.path().join("a.txt")).unwrap(),
            [&[0xEF, 0xBB, 0xBF][..], b"abc"].concat()
        );
        // only existing regular files can be saved
        assert!(write(&root, "missing.txt", "x", false, None, true).is_err());
        let (_d, root2) = project(&[("src/a.txt", "x")]);
        assert!(write(&root2, "src", "x", false, None, true).is_err());
    }

    #[test]
    fn a_change_made_elsewhere_is_noticed_and_only_an_explicit_overwrite_wins() {
        let (dir, root) = project(&[("a.txt", "v1")]);
        let opened = read(&root, "a.txt").unwrap();
        // someone else saves in between: the modified time moves on
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(dir.path().join("a.txt"), "other change").unwrap();
        let moved = read(&root, "a.txt").unwrap();
        if moved.modified_ms == opened.modified_ms {
            // a file system with a coarse clock: nothing to compare, the check cannot fire
            return;
        }
        let error = write(
            &root,
            "a.txt",
            "mine",
            false,
            Some(opened.modified_ms),
            false,
        )
        .unwrap_err();
        assert!(error.starts_with(CONFLICT), "{error}");
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "other change"
        );
        write(
            &root,
            "a.txt",
            "mine",
            false,
            Some(opened.modified_ms),
            true,
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn too_much_text_is_not_saved() {
        let (dir, root) = project(&[("a.txt", "keep")]);
        let big = "a".repeat((MAX_FILE_BYTES + 1) as usize);
        assert!(write(&root, "a.txt", &big, false, None, true).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "keep"
        );
    }

    #[test]
    fn files_and_folders_are_created_without_ever_overwriting() {
        let (dir, root) = project(&[("src/a.txt", "x")]);
        let file = create(&root, "src", "B.java", "file").unwrap();
        assert_eq!(
            (file.name.as_str(), file.kind.as_str(), file.size),
            ("B.java", "file", 0)
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("src/B.java")).unwrap(),
            ""
        );
        assert_eq!(create(&root, "", "docs", "dir").unwrap().kind, "dir");
        assert!(dir.path().join("docs").is_dir());
        // never over something that exists
        assert!(create(&root, "src", "a.txt", "file").is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("src/a.txt")).unwrap(),
            "x"
        );
        assert!(create(&root, "", "src", "dir").is_err());
        // bad names, bad parents and bad kinds
        for bad in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "c:x",
            "x\0",
            "tab\there",
            "trailing.",
            "space ",
        ] {
            assert!(create(&root, "src", bad, "file").is_err(), "{bad:?}");
        }
        assert!(
            create(&root, "src/a.txt", "x", "file").is_err(),
            "the parent is a file"
        );
        assert!(create(&root, "../", "x", "file").is_err());
        assert!(create(&root, "src", "x", "socket").is_err());
    }

    #[test]
    fn the_root_is_described_and_must_be_a_folder() {
        let (dir, root) = project(&[("a.txt", "x")]);
        let info = open_root(&root).unwrap();
        assert_eq!(info.name, dir.path().file_name().unwrap().to_string_lossy());
        assert!(open_root(&dir.path().join("a.txt").to_string_lossy()).is_err());
        assert!(open_root("/nowhere/at/all").is_err());
    }
    #[cfg(windows)]
    #[test]
    fn a_junction_is_never_followed_either() {
        // A junction is not a symbolic link, but it leads outside the folder just the same.
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        let (dir, root) = project(&[("a.txt", "x")]);
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(dir.path().join("out"))
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(made.status.success(), "mklink failed: {made:?}");
        assert!(read(&root, "out/secret.txt").is_err());
        assert!(list(&root, "out").is_err());
        assert!(write(&root, "out/secret.txt", "pwn", false, None, true).is_err());
        assert!(create(&root, "out", "new.txt", "file").is_err());
        assert_eq!(
            fs::read_to_string(outside.path().join("secret.txt")).unwrap(),
            "secret"
        );
        let listing = list(&root, "").unwrap();
        assert_eq!(
            listing
                .entries
                .iter()
                .find(|e| e.name == "out")
                .map(|e| e.kind.as_str()),
            Some("link")
        );
        // remove the junction itself, never what it points to
        fs::remove_dir(dir.path().join("out")).unwrap();
        assert!(outside.path().join("secret.txt").exists());
    }
    #[test]
    fn a_search_finds_every_occurrence_with_its_line_and_column_in_name_order() {
        let (_dir, root) = project(&[
            ("b.txt", "nothing here\n"),
            ("a.txt", "first needle\n  second needle and needle\n"),
            (
                "src/Main.java",
                "class Main {\n\tString s = \"Needle\";\n}\n",
            ),
        ]);
        let report = search(&root, "needle", false, false).unwrap();
        let found: Vec<(&str, u32, u32)> = report
            .matches
            .iter()
            .map(|m| (m.path.as_str(), m.line, m.column))
            .collect();
        assert_eq!(
            found,
            [
                ("a.txt", 1, 7),
                ("a.txt", 2, 10),
                ("a.txt", 2, 21),
                ("src/Main.java", 2, 14)
            ]
        );
        assert_eq!(
            (
                report.files_scanned,
                report.files_with_matches,
                report.files_skipped
            ),
            (3, 2, 0)
        );
        assert!(!report.truncated);
        // the preview drops the indentation and says where the match is
        let second = &report.matches[1];
        assert_eq!(
            (second.preview.as_str(), second.preview_start, second.length),
            ("second needle and needle", 7, 6)
        );
        // case-sensitive: only the exact spelling
        let exact = search(&root, "Needle", true, false).unwrap();
        assert_eq!(exact.matches.len(), 1);
        assert_eq!(exact.matches[0].path, "src/Main.java");
        // the same search gives the same answer
        assert_eq!(search(&root, "needle", false, false).unwrap(), report);
    }

    #[test]
    fn columns_count_utf16_units_like_the_editor_and_folding_keeps_them() {
        // an emoji takes two UTF-16 units; an accented capital folds to one character
        let (_dir, root) = project(&[("u.txt", "😀 ação AÇÃO\n")]);
        let report = search(&root, "ação", false, false).unwrap();
        let columns: Vec<u32> = report.matches.iter().map(|m| m.column).collect();
        assert_eq!(columns, [4, 9]);
        assert_eq!(report.matches[0].length, 4);
        assert_eq!(search(&root, "ação", true, false).unwrap().matches.len(), 1);
    }

    #[test]
    fn dependency_and_build_folders_links_binary_big_and_non_utf8_files_are_not_searched() {
        let (dir, root) = project(&[
            ("src/App.java", "needle\n"),
            ("node_modules/lib/index.js", "needle\n"),
            ("target/classes/Gen.java", "needle\n"),
            (".git/config", "needle\n"),
            ("docs/build/notes.txt", "needle\n"),
        ]);
        fs::write(dir.path().join("image.bin"), [b'n', b'e', 0, b'e']).unwrap();
        fs::write(
            dir.path().join("latin1.txt"),
            [b'n', b'e', b'e', b'd', b'l', b'e', 0xE9],
        )
        .unwrap();
        fs::write(
            dir.path().join("big.txt"),
            vec![b'a'; (MAX_SEARCH_FILE_BYTES + 1) as usize],
        )
        .unwrap();
        let report = search(&root, "needle", false, false).unwrap();
        let files: Vec<&str> = report.matches.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(files, ["src/App.java"]);
        assert_eq!(report.files_skipped, 3, "binary, non-UTF-8 and too big");
        // asked to, the ignored folders are searched too
        let all = search(&root, "needle", false, true).unwrap();
        let mut files: Vec<&str> = all.matches.iter().map(|m| m.path.as_str()).collect();
        files.sort_unstable();
        assert_eq!(
            files,
            [
                ".git/config",
                "docs/build/notes.txt",
                "node_modules/lib/index.js",
                "src/App.java",
                "target/classes/Gen.java"
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_search_never_follows_a_symbolic_link_out_of_the_folder() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "needle outside\n").unwrap();
        let (dir, root) = project(&[("inside.txt", "needle inside\n")]);
        std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), dir.path().join("s.txt"))
            .unwrap();
        let report = search(&root, "needle", false, true).unwrap();
        assert_eq!(
            report
                .matches
                .iter()
                .map(|m| m.path.as_str())
                .collect::<Vec<_>>(),
            ["inside.txt"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_search_never_follows_a_junction_out_of_the_folder() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "needle outside\n").unwrap();
        let (dir, root) = project(&[("inside.txt", "needle inside\n")]);
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(dir.path().join("out"))
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(made.status.success(), "mklink failed: {made:?}");
        let report = search(&root, "needle", false, true).unwrap();
        assert_eq!(
            report
                .matches
                .iter()
                .map(|m| m.path.as_str())
                .collect::<Vec<_>>(),
            ["inside.txt"]
        );
        fs::remove_dir(dir.path().join("out")).unwrap();
    }

    #[test]
    fn too_many_matches_are_cut_and_said() {
        let many = "x\n".repeat(MAX_SEARCH_RESULTS + 50);
        let (_dir, root) = project(&[("a.txt", &many), ("b.txt", "x\n")]);
        let report = search(&root, "x", true, false).unwrap();
        assert_eq!(report.matches.len(), MAX_SEARCH_RESULTS);
        assert!(report.truncated);
        assert!(report.matches.iter().all(|m| m.path == "a.txt"));
    }

    #[test]
    fn a_long_line_is_cut_around_the_match_and_crlf_and_bom_do_not_move_columns() {
        let long = format!("{}needle{}", "a".repeat(300), "b".repeat(300));
        let (dir, root) = project(&[("long.txt", &long), ("crlf.txt", "one\r\ntwo needle\r\n")]);
        fs::write(
            dir.path().join("bom.txt"),
            [&[0xEF, 0xBB, 0xBF][..], b"needle\n"].concat(),
        )
        .unwrap();
        let report = search(&root, "needle", false, false).unwrap();
        let by = |name: &str| report.matches.iter().find(|m| m.path == name).unwrap();
        let cut = by("long.txt");
        assert_eq!(cut.column, 301);
        assert!(cut.preview.chars().count() <= PREVIEW_CHARS);
        let chars: Vec<char> = cut.preview.chars().collect();
        let start = cut.preview_start as usize;
        assert_eq!(chars[start..start + 6].iter().collect::<String>(), "needle");
        let crlf = by("crlf.txt");
        assert_eq!(
            (crlf.line, crlf.column, crlf.preview.as_str()),
            (2, 5, "two needle")
        );
        assert_eq!((by("bom.txt").line, by("bom.txt").column), (1, 1));
    }

    #[test]
    fn a_bad_query_is_refused() {
        let (_dir, root) = project(&[("a.txt", "x")]);
        assert!(search(&root, "", true, false).is_err());
        assert!(search(&root, "two\nlines", true, false).is_err());
        assert!(search(&root, "nul\0", true, false).is_err());
        assert!(search(&root, &"a".repeat(MAX_QUERY_CHARS + 1), true, false).is_err());
        assert!(search("/nowhere/at/all", "x", true, false).is_err());
        assert!(search(&root, &"a".repeat(MAX_QUERY_CHARS), true, false).is_ok());
    }
}
