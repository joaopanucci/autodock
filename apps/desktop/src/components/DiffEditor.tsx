import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import type * as MonacoTypes from "monaco-editor";
import { loadMonaco, type Monaco } from "../editor/monacoSetup";

/** What the page can ask of the side-by-side editor. Files are told apart by an id (their path). */
export type DiffEditorHandle = {
  /**
   * Adds a file: `original` on the left (never editable) and `modified` on the right (editable,
   * unless `readOnly`). Nothing changes on screen until `show`.
   */
  open: (id: string, original: string, modified: string, language: string, readOnly: boolean) => void;
  show: (id: string | null) => void;
  close: (id: string) => void;
  /** The current text of the right side. */
  value: (id: string) => string | null;
  /** Replaces the right side with what is on disk and treats it as saved. */
  replace: (id: string, text: string) => void;
  markSaved: (id: string) => void;
  /** Selects a match on the right side and scrolls it into view (the file must be open). */
  reveal: (id: string, line: number, column: number, length: number) => void;
};

type Props = {
  ariaLabel: string;
  onDirty: (id: string, dirty: boolean) => void;
  onSave: (id: string) => void;
  onCursor: (line: number, column: number) => void;
};

type Entry = {
  original: MonacoTypes.editor.ITextModel;
  modified: MonacoTypes.editor.ITextModel;
  readOnly: boolean;
  saved: number;
};

const dark = () => document.documentElement.dataset.theme === "dark";

/** Monaco's side-by-side editor, with one pair of text models per file. */
export const DiffEditor = forwardRef<DiffEditorHandle, Props>(function DiffEditor(
  { ariaLabel, onDirty, onSave, onCursor },
  ref,
) {
  const container = useRef<HTMLDivElement>(null);
  const monacoRef = useRef<Monaco | null>(null);
  const editorRef = useRef<MonacoTypes.editor.IStandaloneDiffEditor | null>(null);
  const entries = useRef(new Map<string, Entry>());
  const current = useRef<string | null>(null);
  const waiting = useRef<Array<() => void>>([]);
  const callbacks = useRef({ onDirty, onSave, onCursor });
  callbacks.current = { onDirty, onSave, onCursor };

  const whenReady = (action: () => void) => {
    if (monacoRef.current && editorRef.current) action();
    else waiting.current.push(action);
  };

  useEffect(() => {
    let disposed = false;
    let observer: MutationObserver | null = null;
    void loadMonaco().then((monaco) => {
      if (disposed || !container.current) return;
      monacoRef.current = monaco;
      const editor = monaco.editor.createDiffEditor(container.current, {
        theme: dark() ? "vs-dark" : "vs",
        automaticLayout: true,
        renderSideBySide: true,
        // Below 900 px Monaco switches to an inline view by itself; the review is side by side.
        useInlineViewWhenSpaceIsLimited: false,
        originalEditable: false,
        enableSplitViewResizing: true,
        ignoreTrimWhitespace: false,
        fontSize: 13,
        scrollBeyondLastLine: false,
        renderOverviewRuler: true,
        ariaLabel,
      });
      editorRef.current = editor;
      const modified = editor.getModifiedEditor();
      // `addAction`, not `addCommand`: see CodeEditor (a command binds the key for every editor).
      modified.addAction({
        id: "autodock.save",
        label: "Save",
        keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS],
        run: () => {
          if (current.current) callbacks.current.onSave(current.current);
        },
      });
      modified.onDidChangeCursorPosition((event) => {
        callbacks.current.onCursor(event.position.lineNumber, event.position.column);
      });
      observer = new MutationObserver(() => monaco.editor.setTheme(dark() ? "vs-dark" : "vs"));
      observer.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["data-theme"],
      });
      const queued = waiting.current;
      waiting.current = [];
      queued.forEach((action) => action());
    });
    return () => {
      disposed = true;
      observer?.disconnect();
      editorRef.current?.dispose();
      entries.current.forEach((entry) => {
        entry.original.dispose();
        entry.modified.dispose();
      });
      entries.current.clear();
      editorRef.current = null;
      monacoRef.current = null;
    };
    // The editor is created once; its label only matters at that time.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useImperativeHandle(ref, () => ({
    open(id, original, modified, language, readOnly) {
      whenReady(() => {
        const monaco = monacoRef.current!;
        if (entries.current.has(id)) return;
        const left = monaco.editor.createModel(
          original,
          language,
          monaco.Uri.from({ scheme: "autodock-original", path: `/${id}` }),
        );
        const right = monaco.editor.createModel(
          modified,
          language,
          monaco.Uri.from({ scheme: "autodock-copy", path: `/${id}` }),
        );
        const entry: Entry = { original: left, modified: right, readOnly, saved: right.getAlternativeVersionId() };
        right.onDidChangeContent(() => {
          callbacks.current.onDirty(id, right.getAlternativeVersionId() !== entry.saved);
        });
        entries.current.set(id, entry);
      });
    },
    show(id) {
      whenReady(() => {
        const editor = editorRef.current!;
        current.current = id;
        const entry = id ? entries.current.get(id) : undefined;
        if (!entry) {
          editor.setModel(null);
          return;
        }
        editor.getModifiedEditor().updateOptions({ readOnly: entry.readOnly });
        editor.setModel({ original: entry.original, modified: entry.modified });
        editor.getModifiedEditor().focus();
      });
    },
    close(id) {
      whenReady(() => {
        const entry = entries.current.get(id);
        if (!entry) return;
        if (current.current === id) {
          editorRef.current!.setModel(null);
          current.current = null;
        }
        entry.original.dispose();
        entry.modified.dispose();
        entries.current.delete(id);
      });
    },
    value(id) {
      return entries.current.get(id)?.modified.getValue() ?? null;
    },
    replace(id, text) {
      whenReady(() => {
        const entry = entries.current.get(id);
        if (!entry) return;
        entry.modified.setValue(text);
        entry.saved = entry.modified.getAlternativeVersionId();
        callbacks.current.onDirty(id, false);
      });
    },
    markSaved(id) {
      const entry = entries.current.get(id);
      if (!entry) return;
      entry.saved = entry.modified.getAlternativeVersionId();
      callbacks.current.onDirty(id, false);
    },
    reveal(id, line, column, length) {
      whenReady(() => {
        if (current.current !== id) return;
        const modified = editorRef.current!.getModifiedEditor();
        const range = { startLineNumber: line, startColumn: column, endLineNumber: line, endColumn: column + length };
        modified.setSelection(range);
        modified.revealRangeInCenterIfOutsideViewport(range);
        modified.focus();
      });
    },
  }));

  return <div ref={container} className="code-editor" data-testid="diff-editor" />;
});
