import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { inspectWindowsExecutable } from "../../scripts/verify-windows-artifact.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

describe("Windows distribution artifacts", () => {
  it("rejects debug executables and accepts a release executable with embedded assets", () => {
    const debug = inspectWindowsExecutable(
      "src-tauri/target/debug/cc-launch.exe",
      Buffer.from("http://127.0.0.1:3000/../dist"),
    );
    expect(debug.ok).toBe(false);
    expect(debug.reasons).toContain("debug_build");
    expect(debug.reasons).toContain("missing_embedded_frontend");

    const release = inspectWindowsExecutable(
      "src-tauri/target/release/cc-launch.exe",
      Buffer.from(".../assets/index-ABC123.js..."),
    );
    expect(release.ok).toBe(true);
    expect(release.reasons).toEqual([]);
  });

  it("keeps the dev server configuration out of the default runtime config", () => {
    const runtimeConfig = JSON.parse(
      readFileSync(resolve(root, "src-tauri/tauri.conf.json"), "utf8"),
    ) as { build?: Record<string, unknown> };
    const devConfig = JSON.parse(
      readFileSync(resolve(root, "src-tauri/tauri.dev.conf.json"), "utf8"),
    ) as { build?: Record<string, unknown> };

    expect(runtimeConfig.build?.devUrl).toBeUndefined();
    expect(runtimeConfig.build?.beforeDevCommand).toBeUndefined();
    expect(devConfig.build?.devUrl).toBe("http://127.0.0.1:3000");
    expect(devConfig.build?.beforeDevCommand).toBe("pnpm run dev:renderer");
  });

  it("builds and verifies only the release executable", () => {
    const buildScript = readFileSync(resolve(root, "build-pack.cmd"), "utf8");
    expect(buildScript).toContain("target\\release\\cc-launch.exe");
    expect(buildScript).toContain("verify-windows-artifact.mjs");
    expect(buildScript).not.toMatch(
      /(?:call|node).*target\\debug\\cc-launch\.exe/i,
    );
  });
});
