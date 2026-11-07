import { useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { RotateCw, X } from "lucide-react";
import type { Terminal as XTerm } from "@xterm/xterm";
import { call } from "../ipc";
import { useI18n } from "../i18n";
import type { TerminalEvent, TerminalImage } from "../types";

const message = (error: unknown) =>
  typeof error === "string" ? error : error instanceof Error ? error.message : String(error);

const dark = () => document.documentElement.dataset.theme === "dark";

const THEMES = {
  dark: { background: "#1e1e1e", foreground: "#d4d4d4", cursor: "#d4d4d4", selectionBackground: "#264f78" },
  light: { background: "#ffffff", foreground: "#1f1f1f", cursor: "#1f1f1f", selectionBackground: "#add6ff" },
};

/** What is pasted is sent in pieces: the backend refuses more than 64 KiB at once. */
const CHUNK = 8000;

const bytesOf = (base64: string) => Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));

type Status = "starting" | "running" | "exited" | "failed";

/**
 * A terminal inside the editor. The shell does not run on this computer: it runs in a Docker
 * container that has the open folder mounted at /work (the backend chooses the arguments; this
 * only names one of the listed images).
 */
export function TerminalPanel({
  root,
  hidden,
  onHide,
}: {
  /** The folder mounted in the container. */
  root: string;
  hidden: boolean;
  onHide: () => void;
}) {
  const { t } = useI18n();
  const host = useRef<HTMLDivElement>(null);
  const imageSelect = useRef<HTMLSelectElement>(null);
  const term = useRef<XTerm | null>(null);
  const [images, setImages] = useState<TerminalImage[]>([]);
  const [image, setImage] = useState("");
  const [run, setRun] = useState(0);
  const [status, setStatus] = useState<Status>("starting");
  const [detail, setDetail] = useState("");
  const hiddenRef = useRef(hidden);
  hiddenRef.current = hidden;

  // Which environments exist and which one fits the folder.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [listed, fitting] = await Promise.all([
          invoke<TerminalImage[]>("terminal_images"),
          call<string>("terminal_default_image", { root }),
        ]);
        if (cancelled) return;
        setImages(listed);
        setImage((current) => current || fitting);
      } catch (e) {
        if (cancelled) return;
        setStatus("failed");
        setDetail(message(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [root]);

  // One shell per (folder, environment, restart). Leaving ends it and removes its container.
  useEffect(() => {
    if (!image || !host.current) return;
    const element = host.current;
    let disposed = false;
    let id: number | null = null;
    let observer: ResizeObserver | null = null;
    let themes: MutationObserver | null = null;
    let screen: XTerm | null = null;
    setStatus("starting");
    setDetail("");

    void (async () => {
      try {
        const [{ Terminal }, { FitAddon }] = await Promise.all([
          import("@xterm/xterm"),
          import("@xterm/addon-fit"),
          import("@xterm/xterm/css/xterm.css"),
        ]);
        if (disposed) return;
        const fit = new FitAddon();
        screen = new Terminal({
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
          fontSize: 13,
          cursorBlink: true,
          scrollback: 5000,
          theme: dark() ? THEMES.dark : THEMES.light,
        });
        screen.loadAddon(fit);
        screen.open(element);
        screen.textarea?.setAttribute("aria-label", t("terminal.input"));
        term.current = screen;
        // Tab belongs to the shell, so the keyboard needs another way out of the terminal.
        screen.attachCustomKeyEventHandler((event) => {
          if (event.type === "keydown" && event.ctrlKey && event.shiftKey && event.key.toLowerCase() === "e") {
            imageSelect.current?.focus();
            return false;
          }
          return true;
        });
        // The space the last row does not fill takes the colour of the terminal, not xterm's black.
        const paint = () => {
          const theme = dark() ? THEMES.dark : THEMES.light;
          if (screen) screen.options.theme = theme;
          element.style.backgroundColor = theme.background;
        };
        paint();
        const fitNow = () => {
          if (element.clientWidth > 0 && element.clientHeight > 0) fit.fit();
        };
        fitNow();

        // Everything typed goes in order, and waits until the shell exists.
        let started: Promise<void> = Promise.resolve();
        let queue: Promise<void> = Promise.resolve();
        const send = (action: () => Promise<unknown>) => {
          queue = queue.then(() => started).then(action).then(() => undefined, () => undefined);
        };
        screen.onData((data) => {
          for (let at = 0; at < data.length; at += CHUNK) {
            const piece = data.slice(at, at + CHUNK);
            send(() => (id === null ? Promise.resolve() : invoke("terminal_write", { id, data: piece })));
          }
        });
        screen.onResize(({ cols, rows }) => send(() => (id === null ? Promise.resolve() : invoke("terminal_resize", { id, cols, rows }))));
        observer = new ResizeObserver(fitNow);
        observer.observe(element);
        themes = new MutationObserver(paint);
        themes.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

        const channel = new Channel<TerminalEvent>();
        channel.onmessage = (event) => {
          if (disposed || !screen) return;
          if (event.event === "data") {
            screen.write(bytesOf(event.data));
          } else {
            const text = event.code == null ? t("terminal.exited") : t("terminal.exited.code", { code: event.code });
            screen.write(`\r\n\x1b[2m[${text}]\x1b[0m\r\n`);
            setStatus("exited");
          }
        };
        const starting = call<number>("terminal_start", {
          root,
          image,
          cols: screen.cols,
          rows: screen.rows,
          onEvent: channel,
        });
        started = starting.then(() => undefined, () => undefined);
        const opened = await starting;
        if (disposed) {
          void invoke("terminal_stop", { id: opened }).catch(() => undefined);
          return;
        }
        id = opened;
        setStatus((current) => (current === "exited" ? current : "running"));
        if (!hiddenRef.current) screen.focus();
      } catch (e) {
        if (disposed) return;
        setStatus("failed");
        setDetail(message(e));
      }
    })();

    return () => {
      disposed = true;
      observer?.disconnect();
      themes?.disconnect();
      if (id !== null) void invoke("terminal_stop", { id }).catch(() => undefined);
      screen?.dispose();
      term.current = null;
    };
    // `t` is stable per language and a language change must not restart the shell.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [root, image, run]);

  // Showing the panel again puts the cursor back in the shell.
  useEffect(() => {
    if (!hidden) term.current?.focus();
  }, [hidden]);

  return (
    <section className="terminal-panel" hidden={hidden} aria-label={t("terminal.panel.aria")}>
      <div className="terminal-head">
        <label className="terminal-image">
          <span>{t("terminal.image")}</span>
          <select
            ref={imageSelect}
            value={image}
            onChange={(event) => setImage(event.target.value)}
            title={t("terminal.image.hint")}
          >
            {images.map((item) => (
              <option key={item.id} value={item.id}>
                {item.label}
              </option>
            ))}
          </select>
        </label>
        <span className="terminal-state">
          {status === "starting" ? t("terminal.starting") : status === "exited" ? t("terminal.ended") : ""}
        </span>
        <span className="terminal-hint">{t("terminal.leave")}</span>
        <div className="terminal-actions">
          <button
            type="button"
            className="icon-button"
            title={t("terminal.restart")}
            aria-label={t("terminal.restart")}
            onClick={() => setRun((n) => n + 1)}
          >
            <RotateCw size={15} />
          </button>
          <button
            type="button"
            className="icon-button"
            title={t("terminal.hide")}
            aria-label={t("terminal.hide")}
            onClick={onHide}
          >
            <X size={15} />
          </button>
        </div>
      </div>
      {status === "failed" && (
        <div className="terminal-error" role="alert">
          {detail}
        </div>
      )}
      <div className="terminal-screen" ref={host} />
    </section>
  );
}
