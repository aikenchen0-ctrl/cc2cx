import { ShieldAlert } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

export function isPermissionInstallError(
  message: string | null | undefined,
): boolean {
  if (!message) return false;
  const normalized = message.toLowerCase();
  const permissionSignals = [
    "permission denied",
    "operation not permitted",
    "access denied",
    "权限不足",
    "没有写入权限",
    "拒绝访问",
  ];
  return permissionSignals.some((signal) => normalized.includes(signal));
}

export function InstallPermissionGuide() {
  return (
    <Alert
      variant="destructive"
      className="border-amber-500/40 bg-amber-500/5 text-foreground"
      data-testid="agent-install-permission-guide"
    >
      <ShieldAlert className="h-4 w-4" />
      <AlertTitle>
        需要应用目录权限 / Application folder permission required
      </AlertTitle>
      <AlertDescription className="space-y-2 text-sm">
        <p>
          macOS 拒绝了应用目录的写入请求。/ macOS denied the application folder
          write request. 安装器会优先使用
          <span className="mx-1 font-mono">/Applications</span>
          ，失败后回退到当前用户的 / If that fails, it falls back to
          <span className="mx-1 font-mono">~/Applications</span>。
        </p>
        <ol className="list-decimal space-y-1 pl-5">
          <li>
            先完全退出正在运行的 ChatGPT 或 WorkBuddy。/ Quit ChatGPT or
            WorkBuddy first.
          </li>
          <li>
            在 Finder
            中打开“应用程序”，选中旧应用并在“显示简介”的“共享与权限”中给当前用户“读与写”权限。/
            In Finder, open Applications and grant your user Read &amp; Write
            access in Get Info &gt; Sharing &amp; Permissions.
          </li>
          <li>
            返回本页面点击“确定安装”重试；也可以将应用拖到个人的 Applications
            文件夹。/ Return here and retry, or drag the app into your personal
            Applications folder.
          </li>
        </ol>
        <p className="text-muted-foreground">
          不建议使用 sudo 启动本程序。/ Do not launch cc-launch with sudo. If an
          administrator owns the system folder, ask them to authorize it once
          and retry.
        </p>
      </AlertDescription>
    </Alert>
  );
}
