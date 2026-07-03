export type CodexStatus =
  | "offline"
  | "idle"
  | "thinking"
  | "executing"
  | "waitingApproval"
  | "completed"
  | "failed";

export type ThreadConfidence = "normal" | "limited";

export interface ThreadState {
  id: string;
  title: string;
  status: CodexStatus;
  statusSince: number;
  lastChangedAt: number;
  confidence: ThreadConfidence;
}

export interface ProviderHealth {
  level: "healthy" | "degraded" | "unavailable";
  message: string;
  lastSuccessfulRead: number | null;
}

export interface StatusSnapshot {
  aggregate: CodexStatus;
  threads: ThreadState[];
  health: ProviderHealth;
  observedAt: number;
}

export interface WindowPlacement {
  monitorName: string | null;
  anchor: "topLeft" | "topRight" | "bottomLeft" | "bottomRight";
  offsetX: number;
  offsetY: number;
  scaleFactor: number;
}

export interface AppSettings {
  opacity: number;
  animations: boolean;
  alwaysOnTop: boolean;
  edgeSnap: boolean;
  launchAtStartup: boolean;
  rememberPosition: boolean;
  doubleClickFocus: boolean;
  showProviderWarnings: boolean;
  placement: WindowPlacement | null;
}

export interface DisplayInfo {
  name: string;
  scaleFactor: number;
  width: number;
  height: number;
}

export const defaultSettings: AppSettings = {
  opacity: 0.94,
  animations: true,
  alwaysOnTop: true,
  edgeSnap: true,
  launchAtStartup: false,
  rememberPosition: true,
  doubleClickFocus: true,
  showProviderWarnings: true,
  placement: null
};