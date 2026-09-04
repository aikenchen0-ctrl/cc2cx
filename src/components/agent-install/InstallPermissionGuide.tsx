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
      <AlertTitle>需要应用目录权限</AlertTitle>
      <AlertDescription className="space-y-2 text-sm">
        <p>
          macOS 拒绝了应用目录的写入请求。安装器会优先使用
          <span className="mx-1 font-mono">/Applications</span>
          ，失败后回退到当前用户的
          <span className="mx-1 font-mono">~/Applications</span>。
        </p>
        <ol className="list-decimal space-y-1 pl-5">
          <li>先完全退出正在运行的 ChatGPT 或 WorkBuddy。</li>
          <li>
            在 Finder
            中打开“应用程序”，选中旧应用并在“显示简介”的“共享与权限”中给当前用户“读与写”权限。
          </li>
          <li>
            返回本页面点击“确定安装”重试；也可以将应用拖到个人的 Applications
            文件夹。
          </li>
        </ol>
        <p className="text-muted-foreground">
          不建议使用 sudo
          启动本程序。若系统目录由管理员维护，请让管理员完成一次授权后再重试。
        </p>
      </AlertDescription>
    </Alert>
  );
}
