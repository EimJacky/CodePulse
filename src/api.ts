import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, DisplayInfo, StatusSnapshot } from "./types";

const isTauri = () => "__TAURI_INTERNALS__" in window;

export const api = {
  snapshot: (): Promise<StatusSnapshot> => invoke("get_status_snapshot"),
  settings: (): Promise<AppSettings> => invoke("get_settings"),
  displayInfo: (): Promise<DisplayInfo> => invoke("get_display_info"),
  previewSettings: (settings: AppSettings): Promise<void> => invoke("preview_settings", { settings }),
  saveSettings: (settings: AppSettings): Promise<void> => invoke("save_settings_command", { settings }),
  cancelSettings: (): Promise<void> => invoke("cancel_settings_preview"),
  setExpanded: (expanded: boolean): Promise<void> => invoke("set_main_expanded", { expanded }),
  showSettings: (): Promise<void> => invoke("show_settings_window"),
  hideSettings: (cancel: boolean): Promise<void> => invoke("hide_settings_window", { cancel }),
  setMainVisible: (visible: boolean): Promise<void> => invoke("set_main_window_visible", { visible }),
  openCodex: (): Promise<void> => invoke("open_codex"),
  showContextMenu: (): Promise<void> => invoke("show_context_menu"),
  isAvailable: isTauri
};