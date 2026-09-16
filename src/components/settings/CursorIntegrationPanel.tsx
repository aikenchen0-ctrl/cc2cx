import { useCallback, useEffect, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  KeyRound,
  Loader2,
  Play,
  RefreshCw,
  ShieldCheck,
  Square,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  settingsApi,
  type CursorHarnessStatus,
  type CursorIntegrationState,
} from "@/lib/api/settings";

type CursorAction =
  | "refresh"
  | "initialize"
  | "install"
  | "uninstall"
  | "start"
  | "stop"
  | null;

const isProxyActive = (status: CursorHarnessStatus | null) =>
  status?.state === "starting" ||
  status?.state === "running" ||
  status?.state === "degraded" ||
  status?.state === "stopping";

export function CursorIntegrationPanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<CursorHarnessStatus | null>(null);
  const [action, setAction] = useState<CursorAction>("refresh");

  const refresh = useCallback(async () => {
    try {
      setStatus(await settingsApi.getCursorHarnessStatus());
    } catch (error) {
      console.error("[CursorIntegrationPanel] Failed to load status", error);
      setStatus(null);
    } finally {
      setAction((current) => (current === "refresh" ? null : current));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const interval = window.setInterval(
      () => {
        void refresh();
      },
      status && isProxyActive(status) ? 3_000 : 10_000,
    );
    return () => window.clearInterval(interval);
  }, [refresh, status]);

  const runAction = async (
    nextAction: Exclude<CursorAction, "refresh" | null>,
    operation: () => Promise<CursorHarnessStatus>,
    successKey: string,
  ) => {
    setAction(nextAction);
    try {
      setStatus(await operation());
      toast.success(t(successKey));
    } catch (error) {
      console.error(`[CursorIntegrationPanel] ${nextAction} failed`, error);
      toast.error(String(error));
      await refresh();
    } finally {
      setAction(null);
    }
  };

  const busy = action !== null;
  const active = isProxyActive(status);
  const showTransparentHint = Boolean(
    status?.transparentEntry?.running || status?.managedHosts,
  );
  const stateLabel = (state: CursorIntegrationState) =>
    t(`settings.advanced.cursor.states.${state}`);
  const caLabel = status
    ? t(`settings.advanced.cursor.caStates.${status.ca}`)
    : t("common.loading");

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex items-start gap-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-background ring-1 ring-border">
            <ShieldCheck className="h-5 w-5 text-sky-500" />
          </div>
          <div>
            <h4 className="text-sm font-semibold">
              {t("settings.advanced.cursor.title")}
            </h4>
            <p className="mt-1 max-w-2xl text-xs text-muted-foreground">
              {t("settings.advanced.cursor.description")}
            </p>
          </div>
        </div>
        <Button
          variant="outline"
          size="icon"
          title={t("settings.advanced.cursor.refresh")}
          aria-label={t("settings.advanced.cursor.refresh")}
          disabled={busy}
          onClick={() => {
            setAction("refresh");
            void refresh();
          }}
        >
          {action === "refresh" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <RefreshCw className="h-4 w-4" />
          )}
        </Button>
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <div className="rounded-lg border border-border bg-card/50 p-3">
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs text-muted-foreground">
              {t("settings.advanced.cursor.integrationState")}
            </span>
            <Badge
              variant={status?.state === "running" ? "default" : "secondary"}
            >
              {status ? stateLabel(status.state) : t("common.loading")}
            </Badge>
          </div>
          <p className="mt-2 text-xs text-muted-foreground">
            {status?.proxyUrl
              ? `${t("settings.advanced.cursor.proxyAddress")}: ${status.proxyUrl}`
              : t("settings.advanced.cursor.proxyUnavailable")}
          </p>
          {showTransparentHint && (
            <>
              <p className="mt-1 text-xs text-muted-foreground">
                {status?.transparentEntry?.running
                  ? t("settings.advanced.cursor.transparentActive", {
                      addresses: status.transparentEntry.addresses.join(", "),
                    })
                  : t("settings.advanced.cursor.transparentInactive")}
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                {status?.managedHosts
                  ? t("settings.advanced.cursor.hostsManaged")
                  : t("settings.advanced.cursor.hostsUnmanaged")}
              </p>
            </>
          )}
        </div>

        <div className="rounded-lg border border-border bg-card/50 p-3">
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs text-muted-foreground">
              {t("settings.advanced.cursor.caState")}
            </span>
            <Badge variant={status?.ca === "ready" ? "default" : "secondary"}>
              {status?.ca === "ready" && (
                <CheckCircle2 className="mr-1 h-3 w-3" />
              )}
              {caLabel}
            </Badge>
          </div>
          <p className="mt-2 text-xs text-muted-foreground">
            {status?.settingsBackupPresent
              ? t("settings.advanced.cursor.backupPresent")
              : t("settings.advanced.cursor.backupAbsent")}
          </p>
        </div>
      </div>

      {status?.backendHealth && status.backendHealth !== "not_required" && (
        <div className="rounded-lg border border-border bg-card/50 p-3 text-xs">
          <div className="flex items-center justify-between gap-2">
            <span className="text-muted-foreground">
              {t("settings.advanced.cursor.backendHealth")}
            </span>
            <Badge
              variant={
                status.backendHealth === "healthy" ||
                status.backendHealth === "not_observed"
                  ? "default"
                  : "destructive"
              }
            >
              {t(
                `settings.advanced.cursor.backendHealthStates.${status.backendHealth}`,
              )}
            </Badge>
          </div>
          {status.backendErrorCode && (
            <p className="mt-2 text-muted-foreground">
              {t("settings.advanced.cursor.backendError", {
                code: status.backendErrorCode,
              })}
            </p>
          )}
        </div>
      )}

      {status?.ca === "untrusted" && (
        <div className="flex items-start gap-2 rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-700 dark:text-amber-300">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{t("settings.advanced.cursor.untrustedHint")}</span>
        </div>
      )}

      {!active && showTransparentHint && (
        <div className="flex items-start gap-2 rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-amber-700 dark:text-amber-300">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{t("settings.advanced.cursor.hostsElevationHint")}</span>
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        <Button
          size="sm"
          disabled={busy || active || !status}
          onClick={() =>
            void runAction(
              "start",
              settingsApi.startCursorIntegration,
              "settings.advanced.cursor.startSuccess",
            )
          }
        >
          {action === "start" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Play className="h-4 w-4" />
          )}
          {t("settings.advanced.cursor.start")}
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={busy || !active}
          onClick={() =>
            void runAction(
              "stop",
              settingsApi.stopCursorIntegration,
              "settings.advanced.cursor.stopSuccess",
            )
          }
        >
          {action === "stop" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Square className="h-4 w-4" />
          )}
          {t("settings.advanced.cursor.stop")}
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={busy || active || !status || status.ca === "ready"}
          onClick={() =>
            void runAction(
              status?.ca === "untrusted" ? "install" : "initialize",
              status?.ca === "untrusted"
                ? settingsApi.installCursorCa
                : settingsApi.initializeCursorCa,
              status?.ca === "untrusted"
                ? "settings.advanced.cursor.installSuccess"
                : "settings.advanced.cursor.initializeSuccess",
            )
          }
        >
          {action === "install" || action === "initialize" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <KeyRound className="h-4 w-4" />
          )}
          {status?.ca === "untrusted"
            ? t("settings.advanced.cursor.installCa")
            : t("settings.advanced.cursor.initializeCa")}
        </Button>
        <Button
          variant="destructive"
          size="sm"
          disabled={busy || active || !status || status.ca === "missing"}
          onClick={() =>
            void runAction(
              "uninstall",
              settingsApi.uninstallCursorCa,
              "settings.advanced.cursor.uninstallSuccess",
            )
          }
        >
          {action === "uninstall" ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Trash2 className="h-4 w-4" />
          )}
          {t("settings.advanced.cursor.uninstallCa")}
        </Button>
      </div>

      {status?.caInstallCommand && status.ca !== "ready" && (
        <p className="break-all text-[11px] text-muted-foreground">
          {t("settings.advanced.cursor.installCommand")}:{" "}
          {status.caInstallCommand}
        </p>
      )}
    </div>
  );
}
