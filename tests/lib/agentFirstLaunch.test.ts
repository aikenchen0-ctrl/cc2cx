import { describe, expect, it } from "vitest";
import {
  AGENT_FIRST_LAUNCH_STORAGE_KEY,
  shouldOpenAgentInstallOnFirstLaunch,
  shouldOpenAgentInstallOnFirstScan,
} from "@/lib/agentFirstLaunch";
import type { AgentInstallStatus } from "@/lib/api/agentInstall";

const status = (
  id: string,
  overrides: Partial<AgentInstallStatus> = {},
): AgentInstallStatus => ({
  id,
  name: id,
  description: `${id} description`,
  installed: true,
  runnable: true,
  version: "1.0.0",
  error: null,
  supported: true,
  unsupported_reason: null,
  command: null,
  dependencies: [],
  ...overrides,
});

describe("agent first launch", () => {
  it("first scan opens the installer when a target agent is missing", () => {
    const statuses = [
      status("codex", { installed: false, runnable: false, version: null }),
      status("claude"),
      status("opencode"),
      status("codex-ui"),
      status("workbuddy"),
    ];

    expect(shouldOpenAgentInstallOnFirstScan(statuses, false)).toBe(true);
  });

  it("first scan opens the installer when a target dependency is unavailable", () => {
    const statuses = [
      status("codex", {
        dependencies: [
          { name: "Node.js", available: false, detail: "未检测到 node 命令" },
        ],
      }),
      status("claude"),
      status("opencode"),
      status("codex-ui"),
      status("workbuddy"),
    ];

    expect(shouldOpenAgentInstallOnFirstScan(statuses, false)).toBe(true);
  });

  it("does not reopen after the first scan marker was written", () => {
    expect(
      shouldOpenAgentInstallOnFirstScan(
        [status("codex", { installed: false, runnable: false })],
        true,
      ),
    ).toBe(false);
  });

  it("opens the installer after every successful first-launch scan", () => {
    expect(shouldOpenAgentInstallOnFirstLaunch([], false)).toBe(true);
    expect(shouldOpenAgentInstallOnFirstLaunch([], true)).toBe(false);
  });

  it("falls back to the managed status list when legacy ids are returned", () => {
    expect(
      shouldOpenAgentInstallOnFirstScan(
        [
          status("claude", {
            installed: false,
            runnable: false,
            version: null,
          }),
        ],
        false,
      ),
    ).toBe(true);
  });

  it("exports a stable storage key for the first scan marker", () => {
    expect(AGENT_FIRST_LAUNCH_STORAGE_KEY).toBe(
      "cc-launch-agent-first-scan-complete",
    );
  });
});
