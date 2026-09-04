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
import { InstallConfirmationPanel } from "./InstallConfirmationPanel";
import {
  InstallOnboardingVisual,
  InstallProgressVisual,
} from "./InstallProgressVisual";
import {
  InstallPermissionGuide,
  isPermissionInstallError,
} from "./InstallPermissionGuide";

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
    if (!isOpen) return;
    let unlisten: (() => void) | undefined;
    void agentInstallApi
      .listenProgress((progress) => {
        setInstallProgress(progress);
      })
      .then((dispose) => {
        unlisten = dispose;
      });
    return () => unlisten?.();
  }, [isOpen]);

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
        isOpen={isOpen && !selected}
        title="安装 Agent"
        onClose={onClose}
        footer={
          <Button
            variant="outline"
            onClick={() => void load()}
            disabled={isLoading}
          >
            <RefreshCw className="mr-2 h-4 w-4" />
            刷新状态
          </Button>
        }
      >
        <div className="mx-auto w-full max-w-5xl space-y-4">
          <div>
            <h3 className="text-base font-semibold">受管理的 Agent</h3>
            <p className="mt-1 text-sm text-muted-foreground">
              安装后会立即重新检测命令是否可用。
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
                一键安装缺失 Agent
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
      <InstallConfirmationPanel
        agent={selected}
        onClose={() => setSelected(null)}
        onInstalled={load}
      />
    </>
  );
}
