import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import type * as MonacoTypes from "monaco-editor";
import { loadMonaco, type Monaco } from "../editor/monacoSetup";

/** What the page can ask of the editor. Files are told apart by an id (their path). */
export type CodeEditorHandle = {
  /** Adds a file (a model with its own undo history); nothing changes on screen until `show`. */
  open: (id: string, text: string, language: string) => void;
  /** Shows a file, or nothing (`null`). */
  show: (id: string | null) => void;
  /** Forgets a file. */
  close: (id: string) => void;
  /** The current text of a file. */
  value: (id: string) => string | null;
  /** Replaces the text of a file with what is on disk and treats it as saved. */
  replace: (id: string, text: string) => void;
  /** The current text is what is on disk. */
  markSaved: (id: string) => void;
  focus: () => void;
  /** Selects a match and scrolls it into view (the file must be open). */
  reveal: (id: string, line: number, column: number, length: number) => void;
};

type Props = {
  ariaLabel: string;
  onDirty: (id: string, dirty: boolean) => void;
  onSave: (id: string) => void;
  onCursor: (line: number, column: number) => void;
};

type Entry = {
  model: MonacoTypes.editor.ITextModel;
  /** Version of the model when it was last saved. */
  saved: number;
  state: MonacoTypes.editor.ICodeEditorViewState | null;
};

const dark = () => document.documentElement.dataset.theme === "dark";

/** One Monaco editor, with one text model per open file so each keeps its own undo history. */
export const CodeEditor = forwardRef<CodeEditorHandle, Props>(function CodeEditor(
  { ariaLabel, onDirty, onSave, onCursor },
  ref,
) {
  const container = useRef<HTMLDivElement>(null);
  const monacoRef = useRef<Monaco | null>(null);
  const editorRef = useRef<MonacoTypes.editor.IStandaloneCodeEditor | null>(null);
  const entries = useRef(new Map<string, Entry>());
  const current = useRef<string | null>(null);
  /** Calls made before Monaco finished loading wait here. */
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
      const editor = monaco.editor.create(container.current, {
        model: null,
        theme: dark() ? "vs-dark" : "vs",
        automaticLayout: true,
        minimap: { enabled: true },
        fontSize: 13,
        scrollBeyondLastLine: false,
        renderWhitespace: "selection",
        tabSize: 4,
        ariaLabel,
      });
      editorRef.current = editor;
      // `addAction`, not `addCommand`: a command binds the key for every editor on the page, so the
      // side-by-side editor would take the shortcut of this one.
      editor.addAction({
        id: "autodock.save",
        label: "Save",
        keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS],
        run: () => {
          if (current.current) callbacks.current.onSave(current.current);
        },
      });
      editor.onDidChangeCursorPosition((event) => {
        callbacks.current.onCursor(event.position.lineNumber, event.position.column);
      });
      // The app theme changes the editor theme, also when "system" follows the computer.
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
      entries.current.forEach((entry) => entry.model.dispose());
      entries.current.clear();
      editorRef.current = null;
      monacoRef.current = null;
    };
    // The editor is created once; its label only matters at that time.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useImperativeHandle(ref, () => ({
    open(id, text, language) {
      whenReady(() => {
        const monaco = monacoRef.current!;
        if (entries.current.has(id)) return;
        const model = monaco.editor.createModel(text, language, monaco.Uri.from({ scheme: "autodock", path: `/${id}` }));
        const entry: Entry = { model, saved: model.getAlternativeVersionId(), state: null };
        model.onDidChangeContent(() => {
          callbacks.current.onDirty(id, model.getAlternativeVersionId() !== entry.saved);
        });
        entries.current.set(id, entry);
      });
    },
    show(id) {
      whenReady(() => {
        const editor = editorRef.current!;
        if (current.current) {
          const previous = entries.current.get(current.current);
          if (previous) previous.state = editor.saveViewState();
        }
        current.current = id;
        const entry = id ? entries.current.get(id) : undefined;
        editor.setModel(entry?.model ?? null);
        if (entry?.state) editor.restoreViewState(entry.state);
        if (entry) editor.focus();
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
        entry.model.dispose();
        entries.current.delete(id);
      });
    },
    value(id) {
      return entries.current.get(id)?.model.getValue() ?? null;
    },
    replace(id, text) {
      whenReady(() => {
        const entry = entries.current.get(id);
        if (!entry) return;
        entry.model.setValue(text);
        entry.saved = entry.model.getAlternativeVersionId();
        callbacks.current.onDirty(id, false);
      });
    },
    markSaved(id) {
      const entry = entries.current.get(id);
      if (!entry) return;
      entry.saved = entry.model.getAlternativeVersionId();
      callbacks.current.onDirty(id, false);
    },
    focus() {
      editorRef.current?.focus();
    },
    reveal(id, line, column, length) {
      whenReady(() => {
        const editor = editorRef.current!;
        if (current.current !== id) return;
        const range = { startLineNumber: line, startColumn: column, endLineNumber: line, endColumn: column + length };
        editor.setSelection(range);
        editor.revealRangeInCenterIfOutsideViewport(range);
        editor.focus();
      });
    },
  }));

  return <div ref={container} className="code-editor" data-testid="code-editor" />;
});
