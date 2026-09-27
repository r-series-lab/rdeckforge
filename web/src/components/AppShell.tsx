import {
  PanelLeft,
  PanelRight,
  PlayCircle,
  Settings,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useMemo, useState } from "react";
import type { MouseEvent, ReactNode } from "react";
import { appModules, type PageKey } from "@/app-modules";
import { Button } from "@/components/ui/button";

type AppShellProps = {
  activePage: PageKey;
  onPageChange: (page: PageKey) => void;
  children: ReactNode;
  inspector: ReactNode;
  onOpenSettings: () => void;
  quickResume?: {
    title: string;
    detail: string;
    onOpen: () => void;
  } | null;
};

export function AppShell({
  activePage,
  onPageChange,
  children,
  inspector,
  onOpenSettings,
  quickResume,
}: AppShellProps) {
  const [leftCollapsed, setLeftCollapsed] = useState(false);
  const [rightCollapsed, setRightCollapsed] = useState(() => window.innerWidth < 1120);
  const hasInspector = Boolean(inspector);
  const shellClassName = useMemo(
    () =>
      [
        "shell",
        leftCollapsed ? "is-left-collapsed" : "",
        hasInspector ? "has-inspector" : "",
        hasInspector && rightCollapsed ? "is-right-collapsed" : "",
      ]
        .filter(Boolean)
        .join(" "),
    [hasInspector, leftCollapsed, rightCollapsed],
  );
  const startWindowDrag = (event: MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !("__TAURI_INTERNALS__" in window)) {
      return;
    }

    event.preventDefault();
    void getCurrentWindow().startDragging().catch(() => undefined);
  };

  useEffect(() => {
    const collapseInspector = () => {
      if (window.innerWidth < 1120) {
        setRightCollapsed(true);
      }
    };
    window.addEventListener("resize", collapseInspector);
    return () => window.removeEventListener("resize", collapseInspector);
  }, []);

  return (
    <div className={shellClassName}>
      <div className="window-drag-region" onMouseDown={startWindowDrag} />
      <div className="window-toolbar" aria-label="Window tools">
        <Button
          variant="ghost"
          size="icon"
          title={leftCollapsed ? "展开左栏" : "收起左栏"}
          aria-label={leftCollapsed ? "展开左栏" : "收起左栏"}
          aria-pressed={leftCollapsed}
          onClick={() => setLeftCollapsed((value) => !value)}
        >
          <PanelLeft size={16} strokeWidth={1.8} />
        </Button>
        {hasInspector ? (
          <Button
            variant="ghost"
            size="icon"
            title={rightCollapsed ? "展开右栏" : "收起右栏"}
            aria-label={rightCollapsed ? "展开右栏" : "收起右栏"}
            aria-pressed={!rightCollapsed}
            onClick={() => setRightCollapsed((value) => !value)}
          >
            <PanelRight size={16} strokeWidth={1.8} />
          </Button>
        ) : null}
        <Button
          variant="ghost"
          size="icon"
          title="设置"
          aria-label="打开设置"
          onClick={onOpenSettings}
        >
          <Settings size={16} strokeWidth={1.8} />
        </Button>
      </div>

      <aside className="sidebar">
        <button
          type="button"
          className="brand-mark"
          title="回到生成"
          aria-label="回到生成"
          onClick={() => onPageChange("render")}
        >
          <img src="/rdeckforge-app-icon.png" alt="" />
          <span className="brand-copy">
            <strong>rDeckForge</strong>
            <small>文档工作台</small>
          </span>
        </button>
        <nav className="nav-stack" aria-label="主导航">
          {appModules.map((item) => {
            const Icon = item.icon;
            const selected = item.key === activePage;
            return (
              <button
                key={item.key}
                type="button"
                className={`nav-item${selected ? " is-active" : ""}`}
                aria-current={selected ? "page" : undefined}
                aria-label={item.label}
                title={item.label}
                onClick={() => onPageChange(item.key)}
              >
                <Icon size={17} />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>
        {quickResume ? (
          <button
            type="button"
            className="quick-resume-card"
            title={`${quickResume.title} · ${quickResume.detail}`}
            onClick={quickResume.onOpen}
          >
            <PlayCircle size={15} />
            <span>
              <strong>{quickResume.title}</strong>
              <small>{quickResume.detail}</small>
            </span>
          </button>
        ) : null}
      </aside>

      <main className="workspace-frame">
        <section className="workspace-main">{children}</section>
        {hasInspector && !rightCollapsed ? <aside className="inspector">{inspector}</aside> : null}
      </main>
    </div>
  );
}
