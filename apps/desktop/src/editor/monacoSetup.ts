import EditorWorker from "monaco-editor/editor/editor.worker?worker";
import JsonWorker from "monaco-editor/language/json/json.worker?worker";
import CssWorker from "monaco-editor/language/css/css.worker?worker";
import HtmlWorker from "monaco-editor/language/html/html.worker?worker";

export type Monaco = typeof import("monaco-editor");

let loading: Promise<Monaco> | null = null;

/**
 * Loads Monaco once, from the app itself: the workers are files of the app (the page's security
 * policy only allows its own origin, and the editor must work offline), never a CDN.
 *
 * JavaScript and TypeScript are highlighted, but their language service (and its very large
 * worker) stays off: the editor is for any project, and Java is the first one.
 */
export function loadMonaco(): Promise<Monaco> {
  loading ??= (async () => {
    (self as unknown as { MonacoEnvironment: unknown }).MonacoEnvironment = {
      getWorker(_id: string, label: string): Worker {
        switch (label) {
          case "json":
            return new JsonWorker();
          case "css":
          case "scss":
          case "less":
            return new CssWorker();
          case "html":
          case "handlebars":
          case "razor":
            return new HtmlWorker();
          default:
            return new EditorWorker();
        }
      },
    };
    const monaco = await import("monaco-editor");
    const off = {
      completionItems: false,
      hovers: false,
      documentSymbols: false,
      definitions: false,
      references: false,
      documentHighlights: false,
      rename: false,
      diagnostics: false,
      documentRangeFormattingEdits: false,
      signatureHelp: false,
      onTypeFormattingEdits: false,
      codeActions: false,
      inlayHints: false,
    };
    for (const defaults of [
      monaco.typescript.typescriptDefaults,
      monaco.typescript.javascriptDefaults,
    ]) {
      defaults.setModeConfiguration(off);
      defaults.setDiagnosticsOptions({ noSemanticValidation: true, noSyntaxValidation: true });
    }
    return monaco;
  })();
  return loading;
}
