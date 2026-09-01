import { describe, expect, it, vi } from "vitest";
import {
  agentInstallApi,
  type AgentInstallDependency,
  type NodeRuntimeStatus,
} from "@/lib/api/agentInstall";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({
    available: true,
    version: "22.0.0",
    installed: false,
    command: null,
    error: null,
  } satisfies NodeRuntimeStatus),
}));

describe("agent install api", () => {
  it("defines structured dependency metadata for prerequisite rendering", () => {
    const dependency: AgentInstallDependency = {
      name: "Git Bash 或 WSL",
      available: false,
      detail: "Windows 需要 Git for Windows 的 Git Bash 或 WSL",
      kind: "shell",
      blocking: true,
      auto_fixable: false,
      action: "安装 Git for Windows 或启用 WSL",
    };

    expect(dependency.blocking).toBe(true);
    expect(dependency.auto_fixable).toBe(false);
    expect(dependency.action).toContain("Git");
  });

  it("exposes a structured node runtime check", async () => {
    const status = await agentInstallApi.ensureNodeRuntime();

    expect(status).toEqual({
      available: true,
      version: "22.0.0",
      installed: false,
      command: null,
      error: null,
    });
  });

  it("exposes installation progress subscriptions", async () => {
    const dispose = await agentInstallApi.listenProgress(() => undefined);

    expect(typeof dispose).toBe("function");
  });
});
