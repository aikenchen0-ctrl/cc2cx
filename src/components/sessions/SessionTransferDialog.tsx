import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowRightLeft,
  CheckCircle2,
  FolderOpen,
  Loader2,
  Play,
  TriangleAlert,
  X,
} from "lucide-react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { sessionsApi } from "@/lib/api/sessions";
import { settingsApi } from "@/lib/api/settings";
import { extractErrorMessage } from "@/utils/errorUtils";
import type {
  SessionMeta,
  SessionTransferResult,
  SessionTransferTarget,
} from "@/types";

export interface SessionTransferDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  session: SessionMeta | null;
  onTransferred?: (result: SessionTransferResult) => void;
}

type TransferState = "idle" | "transferring" | "success" | "error";

function extractTransferErrorCode(error: unknown): string {
  if (!error || typeof error !== "object") return "";
  const value = error as Record<string, unknown>;
  if (typeof value.code === "string") return value.code;
  if (value.payload && typeof value.payload === "object") {
    const payload = value.payload as Record<string, unknown>;
    return typeof payload.code === "string" ? payload.code : "";
  }
  return "";
}

export function SessionTransferDialog({
  open,
  onOpenChange,
  session,
  onTransferred,
}: SessionTransferDialogProps) {
  const { t } = useTranslation();
  const [targets, setTargets] = useState<SessionTransferTarget[]>([]);
  const [targetsLoading, setTargetsLoading] = useState(false);
  const [targetProvider, setTargetProvider] = useState("");
  const [workspace, setWorkspace] = useState("");
  const [state, setState] = useState<TransferState>("idle");
  const [result, setResult] = useState<SessionTransferResult | null>(null);
  const [errorMessage, setErrorMessage] = useState("");
  const [targetConflict, setTargetConflict] = useState(false);
  const requestGenerationRef = useRef(0);

  const writableTargets = useMemo(
    () =>
      targets.filter(
        (target) =>
          target.writeSupport.kind !== "readOnly" &&
          target.providerId !== session?.providerId,
      ),
    [session?.providerId, targets],
  );
  const selectedTarget = targets.find(
    (target) => target.providerId === targetProvider,
  );
  const resultTarget = result
    ? targets.find((target) => target.providerId === result.targetProviderId)
    : undefined;
  const canAutoLaunch = resultTarget?.launchSupport.kind === "supported";
  const sourceSupported =
    !session ||
    targets.some((target) => target.providerId === session.providerId);
  const sourceUnsupported = Boolean(
    session && !targetsLoading && targets.length > 0 && !sourceSupported,
  );

  const loadTargets = useCallback(
    async (generation = requestGenerationRef.current) => {
      setTargetsLoading(true);
      setErrorMessage("");
      try {
        const nextTargets = await sessionsApi.listTransferTargets();
        if (generation !== requestGenerationRef.current) return;
        setTargets(nextTargets);
        setTargetProvider((current) =>
          nextTargets.some(
            (target) =>
              target.providerId === current &&
              target.writeSupport.kind !== "readOnly" &&
              target.providerId !== session?.providerId,
          )
            ? current
            : (nextTargets.find(
                (target) =>
                  target.writeSupport.kind !== "readOnly" &&
                  target.providerId !== session?.providerId,
              )?.providerId ?? ""),
        );
        setState("idle");
      } catch (loadError) {
        if (generation !== requestGenerationRef.current) return;
        setTargets([]);
        setTargetProvider("");
        setErrorMessage(
          extractErrorMessage(loadError) ||
            t("sessionManager.transferTargetsFailed", {
              defaultValue: "无法加载目标工具",
            }),
        );
        setState("error");
      } finally {
        if (generation === requestGenerationRef.current) {
          setTargetsLoading(false);
        }
      }
    },
    [session?.providerId, t],
  );

  useEffect(() => {
    const generation = requestGenerationRef.current + 1;
    requestGenerationRef.current = generation;
    if (!open) {
      setTargetsLoading(false);
      return;
    }

    setWorkspace(session?.projectDir ?? "");
    setState("idle");
    setResult(null);
    setErrorMessage("");
    setTargetConflict(false);
    void loadTargets(generation);

    return () => {
      if (requestGenerationRef.current === generation) {
        requestGenerationRef.current += 1;
      }
    };
  }, [loadTargets, open, session]);

  const clearTransferError = useCallback(() => {
    setErrorMessage("");
    setTargetConflict(false);
    setState((current) => (current === "error" ? "idle" : current));
  }, []);

  const handlePickWorkspace = async () => {
    const generation = requestGenerationRef.current;
    try {
      const picked = await settingsApi.pickDirectory(workspace || undefined);
      if (generation !== requestGenerationRef.current) return;
      if (picked) {
        clearTransferError();
        setWorkspace(picked);
      }
    } catch (pickError) {
      if (generation !== requestGenerationRef.current) return;
      toast.error(
        extractErrorMessage(pickError) ||
          t("sessionManager.transferWorkspaceFailed", {
            defaultValue: "无法选择工作目录",
          }),
      );
    }
  };

  const handleTransfer = async (force = false) => {
    if (
      !session?.sourcePath ||
      !targetProvider ||
      targetProvider === session.providerId ||
      sourceUnsupported ||
      state === "transferring"
    ) {
      return;
    }

    const generation = requestGenerationRef.current;
    setState("transferring");
    setErrorMessage("");
    setTargetConflict(false);
    try {
      const nextResult = await sessionsApi.transfer({
        sourceProviderId: session.providerId,
        sourceSessionId: session.sessionId,
        sourcePath: session.sourcePath,
        targetProvider: targetProvider,
        workspace: workspace.trim() || null,
        force,
        enrich: false,
        maxContextTokens: 0,
        maxToolOutput: 0,
        keepReasoning: true,
      });
      if (generation !== requestGenerationRef.current) return;
      setResult(nextResult);
      setState("success");
      onTransferred?.(nextResult);
    } catch (transferError) {
      if (generation !== requestGenerationRef.current) return;
      const transferMessage = extractErrorMessage(transferError);
      const conflict =
        extractTransferErrorCode(transferError) === "target_conflict" ||
        transferMessage.toLowerCase().includes("already exists");
      setErrorMessage(
        transferMessage ||
          t("sessionManager.transferFailed", {
            defaultValue: "会话写回失败，请检查目标工具后重试",
          }),
      );
      setTargetConflict(conflict);
      setState("error");
    }
  };

  const handleLaunch = async () => {
    if (!result) return;
    const generation = requestGenerationRef.current;
    try {
      const launchResult = await sessionsApi.launchTransferred({
        targetProvider: result.targetProviderId,
        sessionId: result.targetSessionId,
        workspace: result.workspace ?? null,
      });
      if (generation !== requestGenerationRef.current) return;
      if (launchResult.launched) {
        toast.success(
          t("sessionManager.transferLaunched", {
            defaultValue: "已打开目标会话",
          }),
        );
      } else {
        toast.warning(
          launchResult.warning ||
            t("sessionManager.transferLaunchFailed", {
              defaultValue: "目标会话已写回，但无法自动打开",
            }),
        );
      }
    } catch (launchError) {
      if (generation !== requestGenerationRef.current) return;
      toast.error(
        extractErrorMessage(launchError) ||
          t("sessionManager.transferLaunchFailed", {
            defaultValue: "目标会话已写回，但无法自动打开",
          }),
      );
    }
  };

  const isBusy = targetsLoading || state === "transferring";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <ArrowRightLeft className="size-4" />
            {t("sessionManager.transferTitle", {
              defaultValue: "跨工具写回会话",
            })}
          </DialogTitle>
          <DialogDescription>
            {session
              ? `${session.title || session.sessionId} · ${session.providerId}`
              : t("sessionManager.noSession", {
                  defaultValue: "未选择会话",
                })}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 px-6 py-4">
          {sourceUnsupported && (
            <div
              role="alert"
              className="flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-sm text-amber-700 dark:text-amber-300"
            >
              <TriangleAlert className="mt-0.5 size-4 shrink-0" />
              <span>
                {t("sessionManager.transferSourceUnsupported", {
                  defaultValue: "此来源工具暂不支持跨工具写回",
                })}
              </span>
            </div>
          )}
          <div className="space-y-2">
            <label
              htmlFor="session-transfer-target"
              className="text-sm font-medium"
            >
              {t("sessionManager.transferTarget", { defaultValue: "目标工具" })}
            </label>
            <Select
              value={targetProvider}
              onValueChange={(value) => {
                clearTransferError();
                setTargetProvider(value);
              }}
              disabled={isBusy || state === "success"}
            >
              <SelectTrigger id="session-transfer-target" aria-label="目标工具">
                <SelectValue
                  placeholder={
                    targetsLoading
                      ? t("common.loading", { defaultValue: "加载中..." })
                      : t("sessionManager.transferTargetPlaceholder", {
                          defaultValue: "选择目标工具",
                        })
                  }
                />
              </SelectTrigger>
              <SelectContent>
                {targets.map((target) => {
                  const readOnly = target.writeSupport.kind === "readOnly";
                  const conditional =
                    target.writeSupport.kind === "conditional";
                  const manualLaunch =
                    target.launchSupport?.kind === "unsupported";
                  const executableMissing =
                    target.launchSupport?.kind === "executableMissing";
                  const sameProvider =
                    target.providerId === session?.providerId;
                  return (
                    <SelectItem
                      key={target.providerId}
                      value={target.providerId}
                      disabled={readOnly || sameProvider}
                    >
                      <span className="flex items-center gap-2">
                        <span>{target.name}</span>
                        {target.version && (
                          <span
                            aria-hidden="true"
                            className="text-[10px] text-muted-foreground"
                          >
                            v{target.version}
                          </span>
                        )}
                        {!target.installed && (
                          <Badge
                            aria-hidden="true"
                            variant="outline"
                            className="text-[10px]"
                          >
                            {t("sessionManager.notInstalled", {
                              defaultValue: "未安装",
                            })}
                          </Badge>
                        )}
                        {readOnly && (
                          <Badge variant="outline" className="text-[10px]">
                            {t("sessionManager.readOnly", {
                              defaultValue: "只读",
                            })}
                          </Badge>
                        )}
                        {conditional && (
                          <Badge
                            aria-hidden="true"
                            variant="outline"
                            className="text-[10px]"
                          >
                            {t("sessionManager.conditional", {
                              defaultValue: "条件写入",
                            })}
                          </Badge>
                        )}
                        {manualLaunch && (
                          <Badge
                            aria-hidden="true"
                            variant="outline"
                            className="text-[10px]"
                          >
                            {t("sessionManager.manualResume", {
                              defaultValue: "手动恢复",
                            })}
                          </Badge>
                        )}
                        {executableMissing && (
                          <Badge
                            aria-hidden="true"
                            variant="outline"
                            className="text-[10px]"
                          >
                            {t("sessionManager.executableMissing", {
                              defaultValue: "未找到 CLI",
                            })}
                          </Badge>
                        )}
                        {sameProvider && (
                          <Badge
                            aria-hidden="true"
                            variant="outline"
                            className="text-[10px]"
                          >
                            {t("sessionManager.sameProvider", {
                              defaultValue: "当前来源",
                            })}
                          </Badge>
                        )}
                      </span>
                    </SelectItem>
                  );
                })}
              </SelectContent>
            </Select>
            {selectedTarget?.writeSupport.reason && (
              <p className="text-xs text-muted-foreground">
                {selectedTarget.writeSupport.reason}
              </p>
            )}
            {selectedTarget?.launchSupport?.reason && (
              <p className="text-xs text-muted-foreground">
                {selectedTarget.launchSupport.reason}
              </p>
            )}
          </div>

          <div className="space-y-2">
            <label
              htmlFor="session-transfer-workspace"
              className="text-sm font-medium"
            >
              {t("sessionManager.transferWorkspace", {
                defaultValue: "工作目录",
              })}
            </label>
            <div className="flex items-center gap-2">
              <Input
                id="session-transfer-workspace"
                aria-label="工作目录"
                value={workspace}
                onChange={(event) => {
                  clearTransferError();
                  setWorkspace(event.target.value);
                }}
                placeholder={t("sessionManager.transferWorkspacePlaceholder", {
                  defaultValue: "沿用源会话目录",
                })}
                disabled={isBusy || state === "success"}
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label="选择目录"
                onClick={() => void handlePickWorkspace()}
                disabled={isBusy || state === "success"}
                title={t("sessionManager.transferChooseWorkspace", {
                  defaultValue: "选择目录",
                })}
              >
                <FolderOpen className="size-4" />
              </Button>
            </div>
          </div>

          {state !== "success" && (
            <div className="rounded-md border border-border-default bg-muted/30 p-3 text-xs">
              <p className="font-medium">
                {t("sessionManager.transferPotentialLosses", {
                  defaultValue: "可能丢失的字段",
                })}
              </p>
              <ul className="mt-1 list-disc space-y-1 pl-4 text-muted-foreground">
                <li>
                  {t("sessionManager.transferLossHiddenReasoning", {
                    defaultValue: "隐藏推理",
                  })}
                </li>
                <li>
                  {t("sessionManager.transferLossToolDetails", {
                    defaultValue: "工具调用细节",
                  })}
                </li>
                <li>
                  {t("sessionManager.transferLossModelMetadata", {
                    defaultValue: "模型元数据",
                  })}
                </li>
                <li>
                  {t("sessionManager.transferLossAttachments", {
                    defaultValue: "目标格式不支持的附件",
                  })}
                </li>
              </ul>
            </div>
          )}

          {selectedTarget?.writeSupport.kind === "conditional" && (
            <div className="flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-700 dark:text-amber-300">
              <TriangleAlert className="mt-0.5 size-4 shrink-0" />
              <span>
                {t("sessionManager.transferConditionalWarning", {
                  defaultValue:
                    "目标工具会在写回前检查本地格式；不兼容时不会修改目标数据。",
                })}
              </span>
            </div>
          )}

          {state === "error" && errorMessage && (
            <div
              role="alert"
              className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"
            >
              <X className="mt-0.5 size-4 shrink-0" />
              <span className="flex-1">{errorMessage}</span>
              {targetConflict && (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => void handleTransfer(true)}
                  disabled={isBusy}
                >
                  {t("sessionManager.transferOverwriteRetry", {
                    defaultValue: "覆盖并重试",
                  })}
                </Button>
              )}
              {targets.length === 0 && (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => void loadTargets()}
                  disabled={isBusy}
                >
                  {t("sessionManager.retryTransferTargets", {
                    defaultValue: "重试",
                  })}
                </Button>
              )}
            </div>
          )}

          {state === "success" && result && (
            <div
              role="status"
              className="space-y-3 rounded-md border border-emerald-500/30 bg-emerald-500/10 p-3 text-sm"
            >
              <div className="flex items-center gap-2 font-medium text-emerald-700 dark:text-emerald-300">
                <CheckCircle2 className="size-4" />
                {t("sessionManager.transferSuccess", {
                  defaultValue: "写回成功",
                })}
                {result.lossy && (
                  <Badge variant="outline" className="text-[10px]">
                    {t("sessionManager.transferLossy", {
                      defaultValue: "有损转换",
                    })}
                  </Badge>
                )}
              </div>
              <div className="text-xs text-muted-foreground">
                {result.targetProviderId} · {result.targetSessionId}
              </div>
              {result.writtenPaths.length > 0 && (
                <div className="space-y-1 text-xs text-muted-foreground">
                  <div>
                    {t("sessionManager.transferWrittenPaths", {
                      defaultValue: "写入位置",
                    })}
                  </div>
                  {result.writtenPaths.map((path) => (
                    <code
                      key={path}
                      className="block break-all rounded bg-background/60 px-2 py-1 font-mono"
                    >
                      {path}
                    </code>
                  ))}
                </div>
              )}
              {result.backupPath && (
                <div className="text-xs text-muted-foreground">
                  {t("sessionManager.transferBackupPath", {
                    defaultValue: "备份位置",
                  })}
                  <code className="ml-1 break-all font-mono">
                    {result.backupPath}
                  </code>
                </div>
              )}
              {result.warnings.length > 0 && (
                <ul className="list-disc space-y-1 pl-4 text-xs text-muted-foreground">
                  {result.warnings.map((warning) => (
                    <li key={warning}>{warning}</li>
                  ))}
                </ul>
              )}
              {!canAutoLaunch && (
                <div className="flex items-start gap-2 text-xs text-amber-700 dark:text-amber-300">
                  <TriangleAlert className="mt-0.5 size-4 shrink-0" />
                  <span>
                    {t("sessionManager.transferManualResume", {
                      defaultValue: "请在目标工具中手动恢复此会话",
                    })}
                  </span>
                </div>
              )}
            </div>
          )}
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={state === "transferring"}
          >
            {t("common.cancel", { defaultValue: "取消" })}
          </Button>
          {state === "success" && canAutoLaunch ? (
            <Button type="button" onClick={() => void handleLaunch()}>
              <Play className="size-4" />
              {t("sessionManager.transferOpen", { defaultValue: "打开新会话" })}
            </Button>
          ) : state !== "success" ? (
            <Button
              type="button"
              onClick={() => void handleTransfer()}
              disabled={
                isBusy ||
                !session?.sourcePath ||
                !targetProvider ||
                sourceUnsupported ||
                writableTargets.length === 0
              }
            >
              {state === "transferring" && (
                <Loader2 className="size-4 animate-spin" />
              )}
              {state === "transferring"
                ? t("sessionManager.transferPending", {
                    defaultValue: "写回中...",
                  })
                : t("sessionManager.transferConfirm", {
                    defaultValue: "确认写回",
                  })}
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
