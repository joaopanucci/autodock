import { Smartphone } from "lucide-react";
import { useI18n } from "../i18n";
import type { ExpoWatchState } from "../types";

/**
 * "Your project is on SDK 57, but Expo released stable SDK 58." Shown on every page
 * while a watched project is behind the latest SDK and its alert was not dismissed.
 */
export function ExpoAlert({
  state,
  open,
  dismiss,
}: {
  state: ExpoWatchState | null;
  /** Opens the Expo page; with a path it starts the upgrade plan of that project. */
  open: (path?: string) => void;
  dismiss: (path: string) => void;
}) {
  const { t, tn } = useI18n();
  const pending = (state?.projects ?? []).filter((p) => p.update && !p.update.dismissed);
  if (!pending.length) return null;
  const first = pending[0];
  const update = first.update!;
  return (
    <div className="expo-alert" role="status" data-testid="expo-alert">
      <Smartphone size={20} aria-hidden="true" />
      <p>
        {pending.length === 1
          ? t("expo.alert.single", {
              name: first.name,
              current: update.currentSdk,
              latest: update.latestSdk,
            })
          : tn("expo.alert.many", pending.length, { latest: update.latestSdk })}
      </p>
      <div className="expo-alert-actions">
        <button
          type="button"
          className="button primary"
          onClick={() => open(pending.length === 1 ? first.path : undefined)}
        >
          {pending.length === 1 ? t("expo.alert.view") : t("expo.alert.open")}
        </button>
        {pending.length === 1 && (
          <button type="button" className="button secondary" onClick={() => dismiss(first.path)}>
            {t("expo.alert.dismiss")}
          </button>
        )}
      </div>
    </div>
  );
}
