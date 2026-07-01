import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ChevronRight, EyeOff, ExternalLink, Settings2, X } from "lucide-react";
import { api } from "./api";
import { formatDuration, statusMeta } from "./status";
import type { AppSettings, CodexStatus, StatusSnapshot } from "./types";
import { defaultSettings } from "./types";

const preview: StatusSnapshot = {
  aggregate: "executing",
  threads: [{ id: "preview", title: "构建 CodePulse 桌面指示灯", status: "executing", since: Date.now() - 84000, lastChangedAt: Date.now() - 84000 }],
  health: { level: "healthy", message: "状态探测正常", lastSuccessfulRead: Date.now() },
  observedAt: Date.now()
};

function PulseMark({ status, animated }: { status: CodexStatus; animated: boolean }) {
  return (
    <div className={`pulse-mark status-${status} ${animated ? "is-animated" : ""}`} aria-hidden="true">
      <div className="orbit orbit-one" />
      <div className="orbit orbit-two" />
      <div className="mark-core">C</div>
    </div>
  );
}

export default function App() {
  const [snapshot, setSnapshot] = useState<StatusSnapshot>(preview);
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [expanded, setExpanded] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [now, setNow] = useState(Date.now());
  const clickTimer = useRef<number>();
  const meta = statusMeta[snapshot.aggregate];

  useEffect(() => {
    if (!api.isAvailable()) return;
    Promise.all([api.snapshot(), api.settings()]).then(([next, config]) => {
      setSnapshot(next); setSettings(config);
    }).catch(() => undefined);
    const cleanup = Promise.all([
      listen<StatusSnapshot>("status-changed", event => setSnapshot(event.payload)),
      listen<StatusSnapshot["health"]>("provider-health-changed", event =>
        setSnapshot(current => ({ ...current, health: event.payload })))
    ]);
    return () => { cleanup.then(items => items.forEach(unlisten => unlisten())); };
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!api.isAvailable()) return;
    const height = expanded ? (showSettings ? 414 : 340) : 82;
    import("@tauri-apps/api/dpi").then(({ LogicalSize }) => getCurrentWindow().setSize(new LogicalSize(276, height)));
  }, [expanded, showSettings]);

  const update = useCallback((patch: Partial<AppSettings>) => {
    const next = { ...settings, ...patch };
    setSettings(next);
    if (api.isAvailable()) api.updateSettings(next).catch(() => undefined);
  }, [settings]);

  const handleClick = () => {
    window.clearTimeout(clickTimer.current);
    clickTimer.current = window.setTimeout(() => setExpanded(value => !value), 210);
  };
  const handleDoubleClick = () => {
    window.clearTimeout(clickTimer.current);
    if (api.isAvailable()) api.openCodex().catch(() => undefined);
  };
  const startDrag = (event: React.MouseEvent) => {
    if ((event.target as HTMLElement).closest("button,input")) return;
    if (api.isAvailable()) getCurrentWindow().startDragging().catch(() => undefined);
  };

  const activeThreads = useMemo(() => snapshot.threads.slice(0, 4), [snapshot.threads]);

  return (
    <main className="shell" style={{ "--surface-opacity": settings.opacity } as React.CSSProperties}>
      <section className={`widget status-${snapshot.aggregate} ${expanded ? "expanded" : ""} ${showSettings ? "settings-open" : ""}`} onMouseDown={startDrag}>
        <header className="capsule" onClick={handleClick} onDoubleClick={handleDoubleClick}>
          <PulseMark status={snapshot.aggregate} animated={settings.animations} />
          <div className="headline">
            <div className="brand-row"><strong>Codex</strong><span className="signal" /></div>
            <span className="status-label">{settings.paused ? "已暂停" : meta.label}</span>
          </div>
          <button
            className="toggle-button"
            type="button"
            aria-label={expanded ? "收起详情" : "展开详情"}
            aria-expanded={expanded}
            onMouseDown={event => event.stopPropagation()}
            onDoubleClick={event => event.stopPropagation()}
            onClick={event => {
              event.stopPropagation();
              window.clearTimeout(clickTimer.current);
              setExpanded(value => !value);
            }}
          >
            <ChevronRight className={`chevron ${expanded ? "rotated" : ""}`} size={18} strokeWidth={2.2} />
          </button>
        </header>

        <div className="panel" aria-hidden={!expanded}>
          <div className="panel-rule" />
          <div className="section-label">
            <span>{snapshot.threads.length ? `活跃任务 · ${snapshot.threads.length}` : "当前状态"}</span>
            <span className={`health health-${snapshot.health.level}`}>{snapshot.health.level === "healthy" ? "探测正常" : "状态受限"}</span>
          </div>

          <div className="thread-list">
            {activeThreads.length ? activeThreads.map(thread => (
              <div className="thread" key={thread.id}>
                <span className={`thread-dot status-${thread.status}`} />
                <div><strong>{thread.title || "未命名任务"}</strong><span>{statusMeta[thread.status].label} · {formatDuration(thread.since, now)}</span></div>
              </div>
            )) : <p className="empty">{meta.detail}</p>}
          </div>

          {showSettings && <div className="settings-pane">
            <label><span>透明度</span><input aria-label="透明度" type="range" min="0.72" max="1" step="0.01" value={settings.opacity} onChange={e => update({ opacity: Number(e.target.value) })} /></label>
            <label><span>界面动效</span><input type="checkbox" checked={settings.animations} onChange={e => update({ animations: e.target.checked })} /></label>
            <label><span>始终置顶</span><input type="checkbox" checked={settings.alwaysOnTop} onChange={e => update({ alwaysOnTop: e.target.checked })} /></label>
            <label><span>边缘吸附</span><input type="checkbox" checked={settings.edgeSnap} onChange={e => update({ edgeSnap: e.target.checked })} /></label>
          </div>}

          <nav className="actions">
            <button onClick={() => api.isAvailable() && api.openCodex()}><ExternalLink size={15} />打开 Codex</button>
            <button onClick={() => update({ paused: !settings.paused })}><EyeOff size={15} />{settings.paused ? "恢复" : "暂停"}</button>
            <button className="icon-button" aria-label="设置" onClick={() => setShowSettings(value => !value)}>{showSettings ? <X size={16} /> : <Settings2 size={16} />}</button>
          </nav>
        </div>
      </section>
    </main>
  );
}
