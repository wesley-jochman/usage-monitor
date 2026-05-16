import type { AppSettings, AppStatus, UsageSnapshot } from "@/types/contracts";
import { invoke } from "@tauri-apps/api/core";

export const tauriApi = {
  getAppStatus: (): Promise<AppStatus> => invoke("get_app_status"),
  getUsageSnapshot: (): Promise<UsageSnapshot> => invoke("get_usage_snapshot"),
  refreshUsage: (): Promise<UsageSnapshot> => invoke("refresh_usage"),
  getSettings: (): Promise<AppSettings> => invoke("get_settings"),
  updateSettings: (settings: AppSettings): Promise<AppSettings> =>
    invoke("update_settings", { settings }),
  setStartOnLogin: (enabled: boolean): Promise<void> =>
    invoke("set_start_on_login", { enabled }),
  showMainWindow: (): Promise<void> => invoke("show_main_window"),
  hideMainWindow: (): Promise<void> => invoke("hide_main_window"),
  quitApp: (): Promise<void> => invoke("quit_app"),
};
