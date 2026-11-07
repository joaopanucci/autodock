import { useEffect, useState } from "react";
import { LoaderCircle, RefreshCw } from "lucide-react";
import { Panel } from "./ui";
import { useI18n } from "../i18n";
import { checkNow, getPrefs, setCheckOnStart } from "../updater";
import type { UpdateOffer, UpdatePrefs } from "../types";

const message = (error: unknown) =>
  typeof error === "string" ? error : error instanceof Error ? error.message : String(error);

/** Settings: the version, the automatic check (on by default) and "Check now". */
export function UpdatesPanel({ onOffer }: { onOffer: (offer: UpdateOffer) => void }) {
  const { t, locale } = useI18n();
  const [prefs, setPrefs] = useState<UpdatePrefs | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ kind: "latest" | "found" | "error"; text: string } | null>(null);

  useEffect(() => {
    let alive = true;
    getPrefs()
      .then((value) => alive && setPrefs(value))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);

  async function toggle(enabled: boolean) {
    try {
      setPrefs(await setCheckOnStart(enabled));
    } catch (e) {
      setResult({ kind: "error", text: message(e) });
    }
  }

  async function check() {
    if (busy) return;
    setBusy(true);
    setResult(null);
    try {
      const report = await checkNow();
      if (report.error) {
        setResult({ kind: "error", text: t("update.settings.error", { detail: report.error }) });
      } else if (report.offer) {
        onOffer(report.offer);
        setResult({ kind: "found", text: t("update.settings.found", { version: report.offer.version }) });
      } else {
        setResult({ kind: "latest", text: t("update.settings.upToDate") });
      }
      setPrefs(await getPrefs());
    } catch (e) {
      setResult({ kind: "error", text: t("update.settings.error", { detail: message(e) }) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel title={t("update.settings.title")} eyebrow={t("update.settings.eyebrow")}>
      <p className="section-description">
        {t("update.settings.current", { version: __APP_VERSION__ })}
        {prefs?.lastCheck
          ? ` · ${t("update.settings.lastCheck", {
              date: new Date(prefs.lastCheck * 1000).toLocaleString(locale),
            })}`
          : ""}
      </p>
      <label className="multi-confirm">
        <input
          type="checkbox"
          checked={!!prefs?.checkOnStart}
          disabled={!prefs}
          onChange={(e) => void toggle(e.target.checked)}
        />
        <span>{t("update.settings.auto")}</span>
      </label>
      <small className="field-hint">{t("update.settings.autoHint")}</small>
      <div className="multi-run-actions">
        <button type="button" className="button secondary" disabled={busy} onClick={() => void check()}>
          {busy ? (
            <LoaderCircle className="spin" size={16} aria-hidden="true" />
          ) : (
            <RefreshCw size={16} aria-hidden="true" />
          )}
          {busy ? t("update.settings.checking") : t("update.settings.check")}
        </button>
      </div>
      {prefs?.skippedVersion && (
        <p className="field-hint">{t("update.settings.skipped", { version: prefs.skippedVersion })}</p>
      )}
      {result && (
        <p className={`field-hint ${result.kind === "error" ? "warn" : ""}`} role="status">
          {result.text}
        </p>
      )}
    </Panel>
  );
}
