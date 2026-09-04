import { useCallback, useEffect, useState } from "react";
import {
  AlertCircle,
  CheckCircle2,
  Download,
  ExternalLink,
  Loader2,
  RefreshCw,
  Wrench,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { FullScreenPanel } from "@/components/common/FullScreenPanel";
import {
  agentInstallApi,
  type AgentInstallProgress,
  type AgentInstallStatus,
} from "@/lib/api/agentInstall";
import { settingsApi } from "@/lib/api/settings";
import {
  legacyMigrationApi,
  type LegacyCcSwitchStatus,
} from "@/lib/api/legacyMigration";
import { InstallConfirmationPanel } from "./InstallConfirmationPanel";
import {
  InstallOnboardingVisual,
  InstallProgressVisual,
} from "./InstallProgressVisual";
import {
  InstallPermissionGuide,
  isPermissionInstallError,
} from "./InstallPermissionGuide";

const AGENT_USAGE: Record<string, { command: string; hint: string }> = {
  "codex-ui": { command: "Codex UI", hint: "打开桌面应用后登录并开始对话" },
  claude: {
    command: "claude",
    hint: "在终端运行 claude 开始 Claude Code 会话",
  },
  codex: { command: "codex", hint: "在终端运行 codex 开始 Codex 会话" },
  opencode: { command: "opencode", hint: "在终端运行 opencode 开始会话" },
  workbuddy: { command: "WorkBuddy", hint: "打开 WorkBuddy 桌面应用" },
  gemini: { command: "gemini", hint: "在终端运行 gemini 开始 Gemini CLI 会话" },
  grokbuild: { command: "grok", hint: "在终端运行 grok 开始 Grok Build 会话" },
  openclaw: { command: "openclaw", hint: "在终端运行 openclaw 开始会话" },
  hermes: { command: "hermes", hint: "在终端运行 hermes 开始会话" },
  pi: { command: "pi", hint: "在终端运行 pi 开始会话" },
};

function hasUnfixableBlockingDependency(agent: AgentInstallStatus): boolean {
  return agent.dependencies.some(
    (dependency) =>
      dependency.blocking === true &&
      dependency.available === false &&
      dependency.auto_fixable !== true,
  );
}

function dependencyStatusLabel(
  dependency: AgentInstallStatus["dependencies"][number],
): string {
  if (dependency.available) return "可用";
  if (dependency.auto_fixable) return "可自动修复";
  if (dependency.blocking) return "需要处理";
  return "建议项";
}

export function AgentInstallPanel({
  isOpen,
  onClose,
}: {
  isOpen: boolean;
  onClose: () => void;
}) {
  const [agents, setAgents] = useState<AgentInstallStatus[]>([]);
  const [selected, setSelected] = useState<AgentInstallStatus | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isBatchInstalling, setIsBatchInstalling] = useState(false);
  const [batchMessage, setBatchMessage] = useState<string | null>(null);
  const [installProgress, setInstallProgress] =
    useState<AgentInstallProgress | null>(null);
  const [batchStep, setBatchStep] = useState({ index: 0, total: 0 });
  const [error, setError] = useState<string | null>(null);
  const [backgroundProgress, setBackgroundProgress] =
    useState<AgentInstallProgress | null>(null);
  const [isBackgroundInstalling, setIsBackgroundInstalling] = useState(false);
  const [legacyStatus, setLegacyStatus] = useState<LegacyCcSwitchStatus | null>(
    null,
  );
  const [selectedLegacySql, setSelectedLegacySql] = useState<string | null>(
    null,
  );
  const [legacyBusy, setLegacyBusy] = useState(false);
  const [legacyMessage, setLegacyMessage] = useState<string | null>(null);
  const load = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      setAgents(await agentInstallApi.getStatuses());
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setIsLoading(false);
    }
  }, []);
  useEffect(() => {
    if (isOpen) void load();
  }, [isOpen, load]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void agentInstallApi
      .listenProgress((progress) => {
        setInstallProgress(progress);
        if (isBackgroundInstalling) {
          setBackgroundProgress(progress);
        }
      })
      .then((dispose) => {
        unlisten = dispose;
      });
    return () => unlisten?.();
  }, [isBackgroundInstalling]);

  useEffect(() => {
    if (!backgroundProgress || backgroundProgress.status === "running") return;
    const timer = window.setTimeout(() => setBackgroundProgress(null), 4_000);
    return () => window.clearTimeout(timer);
  }, [backgroundProgress]);

  const runInBackground = () => {
    if (!selected) return;
    const agent = selected;
    setBackgroundProgress(null);
    setInstallProgress(null);
    setIsBackgroundInstalling(true);
    setSelected(null);
    void agentInstallApi
      .runInstall(agent.id)
      .then(() => load())
      .finally(() => setIsBackgroundInstalling(false))
      .catch((reason) => {
        const message =
          reason instanceof Error ? reason.message : String(reason);
        setError(`${agent.name}: ${message}`);
      });
  };

  const detectLegacy = async () => {
    setLegacyBusy(true);
    setLegacyMessage(null);
    try {
      const status = await legacyMigrationApi.detect();
      setLegacyStatus(status);
      setSelectedLegacySql(status.sql_exports?.[0]?.path ?? null);
      setLegacyMessage(
        status.detected
          ? "已发现旧 CC Switch 痕迹，请先备份再同步。/ Legacy CC Switch data found; back up before syncing."
          : "未发现旧 CC Switch 安装或数据。/ No legacy CC Switch installation or data found.",
      );
    } catch (reason) {
      setLegacyMessage(
        reason instanceof Error ? reason.message : String(reason),
      );
    } finally {
      setLegacyBusy(false);
    }
  };

  useEffect(() => {
    if (isOpen) void detectLegacy();
  }, [isOpen]);

  const importLegacy = async () => {
    setLegacyBusy(true);
    setLegacyMessage(null);
    try {
      const filePath =
        selectedLegacySql ?? (await settingsApi.openFileDialog());
      if (!filePath) return;
      await settingsApi.importConfigFromFile(filePath);
      await load();
      setLegacyMessage(
        "旧 CC Switch 配置已导入。原数据未删除，敏感登录凭据仍需在 Agent 中重新确认。/ Imported successfully. Legacy data was kept; recheck sensitive logins in each Agent.",
      );
    } catch (reason) {
      setLegacyMessage(
        reason instanceof Error ? reason.message : String(reason),
      );
    } finally {
      setLegacyBusy(false);
    }
  };

  const openLegacyUninstall = async () => {
    setLegacyBusy(true);
    try {
      await legacyMigrationApi.openUninstall();
    } catch (reason) {
      setLegacyMessage(
        reason instanceof Error ? reason.message : String(reason),
      );
    } finally {
      setLegacyBusy(false);
    }
  };

  const installMissing = async () => {
    const candidates = agents.filter(
      (agent) => agent.supported && !agent.installed && agent.command,
    );
    const blocked = candidates.filter(hasUnfixableBlockingDependency);
    const pending = candidates.filter(
      (agent) => !hasUnfixableBlockingDependency(agent),
    );
    if (!pending.length) {
      setBatchMessage(
        blocked.length
          ? `没有可直接安装的 Agent：${blocked
              .map((agent) => agent.name)
              .join("、")} 存在未处理的必要环境`
          : "没有需要安装的 Agent。",
      );
      return;
    }
    if (!candidates.length) {
      setBatchMessage("没有需要安装的 Agent。");
      return;
    }
    const nodeDependencyMissing = pending.some((agent) =>
      agent.dependencies.some(
        (dependency) => dependency.name === "Node.js" && !dependency.available,
      ),
    );

    setIsBatchInstalling(true);
    setBatchMessage(null);
    setError(null);
    setInstallProgress(null);
    const totalSteps = pending.length + (nodeDependencyMissing ? 1 : 0);
    setBatchStep({ index: 0, total: totalSteps });
    try {
      if (nodeDependencyMissing) {
        setBatchStep({ index: 0, total: totalSteps });
        setBatchMessage("正在准备 Node.js 运行时...");
        const node = await agentInstallApi.ensureNodeRuntime();
        if (!node.available) {
          throw new Error(
            node.error ?? "Node.js 安装失败，无法继续安装依赖 Agent",
          );
        }
      }

      const failures: string[] = [];
      for (const [index, agent] of pending.entries()) {
        setBatchStep({
          index: index + (nodeDependencyMissing ? 1 : 0),
          total: totalSteps,
        });
        setBatchMessage(`正在安装 ${agent.name}...`);
        try {
          await agentInstallApi.runInstall(agent.id);
        } catch (reason) {
          failures.push(
            `${agent.name}: ${reason instanceof Error ? reason.message : String(reason)}`,
          );
        }
      }
      await load();
      setBatchStep({ index: totalSteps, total: totalSteps });
      const skippedMessage = blocked.length
        ? `已跳过环境未满足项：${blocked.map((agent) => agent.name).join("、")}`
        : null;
      const resultMessage = failures.length
        ? `部分安装失败：${failures.join("；")}`
        : "缺失 Agent 安装完成。";
      setBatchMessage(
        skippedMessage ? `${resultMessage} ${skippedMessage}` : resultMessage,
      );
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setBatchMessage(null);
    } finally {
      setIsBatchInstalling(false);
    }
  };

  const nodeDependencyMissing = agents.some(
    (agent) =>
      !agent.installed &&
      agent.dependencies.some(
        (dependency) => dependency.name === "Node.js" && !dependency.available,
      ),
  );

  const visibleProgress =
    installProgress ??
    (isBatchInstalling
      ? {
          agent_id: nodeDependencyMissing ? "node-runtime" : "agent",
          phase: nodeDependencyMissing ? "prepare" : "install",
          status: "running",
          progress: null,
          message: batchMessage ?? "正在准备安装任务",
        }
      : null);

  return (
    <>
      <FullScreenPanel
        isOpen={isOpen && !selected && !isBackgroundInstalling}
        title="安装 Agent / Install Agents"
        onClose={onClose}
        footer={
          <Button
            variant="outline"
            onClick={() => void load()}
            disabled={isLoading}
          >
            <RefreshCw className="mr-2 h-4 w-4" />
            刷新状态 / Refresh
          </Button>
        }
      >
        <div className="mx-auto w-full max-w-5xl space-y-4">
          <div>
            <h3 className="text-base font-semibold">
              受管理的 Agent / Managed Agents
            </h3>
            <p className="mt-1 text-sm text-muted-foreground">
              安装后会立即重新检测命令是否可用。/ Status is rechecked after
              installation.
            </p>
          </div>
          <InstallOnboardingVisual phase={visibleProgress?.phase ?? null} />
          {visibleProgress && (
            <InstallProgressVisual
              progress={visibleProgress}
              stepIndex={batchStep.index}
              totalSteps={batchStep.total || 1}
            />
          )}
          <section className="space-y-3 border border-border-default px-4 py-3">
            <div>
              <h4 className="text-sm font-semibold">
                CC Switch 迁移 / CC Switch migration
              </h4>
              <p className="mt-1 text-xs text-muted-foreground">
                只读检测旧目录；配置同步使用 SQL
                导出并自动备份，不会删除旧数据。/ Read-only detection; SQL
                import creates a backup and never deletes legacy data.
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={() => void detectLegacy()}
                disabled={legacyBusy}
              >
                <RefreshCw className="mr-2 h-4 w-4" />
                检测旧 CC Switch / Detect
              </Button>
              <Button
                size="sm"
                onClick={() => void importLegacy()}
                disabled={legacyBusy}
              >
                <Download className="mr-2 h-4 w-4" />
                从导出文件同步 / Import export
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => void openLegacyUninstall()}
                disabled={legacyBusy}
              >
                <ExternalLink className="mr-2 h-4 w-4" />
                打开卸载入口 / Uninstall settings
              </Button>
            </div>
            {legacyStatus?.sql_exports &&
              legacyStatus.sql_exports.length > 0 && (
                <div className="space-y-2">
                  <label
                    htmlFor="legacy-sql-export"
                    className="text-xs font-medium text-foreground"
                  >
                    自动选择最新导出 / Auto-selected SQL export
                  </label>
                  <select
                    id="legacy-sql-export"
                    aria-label="自动选择 SQL 导出文件 / Auto-selected SQL export"
                    value={selectedLegacySql ?? ""}
                    onChange={(event) =>
                      setSelectedLegacySql(event.target.value)
                    }
                    className="h-9 w-full border border-border-default bg-background px-3 text-xs font-mono text-foreground"
                  >
                    {legacyStatus.sql_exports.map((candidate) => (
                      <option key={candidate.path} value={candidate.path}>
                        {candidate.path} (
                        {Math.ceil(candidate.size_bytes / 1024)} KB)
                      </option>
                    ))}
                  </select>
                  <p className="text-xs text-muted-foreground">
                    已自动选中最近修改的有效导出文件；也可以手动选择其他 SQL。/
                    The newest valid export is selected; you can choose another
                    SQL file manually.
                  </p>
                </div>
              )}
            <Button
              size="sm"
              variant="ghost"
              onClick={() =>
                void settingsApi.openFileDialog().then((path) => {
                  if (path) setSelectedLegacySql(path);
                })
              }
              disabled={legacyBusy}
            >
              <ExternalLink className="mr-2 h-4 w-4" />
              手动选择其他 SQL / Choose another SQL
            </Button>
            {legacyStatus?.detected && (
              <div className="space-y-1 text-xs text-amber-600 dark:text-amber-400">
                <div>
                  检测到旧数据目录 / Legacy data: {legacyStatus.data_dir}
                </div>
                {legacyStatus.install_paths.map((path) => (
                  <div key={path}>检测到旧程序 / Legacy app: {path}</div>
                ))}
                <div>
                  请先退出旧 CC Switch，迁移后再卸载；两个程序同时运行会争用
                  Agent 配置。/ Quit legacy CC Switch before syncing; running
                  both can overwrite Agent configuration.
                </div>
              </div>
            )}
            {legacyMessage && (
              <p className="text-xs text-muted-foreground">{legacyMessage}</p>
            )}
          </section>
          {agents.length > 0 && (
            <div className="flex flex-wrap items-center gap-2 border border-border-default px-4 py-3">
              <div className="min-w-0 flex-1 text-sm">
                <span className="font-medium">Node.js 运行时</span>
                <span className="ml-2 text-muted-foreground">
                  {nodeDependencyMissing
                    ? "缺失，安装 CLI Agent 时会自动补齐"
                    : "已满足 CLI Agent 依赖"}
                </span>
              </div>
              <Button
                size="sm"
                onClick={() => void installMissing()}
                disabled={isBatchInstalling || isLoading}
              >
                {isBatchInstalling ? (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                ) : (
                  <Download className="mr-2 h-4 w-4" />
                )}
                一键安装缺失 Agent / Install Missing
              </Button>
            </div>
          )}
          {batchMessage && (
            <p className="text-sm text-muted-foreground">{batchMessage}</p>
          )}
          {isLoading && !agents.length ? (
            <div className="flex items-center justify-center py-16 text-sm text-muted-foreground">
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              正在检测安装状态...
            </div>
          ) : (
            <div className="divide-y divide-border-default border border-border-default">
              {agents.map((agent) => (
                <div
                  key={agent.id}
                  className="flex min-h-20 items-center gap-4 px-4 py-3"
                >
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center border border-border-default">
                    <Wrench className="h-4 w-4 text-muted-foreground" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="font-medium">{agent.name}</div>
                    <div className="truncate text-sm text-muted-foreground">
                      {agent.description}
                      {agent.version ? ` · ${agent.version}` : ""}
                    </div>
                    {agent.install_path && (
                      <div className="mt-1 truncate text-xs text-muted-foreground">
                        <span className="font-medium text-foreground">
                          安装位置：
                        </span>
                        <span className="font-mono">{agent.install_path}</span>
                      </div>
                    )}
                    {agent.dependencies.length > 0 && (
                      <div
                        className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-xs"
                        data-testid={`agent-dependencies-${agent.id}`}
                      >
                        {agent.dependencies.map((dependency) => {
                          const available = dependency.available;
                          const blocking =
                            dependency.blocking === true && !available;
                          return (
                            <span
                              key={dependency.name}
                              className={
                                available
                                  ? "text-emerald-600 dark:text-emerald-400"
                                  : blocking
                                    ? "text-destructive"
                                    : "text-amber-600 dark:text-amber-400"
                              }
                              title={dependency.detail}
                            >
                              {dependency.name}:{" "}
                              {dependencyStatusLabel(dependency)}
                              {!available && `（${dependency.detail}）`}
                              {!available && dependency.action_url && (
                                <button
                                  type="button"
                                  className="ml-1 inline-flex items-center gap-1 underline underline-offset-2"
                                  onClick={() =>
                                    void settingsApi.openExternal(
                                      dependency.action_url!,
                                    )
                                  }
                                >
                                  {dependency.action ?? "查看官方说明"}
                                  <ExternalLink className="h-3 w-3" />
                                </button>
                              )}
                            </span>
                          );
                        })}
                      </div>
                    )}
                    {agent.installed && AGENT_USAGE[agent.id] && (
                      <div className="mt-2 flex flex-wrap items-center gap-2 text-xs">
                        <span className="text-muted-foreground">
                          使用说明 / How to use：{AGENT_USAGE[agent.id].hint}
                        </span>
                        <Button
                          size="sm"
                          variant="outline"
                          aria-label={`打开 ${agent.name}`}
                          onClick={() =>
                            void agentInstallApi
                              .launch(agent.id)
                              .catch((reason) =>
                                setError(
                                  reason instanceof Error
                                    ? reason.message
                                    : String(reason),
                                ),
                              )
                          }
                        >
                          <ExternalLink className="mr-1 h-3 w-3" />
                          打开 / Open
                        </Button>
                      </div>
                    )}
                    {!agent.supported && (
                      <div className="mt-1 text-xs text-amber-600 dark:text-amber-400">
                        {agent.unsupported_reason}
                      </div>
                    )}
                  </div>
                  <div className="flex shrink-0 items-center gap-2">
                    {agent.installed ? (
                      <>
                        <span title="已安装">
                          <CheckCircle2 className="h-5 w-5 text-emerald-500" />
                        </span>
                        <span
                          title={
                            agent.runnable
                              ? "可正常使用"
                              : (agent.error ?? "无法正常使用")
                          }
                        >
                          {agent.runnable ? (
                            <CheckCircle2 className="h-5 w-5 text-emerald-500" />
                          ) : (
                            <AlertCircle className="h-5 w-5 text-destructive" />
                          )}
                        </span>
                      </>
                    ) : agent.supported ? (
                      <Button
                        size="sm"
                        aria-label={`安装 ${agent.name}`}
                        title={
                          hasUnfixableBlockingDependency(agent)
                            ? "请先处理必要环境依赖"
                            : undefined
                        }
                        disabled={hasUnfixableBlockingDependency(agent)}
                        onClick={() => setSelected(agent)}
                      >
                        <Download className="mr-2 h-4 w-4" />
                        安装
                      </Button>
                    ) : (
                      <span
                        title={agent.unsupported_reason ?? "暂不支持自动安装"}
                      >
                        <AlertCircle className="h-5 w-5 text-muted-foreground" />
                      </span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
          {(error || batchMessage) &&
            isPermissionInstallError(error ?? batchMessage) && (
              <InstallPermissionGuide />
            )}
          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>
      </FullScreenPanel>
      {backgroundProgress && !selected && (
        <div
          data-testid="agent-install-floating-progress"
          className="fixed bottom-5 right-5 z-[80] w-80 rounded-xl border border-cyan-400/40 bg-slate-950/95 p-3 text-slate-100 shadow-2xl shadow-cyan-950/30"
        >
          <div className="flex items-center justify-between gap-3 text-xs">
            <span className="truncate">{backgroundProgress.message}</span>
            <span className="font-mono text-cyan-200">
              {backgroundProgress.progress == null
                ? "进行中 / Running"
                : `${backgroundProgress.progress}%`}
            </span>
          </div>
          <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-slate-800">
            <div
              className="h-full rounded-full bg-cyan-300 transition-all"
              style={{ width: `${backgroundProgress.progress ?? 38}%` }}
            />
          </div>
        </div>
      )}
      <InstallConfirmationPanel
        agent={selected}
        onClose={() => setSelected(null)}
        onInstalled={load}
        onBackgroundInstall={runInBackground}
      />
    </>
  );
}
