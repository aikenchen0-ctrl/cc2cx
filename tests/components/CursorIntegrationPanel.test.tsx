import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { CursorIntegrationPanel } from "@/components/settings/CursorIntegrationPanel";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { addresses?: string }) => {
      if (key === "settings.advanced.cursor.transparentActive") {
        return `transparent:${opts?.addresses ?? ""}`;
      }
      return key;
    },
  }),
}));

const getStatus = vi.fn();

vi.mock("@/lib/api/settings", () => ({
  settingsApi: {
    getCursorHarnessStatus: () => getStatus(),
    startCursorIntegration: vi.fn(),
    stopCursorIntegration: vi.fn(),
    initializeCursorCa: vi.fn(),
    installCursorCa: vi.fn(),
    uninstallCursorCa: vi.fn(),
  },
}));

describe("CursorIntegrationPanel", () => {
  beforeEach(() => {
    getStatus.mockReset();
    getStatus.mockResolvedValue({
      state: "disabled",
      ca: "missing",
      caInstallCommand: null,
      caUninstallCommand: null,
      settingsApplied: false,
      proxyUrl: null,
      proxyPort: null,
      backendPort: null,
      backendHealth: null,
      backendErrorCode: null,
      settingsBackupPresent: false,
      transparentEntry: null,
      managedHosts: false,
      fakeIpEntries: [],
    });
  });

  it("does not show transparent-only warnings in default proxy mode", async () => {
    render(<CursorIntegrationPanel />);
    await waitFor(() => {
      expect(
        screen.getByText("settings.advanced.cursor.integrationState"),
      ).toBeInTheDocument();
    });
    expect(
      screen.queryByText("settings.advanced.cursor.hostsElevationHint"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("settings.advanced.cursor.transparentInactive"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("settings.advanced.cursor.hostsUnmanaged"),
    ).not.toBeInTheDocument();
  });

  it("shows transparent addresses when the entry is running", async () => {
    getStatus.mockResolvedValue({
      state: "running",
      ca: "ready",
      caInstallCommand: null,
      caUninstallCommand: null,
      settingsApplied: true,
      proxyUrl: "http://127.0.0.1:15722",
      proxyPort: 15722,
      backendPort: 43100,
      backendHealth: "not_observed",
      backendErrorCode: null,
      settingsBackupPresent: true,
      transparentEntry: {
        running: true,
        addresses: ["127.0.0.2:443"],
      },
      managedHosts: true,
      fakeIpEntries: [{ ip: "127.0.0.2", hostname: "api2.cursor.sh" }],
    });
    render(<CursorIntegrationPanel />);
    await waitFor(() => {
      expect(screen.getByText("transparent:127.0.0.2:443")).toBeInTheDocument();
    });
    expect(
      screen.getByText("settings.advanced.cursor.hostsManaged"),
    ).toBeInTheDocument();
  });
});
