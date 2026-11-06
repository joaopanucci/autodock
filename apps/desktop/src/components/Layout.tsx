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
  ChevronRight,
  ArrowUpRight,
  type LucideIcon,
} from "lucide-react";
import type { Page } from "../types";
import logo from "../../src-tauri/icons/128x128.png";

export const navigation: {
  id: Page;
  label: string;
  icon: LucideIcon;
  planned?: boolean;
}[] = [
  { id: "overview", label: "Visão geral", icon: LayoutDashboard },
  { id: "projects", label: "Projetos", icon: FolderGit2 },
  { id: "environment", label: "Ambiente", icon: SlidersHorizontal },
  { id: "containers", label: "Containers", icon: Boxes },
  { id: "monitoring", label: "Monitoramento", icon: Activity },
  { id: "console", label: "Console", icon: TerminalSquare },
  { id: "settings", label: "Configurações", icon: Settings2 },
];
export function Layout({
  page,
  navigate,
  children,
  projects,
}: {
  page: Page;
  navigate: (page: Page) => void;
  children: ReactNode;
  projects: number;
}) {
  return (
    <div className="app-shell">
      <a className="skip-link" href="#content">
        Pular para o conteúdo
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
            Workspace local<small>Sessão atual</small>
          </div>
          <ShieldCheck size={16} aria-hidden="true" />
        </div>
        <nav aria-label="Navegação principal">
          <p className="nav-label">WORKSPACE</p>
          {navigation.slice(0, 3).map((item) => (
            <button
              key={item.id}
              aria-current={page === item.id ? "page" : undefined}
              onClick={() => navigate(item.id)}
            >
              <item.icon size={19} aria-hidden="true" />
              <span>{item.label}</span>
              {item.id === "projects" && (
                <span className="nav-count">{projects}</span>
              )}
            </button>
          ))}
          <p className="nav-label secondary-label">OPERAÇÕES</p>
          {navigation.slice(3, 6).map((item) => (
            <button
              key={item.id}
              aria-current={page === item.id ? "page" : undefined}
              onClick={() => navigate(item.id)}
            >
              <item.icon size={19} aria-hidden="true" />
              <span>{item.label}</span>
              {item.planned && (
                <span className="nav-soon" title="Em desenvolvimento">
                  Em breve
                </span>
              )}
            </button>
          ))}
          <div className="nav-settings">
            <button
              aria-current={page === "settings" ? "page" : undefined}
              onClick={() => navigate("settings")}
            >
              <Settings2 size={19} aria-hidden="true" />
              <span>Configurações</span>
            </button>
          </div>
        </nav>
        <div className="sidebar-note">
          <ShieldCheck size={18} aria-hidden="true" />
          <div>
            <strong>Você está no controle</strong>
            <p>
              Análises somente leitura.
              <br />
              Seu ambiente é preservado.
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
            <strong>{navigation.find((n) => n.id === page)?.label}</strong>
          </div>
          <span className="local-label">
            <ShieldCheck size={14} aria-hidden="true" /> Somente leitura
          </span>
        </header>
        <main id="content" tabIndex={-1}>
          {children}
        </main>
        <footer className="workspace-footer">
          <span>Análise local. Sem execução de scripts do projeto.</span>
          <span>AutoDock · Etapa 002.3</span>
        </footer>
      </div>
    </div>
  );
}
