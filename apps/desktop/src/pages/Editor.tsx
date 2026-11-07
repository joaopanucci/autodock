import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  File as FileIcon,
  FileDiff,
  FileMinus,
  FilePlus2,
  FolderClosed,
  FolderOpen,
  FolderPlus,
  GitCompare,
  Link2,
  LoaderCircle,
  RefreshCw,
  Save,
  Search as SearchIcon,
  ShieldAlert,
  SquareTerminal,
  X,
} from "lucide-react";
import { call } from "../ipc";
import { CodeEditor, type CodeEditorHandle } from "../components/CodeEditor";
import { DiffEditor, type DiffEditorHandle } from "../components/DiffEditor";
import { TerminalPanel } from "../components/TerminalPanel";
import { EmptyState, Panel } from "../components/ui";
import { baseName, joinPath, languageFor, parentOf } from "../editor/languages";
import { displayPath } from "../model";
import { useI18n } from "../i18n";
import type {
  EditorEntry,
  EditorFile,
  EditorListing,
  EditorReview,
  EditorRoot,
  EditorSaveReport,
  EditorSearchMatch,
  EditorSearchReport,
  SessionProject,
} from "../types";

const message = (error: unknown) =>
  typeof error === "string" ? error : error instanceof Error ? error.message : String(error);

type Tab = {
  path: string;
  /** `diff`: the original on the left and the editable copy on the right (reviewing an upgrade). */
  kind: "file" | "diff";
  /** Nothing can be typed (a file the upgrade removed). */
  readOnly: boolean;
  dirty: boolean;
  /** Last change on disk, as read or saved: sent back so a change made elsewhere is noticed. */
  modifiedMs: number;
  bom: boolean;
  sensitive: boolean;
};

type Dir = { entries: EditorEntry[]; truncated: boolean; loading: boolean; error: string };

export function Editor({
  sessionProjects,
  reviewRequest,
}: {
  sessionProjects: SessionProject[];
  /** The Java page asked to review the changes of an upgrade. */
  reviewRequest: EditorReview | null;
}) {
  const { t } = useI18n();
  const editor = useRef<CodeEditorHandle>(null);
  const diffEditor = useRef<DiffEditorHandle>(null);

  const [root, setRoot] = useState<EditorRoot | null>(null);
  const [dirs, setDirs] = useState<Record<string, Dir>>({});
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  /** What was last clicked in the tree: new files and folders go inside it. */
  const [selected, setSelected] = useState<{ path: string; dir: boolean }>({ path: "", dir: true });
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [flash, setFlash] = useState("");
  const [saving, setSaving] = useState(false);
  const [conflict, setConflict] = useState<string | null>(null);
  const [closing, setClosing] = useState<string | null>(null);
  const [creating, setCreating] = useState<"file" | "dir" | null>(null);
  const [newName, setNewName] = useState("");
  const [typedPath, setTypedPath] = useState("");
  const [cursor, setCursor] = useState<{ line: number; col: number } | null>(null);
  /** The upgrade being reviewed: its original folder and what changed. */
  const [review, setReview] = useState<EditorReview | null>(null);
  const [view, setView] = useState<"files" | "search" | "changes">("files");
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [includeIgnored, setIncludeIgnored] = useState(false);
  const [searching, setSearching] = useState(false);
  const [results, setResults] = useState<EditorSearchReport | null>(null);
  /** The terminal starts the first time it is asked for; hiding it keeps the shell running. */
  const [terminalStarted, setTerminalStarted] = useState(false);
  const [terminalShown, setTerminalShown] = useState(false);
  /** A folder waits to be opened (or the open one to be closed) until unsaved changes are decided. */
  const [switching, setSwitching] = useState<{ path: string | null; review: EditorReview | null } | null>(null);

  // The callbacks of the editor always see the latest tabs and folder.
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;
  const rootRef = useRef(root);
  rootRef.current = root;

  const rootPath = root?.path ?? "";
  const activeTab = tabs.find((tab) => tab.path === active) ?? null;

  const say = (text: string) => {
    setFlash(text);
    window.setTimeout(() => setFlash((current) => (current === text ? "" : current)), 3500);
  };

  // ───────────── folders ─────────────

  const loadDir = useCallback(async (folder: string, relative: string) => {
    setDirs((current) => ({
      ...current,
      [relative]: { entries: current[relative]?.entries ?? [], truncated: false, loading: true, error: "" },
    }));
    try {
      const listing = await call<EditorListing>("editor_list", { root: folder, path: relative });
      setDirs((current) => ({
        ...current,
        [relative]: { entries: listing.entries, truncated: listing.truncated, loading: false, error: "" },
      }));
    } catch (e) {
      setDirs((current) => ({
        ...current,
        [relative]: { entries: [], truncated: false, loading: false, error: message(e) },
      }));
    }
  }, []);

  const closeAllTabs = () => {
    tabsRef.current.forEach((tab) => (tab.kind === "diff" ? diffEditor : editor).current?.close(tab.path));
    editor.current?.show(null);
    diffEditor.current?.show(null);
  };

  /** Opens a folder (`null` closes the open one), asking first when unsaved changes would be lost. */
  function requestOpen(path: string | null, reviewed: EditorReview | null = null) {
    if (tabsRef.current.some((tab) => tab.dirty)) setSwitching({ path, review: reviewed });
    else if (path === null) closeFolder();
    else void openFolder(path, reviewed);
  }

  async function openFolder(path: string, reviewed: EditorReview | null = null) {
    setError("");
    setSwitching(null);
    try {
      const info = await call<EditorRoot>("editor_open_root", { path });
      closeAllTabs();
      setReview(reviewed);
      setView(reviewed ? "changes" : "files");
      setResults(null);
      setQuery("");
      setRoot(info);
      setDirs({});
      setExpanded(new Set([""]));
      setSelected({ path: "", dir: true });
      setTabs([]);
      setActive(null);
      setConflict(null);
      setClosing(null);
      setCursor(null);
      await loadDir(info.path, "");
    } catch (e) {
      setError(message(e));
    }
  }

  async function pickFolder() {
    try {
      const folder = await open({ directory: true, multiple: false, title: t("editor.pick.dialog") });
      if (folder) await openFolder(folder);
    } catch (e) {
      setError(message(e));
    }
  }

  function closeFolder() {
    setSwitching(null);
    closeAllTabs();
    setReview(null);
    setResults(null);
    setRoot(null);
    setTabs([]);
    setActive(null);
    setClosing(null);
    setConflict(null);
    setTerminalStarted(false);
    setTerminalShown(false);
  }

  /** Lists again the folders that are open in the tree (a terminal command may have changed them). */
  function refreshTree() {
    if (!root) return;
    new Set([...expanded, ""]).forEach((relative) => void loadDir(root.path, relative));
  }

  function toggleDir(relative: string) {
    setSelected({ path: relative, dir: true });
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(relative)) next.delete(relative);
      else next.add(relative);
      return next;
    });
    if (!dirs[relative]) void loadDir(rootPath, relative);
  }

  // ───────────── files ─────────────

  const activate = useCallback((path: string | null, kind: "file" | "diff" = "file") => {
    setActive(path);
    editor.current?.show(kind === "file" ? path : null);
    diffEditor.current?.show(kind === "diff" ? path : null);
    setConflict(null);
  }, []);

  async function openFile(relative: string) {
    setSelected({ path: relative, dir: false });
    setError("");
    const existing = tabsRef.current.find((tab) => tab.path === relative);
    if (existing) {
      activate(relative, existing.kind);
      return;
    }
    try {
      const file = await call<EditorFile>("editor_read", { root: rootPath, path: relative });
      editor.current?.open(file.path, file.text, languageFor(file.path));
      setTabs((current) => [
        ...current,
        {
          path: file.path,
          kind: "file",
          readOnly: false,
          dirty: false,
          modifiedMs: file.modifiedMs,
          bom: file.bom,
          sensitive: file.sensitive,
        },
      ]);
      activate(file.path);
    } catch (e) {
      setError(message(e));
    }
  }

  const setDirty = useCallback((path: string, dirty: boolean) => {
    setTabs((current) =>
      current.map((tab) => (tab.path === path && tab.dirty !== dirty ? { ...tab, dirty } : tab)),
    );
  }, []);

  /** Saves a file; `overwrite` keeps going over a change made elsewhere. Returns whether it saved. */
  const save = useCallback(
    async (path: string, overwrite = false): Promise<boolean> => {
      const tab = tabsRef.current.find((item) => item.path === path);
      const text = (tab?.kind === "diff" ? diffEditor : editor).current?.value(path);
      if (!tab || text == null || !rootRef.current) return false;
      setSaving(true);
      setError("");
      try {
        const report = await call<EditorSaveReport>("editor_write", {
          root: rootRef.current.path,
          path,
          text,
          bom: tab.bom,
          expectedMs: tab.modifiedMs,
          overwrite,
        });
        setTabs((current) =>
          current.map((item) => (item.path === path ? { ...item, modifiedMs: report.modifiedMs } : item)),
        );
        (tab.kind === "diff" ? diffEditor : editor).current?.markSaved(path);
        setConflict(null);
        say(t("editor.saved", { name: baseName(path) }));
        return true;
      } catch (e) {
        const text = message(e);
        if (text.startsWith("conflict")) setConflict(path);
        else setError(text);
        return false;
      } finally {
        setSaving(false);
      }
    },
    // `t` only changes with the language; the saved message is built when it is shown.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  async function reloadFromDisk(path: string) {
    try {
      const file = await call<EditorFile>("editor_read", { root: rootPath, path });
      const kind = tabsRef.current.find((tab) => tab.path === path)?.kind;
      (kind === "diff" ? diffEditor : editor).current?.replace(path, file.text);
      setTabs((current) =>
        current.map((tab) => (tab.path === path ? { ...tab, modifiedMs: file.modifiedMs, bom: file.bom } : tab)),
      );
      setConflict(null);
      say(t("editor.reloaded", { name: baseName(path) }));
    } catch (e) {
      setError(message(e));
    }
  }

  function removeTab(path: string) {
    const closingKind = tabsRef.current.find((tab) => tab.path === path)?.kind;
    (closingKind === "diff" ? diffEditor : editor).current?.close(path);
    const index = tabsRef.current.findIndex((tab) => tab.path === path);
    const rest = tabsRef.current.filter((tab) => tab.path !== path);
    setTabs(rest);
    setClosing(null);
    if (active === path) {
      const next = rest[Math.min(index, rest.length - 1)];
      activate(next?.path ?? null, next?.kind);
    }
  }

  function closeTab(path: string) {
    const tab = tabsRef.current.find((item) => item.path === path);
    if (tab?.dirty) setClosing(path);
    else removeTab(path);
  }

  // ───────────── search ─────────────

  async function runSearch() {
    if (query.length < 2 || searching || !root) return;
    setSearching(true);
    setError("");
    try {
      setResults(
        await call<EditorSearchReport>("editor_search", {
          root: rootPath,
          query,
          caseSensitive,
          includeIgnored,
        }),
      );
    } catch (e) {
      setError(message(e));
    } finally {
      setSearching(false);
    }
  }

  /** Opens the file of a match (or shows its tab again) and selects the match. */
  async function openMatch(match: EditorSearchMatch) {
    const kind = tabsRef.current.find((tab) => tab.path === match.path)?.kind ?? "file";
    await openFile(match.path);
    (kind === "diff" ? diffEditor : editor).current?.reveal(match.path, match.line, match.column, match.length);
  }

  // ───────────── reviewing an upgrade ─────────────

  /** Opens a file of the upgrade: a changed one side by side, an added one as a normal file. */
  async function openChange(relative: string, status: "modified" | "added" | "removed") {
    if (!review) return;
    setSelected({ path: relative, dir: false });
    setError("");
    if (status === "added") {
      await openFile(relative);
      return;
    }
    const existing = tabsRef.current.find((tab) => tab.path === relative);
    // A side-by-side tab is just shown again; a normal tab with unsaved work is never replaced.
    if (existing && (existing.kind === "diff" || existing.dirty)) {
      activate(relative, existing.kind);
      return;
    }
    try {
      const original = await call<EditorFile>("editor_read", { root: review.original, path: relative });
      const copy =
        status === "removed" ? null : await call<EditorFile>("editor_read", { root: rootPath, path: relative });
      if (existing) removeTab(relative);
      diffEditor.current?.open(relative, original.text, copy?.text ?? "", languageFor(relative), status === "removed");
      setTabs((current) => [
        ...current,
        {
          path: relative,
          kind: "diff",
          readOnly: status === "removed",
          dirty: false,
          modifiedMs: copy?.modifiedMs ?? 0,
          bom: copy?.bom ?? false,
          sensitive: false,
        },
      ]);
      activate(relative, "diff");
    } catch (e) {
      setError(message(e));
    }
  }

  // The Java page asks, once per upgrade, to review its changes.
  const handledReview = useRef<number | null>(null);
  useEffect(() => {
    if (!reviewRequest || handledReview.current === reviewRequest.id) return;
    handledReview.current = reviewRequest.id;
    requestOpen(reviewRequest.copy, reviewRequest);
    // requestOpen reads the tabs through a ref and only calls setters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reviewRequest]);

  // ───────────── new file / folder ─────────────

  const createParent = selected.dir ? selected.path : parentOf(selected.path);

  async function createEntry() {
    const kind = creating;
    const name = newName.trim();
    if (!kind || !name) return;
    setError("");
    try {
      await call<EditorEntry>("editor_create", { root: rootPath, parent: createParent, name, kind });
      setCreating(null);
      setNewName("");
      setExpanded((current) => new Set(current).add(createParent));
      await loadDir(rootPath, createParent);
      if (kind === "file") await openFile(joinPath(createParent, name));
    } catch (e) {
      setError(message(e));
    }
  }

  // ───────────── session suggestions ─────────────

  const suggestions = sessionProjects.slice(0, 12).map((p) => displayPath(p.scan.project));

  // Keep the editor's own shortcut and the page's save button on the same code path.
  const onEditorSave = useCallback((path: string) => void save(path), [save]);
  const onCursor = useCallback((line: number, col: number) => setCursor({ line, col }), []);

  useEffect(() => {
    if (!active) setCursor(null);
  }, [active]);

  // ───────────── rendering ─────────────

  const renderDir = (relative: string, depth: number): ReactNode => {
    const dir = dirs[relative];
    if (!dir) return null;
    return (
      <>
        {dir.entries.map((entry) => {
          const path = joinPath(relative, entry.name);
          if (entry.kind === "dir") {
            const open = expanded.has(path);
            return (
              <li key={path} role="none">
                <button
                  type="button"
                  role="treeitem"
                  aria-expanded={open}
                  aria-label={t(open ? "editor.explorer.collapse" : "editor.explorer.expand", { name: entry.name })}
                  className={`editor-row ${selected.path === path ? "is-selected" : ""}`}
                  style={{ paddingLeft: 10 + depth * 14 }}
                  onClick={() => toggleDir(path)}
                >
                  {open ? <ChevronDown size={14} aria-hidden="true" /> : <ChevronRight size={14} aria-hidden="true" />}
                  {open ? <FolderOpen size={15} aria-hidden="true" /> : <FolderClosed size={15} aria-hidden="true" />}
                  <span>{entry.name}</span>
                </button>
                {open && <ul role="group">{renderChildren(path, depth + 1)}</ul>}
              </li>
            );
          }
          if (entry.kind === "file") {
            return (
              <li key={path} role="none">
                <button
                  type="button"
                  role="treeitem"
                  className={`editor-row ${active === path ? "is-active" : ""} ${selected.path === path ? "is-selected" : ""}`}
                  style={{ paddingLeft: 10 + depth * 14 + 18 }}
                  onClick={() => void openFile(path)}
                >
                  <FileIcon size={15} aria-hidden="true" />
                  <span>{entry.name}</span>
                </button>
              </li>
            );
          }
          return (
            <li key={path} role="none">
              <span
                className="editor-row is-link"
                style={{ paddingLeft: 10 + depth * 14 + 18 }}
                title={t("editor.explorer.link")}
              >
                <Link2 size={15} aria-hidden="true" />
                <span>{entry.name}</span>
              </span>
            </li>
          );
        })}
      </>
    );
  };

  const renderChildren = (relative: string, depth: number): ReactNode => {
    const dir = dirs[relative];
    const pad = { paddingLeft: 10 + depth * 14 + 18 };
    if (!dir || dir.loading) {
      return (
        <li className="editor-note" style={pad}>
          <LoaderCircle className="spin" size={12} aria-hidden="true" /> {t("editor.explorer.loading")}
        </li>
      );
    }
    if (dir.error) {
      return (
        <li className="editor-note warn" style={pad}>
          {dir.error}
        </li>
      );
    }
    return (
      <>
        {dir.entries.length === 0 && (
          <li className="editor-note" style={pad}>
            {t("editor.explorer.empty")}
          </li>
        )}
        {renderDir(relative, depth)}
        {dir.truncated && (
          <li className="editor-note warn" style={pad}>
            {t("editor.explorer.truncated")}
          </li>
        )}
      </>
    );
  };

  const renderSearch = (): ReactNode => {
    const groups: { path: string; hits: EditorSearchMatch[] }[] = [];
    for (const hit of results?.matches ?? []) {
      const last = groups[groups.length - 1];
      if (last && last.path === hit.path) last.hits.push(hit);
      else groups.push({ path: hit.path, hits: [hit] });
    }
    return (
      <div className="editor-search">
        <form
          className="editor-search-form"
          onSubmit={(event) => {
            event.preventDefault();
            void runSearch();
          }}
        >
          <div className="editor-search-field">
            <input
              value={query}
              placeholder={t("editor.search.placeholder")}
              aria-label={t("editor.search.label")}
              spellCheck={false}
              onChange={(event) => setQuery(event.target.value)}
            />
            <button
              type="button"
              className={`icon-button editor-case ${caseSensitive ? "is-on" : ""}`}
              aria-pressed={caseSensitive}
              title={t("editor.search.case")}
              aria-label={t("editor.search.case")}
              onClick={() => setCaseSensitive((value) => !value)}
            >
              Aa
            </button>
          </div>
          <label className="editor-search-option">
            <input
              type="checkbox"
              checked={includeIgnored}
              onChange={(event) => setIncludeIgnored(event.target.checked)}
            />
            <span>{t("editor.search.ignored")}</span>
          </label>
          <button type="submit" className="button primary" disabled={query.length < 2 || searching}>
            {searching ? <LoaderCircle className="spin" size={14} aria-hidden="true" /> : <SearchIcon size={14} aria-hidden="true" />}
            {searching ? t("editor.search.running") : t("editor.search.button")}
          </button>
          {query.length > 0 && query.length < 2 && <small className="field-hint">{t("editor.search.short")}</small>}
        </form>
        {results && (
          <div className="editor-search-results" role="status">
            <p className="field-hint">
              {results.matches.length === 0
                ? t("editor.search.none")
                : t("editor.search.summary", { count: results.matches.length, files: results.filesWithMatches })}
            </p>
            {results.truncated && (
              <p className="editor-note warn">{t("editor.search.truncated", { count: results.matches.length })}</p>
            )}
            {results.filesSkipped > 0 && (
              <p className="editor-note">{t("editor.search.skipped", { count: results.filesSkipped })}</p>
            )}
            {groups.map((group) => (
              <div key={group.path}>
                <p className="editor-changes-title" title={group.path}>
                  {group.path} ({group.hits.length})
                </p>
                <ul>
                  {group.hits.map((hit) => {
                    const chars = Array.from(hit.preview);
                    return (
                      <li key={`${hit.line}:${hit.column}`}>
                        <button
                          type="button"
                          className="editor-hit"
                          onClick={() => void openMatch(hit)}
                          aria-label={`${group.path}:${hit.line}:${hit.column}`}
                        >
                          <span className="editor-hit-line">{hit.line}</span>
                          <span className="editor-hit-text">
                            {chars.slice(0, hit.previewStart).join("")}
                            <mark>{chars.slice(hit.previewStart, hit.previewStart + hit.length).join("")}</mark>
                            {chars.slice(hit.previewStart + hit.length).join("")}
                          </span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  };

  const changeIcons = { modified: FileDiff, added: FilePlus2, removed: FileMinus } as const;

  const renderChanges = (): ReactNode => {
    if (!review) return null;
    const groups = [
      ["modified", review.changes.modified],
      ["added", review.changes.added],
      ["removed", review.changes.removed],
    ] as const;
    return (
      <div className="editor-changes">
        {review.changes.total === 0 && <p className="editor-note">{t("editor.changes.empty")}</p>}
        {groups.map(([kind, files]) => {
          const Icon = changeIcons[kind];
          return (
            files.length > 0 && (
              <div key={kind}>
                <p className="editor-changes-title">
                  {t(`editor.changes.${kind}` as "editor.changes.modified")} ({files.length})
                </p>
                <ul>
                  {files.map((file) => (
                    <li key={file}>
                      <button
                        type="button"
                        className={`editor-row ${active === file ? "is-active" : ""}`}
                        title={file}
                        style={{ paddingLeft: 10 }}
                        onClick={() => void openChange(file, kind)}
                      >
                        <Icon size={15} aria-hidden="true" />
                        <span>{file}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )
          );
        })}
        {review.changes.truncated && <p className="editor-note warn">{t("editor.changes.truncated")}</p>}
      </div>
    );
  };

  const where = createParent || root?.name || "";

  return (
    <div className="editor-page">
      {error && (
        <div className="feedback error" role="alert">
          <AlertTriangle size={19} aria-hidden="true" />
          <div>
            <strong>{t("wizard.error.title")}</strong>
            <p>{error}</p>
          </div>
          <button className="icon-button" aria-label={t("app.closeError")} onClick={() => setError("")}>
            <X size={16} />
          </button>
        </div>
      )}

      {!root && (
        <Panel
          title={t("editor.pick.title")}
          eyebrow={t("editor.pick.eyebrow")}
          action={
            <button type="button" className="button primary" onClick={() => void pickFolder()}>
              <FolderOpen size={16} aria-hidden="true" /> {t("editor.pick.button")}
            </button>
          }
        >
          <p className="section-description">{t("editor.pick.body")}</p>
          <form
            className="editor-typed"
            onSubmit={(event) => {
              event.preventDefault();
              if (typedPath.trim()) void openFolder(typedPath.trim());
            }}
          >
            <label>
              <span className="field-label">{t("editor.pick.typed")}</span>
              <input
                value={typedPath}
                placeholder={t("editor.pick.typedPlaceholder")}
                spellCheck={false}
                onChange={(event) => setTypedPath(event.target.value)}
              />
            </label>
            <button type="submit" className="button secondary" disabled={!typedPath.trim()}>
              {t("editor.pick.typedOpen")}
            </button>
          </form>
          {suggestions.length > 0 && (
            <div className="expo-suggest">
              <p className="field-label">{t("editor.suggest.title")}</p>
              <p className="field-hint">{t("editor.suggest.body")}</p>
              <ul className="expo-projects">
                {suggestions.map((path) => (
                  <li key={path}>
                    <div className="expo-project-main">
                      <strong>{baseName(path.replace(/\\/g, "/"))}</strong>
                      <small className="mono">{path}</small>
                    </div>
                    <div className="expo-project-actions">
                      <button type="button" className="button secondary" onClick={() => void openFolder(path)}>
                        {t("editor.suggest.use")}
                      </button>
                    </div>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </Panel>
      )}

      {/* The editor stays mounted while a folder is open, so its tabs and undo history survive. */}
      <div className="editor-shell" hidden={!root}>
        <aside className="editor-explorer" aria-label={t("editor.explorer.aria")}>
          <div className="editor-explorer-head">
            <strong title={root ? displayPath(root.path) : ""}>{root?.name ?? t("editor.explorer.title")}</strong>
            <div className="editor-explorer-actions">
              <button
                type="button"
                className="icon-button"
                title={t("editor.explorer.newFile")}
                aria-label={t("editor.explorer.newFile")}
                onClick={() => {
                  setCreating("file");
                  setNewName("");
                }}
              >
                <FilePlus2 size={16} />
              </button>
              <button
                type="button"
                className="icon-button"
                title={t("editor.explorer.newFolder")}
                aria-label={t("editor.explorer.newFolder")}
                onClick={() => {
                  setCreating("dir");
                  setNewName("");
                }}
              >
                <FolderPlus size={16} />
              </button>
              <button
                type="button"
                className="icon-button"
                title={t("editor.explorer.refresh")}
                aria-label={t("editor.explorer.refresh")}
                onClick={refreshTree}
              >
                <RefreshCw size={16} />
              </button>
              <button
                type="button"
                className="icon-button"
                title={t("editor.explorer.close")}
                aria-label={t("editor.explorer.close")}
                onClick={() => requestOpen(null)}
              >
                <X size={16} />
              </button>
            </div>
          </div>
          {creating && (
            <form
              className="editor-create"
              onSubmit={(event) => {
                event.preventDefault();
                void createEntry();
              }}
            >
              <label>
                <span className="field-hint">
                  {t(creating === "file" ? "editor.create.file" : "editor.create.folder")} (
                  {t("editor.create.in", { where })})
                </span>
                <input
                  autoFocus
                  value={newName}
                  aria-label={t(creating === "file" ? "editor.create.file" : "editor.create.folder")}
                  onChange={(event) => setNewName(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Escape") setCreating(null);
                  }}
                />
              </label>
              <div className="editor-create-actions">
                <button type="submit" className="button primary" disabled={!newName.trim()}>
                  {t("editor.create.confirm")}
                </button>
                <button type="button" className="button secondary" onClick={() => setCreating(null)}>
                  {t("editor.create.cancel")}
                </button>
              </div>
            </form>
          )}
          <div className="editor-views" role="tablist" aria-label={t("editor.view.aria")}>
            <button
              type="button"
              role="tab"
              aria-selected={view === "files"}
              className={view === "files" ? "is-active" : ""}
              onClick={() => setView("files")}
            >
              {t("editor.view.files")}
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={view === "search"}
              className={view === "search" ? "is-active" : ""}
              onClick={() => setView("search")}
            >
              {t("editor.view.search")}
            </button>
            {review && (
              <button
                type="button"
                role="tab"
                aria-selected={view === "changes"}
                className={view === "changes" ? "is-active" : ""}
                onClick={() => setView("changes")}
              >
                {t("editor.view.changes", { count: review.changes.total })}
              </button>
            )}
          </div>
          {view === "changes" && review ? (
            renderChanges()
          ) : view === "search" ? (
            renderSearch()
          ) : (
            <ul className="editor-tree" role="tree" aria-label={t("editor.explorer.aria")}>
              {renderChildren("", 0)}
            </ul>
          )}
        </aside>

        <section className="editor-main">
          <div className="editor-tabs" role="tablist" aria-label={t("editor.tabs.aria")}>
            {tabs.map((tab) => (
              <div key={tab.path} className={`editor-tab ${active === tab.path ? "is-active" : ""}`}>
                <button
                  type="button"
                  role="tab"
                  aria-selected={active === tab.path}
                  title={tab.path}
                  className="editor-tab-name"
                  onClick={() => activate(tab.path, tab.kind)}
                  onAuxClick={(event) => {
                    if (event.button === 1) closeTab(tab.path);
                  }}
                >
                  {tab.kind === "diff" && <FileDiff size={13} aria-hidden="true" />}
                  {baseName(tab.path)}
                  {tab.dirty && (
                    <span className="editor-dirty" role="img" aria-label={t("editor.tab.unsaved")} title={t("editor.tab.unsaved")}>
                      ●
                    </span>
                  )}
                </button>
                <button
                  type="button"
                  className="editor-tab-close"
                  aria-label={t("editor.tab.close", { name: baseName(tab.path) })}
                  onClick={() => closeTab(tab.path)}
                >
                  <X size={13} />
                </button>
              </div>
            ))}
            <div className="editor-tools">
              <button
                type="button"
                className="button secondary"
                aria-pressed={terminalShown}
                title={t("terminal.toggle.hint")}
                onClick={() => {
                  setTerminalStarted(true);
                  setTerminalShown((shown) => !shown);
                }}
              >
                <SquareTerminal size={14} aria-hidden="true" />
                {t("terminal.toggle")}
              </button>
              {activeTab && (
                <button
                  type="button"
                  className="button secondary"
                  disabled={saving || !activeTab.dirty}
                  onClick={() => void save(activeTab.path)}
                >
                  {saving ? <LoaderCircle className="spin" size={14} aria-hidden="true" /> : <Save size={14} aria-hidden="true" />}
                  {saving ? t("editor.saving") : t("editor.save")}
                </button>
              )}
            </div>
          </div>

          {activeTab?.sensitive && (
            <div className="notice warning editor-notice" role="note">
              <ShieldAlert size={18} aria-hidden="true" />
              <p>{t("editor.sensitive")}</p>
            </div>
          )}

          {conflict && (
            <div className="notice warning editor-notice" role="alert">
              <AlertTriangle size={18} aria-hidden="true" />
              <div>
                <strong>{t("editor.conflict.title")}</strong>
                <p>{t("editor.conflict.body", { path: conflict })}</p>
                <div className="editor-create-actions">
                  <button type="button" className="button primary" onClick={() => void save(conflict, true)}>
                    {t("editor.conflict.overwrite")}
                  </button>
                  <button type="button" className="button secondary" onClick={() => void reloadFromDisk(conflict)}>
                    {t("editor.conflict.reload")}
                  </button>
                  <button type="button" className="button secondary" onClick={() => setConflict(null)}>
                    {t("editor.conflict.cancel")}
                  </button>
                </div>
              </div>
            </div>
          )}

          {closing && (
            <div className="notice warning editor-notice" role="alertdialog" aria-label={t("editor.closing.title", { name: baseName(closing) })}>
              <AlertTriangle size={18} aria-hidden="true" />
              <div>
                <strong>{t("editor.closing.title", { name: baseName(closing) })}</strong>
                <div className="editor-create-actions">
                  <button
                    type="button"
                    className="button primary"
                    onClick={() => void save(closing).then((saved) => saved && removeTab(closing))}
                  >
                    {t("editor.closing.save")}
                  </button>
                  <button type="button" className="button secondary" onClick={() => removeTab(closing)}>
                    {t("editor.closing.discard")}
                  </button>
                  <button type="button" className="button secondary" onClick={() => setClosing(null)}>
                    {t("editor.closing.cancel")}
                  </button>
                </div>
              </div>
            </div>
          )}

          {switching && (
            <div className="notice warning editor-notice" role="alertdialog" aria-label={t("editor.switch.title")}>
              <AlertTriangle size={18} aria-hidden="true" />
              <div>
                <strong>{t("editor.switch.title")}</strong>
                <p>{t("editor.switch.body")}</p>
                <div className="editor-create-actions">
                  <button
                    type="button"
                    className="button primary"
                    onClick={() =>
                      switching.path === null ? closeFolder() : void openFolder(switching.path, switching.review)
                    }
                  >
                    {t("editor.switch.discard")}
                  </button>
                  <button type="button" className="button secondary" onClick={() => setSwitching(null)}>
                    {t("editor.switch.cancel")}
                  </button>
                </div>
              </div>
            </div>
          )}

          {review && (
            <div className="notice editor-notice editor-review" role="note">
              <GitCompare size={18} aria-hidden="true" />
              <p>{t("editor.review.banner", { name: baseName(review.original.replace(/\\/g, "/")) })}</p>
              <button
                type="button"
                className="button secondary"
                onClick={() => {
                  setReview(null);
                  setView("files");
                }}
              >
                {t("editor.review.stop")}
              </button>
            </div>
          )}

          <div className="editor-stage">
            <div className="editor-pane" hidden={activeTab?.kind === "diff"}>
              <CodeEditor
                ref={editor}
                ariaLabel={t("page.editor.title")}
                onDirty={setDirty}
                onSave={onEditorSave}
                onCursor={onCursor}
              />
            </div>
            <div className="editor-pane" hidden={activeTab?.kind !== "diff"}>
              <div className="editor-diff-head">
                <span>{t("editor.diff.original")}</span>
                <span>{activeTab?.readOnly ? t("editor.diff.removed") : t("editor.diff.copy")}</span>
              </div>
              <div className="editor-diff-body">
                <DiffEditor
                  ref={diffEditor}
                  ariaLabel={t("editor.diff.aria")}
                  onDirty={setDirty}
                  onSave={onEditorSave}
                  onCursor={onCursor}
                />
              </div>
            </div>
            {tabs.length === 0 && (
              <div className="editor-empty">
                <EmptyState title={t("editor.empty.title")} description={t("editor.empty.body")} />
              </div>
            )}
          </div>

          {root && terminalStarted && (
            <TerminalPanel key={root.path} root={root.path} hidden={!terminalShown} onHide={() => setTerminalShown(false)} />
          )}

          <div className="editor-status" role="status">
            <span>{activeTab ? languageFor(activeTab.path) : ""}</span>
            <span>{cursor && activeTab ? t("editor.status.position", { line: cursor.line, col: cursor.col }) : ""}</span>
            <span>{flash || (activeTab?.dirty ? t("editor.status.unsaved") : "")}</span>
          </div>
        </section>
      </div>
    </div>
  );
}
