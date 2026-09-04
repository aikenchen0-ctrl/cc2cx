import { useEffect, useState } from "react";
import {
  CircleCheck,
  CircleX,
  ExternalLink,
  Loader2,
  Terminal,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { FullScreenPanel } from "@/components/common/FullScreenPanel";
import {
  agentInstallApi,
  type AgentInstallProgress,
  type AgentInstallStatus,
} from "@/lib/api/agentInstall";
import { settingsApi } from "@/lib/api/settings";
import { InstallProgressVisual } from "./InstallProgressVisual";
import {
  InstallPermissionGuide,
  isPermissionInstallError,
} from "./InstallPermissionGuide";

const COMPLETION_USAGE: Record<string, string> = {
  "codex-ui":
    "打开桌面应用后登录并开始对话。/ Open the desktop app, sign in, and start a conversation.",
  claude:
    "在终端运行 claude 开始 Claude Code 会话。/ Run claude in a terminal to start a session.",
  codex:
    "在终端运行 codex 开始 Codex 会话。/ Run codex in a terminal to start a session.",
  opencode:
    "在终端运行 opencode 开始会话。/ Run opencode in a terminal to start a session.",
  workbuddy: "打开 WorkBuddy 桌面应用。/ Open the WorkBuddy desktop app.",
  gemini:
    "在终端运行 gemini 开始会话。/ Run gemini in a terminal to start a session.",
  grokbuild:
    "在终端运行 grok 开始会话。/ Run grok in a terminal to start a session.",
  openclaw:
    "在终端运行 openclaw 开始会话。/ Run openclaw in a terminal to start a session.",
  hermes:
    "在终端运行 hermes 开始会话。/ Run hermes in a terminal to start a session.",
  pi: "在终端运行 pi 开始会话。/ Run pi in a terminal to start a session.",
};

interface InstallConfirmationPanelProps {
  agent: AgentInstallStatus | null;
  onClose: () => void;
  onInstalled: () => Promise<void>;
  onBackgroundInstall: () => void;
}

function hasUnfixableBlockingDependency(agent: AgentInstallStatus): boolean {
  return agent.dependencies.some(
    (dependency) =>
      dependency.blocking === true &&
      dependency.available === false &&
      dependency.auto_fixable !== true,
  );
}

export function InstallConfirmationPanel({
  agent,
  onClose,
  onInstalled,
  onBackgroundInstall,
}: InstallConfirmationPanelProps) {
  const [isInstalling, setIsInstalling] = useState(false);
  const [lines, setLines] = useState<string[]>([]);
  const [progress, setProgress] = useState<AgentInstallProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [completed, setCompleted] = useState(false);

  useEffect(() => {
    if (!agent) return;
    setLines([]);
    setProgress(null);
    setError(null);
    setIsInstalling(false);
    setCompleted(false);
  }, [agent]);

  useEffect(() => {
    if (!agent) return;
    let unlisten: (() => void) | undefined;
    void agentInstallApi
      .listenOutput((output) => {
        if (output.agent_id === agent?.id)
          setLines((current) => [...current, output.line]);
      })
      .then((dispose) => {
        unlisten = dispose;
      });
    return () => unlisten?.();
  }, [agent]);

  useEffect(() => {
    if (!agent) return;
    let unlisten: (() => void) | undefined;
    void agentInstallApi
      .listenProgress((nextProgress) => {
        if (
          nextProgress.agent_id === agent?.id ||
          nextProgress.agent_id === "node-runtime"
        ) {
          setProgress(nextProgress);
        }
      })
      .then((dispose) => {
        unlisten = dispose;
      });
    return () => unlisten?.();
  }, [agent]);

  const install = async () => {
    if (!agent) return;
    setIsInstalling(true);
    setError(null);
    try {
      await agentInstallApi.runInstall(agent.id);
      await onInstalled();
      setCompleted(true);
      setLines((current) => [...current, "安装完成，已重新检测 Agent 状态。"]);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setError(message);
      setLines((current) => [...current, `安装失败: ${message}`]);
    } finally {
      setIsInstalling(false);
    }
  };

  return (
    <FullScreenPanel
      isOpen={Boolean(agent)}
      title={`安装 ${agent?.name ?? ""} / Install`}
      onClose={isInstalling ? () => undefined : onClose}
      motionPreset="slide-from-right"
      footer={
        <>
          <Button variant="outline" onClick={onClose} disabled={isInstalling}>
            取消 / Cancel
          </Button>
          <Button
            variant="outline"
            onClick={onBackgroundInstall}
            disabled={isInstalling || !agent?.command}
          >
            后台安装 / Install in background
          </Button>
          <Button
            onClick={() => void install()}
            disabled={
              isInstalling ||
              !agent?.command ||
              (agent ? hasUnfixableBlockingDependency(agent) : false)
            }
          >
            {isInstalling && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            确定安装 / Install
          </Button>
        </>
      }
    >
      {agent && (
        <div className="mx-auto w-full max-w-4xl space-y-6">
          <div>
            <h3 className="text-base font-semibold">{agent.name}</h3>
            <p className="mt-1 text-sm text-muted-foreground">
              {agent.description}
            </p>
          </div>
          <section className="space-y-2">
            <h4 className="text-sm font-medium">安装命令 / Install command</h4>
            <pre className="overflow-x-auto border border-border-default bg-muted p-4 text-sm font-mono whitespace-pre-wrap">
              {agent.command}
            </pre>
          </section>
          {progress && (
            <InstallProgressVisual
              progress={progress}
              stepIndex={0}
              totalSteps={1}
            />
          )}
          {completed && agent && COMPLETION_USAGE[agent.id] && (
            <section className="space-y-3 rounded-lg border border-emerald-400/40 bg-emerald-500/5 p-4">
              <h4 className="text-sm font-semibold">
                安装完成，如何使用 / Installed, how to use
              </h4>
              <p className="text-sm text-muted-foreground">
                {COMPLETION_USAGE[agent.id]}
              </p>
              <Button
                size="sm"
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
                <ExternalLink className="mr-2 h-4 w-4" />
                打开 / Open
              </Button>
            </section>
          )}
          {error && isPermissionInstallError(error) && (
            <InstallPermissionGuide />
          )}
          <section className="space-y-2">
            <h4 className="text-sm font-medium">依赖检查 / Dependency check</h4>
            <div className="divide-y divide-border-default border border-border-default">
              {agent.dependencies.map((dependency) => (
                <div
                  key={dependency.name}
                  className="flex items-center gap-3 px-4 py-3 text-sm"
                >
                  {dependency.available ? (
                    <CircleCheck className="h-4 w-4 text-emerald-500" />
                  ) : (
                    <CircleX className="h-4 w-4 text-destructive" />
                  )}
                  <span className="font-medium">{dependency.name}</span>
                  <span
                    className={
                      dependency.available
                        ? "text-emerald-600 dark:text-emerald-400"
                        : dependency.blocking
                          ? "text-destructive"
                          : "text-amber-600 dark:text-amber-400"
                    }
                  >
                    {dependency.available
                      ? "可用"
                      : dependency.auto_fixable
                        ? "可自动修复"
                        : dependency.blocking
                          ? "需要处理"
                          : "建议项"}
                  </span>
                  <span className="min-w-0 text-muted-foreground">
                    {dependency.detail}
                    {!dependency.available && dependency.action_url && (
                      <button
                        type="button"
                        className="ml-2 inline-flex items-center gap-1 underline underline-offset-2"
                        onClick={() =>
                          void settingsApi.openExternal(dependency.action_url!)
                        }
                      >
                        {dependency.action ?? "查看官方说明"}
                        <ExternalLink className="h-3 w-3" />
                      </button>
                    )}
                  </span>
                </div>
              ))}
            </div>
          </section>
          <section className="space-y-2">
            <h4 className="flex items-center gap-2 text-sm font-medium">
              <Terminal className="h-4 w-4" />
              执行输出 / Execution output
            </h4>
            <pre className="min-h-48 overflow-auto border border-border-default bg-zinc-950 p-4 text-xs text-zinc-100 font-mono whitespace-pre-wrap">
              {lines.length
                ? lines.join("\n")
                : "等待执行安装命令... / Waiting for the install command..."}
            </pre>
            {error && <p className="text-sm text-destructive">{error}</p>}
          </section>
        </div>
      )}
    </FullScreenPanel>
  );
}
