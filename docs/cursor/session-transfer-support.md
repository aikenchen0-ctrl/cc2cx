# CASR Session Transfer Support Matrix

This matrix describes the provider contract exposed by the cc2cx session-transfer IPC. The target list is built from CASR's default registry, so all 17 providers remain visible even when a local executable or session store is not detected.
Hermes is intentionally absent from this table: cc2cx still scans Hermes through its legacy session provider, but Hermes is not registered in CASR's first-release conversion matrix. Hermes source sessions are therefore shown as unsupported by the transfer dialog.

| Provider | cc2cx ID | CASR alias | Write support | Structured local launch |
| --- | --- | --- | --- | --- |
| Claude Code | `claude` | `cc` | Supported | `claude --resume <id>` |
| Codex | `codex` | `cod` | Supported | `codex resume <id>` |
| Gemini CLI | `gemini` | `gmi` | Supported | `gemini --resume <id>` |
| Antigravity CLI | `antigravity` | `agy` | Read-only | Not available; `agy` owns conversation creation |
| Cursor | `cursor` | `cur` | Supported | Not available; no session-ID resume flag |
| Cline | `cline` | `cln` | Supported | Not available |
| Aider | `aider` | `aid` | Supported | Not available |
| Amp | `amp` | `amp` | Supported | `amp threads continue --execute "Continue from @<id>"` |
| OpenCode | `opencode` | `opc` | Conditional | `opencode --session <id>` when the local CLI is available |
| ChatGPT | `chatgpt` | `gpt` | Supported | Not available |
| ClawdBot | `clawdbot` | `cwb` | Supported | `clawdbot --resume <id>` |
| Vibe | `vibe` | `vib` | Supported | `vibe --resume <id>` |
| Factory | `factory` | `fac` | Supported | `factory --resume <id>` |
| OpenClaw | `openclaw` | `ocl` | Supported | `openclaw --resume <id>` |
| Pi-Agent | `pi` | `pi` | Supported | `pi --session <session-file>` |
| Kiro CLI | `kiro` | `kr` | Supported | `kiro-cli --resume-id <id>`; fallback `kiro --resume-id <id>` |
| Grok Build | `grokbuild` | `grk` | Supported | `grok --resume <id>` |

## Capability semantics

- `Supported` means CASR exposes a native writer. The provider may still return non-fatal warnings when a local index or optional registration step is unavailable.
- `Read-only` is a hard pre-write boundary. Antigravity conversations are runtime-owned and the adapter rejects them as targets with `target_read_only`.
- `Conditional` is a runtime schema boundary. OpenCode's writer refuses live 1.x/2.x databases and surfaces `target_schema_mismatch`; CASR-compatible legacy layouts remain eligible.
- OpenCode session deletion follows the same safety boundary: the session manager deletes `files/messages/sessions` only in the legacy layout. It refuses direct deletion from 1.x `session/message/part` and 2.x `session_v2/session_message` projections because those rows are rebuilt from OpenCode's event log.
- `installed` is detection metadata only. A missing CLI does not prevent a writer from creating native files. Automatic launch resolves the effective user/machine PATH on Windows and treats missing or unsupported launchers as a non-fatal warning after the write succeeds.
- `launchSupport` is separate from write support: `supported` means a structured resume contract and executable were found, `executableMissing` means the native write remains available but the CLI cannot currently be launched, and `unsupported` means the provider has no session-ID-specific launcher. The dialog shows these states before conversion and only offers “Open new session” for `supported` targets.
- The dialog submits `force: false` by default. When CASR reports `target_conflict`, it exposes an explicit “overwrite and retry” action bound to that destination; changing the provider or workspace clears the consent. CASR atomically refuses concurrent no-force publication and keeps its database backup before an explicit overwrite. CASR performs automatic rollback after an unverified write, while a rollback failure is returned as `rollback_failed`; the dialog currently renders the error message rather than a dedicated rollback view.

The launch command accepts only provider ID, session ID, and an optional canonical workspace. It constructs the invocation in the backend; renderer-provided shell text is never accepted. Providers without a session-specific launcher keep the successful write result and return `launched: false` with a warning when an automatic open is requested. Windows starts native executables or generated `.cmd`/`.bat` shim arguments in a new visible console. macOS opens Terminal.app with every backend-generated argument POSIX-quoted and then AppleScript-escaped. Linux currently reports automatic opening as unsupported and requires manual resume until a visible-terminal adapter is available. Unix executable discovery also requires an execute permission bit.

## Verification

`src-tauri/tests/session_transfer_matrix.rs` checks the 17-ID/alias/name mapping, static write boundaries, and structured launcher coverage. Provider reader/writer round trips remain covered by CASR's per-provider fixtures and round-trip suites; this matrix intentionally uses no real sessions, credentials, or message content.

## Verification status

- CASR all-target run: `1188/1188` passed (611 library tests plus all integration binaries).
- cc2cx fresh targeted suites: `cursor_adapter` 15/15, `cursor_backend` 30/30,
  `cursor_commands` 20/20, `cursor_protocol` 18/18, `cursor_provider` 34/34,
  `cursor_routes` 3/3, `cursor_settings` 7/7, `cursor_transport` 11/11,
  `session_transfer` 18/18, provider matrix 2/2, and the `session_manager`
  filter 89/89. The non-interactive CA checks passed; the Windows Root-store
  installation test is excluded from unattended evidence because it can invoke
  system certificate UI.
- Frontend transfer coverage: 14 dialog tests plus 16 session-manager tests; the fresh combined run passed `30/30`.
- The matrix test verifies capability metadata, static read-only/conditional boundaries, provider mapping, and argv construction. It does not prove that every provider has completed a live `transfer_session` through a real local installation.

## Hermes and logging boundary

Hermes remains available to the legacy session scanner and legacy read/delete paths. It is not a CASR target or source for the first-release transfer workflow, so the UI rejects a Hermes source with an explicit unsupported-source state.

The cc2cx adapter does not intentionally serialize session bodies or credentials. CASR provider debug/trace logs can still include local paths, Session IDs, and detection evidence; treat verbose diagnostics as local identifier-bearing logs.
