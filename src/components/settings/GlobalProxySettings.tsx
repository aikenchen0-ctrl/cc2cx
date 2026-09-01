/**
 * 全局出站代理设置组件
 *
 * 提供配置全局代理的输入界面，支持用户名密码认证。
 */

import { useState, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  AlertTriangle,
  Loader2,
  TestTube2,
  Search,
  Eye,
  EyeOff,
  X,
} from "lucide-react";
import {
  useGlobalProxyUrl,
  useSetGlobalProxyUrl,
  useTestProxy,
  useScanProxies,
  useCodexProxyStatus,
  useSetCodexProxyEnabled,
  type DetectedProxy,
} from "@/hooks/useGlobalProxy";

/** 从完整 URL 提取认证信息 */
function extractAuth(url: string): {
  baseUrl: string;
  username: string;
  password: string;
} {
  if (!url.trim()) return { baseUrl: "", username: "", password: "" };

  try {
    const parsed = new URL(url);
    const username = decodeURIComponent(parsed.username || "");
    const password = decodeURIComponent(parsed.password || "");
    // 移除认证信息，获取基础 URL
    parsed.username = "";
    parsed.password = "";
    return { baseUrl: parsed.toString(), username, password };
  } catch {
    return { baseUrl: url, username: "", password: "" };
  }
}

/** 将认证信息合并到 URL */
function mergeAuth(
  baseUrl: string,
  username: string,
  password: string,
): string {
  if (!baseUrl.trim()) return "";
  if (!username.trim()) return baseUrl;

  try {
    const parsed = new URL(baseUrl);
    // URL 对象的 username/password setter 会自动进行 percent-encoding
    // 不要使用 encodeURIComponent，否则会导致双重编码
    parsed.username = username.trim();
    if (password) {
      parsed.password = password;
    }
    return parsed.toString();
  } catch {
    // URL 解析失败，尝试手动插入（此时需要手动编码）
    const match = baseUrl.match(/^(\w+:\/\/)(.+)$/);
    if (match) {
      const auth = password
        ? `${encodeURIComponent(username.trim())}:${encodeURIComponent(password)}@`
        : `${encodeURIComponent(username.trim())}@`;
      return `${match[1]}${auth}${match[2]}`;
    }
    return baseUrl;
  }
}

export function GlobalProxySettings() {
  const { t } = useTranslation();
  const { data: savedUrl, isLoading } = useGlobalProxyUrl();
  const setMutation = useSetGlobalProxyUrl();
  const testMutation = useTestProxy();
  const scanMutation = useScanProxies();

  const [url, setUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [detected, setDetected] = useState<DetectedProxy[]>([]);

  // 计算完整 URL（含认证信息）
  const fullUrl = useMemo(
    () => mergeAuth(url, username, password),
    [url, username, password],
  );
  const codexProxyStatus = useCodexProxyStatus();
  const codexProxyMutation = useSetCodexProxyEnabled(fullUrl);

  // 同步远程配置
  useEffect(() => {
    if (savedUrl !== undefined) {
      const { baseUrl, username: u, password: p } = extractAuth(savedUrl || "");
      setUrl(baseUrl);
      setUsername(u);
      setPassword(p);
      setDirty(false);
    }
  }, [savedUrl]);

  const handleSave = async () => {
    if (codexProxyStatus.data?.enabled && fullUrl) {
      const result = await testMutation.mutateAsync(fullUrl);
      if (!result.success) return;
    }
    await setMutation.mutateAsync(fullUrl);
    if (codexProxyStatus.data?.enabled) {
      await codexProxyMutation.mutateAsync(Boolean(fullUrl));
    }
    setDirty(false);
  };

  const handleTest = async () => {
    if (fullUrl) {
      await testMutation.mutateAsync(fullUrl);
    }
  };

  const handleScan = async () => {
    const result = await scanMutation.mutateAsync();
    setDetected(result);
  };

  const handleCodexProxyToggle = async (enabled: boolean) => {
    if (enabled) {
      const result = await testMutation.mutateAsync(fullUrl);
      if (!result.success) return;
    }
    await codexProxyMutation.mutateAsync(enabled);
  };

  const handleSelect = (proxyUrl: string) => {
    const { baseUrl, username: u, password: p } = extractAuth(proxyUrl);
    setUrl(baseUrl);
    setUsername(u);
    setPassword(p);
    setDirty(true);
    setDetected([]);
  };

  const handleClear = () => {
    setUrl("");
    setUsername("");
    setPassword("");
    setDirty(true);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && dirty && !setMutation.isPending) {
      handleSave();
    }
  };

  // 只在首次加载且无数据时显示加载状态
  if (isLoading && savedUrl === undefined) {
    return (
      <div className="flex items-center justify-center p-4">
        <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {/* 描述 */}
      <p className="text-sm text-muted-foreground">
        {t("settings.globalProxy.hint")}
      </p>

      {/* 代理地址输入框和按钮 */}
      <div className="flex gap-2">
        <Input
          placeholder="http://127.0.0.1:7890 / socks5://127.0.0.1:1080"
          value={url}
          onChange={(e) => {
            setUrl(e.target.value);
            setDirty(true);
          }}
          onKeyDown={handleKeyDown}
          className="font-mono text-sm flex-1"
        />
        <Button
          variant="outline"
          size="icon"
          disabled={scanMutation.isPending}
          onClick={handleScan}
          title={t("settings.globalProxy.scan")}
        >
          {scanMutation.isPending ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Search className="h-4 w-4" />
          )}
        </Button>
        <Button
          variant="outline"
          size="icon"
          disabled={!fullUrl || testMutation.isPending}
          onClick={handleTest}
          title={t("settings.globalProxy.test")}
        >
          {testMutation.isPending ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <TestTube2 className="h-4 w-4" />
          )}
        </Button>
        <Button
          variant="outline"
          size="icon"
          disabled={!url && !username && !password}
          onClick={handleClear}
          title={t("settings.globalProxy.clear")}
        >
          <X className="h-4 w-4" />
        </Button>
        <Button
          onClick={handleSave}
          disabled={!dirty || setMutation.isPending}
          size="sm"
        >
          {setMutation.isPending && (
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          )}
          {t("common.save")}
        </Button>
      </div>

      {/* 认证信息：用户名 + 密码（可选） */}
      <div className="flex gap-2">
        <Input
          placeholder={t("settings.globalProxy.username")}
          value={username}
          onChange={(e) => {
            setUsername(e.target.value);
            setDirty(true);
          }}
          onKeyDown={handleKeyDown}
          className="font-mono text-sm flex-1"
        />
        <div className="relative flex-1">
          <Input
            type={showPassword ? "text" : "password"}
            placeholder={t("settings.globalProxy.password")}
            value={password}
            onChange={(e) => {
              setPassword(e.target.value);
              setDirty(true);
            }}
            onKeyDown={handleKeyDown}
            className="font-mono text-sm pr-10"
          />
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="absolute right-0 top-0 h-full px-3 hover:bg-transparent"
            onClick={() => setShowPassword(!showPassword)}
            tabIndex={-1}
          >
            {showPassword ? (
              <EyeOff className="h-4 w-4 text-muted-foreground" />
            ) : (
              <Eye className="h-4 w-4 text-muted-foreground" />
            )}
          </Button>
        </div>
      </div>

      {/* 扫描结果 */}
      {detected.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {detected.map((p) => (
            <Button
              key={p.url}
              variant="secondary"
              size="sm"
              onClick={() => handleSelect(p.url)}
              className="font-mono text-xs"
            >
              {p.url}
            </Button>
          ))}
        </div>
      )}

      <div className="rounded-lg border border-border-default bg-muted/30 p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 space-y-1">
            <label htmlFor="codex-proxy-sync" className="text-sm font-medium">
              {t("settings.globalProxy.codex.enable")}
            </label>
            <p className="text-xs text-muted-foreground">
              {t("settings.globalProxy.codex.description")}
            </p>
            {codexProxyStatus.data?.path && (
              <p className="break-all font-mono text-[11px] text-muted-foreground">
                {codexProxyStatus.data.path}
              </p>
            )}
          </div>
          <Switch
            id="codex-proxy-sync"
            aria-label={t("settings.globalProxy.codex.enable")}
            checked={codexProxyStatus.data?.enabled ?? false}
            disabled={
              codexProxyStatus.isLoading ||
              codexProxyMutation.isPending ||
              (!codexProxyStatus.data?.enabled && !fullUrl) ||
              (!codexProxyStatus.data?.enabled && dirty)
            }
            onCheckedChange={(enabled) =>
              void handleCodexProxyToggle(enabled).catch(() => undefined)
            }
          />
        </div>
        {codexProxyStatus.data?.enabled && (
          <p className="mt-3 text-xs text-amber-600 dark:text-amber-400">
            {t("settings.globalProxy.codex.restartHint")}
          </p>
        )}
        {codexProxyStatus.data?.enabled &&
          codexProxyStatus.data.portReachable === false && (
            <div className="mt-3 flex items-start gap-2 text-xs text-destructive">
              <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              <span>{t("settings.globalProxy.codex.portUnavailable")}</span>
            </div>
          )}
        {codexProxyStatus.data?.envTxtDetected && (
          <div className="mt-3 flex items-start gap-2 text-xs text-amber-600 dark:text-amber-400">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>{t("settings.globalProxy.codex.envTxtDetected")}</span>
          </div>
        )}
        {codexProxyStatus.error && (
          <div className="mt-3 flex items-start gap-2 text-xs text-destructive">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>
              {t("settings.globalProxy.codex.statusFailed", {
                error:
                  codexProxyStatus.error instanceof Error
                    ? codexProxyStatus.error.message
                    : String(codexProxyStatus.error),
              })}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
