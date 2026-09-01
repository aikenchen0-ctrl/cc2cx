import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface AgentInstallDependency {
  name: string;
  available: boolean;
  detail: string;
  kind?: "runtime" | "shell" | "system" | "permission" | "advisory";
  blocking?: boolean;
  auto_fixable?: boolean;
  action?: string | null;
  action_url?: string | null;
}

export interface AgentInstallStatus {
  id: string;
  name: string;
  description: string;
  installed: boolean;
  runnable: boolean;
  version: string | null;
  error: string | null;
  supported: boolean;
  unsupported_reason: string | null;
  command: string | null;
  dependencies: AgentInstallDependency[];
}

export interface AgentInstallOutput {
  agent_id: string;
  stream: "stdout" | "stderr";
  line: string;
}

export type AgentInstallPhase = "prepare" | "install" | "verify" | "complete";
export type AgentInstallProgressStatus = "running" | "success" | "error";

export interface AgentInstallProgress {
  agent_id: string;
  phase: AgentInstallPhase;
  status: AgentInstallProgressStatus;
  progress: number | null;
  message: string;
}

export interface NodeRuntimeStatus {
  available: boolean;
  version: string | null;
  installed: boolean;
  command: string | null;
  error: string | null;
}

export const agentInstallApi = {
  getStatuses: (): Promise<AgentInstallStatus[]> =>
    invoke("get_agent_install_statuses"),
  ensureNodeRuntime: (): Promise<NodeRuntimeStatus> =>
    invoke("ensure_node_runtime"),
  runInstall: (agentId: string): Promise<{ success: boolean }> =>
    invoke("run_agent_install", { agentId }),
  listenOutput: (handler: (output: AgentInstallOutput) => void) =>
    listen<AgentInstallOutput>("agent-install-output", ({ payload }) =>
      handler(payload),
    ),
  listenProgress: (handler: (progress: AgentInstallProgress) => void) =>
    listen<AgentInstallProgress>("agent-install-progress", ({ payload }) =>
      handler(payload),
    ),
};
