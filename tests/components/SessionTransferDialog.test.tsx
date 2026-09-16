import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SessionTransferDialog } from "@/components/sessions/SessionTransferDialog";
import type { SessionMeta } from "@/types";

const { api, pickDirectory } = vi.hoisted(() => ({
  api: {
    listTransferTargets: vi.fn(),
    transfer: vi.fn(),
    launchTransferred: vi.fn(),
  },
  pickDirectory: vi.fn(),
}));

vi.mock("@/lib/api/sessions", () => ({ sessionsApi: api }));
vi.mock("@/lib/api/settings", () => ({ settingsApi: { pickDirectory } }));

const source: SessionMeta = {
  providerId: "codex",
  sessionId: "source-1",
  title: "Source session",
  sourcePath: "/tmp/source.jsonl",
  projectDir: "/tmp/project",
};

const providerIds = [
  "claude",
  "codex",
  "gemini",
  "antigravity",
  "cursor",
  "cline",
  "aider",
  "amp",
  "opencode",
  "chatgpt",
  "clawdbot",
  "vibe",
  "factory",
  "openclaw",
  "pi",
  "kiro",
  "grokbuild",
] as const;

function makeTargets() {
  return providerIds.map((providerId) => ({
    providerId,
    alias: providerId,
    name: providerId,
    installed: true,
    writeSupport:
      providerId === "antigravity"
        ? { kind: "readOnly", reason: "read-only" }
        : { kind: "supported" },
    launchSupport: [
      "cursor",
      "cline",
      "aider",
      "chatgpt",
      "antigravity",
    ].includes(providerId)
      ? { kind: "unsupported", reason: "manual resume only" }
      : { kind: "supported" },
  }));
}

describe("SessionTransferDialog", () => {
  beforeEach(() => {
    Element.prototype.scrollIntoView = vi.fn();
    api.listTransferTargets.mockReset();
    api.transfer.mockReset();
    api.launchTransferred.mockReset();
    pickDirectory.mockReset();
  });

  it("renders every CASR target and disables read-only providers", async () => {
    api.listTransferTargets.mockResolvedValueOnce(makeTargets());

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    const targetSelect = await screen.findByRole("combobox", {
      name: "目标工具",
    });
    await userEvent.click(targetSelect);

    for (const providerId of providerIds) {
      if (providerId === "antigravity") {
        // Radix omits disabled collection items from the default accessible
        // role query in jsdom; assert its rendered item and disabled state.
        const label = await screen.findByText(providerId);
        expect(label.closest('[role="option"]')).toHaveAttribute(
          "aria-disabled",
          "true",
        );
      } else {
        expect(
          await screen.findByRole("option", { name: providerId }),
        ).toBeInTheDocument();
      }
    }

    expect(screen.getByText("antigravity")).toBeInTheDocument();
  });

  it("shows installation, version, and conditional write status before selection", async () => {
    api.listTransferTargets.mockResolvedValueOnce([
      {
        providerId: "claude",
        alias: "claude",
        name: "Claude Code",
        installed: false,
        version: "1.2.3",
        writeSupport: { kind: "supported" },
      },
      {
        providerId: "opencode",
        alias: "opencode",
        name: "OpenCode",
        installed: true,
        version: "2.0.0",
        writeSupport: { kind: "conditional", reason: "schema check" },
      },
      {
        providerId: "antigravity",
        alias: "antigravity",
        name: "Antigravity",
        installed: true,
        writeSupport: { kind: "readOnly", reason: "runtime-owned" },
      },
    ]);

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    await userEvent.click(
      await screen.findByRole("combobox", { name: "目标工具" }),
    );

    expect(screen.getAllByText("未安装").length).toBeGreaterThan(0);
    expect(screen.getAllByText("v1.2.3").length).toBeGreaterThan(0);
    expect(screen.getByText("条件写入")).toBeInTheDocument();
    expect(screen.getByText("只读")).toBeInTheDocument();
  });

  it("warns about fields that may be lost before confirmation", async () => {
    api.listTransferTargets.mockResolvedValueOnce(makeTargets());

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    expect(await screen.findByText("可能丢失的字段")).toBeInTheDocument();
    expect(screen.getByText("隐藏推理")).toBeInTheDocument();
    expect(screen.getByText("工具调用细节")).toBeInTheDocument();
    expect(screen.getByText("模型元数据")).toBeInTheDocument();
    expect(screen.getByText("目标格式不支持的附件")).toBeInTheDocument();
  });

  it("offers retry when loading transfer targets fails", async () => {
    api.listTransferTargets
      .mockRejectedValueOnce(new Error("network down"))
      .mockResolvedValueOnce(makeTargets());

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("network down");
    await userEvent.click(screen.getByRole("button", { name: "重试" }));
    expect(await screen.findByText("claude")).toBeInTheDocument();
    expect(api.listTransferTargets).toHaveBeenCalledTimes(2);
  });

  it("blocks the source provider as a no-op transfer target", async () => {
    api.listTransferTargets.mockResolvedValueOnce([
      {
        providerId: "codex",
        alias: "codex",
        name: "Codex",
        installed: true,
        writeSupport: { kind: "supported" },
      },
    ]);

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    await userEvent.click(
      await screen.findByRole("combobox", { name: "目标工具" }),
    );
    const sourceOption = screen.getByText("Codex").closest('[role="option"]');
    expect(sourceOption).toHaveAttribute("aria-disabled", "true");
    await userEvent.keyboard("{Escape}");
    expect(screen.getByRole("button", { name: "确认写回" })).toBeDisabled();
    expect(api.transfer).not.toHaveBeenCalled();
  });

  it("blocks sources that are outside the CASR provider registry", async () => {
    api.listTransferTargets.mockResolvedValueOnce(makeTargets());

    render(
      <SessionTransferDialog
        open
        onOpenChange={vi.fn()}
        session={{ ...source, providerId: "hermes" }}
      />,
    );

    expect(
      await screen.findByText("此来源工具暂不支持跨工具写回"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "确认写回" })).toBeDisabled();
    expect(api.transfer).not.toHaveBeenCalled();
  });

  it("lets the user choose a workspace and reports a completed transfer", async () => {
    api.listTransferTargets.mockResolvedValueOnce(makeTargets());
    pickDirectory.mockResolvedValueOnce("/tmp/selected-project");
    api.transfer.mockResolvedValueOnce({
      sourceProviderId: "codex",
      sourceSessionId: "source-1",
      sourcePath: "/tmp/source.jsonl",
      targetProviderId: "claude",
      targetSessionId: "target-1",
      workspace: "/tmp/selected-project",
      writtenPaths: ["/tmp/target.jsonl"],
      warnings: ["lossy conversion"],
      lossy: true,
    });

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    await userEvent.click(
      await screen.findByRole("button", { name: "选择目录" }),
    );
    expect(
      await screen.findByDisplayValue("/tmp/selected-project"),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "确认写回" }));
    expect(await screen.findByRole("status")).toHaveTextContent("写回成功");
    expect(screen.getByText("lossy conversion")).toBeInTheDocument();
    expect(screen.getByText("/tmp/target.jsonl")).toBeInTheDocument();
    expect(api.transfer).toHaveBeenCalledWith(
      expect.objectContaining({
        sourceProviderId: "codex",
        sourceSessionId: "source-1",
        targetProvider: "claude",
        workspace: "/tmp/selected-project",
      }),
    );
  });

  it("shows pending state and launches the written session", async () => {
    api.listTransferTargets.mockResolvedValueOnce(makeTargets());
    let resolveTransfer!: (value: unknown) => void;
    api.transfer.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveTransfer = resolve;
      }),
    );
    api.launchTransferred.mockResolvedValueOnce({ launched: true });

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    await userEvent.click(
      await screen.findByRole("button", { name: "确认写回" }),
    );
    expect(screen.getByRole("button", { name: "写回中..." })).toBeDisabled();

    resolveTransfer({
      sourceProviderId: "codex",
      sourceSessionId: "source-1",
      sourcePath: "/tmp/source.jsonl",
      targetProviderId: "claude",
      targetSessionId: "target-1",
      workspace: "/tmp/project",
      writtenPaths: ["/tmp/target.jsonl"],
      warnings: [],
      lossy: false,
    });

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "打开新会话" }),
      ).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByRole("button", { name: "打开新会话" }));
    expect(api.launchTransferred).toHaveBeenCalledWith({
      targetProvider: "claude",
      sessionId: "target-1",
      workspace: "/tmp/project",
    });
  });

  it("offers an explicit overwrite retry when the target session already exists", async () => {
    api.listTransferTargets.mockResolvedValueOnce(makeTargets());
    api.transfer
      .mockRejectedValueOnce({
        code: "target_conflict",
        message:
          "A target session already exists. Enable overwrite and retry if appropriate.",
      })
      .mockResolvedValueOnce({
        sourceProviderId: "codex",
        sourceSessionId: "source-1",
        sourcePath: "/tmp/source.jsonl",
        targetProviderId: "opencode",
        targetSessionId: "target-1",
        workspace: "/tmp/project",
        writtenPaths: ["/tmp/target.jsonl"],
        warnings: [],
        lossy: true,
      });

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    await userEvent.click(
      await screen.findByRole("button", { name: "确认写回" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "A target session already exists",
    );

    await userEvent.click(screen.getByRole("button", { name: "覆盖并重试" }));
    await waitFor(() => expect(api.transfer).toHaveBeenCalledTimes(2));
    expect(api.transfer).toHaveBeenLastCalledWith(
      expect.objectContaining({ force: true, targetProvider: "claude" }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent("写回成功");
  });

  it("clears overwrite consent when the destination changes", async () => {
    api.listTransferTargets.mockResolvedValueOnce(makeTargets());
    api.transfer.mockRejectedValueOnce({
      code: "target_conflict",
      message: "A target session already exists.",
    });

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    await userEvent.click(
      await screen.findByRole("button", { name: "确认写回" }),
    );
    expect(
      await screen.findByRole("button", { name: "覆盖并重试" }),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole("combobox", { name: "目标工具" }));
    await userEvent.click(
      await screen.findByRole("option", { name: /gemini/i }),
    );

    expect(
      screen.queryByRole("button", { name: "覆盖并重试" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("ignores a transfer result after the dialog reopens for another session", async () => {
    api.listTransferTargets.mockResolvedValue(makeTargets());
    let resolveTransfer!: (value: unknown) => void;
    api.transfer.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveTransfer = resolve;
      }),
    );
    const onTransferred = vi.fn();
    const secondSource: SessionMeta = {
      ...source,
      sessionId: "source-2",
      title: "Second session",
      sourcePath: "/tmp/source-2.jsonl",
    };
    const { rerender } = render(
      <SessionTransferDialog
        open
        onOpenChange={vi.fn()}
        session={source}
        onTransferred={onTransferred}
      />,
    );

    await userEvent.click(
      await screen.findByRole("button", { name: "确认写回" }),
    );
    rerender(
      <SessionTransferDialog
        open={false}
        onOpenChange={vi.fn()}
        session={source}
        onTransferred={onTransferred}
      />,
    );
    rerender(
      <SessionTransferDialog
        open
        onOpenChange={vi.fn()}
        session={secondSource}
        onTransferred={onTransferred}
      />,
    );
    expect(await screen.findByText(/Second session/)).toBeInTheDocument();

    await act(async () => {
      resolveTransfer({
        sourceProviderId: "codex",
        sourceSessionId: "source-1",
        sourcePath: "/tmp/source.jsonl",
        targetProviderId: "claude",
        targetSessionId: "target-1",
        workspace: "/tmp/project",
        writtenPaths: ["/tmp/target.jsonl"],
        warnings: [],
        lossy: false,
      });
      await Promise.resolve();
    });

    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "打开新会话" }),
    ).not.toBeInTheDocument();
    expect(onTransferred).not.toHaveBeenCalled();
  });

  it("keeps a successful write when automatic launch is unavailable", async () => {
    api.listTransferTargets.mockResolvedValueOnce(makeTargets());
    api.transfer.mockResolvedValueOnce({
      sourceProviderId: "codex",
      sourceSessionId: "source-1",
      sourcePath: "/tmp/source.jsonl",
      targetProviderId: "opencode",
      targetSessionId: "target-1",
      workspace: "/tmp/project",
      writtenPaths: ["/tmp/target.jsonl"],
      warnings: [],
      lossy: true,
    });
    api.launchTransferred.mockResolvedValueOnce({
      launched: false,
      warning: "目标工具未能自动打开，请手动恢复。",
    });

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "确认写回" }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "打开新会话" }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent("写回成功");
    expect(api.launchTransferred).toHaveBeenCalled();
  });

  it("does not offer automatic launch for a manual-resume-only target", async () => {
    api.listTransferTargets.mockResolvedValueOnce(
      makeTargets().map((target) => ({
        ...target,
        launchSupport:
          target.providerId === "cursor"
            ? {
                kind: "unsupported",
                reason: "Cursor has no session-ID resume launcher.",
              }
            : { kind: "supported" },
      })),
    );
    api.transfer.mockResolvedValueOnce({
      sourceProviderId: "codex",
      sourceSessionId: "source-1",
      sourcePath: "/tmp/source.jsonl",
      targetProviderId: "cursor",
      targetSessionId: "target-1",
      workspace: "/tmp/project",
      writtenPaths: ["/tmp/target.jsonl"],
      warnings: [],
      lossy: true,
    });

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    await userEvent.click(
      await screen.findByRole("combobox", { name: "目标工具" }),
    );
    await userEvent.click(
      await screen.findByRole("option", { name: /cursor/i }),
    );
    expect(
      await screen.findByText("Cursor has no session-ID resume launcher."),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "确认写回" }));
    expect(await screen.findByRole("status")).toHaveTextContent("写回成功");
    expect(
      screen.queryByRole("button", { name: "打开新会话" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByText("请在目标工具中手动恢复此会话"),
    ).toBeInTheDocument();
    expect(api.launchTransferred).not.toHaveBeenCalled();
  });

  it("does not offer automatic launch when the target CLI is missing", async () => {
    api.listTransferTargets.mockResolvedValueOnce(
      makeTargets().map((target) => ({
        ...target,
        launchSupport:
          target.providerId === "claude"
            ? {
                kind: "executableMissing",
                reason: "Claude Code CLI was not found on this device.",
              }
            : target.launchSupport,
      })),
    );
    api.transfer.mockResolvedValueOnce({
      sourceProviderId: "codex",
      sourceSessionId: "source-1",
      sourcePath: "/tmp/source.jsonl",
      targetProviderId: "claude",
      targetSessionId: "target-1",
      workspace: "/tmp/project",
      writtenPaths: ["/tmp/target.jsonl"],
      warnings: [],
      lossy: true,
    });

    render(
      <SessionTransferDialog open onOpenChange={vi.fn()} session={source} />,
    );

    expect(
      await screen.findByText("Claude Code CLI was not found on this device."),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "确认写回" }));

    expect(await screen.findByRole("status")).toHaveTextContent("写回成功");
    expect(
      screen.queryByRole("button", { name: "打开新会话" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByText("请在目标工具中手动恢复此会话"),
    ).toBeInTheDocument();
    expect(api.launchTransferred).not.toHaveBeenCalled();
  });
});
