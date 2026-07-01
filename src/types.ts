export type CodexStatus =
  | "offline"
  | "idle"
  | "thinking"
  | "executing"
  | "waitingApproval"
  | "completed"
  | "failed";

export interface ThreadState {
  id: string;
  title: string;
  status: CodexStatus;
  since: number;
  lastChangedAt: number;
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

export interface AppSettings {
  opacity: number;
  animations: boolean;
  alwaysOnTop: boolean;
  edgeSnap: boolean;
  launchAtStartup: boolean;
  paused: boolean;
}

export const defaultSettings: AppSettings = {
  opacity: 0.96,
  animations: true,
  alwaysOnTop: true,
  edgeSnap: true,
  launchAtStartup: false,
  paused: false
};
