import { useSyncExternalStore } from "react";
import { Monitor, Moon, Sun, type LucideIcon } from "lucide-react";
import { useI18n } from "../i18n";
import { THEME_MODES, currentMode, setMode, subscribe, type ThemeMode } from "../theme";

const ICONS: Record<ThemeMode, LucideIcon> = { light: Sun, dark: Moon, system: Monitor };

/** Light / dark / follow-the-system, available from every screen. */
export function ThemeSwitch() {
  const { t } = useI18n();
  const mode = useSyncExternalStore(subscribe, currentMode);
  return (
    <div className="theme-switch" role="group" aria-label={t("theme.aria")}>
      {THEME_MODES.map((item) => {
        const Icon = ICONS[item];
        return (
          <button
            key={item}
            type="button"
            aria-pressed={mode === item}
            title={t(`theme.${item}`)}
            aria-label={t(`theme.${item}`)}
            className={mode === item ? "is-active" : ""}
            onClick={() => setMode(item)}
          >
            <Icon size={15} aria-hidden="true" />
          </button>
        );
      })}
    </div>
  );
}
