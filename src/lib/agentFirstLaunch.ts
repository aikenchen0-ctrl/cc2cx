import type { AgentInstallStatus } from "./api/agentInstall";

export const AGENT_FIRST_LAUNCH_STORAGE_KEY =
  "cc-launch-agent-first-scan-complete";

export const FIRST_LAUNCH_AGENT_IDS = [
  "codex-ui",
  "codex",
  "claude",
  "opencode",
  "workbuddy",
] as const;

export function hasAgentInstallIssue(statuses: AgentInstallStatus[]): boolean {
  const relevantStatuses = statuses.some((status) =>
    FIRST_LAUNCH_AGENT_IDS.includes(
      status.id as (typeof FIRST_LAUNCH_AGENT_IDS)[number],
    ),
  )
    ? statuses.filter((status) =>
        FIRST_LAUNCH_AGENT_IDS.includes(
          status.id as (typeof FIRST_LAUNCH_AGENT_IDS)[number],
        ),
      )
    : statuses;
  const byId = new Map(relevantStatuses.map((status) => [status.id, status]));

  const ids = relevantStatuses.some((status) =>
    FIRST_LAUNCH_AGENT_IDS.includes(
      status.id as (typeof FIRST_LAUNCH_AGENT_IDS)[number],
    ),
  )
    ? FIRST_LAUNCH_AGENT_IDS
    : relevantStatuses.map((status) => status.id);

  return ids.some((id) => {
    const status = byId.get(id);
    if (!status) return true;
    if (!status.supported || !status.installed || !status.runnable) return true;
    return status.dependencies.some((dependency) => !dependency.available);
  });
}

export function shouldOpenAgentInstallOnFirstScan(
  statuses: AgentInstallStatus[],
  firstScanComplete: boolean,
): boolean {
  return !firstScanComplete && hasAgentInstallIssue(statuses);
}

export function shouldOpenAgentInstallOnFirstLaunch(
  _statuses: AgentInstallStatus[],
  firstScanComplete: boolean,
): boolean {
  return !firstScanComplete;
}
