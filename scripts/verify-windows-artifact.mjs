import { readFileSync } from "node:fs";

const EMBEDDED_FRONTEND_MARKER = Buffer.from("/assets/index-");

/**
 * Inspect a Windows executable before it is handed to another machine.
 *
 * A Tauri debug executable built with `devUrl` is a development launcher, not a
 * distributable application: it points at the developer's Vite server and has
 * no embedded frontend assets. Keep this check deliberately small and
 * deterministic so it can run in CI and from the Windows build script.
 */
export function inspectWindowsExecutable(executablePath, bytes) {
  const normalizedPath = String(executablePath)
    .replaceAll("\\", "/")
    .toLowerCase();
  const reasons = [];

  if (
    normalizedPath.includes("/target/debug/") ||
    normalizedPath.endsWith("/debug/cc-launch.exe")
  ) {
    reasons.push("debug_build");
  }
  if (!bytes.includes(EMBEDDED_FRONTEND_MARKER)) {
    reasons.push("missing_embedded_frontend");
  }

  return {
    ok: reasons.length === 0,
    reasons,
  };
}

export function verifyWindowsExecutable(executablePath) {
  const inspection = inspectWindowsExecutable(
    executablePath,
    readFileSync(executablePath),
  );
  if (!inspection.ok) {
    throw new Error(
      `Invalid distributable executable ${executablePath}: ${inspection.reasons.join(", ")}`,
    );
  }
  return inspection;
}

if (
  process.argv[1] &&
  process.argv[1].endsWith("verify-windows-artifact.mjs")
) {
  const executablePath = process.argv[2];
  if (!executablePath) {
    console.error(
      "Usage: node scripts/verify-windows-artifact.mjs <release-exe>",
    );
    process.exitCode = 2;
  } else {
    try {
      verifyWindowsExecutable(executablePath);
      console.log(`Verified Windows executable: ${executablePath}`);
    } catch (error) {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 1;
    }
  }
}
