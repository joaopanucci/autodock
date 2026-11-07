import { AlertTriangle, CheckCircle2, Download, LoaderCircle } from "lucide-react";
import { useI18n } from "../i18n";
import { percentOf } from "../updater";
import type { UpdateOffer } from "../types";

export type UpdatePhase = "offer" | "downloading" | "ready" | "error";

export type UpdateState = {
  offer: UpdateOffer;
  phase: UpdatePhase;
  done: number;
  total: number | null;
  error: string;
};

/**
 * "AutoDock 0.4.0 is available". Shown on every page; nothing is installed until the user
 * clicks Update. The release notes are plain text on purpose (never rendered as HTML).
 */
export function UpdateBanner({
  state,
  install,
  later,
  skip,
  restart,
}: {
  state: UpdateState | null;
  install: () => void;
  later: () => void;
  skip: () => void;
  restart: () => void;
}) {
  const { t } = useI18n();
  if (!state) return null;
  const { offer, phase } = state;
  const percent = percentOf(state.done, state.total);
  return (
    <div className="update-banner" role="status" data-testid="update-banner">
      {phase === "error" ? (
        <AlertTriangle size={20} aria-hidden="true" />
      ) : phase === "ready" ? (
        <CheckCircle2 size={20} aria-hidden="true" />
      ) : phase === "downloading" ? (
        <LoaderCircle className="spin" size={20} aria-hidden="true" />
      ) : (
        <Download size={20} aria-hidden="true" />
      )}
      <div className="update-banner-text">
        <p>
          {phase === "offer" &&
            t("update.banner.available", {
              version: offer.version,
              current: offer.currentVersion,
            })}
          {phase === "downloading" &&
            (percent === null
              ? t("update.downloading.unknown", { version: offer.version })
              : t("update.downloading", { version: offer.version, percent }))}
          {phase === "ready" && t("update.ready", { version: offer.version })}
          {phase === "error" && t("update.failed", { detail: state.error })}
        </p>
        {phase === "downloading" && (
          <progress
            max={100}
            value={percent ?? undefined}
            aria-label={t("update.progress.aria")}
          />
        )}
        {phase === "offer" && offer.notes && (
          <details>
            <summary>{t("update.notes.toggle")}</summary>
            <pre className="update-notes">{offer.notes}</pre>
          </details>
        )}
      </div>
      <div className="update-banner-actions">
        {(phase === "offer" || phase === "error") && (
          <button type="button" className="button primary" onClick={install}>
            {phase === "error" ? t("update.action.retry") : t("update.action.install")}
          </button>
        )}
        {phase === "ready" && (
          <button type="button" className="button primary" onClick={restart}>
            {t("update.action.restart")}
          </button>
        )}
        {(phase === "offer" || phase === "error") && (
          <button type="button" className="button secondary" onClick={later}>
            {t("update.action.later")}
          </button>
        )}
        {phase === "offer" && (
          <button type="button" className="button secondary" onClick={skip}>
            {t("update.action.skip")}
          </button>
        )}
      </div>
    </div>
  );
}
