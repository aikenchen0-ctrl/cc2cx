import { invoke } from "@tauri-apps/api/core";

export interface LegacyCcSwitchStatus {
  detected: boolean;
  data_dir: string | null;
  database_path: string | null;
  config_path: string | null;
  skills_dir: string | null;
  backups_dir: string | null;
  install_paths: string[];
}

export const legacyMigrationApi = {
  detect: (): Promise<LegacyCcSwitchStatus> =>
    invoke("detect_legacy_cc_switch"),
  openUninstall: (): Promise<void> =>
    invoke("open_legacy_cc_switch_uninstall").then(() => undefined),
};
