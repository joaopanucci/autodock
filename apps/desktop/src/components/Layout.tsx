import type { ReactNode } from "react";
import {
  LayoutDashboard,
  FolderGit2,
  SlidersHorizontal,
  Boxes,
  Activity,
  TerminalSquare,
  Settings2,
  ShieldCheck,
  WandSparkles,
  HeartHandshake,
  GraduationCap,
  Smartphone,
  Coffee,
  ChevronRight,
  ArrowUpRight,
  type LucideIcon,
} from "lucide-react";
import type { Page } from "../types";
import { useI18n, type TranslationKey } from "../i18n";
import { ThemeSwitch } from "./ThemeSwitch";
import logo from "../../src-tauri/icons/128x128.png";

export const navigation: {
  id: Page;
  labelKey: TranslationKey;
  icon: LucideIcon;
  planned?: boolean;
}[] = [
  { id: "overview", labelKey: "nav.overview", icon: LayoutDashboard },
  { id: "projects", labelKey: "nav.projects", icon: FolderGit2 },
  { id: "setup", labelKey: "nav.setup", icon: WandSparkles },
  { id: "expo", labelKey: "nav.expo", icon: Smartphone },
  { id: "java", labelKey: "nav.java", icon: Coffee },
  { id: "environment", labelKey: "nav.environment", icon: SlidersHorizontal },
  { id: "containers", labelKey: "nav.containers", icon: Boxes },
  { id: "monitoring", labelKey: "nav.monitoring", icon: Activity },
  { id: "console", labelKey: "nav.console", icon: TerminalSquare },
  { id: "learn", labelKey: "nav.learn", icon: GraduationCap },
  { id: "settings", labelKey: "nav.settings", icon: Settings2 },
  { id: "contribute", labelKey: "nav.contribute", icon: HeartHandshake },
];
export function Layout({
  page,
  navigate,
  children,
  projects,
  expoAlerts = 0,
}: {
  page: Page;
  navigate: (page: Page) => void;
  children: ReactNode;
  projects: number;
  /** Watched Expo projects behind the latest SDK. */
  expoAlerts?: number;
}) {
  const { t } = useI18n();
  return (
    <div className="app-shell">
      <a className="skip-link" href="#content">
        {t("nav.skip")}
      </a>
      <aside className="sidebar">
        <div className="brand">
          <img src={logo} alt="" />
          <div>
            <strong>AutoDock</strong>
            <span>LOCAL DEVELOPMENT</span>
          </div>
        </div>
        <div className="workspace-label">
          <span className="workspace-icon">A</span>
          <div>
            {t("layout.workspace")}<small>{t("layout.session")}</small>
          </div>
          <ShieldCheck size={16} aria-hidden="true" />
        </div>
        <nav aria-label={t("nav.aria")}>
          <p className="nav-label">{t("nav.group.workspace")}</p>
          {navigation.slice(0, 5).map((item) => (
            <button
              key={item.id}
              aria-current={page === item.id ? "page" : undefined}
              onClick={() => navigate(item.id)}
            >
              <item.icon size={19} aria-hidden="true" />
              <span>{t(item.labelKey)}</span>
              {item.id === "projects" && (
                <span className="nav-count">{projects}</span>
              )}
              {item.id === "expo" && expoAlerts > 0 && (
                <span className="nav-count" title={t("expo.nav.alerts")}>
                  {expoAlerts}
                </span>
              )}
            </button>
          ))}
          <p className="nav-label secondary-label">{t("nav.group.operations")}</p>
          {navigation.slice(5, 9).map((item) => (
            <button
              key={item.id}
              aria-current={page === item.id ? "page" : undefined}
              onClick={() => navigate(item.id)}
            >
              <item.icon size={19} aria-hidden="true" />
              <span>{t(item.labelKey)}</span>
              {item.planned && (
                <span className="nav-soon" title={t("nav.soonTitle")}>
                  {t("nav.soon")}
                </span>
              )}
            </button>
          ))}
          <div className="nav-settings">
            <button
              aria-current={page === "learn" ? "page" : undefined}
              onClick={() => navigate("learn")}
            >
              <GraduationCap size={19} aria-hidden="true" />
              <span>{t("nav.learn")}</span>
            </button>
            <button
              aria-current={page === "settings" ? "page" : undefined}
              onClick={() => navigate("settings")}
            >
              <Settings2 size={19} aria-hidden="true" />
              <span>{t("nav.settings")}</span>
            </button>
            <button
              aria-current={page === "contribute" ? "page" : undefined}
              onClick={() => navigate("contribute")}
            >
              <HeartHandshake size={19} aria-hidden="true" />
              <span>{t("nav.contribute")}</span>
            </button>
          </div>
        </nav>
        <div className="sidebar-note">
          <ShieldCheck size={18} aria-hidden="true" />
          <div>
            <strong>{t("layout.control.title")}</strong>
            <p>
              {t("layout.control.line1")}
              <br />
              {t("layout.control.line2")}
            </p>
          </div>
        </div>
        <div className="sidebar-footer">
          <span className="status-dot" /> AutoDock{" "}
          <span>
            Local · Open source <ArrowUpRight size={12} aria-hidden="true" />
          </span>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <div>
            <span>Workspace</span>
            <ChevronRight size={14} aria-hidden="true" />
            <strong>{t(navigation.find((n) => n.id === page)!.labelKey)}</strong>
          </div>
          <div className="topbar-actions">
            <span className="local-label">
              <ShieldCheck size={14} aria-hidden="true" /> {t("layout.topbar.confirm")}
            </span>
            <ThemeSwitch />
          </div>
        </header>
        <main id="content" tabIndex={-1}>
          {children}
        </main>
        <footer className="workspace-footer">
          <span>{t("layout.footer.local")}</span>
          <span>AutoDock v{__APP_VERSION__} · Open source</span>
        </footer>
      </div>
    </div>
  );
}
