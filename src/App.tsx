import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Activity, BellRing, ChevronDown, ChevronUp, ExternalLink, EyeOff, Gauge,
  Monitor, MousePointer2, Move, Play, Power, RotateCcw, Save, Settings2,
  ShieldCheck, Sparkles, X
} from "lucide-react";
import { api } from "./api";
import { formatDuration, formatRelative, statusMeta } from "./status";
import type { AppSettings, CodexStatus, DisplayInfo, StatusSnapshot, ThreadState } from "./types";
import { defaultSettings } from "./types";

const previewThreads: ThreadState[] = [
  { id: "1", title: "生成登录页重构", status: "thinking", statusSince: Date.now() - 151000, lastChangedAt: Date.now() - 20000, confidence: "normal" },
  { id: "2", title: "修复桌面拖拽", status: "executing", statusSince: Date.now() - 372000, lastChangedAt: Date.now() - 3000, confidence: "normal" },
  { id: "3", title: "安装包打包", status: "waitingApproval", statusSince: Date.now() - 48000, lastChangedAt: Date.now() - 10000, confidence: "normal" },
  { id: "4", title: "数据同步模块", status: "idle", statusSince: Date.now() - 60000, lastChangedAt: Date.now() - 60000, confidence: "limited" }
];

const previewSnapshot: StatusSnapshot = {
  aggregate: "executing",
  threads: previewThreads,
  health: { level: "healthy", message: "状态探测正常", lastSuccessfulRead: Date.now() },
  observedAt: Date.now()
};

function OrbitMark({ status, animated, compact = false }: { status: CodexStatus; animated: boolean; compact?: boolean }) {
  return (
    <div className={`orbit-mark status-${status} ${animated ? "is-animated" : ""} ${compact ? "compact" : ""}`} aria-hidden="true">
      <span className="orbit-segments" />
      <span className="orbit-track" />
      <span className="orbit-glow" />
      <span className="orbit-core" />
    </div>
  );
}

function StatusGlyph({ status }: { status: CodexStatus }) {
  return <span className={`status-glyph status-${status}`}><i /></span>;
}

function MainWidget() {
  const [snapshot, setSnapshot] = useState<StatusSnapshot>(previewSnapshot);
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [expanded, setExpanded] = useState(false);
  const [now, setNow] = useState(Date.now());
  const meta = statusMeta[snapshot.aggregate];

  useEffect(() => {
    if (!api.isAvailable()) return;
    Promise.all([api.snapshot(), api.settings()]).then(([next, config]) => {
      setSnapshot(next);
      setSettings(config);
    }).catch(() => undefined);
    const cleanup = Promise.all([
      listen<StatusSnapshot>("status-changed", event => setSnapshot(event.payload)),
      listen<AppSettings>("settings-previewed", event => setSettings(event.payload)),
      listen<AppSettings>("settings-saved", event => setSettings(event.payload))
    ]);
    return () => { cleanup.then(items => items.forEach(unlisten => unlisten())); };
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const setPanel = (next: boolean) => {
    setExpanded(next);
    if (api.isAvailable()) api.setExpanded(next).catch(() => setExpanded(!next));
  };

  const handleDoubleClick = () => {
    if (settings.doubleClickFocus && api.isAvailable()) api.openCodex().catch(() => undefined);
  };

  const threads = useMemo(() => snapshot.threads.slice(0, 8), [snapshot.threads]);
  const providerWarning = settings.showProviderWarnings && snapshot.health.level !== "healthy";

  return (
    <main
      className={`widget-stage ${expanded ? "is-expanded" : ""}`}
      style={{ "--surface-opacity": settings.opacity } as React.CSSProperties}
      onContextMenu={event => {
        event.preventDefault();
        if (api.isAvailable()) api.showContextMenu().catch(() => undefined);
      }}
    >
      <section className={`pulse-widget status-${snapshot.aggregate} ${expanded ? "expanded" : ""}`}>
        <header
          className="capsule-bar"
          onClick={() => { if (!expanded) setPanel(true); }}
          onDoubleClick={handleDoubleClick}
        >
          <div
            className="drag-mark"
            title="拖动 CodePulse"
            onClick={event => event.stopPropagation()}
            onDoubleClick={event => event.stopPropagation()}
            onMouseDown={event => {
              if (event.button === 0 && api.isAvailable()) getCurrentWindow().startDragging().catch(() => undefined);
            }}
          >
            <OrbitMark status={snapshot.aggregate} animated={settings.animations} />
          </div>
          <div className="capsule-copy">
            <div className="capsule-brand">Codex</div>
            <div className="capsule-status">{meta.label}</div>
          </div>
          <span className="live-dot" aria-hidden="true" />
          <button
            className="expand-control"
            type="button"
            aria-label={expanded ? "收起线程详情" : "展开线程详情"}
            aria-expanded={expanded}
            onClick={event => { event.stopPropagation(); setPanel(!expanded); }}
            onDoubleClick={event => event.stopPropagation()}
          >
            {expanded ? <ChevronUp size={19} /> : <ChevronDown size={19} />}
          </button>
        </header>

        {expanded && <div className="thread-panel">
          <div className="panel-heading">
            <div><strong>活跃线程</strong><span className="thread-count">{threads.length}</span></div>
            <button className="collapse-text" onClick={() => setPanel(false)}>收起 <ChevronUp size={15} /></button>
          </div>

          {providerWarning && <div className="provider-warning"><BellRing size={13} /><span>{snapshot.health.message}</span></div>}

          <div className="thread-table">
            <div className="thread-header"><span>线程</span><span>状态</span><span>耗时</span><span>最近变更</span></div>
            <div className="thread-scroll">
              {threads.length ? threads.map(thread => (
                <div className="thread-row" key={thread.id}>
                  <div className="thread-name"><span className="thread-file">⌘</span><strong title={thread.title}>{thread.title || "未命名任务"}</strong></div>
                  <div className="thread-state"><StatusGlyph status={thread.status} /><span>{thread.confidence === "limited" ? "状态受限" : statusMeta[thread.status].label}</span></div>
                  <span className="thread-time">{formatDuration(thread.statusSince, now)}</span>
                  <span className="thread-change">{formatRelative(thread.lastChangedAt, now)}</span>
                </div>
              )) : <div className="thread-empty"><OrbitMark status={snapshot.aggregate} animated={settings.animations} compact /><span>{meta.detail}</span></div>}
            </div>
          </div>

          <nav className="panel-actions">
            <button className="primary-action" onClick={() => api.isAvailable() && api.openCodex()}><ExternalLink size={16} />打开 Codex</button>
            <button onClick={() => api.isAvailable() && api.setMainVisible(false)}><EyeOff size={16} />隐藏悬浮</button>
            <button onClick={() => api.isAvailable() && api.showSettings()}><Settings2 size={16} />设置</button>
          </nav>
        </div>}
      </section>
    </main>
  );
}

function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (next: boolean) => void; label: string }) {
  return (
    <button className={`switch ${checked ? "on" : ""}`} type="button" role="switch" aria-checked={checked} aria-label={label} onClick={() => onChange(!checked)}>
      <span />
    </button>
  );
}

function SettingRow({ icon, title, description, control }: { icon: React.ReactNode; title: string; description: string; control: React.ReactNode }) {
  return (
    <div className="setting-row">
      <span className="setting-icon">{icon}</span>
      <div className="setting-copy"><strong>{title}</strong><span>{description}</span></div>
      <div className="setting-control">{control}</div>
    </div>
  );
}

function SettingsWindow() {
  const [saved, setSaved] = useState<AppSettings>(defaultSettings);
  const [draft, setDraft] = useState<AppSettings>(defaultSettings);
  const [display, setDisplay] = useState<DisplayInfo | null>(null);
  const [health, setHealth] = useState<StatusSnapshot["health"]>(previewSnapshot.health);
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);

  const load = async () => {
    if (!api.isAvailable()) { setLoaded(true); return; }
    const [settings, info, snapshot] = await Promise.all([api.settings(), api.displayInfo(), api.snapshot()]);
    setSaved(settings); setDraft(settings); setDisplay(info); setHealth(snapshot.health); setLoaded(true);
  };

  useEffect(() => {
    load().catch(() => setLoaded(true));
    if (!api.isAvailable()) return;
    const cleanup = Promise.all([
      listen("settings-opened", () => load().catch(() => undefined)),
      listen<StatusSnapshot["health"]>("provider-health-changed", event => setHealth(event.payload))
    ]);
    return () => { cleanup.then(items => items.forEach(unlisten => unlisten())); };
  }, []);

  useEffect(() => {
    if (!loaded || !api.isAvailable()) return;
    const timer = window.setTimeout(() => api.previewSettings(draft).catch(() => undefined), 70);
    return () => window.clearTimeout(timer);
  }, [draft, loaded]);

  const patch = (next: Partial<AppSettings>) => setDraft(current => ({ ...current, ...next }));
  const close = async (cancel: boolean) => {
    if (api.isAvailable()) await api.hideSettings(cancel);
  };
  const save = async () => {
    setSaving(true);
    try {
      if (api.isAvailable()) {
        await api.saveSettings(draft);
        await api.hideSettings(false);
      }
      setSaved(draft);
    } finally { setSaving(false); }
  };
  const dirty = JSON.stringify(saved) !== JSON.stringify(draft);

  return (
    <main className="settings-stage" style={{ "--surface-opacity": draft.opacity } as React.CSSProperties}>
      <section className="settings-window">
        <header className="settings-titlebar" onMouseDown={event => {
          if (!(event.target as HTMLElement).closest("button") && api.isAvailable()) getCurrentWindow().startDragging().catch(() => undefined);
        }}>
          <div><span className="title-icon"><Settings2 size={20} /></span><strong>CodePulse 设置</strong></div>
          <button aria-label="关闭设置" onClick={() => close(true)}><X size={19} /></button>
        </header>

        <div className="settings-content">
          <section className="settings-group">
            <h2>界面与行为</h2>
            <SettingRow icon={<Move size={19} />} title="窗口位置记忆" description="恢复上次使用的显示器和边缘位置" control={<Toggle label="窗口位置记忆" checked={draft.rememberPosition} onChange={value => patch({ rememberPosition: value })} />} />
            <SettingRow icon={<Gauge size={19} />} title="透明度" description="即时预览悬浮窗背景透明度" control={<div className="range-control"><input aria-label="透明度" type="range" min="0.72" max="1" step="0.01" value={draft.opacity} onChange={event => patch({ opacity: Number(event.target.value) })} /><span>{Math.round(draft.opacity * 100)}%</span></div>} />
            <SettingRow icon={<Sparkles size={19} />} title="动画效果" description="启用状态轨道和面板过渡" control={<Toggle label="动画效果" checked={draft.animations} onChange={value => patch({ animations: value })} />} />
            <SettingRow icon={<Activity size={19} />} title="始终置顶" description="让状态胶囊保持在其他窗口前方" control={<Toggle label="始终置顶" checked={draft.alwaysOnTop} onChange={value => patch({ alwaysOnTop: value })} />} />
            <SettingRow icon={<Move size={19} />} title="边缘吸附" description="拖到工作区边缘时自动对齐" control={<Toggle label="边缘吸附" checked={draft.edgeSnap} onChange={value => patch({ edgeSnap: value })} />} />
          </section>

          <section className="settings-group">
            <h2>启动与快捷</h2>
            <SettingRow icon={<Power size={19} />} title="开机启动" description="登录 Windows 后自动启动" control={<Toggle label="开机启动" checked={draft.launchAtStartup} onChange={value => patch({ launchAtStartup: value })} />} />
            <SettingRow icon={<MousePointer2 size={19} />} title="双击聚焦 Codex" description="双击胶囊切换到 Codex 窗口" control={<Toggle label="双击聚焦 Codex" checked={draft.doubleClickFocus} onChange={value => patch({ doubleClickFocus: value })} />} />
            <SettingRow icon={<BellRing size={19} />} title="状态受限提示" description="探测降级时在详情面板显示说明" control={<Toggle label="状态受限提示" checked={draft.showProviderWarnings} onChange={value => patch({ showProviderWarnings: value })} />} />

            <div className="capability-box">
              <div><Monitor size={18} /><span><strong>{display?.name || "当前显示器"}</strong><small>{display ? `${display.width}×${display.height} · ${Math.round(display.scaleFactor * 100)}%` : "正在读取显示信息"}</small></span></div>
              <div><ShieldCheck size={18} /><span><strong>{health.level === "healthy" ? "状态探测正常" : "状态探测受限"}</strong><small>{health.message}</small></span></div>
            </div>
          </section>
        </div>

        <footer className="settings-footer">
          <button className="reset-button" disabled={!dirty} onClick={() => setDraft(saved)}><RotateCcw size={15} />撤销更改</button>
          <div><button onClick={() => close(true)}>取消</button><button className="save-button" disabled={saving || !dirty} onClick={save}><Save size={15} />{saving ? "保存中" : "保存设置"}</button></div>
        </footer>
      </section>
    </main>
  );
}

export default function App() {
  const isSettings = new URLSearchParams(window.location.search).get("view") === "settings";
  return isSettings ? <SettingsWindow /> : <MainWidget />;
}