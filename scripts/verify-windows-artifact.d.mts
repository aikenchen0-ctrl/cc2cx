export interface WindowsArtifactInspection {
  ok: boolean;
  reasons: string[];
}

export declare function inspectWindowsExecutable(
  executablePath: string,
  bytes: Uint8Array,
): WindowsArtifactInspection;

export declare function verifyWindowsExecutable(
  executablePath: string,
): WindowsArtifactInspection;
