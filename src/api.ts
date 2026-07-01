import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, StatusSnapshot } from "./types";

const isTauri = () => "__TAURI_INTERNALS__" in window;

export const api = {
  snapshot: (): Promise<StatusSnapshot> => invoke("get_status_snapshot"),
  settings: (): Promise<AppSettings> => invoke("get_settings"),
  updateSettings: (settings: AppSettings): Promise<void> => invoke("update_settings", { settings }),
  openCodex: (): Promise<void> => invoke("open_codex"),
  toggleWindow: (): Promise<void> => invoke("toggle_window"),
  isAvailable: isTauri
};
