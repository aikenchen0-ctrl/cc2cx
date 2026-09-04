import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AgentInstallPanel } from "@/components/agent-install/AgentInstallPanel";
import {
  agentInstallApi,
  type AgentInstallStatus,
  type AgentInstallProgress,
} from "@/lib/api/agentInstall";

let progressHandler: ((progress: AgentInstallProgress) => void) | undefined;

vi.mock("@/lib/api/agentInstall", () => ({
  agentInstallApi: {
    getStatuses: vi.fn(),
    ensureNodeRuntime: vi.fn(),
    runInstall: vi.fn(),
    listenOutput: vi.fn().mockResolvedValue(() => undefined),
    listenProgress: vi.fn().mockImplementation(async (handler) => {
      progressHandler = handler;
      return () => {
        progressHandler = undefined;
      };
    }),
  },
}));

const statuses: AgentInstallStatus[] = [
  ["codex-ui", "Codex UI", false, false],
  ["claude", "Claude Code", true, true],
  ["claude-desktop", "Claude Desktop", false, false],
  ["codex", "Codex", false, false],
  ["gemini", "Gemini CLI", true, false],
  ["grokbuild", "Grok Build", true, true],
  ["opencode", "OpenCode", true, true],
  ["openclaw", "OpenClaw", true, true],
  ["hermes", "Hermes", true, true],
  ["pi", "Pi", true, true],
  ["workbuddy", "WorkBuddy", false, false],
].map(([id, name, installed, runnable]) => ({
  id: String(id),
  name: String(name),
  description: `${name} description`,
  installed: Boolean(installed),
  runnable: Boolean(runnable),
  version: installed ? "1.0.0" : null,
  error: runnable ? null : "not available",
  supported: id !== "claude-desktop",
  unsupported_reason:
    id === "claude-desktop"
      ? "请通过 Anthropic 官方安装器安装 Claude Desktop"
      : null,
  command:
    id === "codex"
      ? "npm install -g @openai/codex"
      : id === "codex-ui"
        ? "winget install --id 9PLM9XGG6VKS"
        : id === "workbuddy"
          ? "官方 WorkBuddy 安装器"
          : null,
  dependencies: [
    {
      name: "Node.js",
      available: true,
      detail: "node --version",
      kind: "runtime" as const,
      blocking: true,
      auto_fixable: true,
      action: "安装 Node.js LTS",
    },
  ],
}));

const blockingClaudeStatus: AgentInstallStatus = {
  ...statuses.find((agent) => agent.id === "claude")!,
  installed: false,
  runnable: false,
  version: null,
  command: "npm install -g @anthropic-ai/claude-code",
  dependencies: [
    {
      name: "Node.js",
      available: true,
      detail: "已检测到 Node.js 18+ 与 npm",
      kind: "runtime",
      blocking: true,
      auto_fixable: true,
      action: "安装 Node.js LTS",
    },
    {
      name: "Git Bash 或 WSL",
      available: false,
      detail: "Windows 需要 Git for Windows 的 Git Bash 或 WSL",
      kind: "shell",
      blocking: true,
      auto_fixable: false,
      action: "安装 Git for Windows 或启用 WSL",
    },
  ],
};

describe("AgentInstallPanel", () => {
  beforeEach(() => {
    progressHandler = undefined;
    vi.clearAllMocks();
  });

  it("shows animated onboarding guidance when opened", async () => {
    vi.mocked(agentInstallApi.getStatuses).mockResolvedValue(statuses);

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);

    const onboarding = await screen.findByTestId("agent-install-onboarding");
    expect(onboarding).toBeInTheDocument();
    expect(within(onboarding).getByText("环境检测")).toBeInTheDocument();
    expect(within(onboarding).getByText("准备运行时")).toBeInTheDocument();
    expect(
      within(onboarding).getByText("安装 Agent", { exact: true }),
    ).toBeInTheDocument();
  });

  it("renders all managed agents and opens an install confirmation before execution", async () => {
    vi.mocked(agentInstallApi.getStatuses).mockResolvedValue(statuses);

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);

    await waitFor(() =>
      expect(screen.getByText("Claude Code")).toBeInTheDocument(),
    );
    expect(screen.getByText("Claude Desktop")).toBeInTheDocument();
    expect(screen.getByText("Codex UI")).toBeInTheDocument();
    expect(screen.getByText("Codex")).toBeInTheDocument();
    expect(screen.getByText("Gemini CLI")).toBeInTheDocument();
    expect(screen.getByText("Grok Build")).toBeInTheDocument();
    expect(screen.getByText("WorkBuddy")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /^安装 / })).toHaveLength(3);

    fireEvent.click(screen.getByRole("button", { name: "安装 Codex" }));

    expect(
      screen.getByText("npm install -g @openai/codex"),
    ).toBeInTheDocument();
    expect(screen.getByText("Node.js")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "确定安装" }),
    ).toBeInTheDocument();
    expect(agentInstallApi.runInstall).not.toHaveBeenCalled();
  });

  it("disables only agents with unavailable non-fixable dependencies", async () => {
    vi.mocked(agentInstallApi.getStatuses).mockResolvedValue([
      ...statuses.filter((agent) => agent.id !== "claude"),
      blockingClaudeStatus,
    ]);

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "安装 Claude Code" }),
      ).toBeDisabled(),
    );
    expect(
      screen.getByRole("button", { name: "安装 Codex UI" }),
    ).not.toBeDisabled();
  });

  it("keeps advisory dependencies visible without blocking installation", async () => {
    const workbuddy = statuses.find((agent) => agent.id === "workbuddy")!;
    vi.mocked(agentInstallApi.getStatuses).mockResolvedValue([
      ...statuses.filter((agent) => agent.id !== "workbuddy"),
      {
        ...workbuddy,
        dependencies: [
          {
            name: ".NET Runtime",
            available: false,
            detail: "WorkBuddy 启动异常时请检查 .NET Runtime",
            kind: "advisory",
            blocking: false,
            auto_fixable: false,
            action: "安装 .NET Desktop Runtime",
          },
        ],
      },
    ]);

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);

    await waitFor(() =>
      expect(
        screen.getByText(/WorkBuddy 启动异常时请检查 \.NET Runtime/),
      ).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("button", { name: "安装 WorkBuddy" }),
    ).toBeEnabled();
  });

  it("skips blocked agents during batch installation and continues others", async () => {
    vi.mocked(agentInstallApi.getStatuses).mockResolvedValue([
      ...statuses.filter((agent) => agent.id !== "claude"),
      blockingClaudeStatus,
    ]);
    vi.mocked(agentInstallApi.runInstall).mockResolvedValue({ success: true });

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);

    fireEvent.click(
      await screen.findByRole("button", { name: "一键安装缺失 Agent" }),
    );
    await waitFor(() =>
      expect(agentInstallApi.runInstall).toHaveBeenCalledWith("codex"),
    );
    expect(agentInstallApi.runInstall).not.toHaveBeenCalledWith("claude");
    expect(
      await screen.findByText(/已跳过环境未满足项：Claude Code/),
    ).toBeInTheDocument();
  });

  it("reloads dependency status when the refresh action is clicked", async () => {
    vi.mocked(agentInstallApi.getStatuses)
      .mockResolvedValueOnce(statuses)
      .mockResolvedValueOnce([blockingClaudeStatus]);

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);
    await screen.findByText("Claude Code");

    fireEvent.click(screen.getByRole("button", { name: "刷新状态" }));
    await waitFor(() =>
      expect(agentInstallApi.getStatuses).toHaveBeenCalledTimes(2),
    );
    expect(
      screen.getByRole("button", { name: "安装 Claude Code" }),
    ).toBeDisabled();
  });

  it("ensures Node.js before installing missing agents in batch", async () => {
    const missingNode = statuses.map((agent) =>
      agent.id === "codex"
        ? {
            ...agent,
            dependencies: [
              {
                name: "Node.js",
                available: false,
                detail: "未检测到 node 命令",
                kind: "runtime" as const,
                blocking: true,
                auto_fixable: true,
                action: "安装 Node.js LTS",
              },
            ],
          }
        : agent,
    );
    vi.mocked(agentInstallApi.getStatuses)
      .mockResolvedValueOnce(missingNode)
      .mockResolvedValueOnce(missingNode);
    vi.mocked(agentInstallApi.ensureNodeRuntime).mockResolvedValue({
      available: true,
      version: "22.0.0",
      installed: true,
      command: "winget install OpenJS.NodeJS.LTS",
      error: null,
    });
    vi.mocked(agentInstallApi.runInstall).mockResolvedValue({ success: true });

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);
    await waitFor(() =>
      expect(
        screen.getByText(/缺失，安装 CLI Agent 时会自动补齐/),
      ).toBeInTheDocument(),
    );

    fireEvent.click(screen.getByRole("button", { name: "一键安装缺失 Agent" }));
    await waitFor(() =>
      expect(agentInstallApi.ensureNodeRuntime).toHaveBeenCalled(),
    );
    expect(
      vi.mocked(agentInstallApi.ensureNodeRuntime).mock.invocationCallOrder[0],
    ).toBeLessThan(
      vi.mocked(agentInstallApi.runInstall).mock.invocationCallOrder[0],
    );
    expect(agentInstallApi.runInstall).toHaveBeenCalledWith("codex");
    expect(agentInstallApi.runInstall).toHaveBeenCalledWith("codex-ui");
    expect(agentInstallApi.runInstall).toHaveBeenCalledWith("workbuddy");
  });

  it("renders Node.js and Agent progress events in the guided progress card", async () => {
    vi.mocked(agentInstallApi.getStatuses).mockResolvedValue(statuses);
    vi.mocked(agentInstallApi.ensureNodeRuntime).mockImplementation(
      () => new Promise(() => undefined),
    );

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "一键安装缺失 Agent" }),
      ).toBeEnabled(),
    );

    fireEvent.click(screen.getByRole("button", { name: "一键安装缺失 Agent" }));
    await waitFor(() => expect(progressHandler).toBeDefined());

    act(() => {
      progressHandler?.({
        agent_id: "node-runtime",
        phase: "install",
        status: "running",
        progress: 42,
        message: "正在安装 Node.js",
      });
    });
    expect(await screen.findByText("正在安装 Node.js")).toBeInTheDocument();
    expect(screen.getByText("42%")).toBeInTheDocument();

    act(() => {
      progressHandler?.({
        agent_id: "codex",
        phase: "verify",
        status: "running",
        progress: 88,
        message: "正在验证 Codex",
      });
    });
    expect(await screen.findByText("正在验证 Codex")).toBeInTheDocument();
    expect(screen.getByText("88%")).toBeInTheDocument();
  });

  it("renders fine-grained stage and byte progress details", async () => {
    vi.mocked(agentInstallApi.getStatuses).mockResolvedValue(statuses);
    vi.mocked(agentInstallApi.ensureNodeRuntime).mockImplementation(
      () => new Promise(() => undefined),
    );

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);
    await waitFor(() => expect(progressHandler).toBeDefined());

    act(() => {
      progressHandler?.({
        agent_id: "node-runtime",
        phase: "install",
        stage: "download",
        status: "running",
        progress: 42,
        current: 1048576,
        total: 2097152,
        unit: "bytes",
        message: "正在下载 Node.js",
      });
    });

    expect(await screen.findByText("下载文件")).toBeInTheDocument();
    expect(screen.getByText(/1 MB \/ 2 MB/)).toBeInTheDocument();
  });

  it("shows the detected desktop app installation path", async () => {
    vi.mocked(agentInstallApi.getStatuses).mockResolvedValue(
      statuses.map((agent) =>
        agent.id === "codex-ui"
          ? {
              ...agent,
              installed: true,
              runnable: true,
              install_path: "/Applications/ChatGPT.app",
            }
          : agent,
      ),
    );

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);

    expect(
      await screen.findByText("/Applications/ChatGPT.app"),
    ).toBeInTheDocument();
    expect(screen.getByText(/安装位置/)).toBeInTheDocument();
  });

  it("shows permission guidance when a desktop install is denied", async () => {
    vi.mocked(agentInstallApi.getStatuses).mockResolvedValue(statuses);
    vi.mocked(agentInstallApi.runInstall).mockRejectedValue(
      new Error(
        "复制 ChatGPT.app 失败：/Applications：Operation not permitted",
      ),
    );

    render(<AgentInstallPanel isOpen onClose={() => undefined} />);
    fireEvent.click(
      await screen.findByRole("button", { name: "安装 Codex UI" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "确定安装" }));

    expect(
      await screen.findByTestId("agent-install-permission-guide"),
    ).toBeInTheDocument();
    expect(screen.getByText(/需要应用目录权限/)).toBeInTheDocument();
    expect(screen.getByText(/共享与权限/)).toBeInTheDocument();
  });
});
