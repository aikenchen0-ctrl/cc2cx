# CASR Session Transfer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a session-management workflow that discovers all CASR providers, converts a selected native session through CASR, writes and verifies the target native session, and launches it with a structured provider/workspace request.

**Architecture:** Keep CASR as the conversion/discovery authority and add a thin cc2cx adapter. CASR exposes provider capabilities, unified discovery, and explicit source-path conversion; cc2cx validates DTOs, merges CASR sessions with its legacy eight-provider scanner, exposes Tauri commands, and renders a transfer dialog in `SessionManagerPage`.

**Tech Stack:** Rust 2021/2024, Tauri 2, serde, CASR provider registry, React/TypeScript, React Query, existing shadcn/ui controls.

---

## Current implementation status (2026-09-16)

The implementation described by this plan is present in the working tree. CASR
now provides the 17-provider capability registry, unified discovery, explicit
source-path conversion, canonical IR validation, atomic write/read-back
verification, and rollback. cc2cx provides the adapter, three Tauri commands,
CASR/legacy session-list merge, and the session-management transfer dialog.

Hermes remains a cc2cx legacy-scanner provider and is not part of CASR's
17-provider registry. A Hermes session can still be listed/read through the
legacy paths, but the transfer dialog deliberately marks it as an unsupported
source instead of inventing a CASR mapping. Kiro's structured launcher tries
`kiro-cli` first and falls back to `kiro`.

The implementation is still uncommitted; the unchecked commit steps below are
intentional.

### Task 1: Expose CASR provider capabilities and unified discovery

**Files:**
- Modify: `casr/src/providers/mod.rs`
- Modify: `casr/src/discovery.rs`
- Modify: `casr/src/lib.rs`
- Test: `casr/src/discovery.rs` (unit tests)

- [x] **Step 1: Write failing tests**

Add tests for a registry containing a synthetic provider: capability metadata is returned, `list_sessions` entries are parsed into canonical sessions, and invalid entries are skipped without exposing message content in diagnostics.

- [x] **Step 2: Run tests to verify failure**

Run `cargo test --manifest-path vendor/casr/Cargo.toml --lib discovery::tests -- --nocapture`.
Expected: compile/test failure because `WriteSupport`, `DiscoveredSession`, and `ProviderRegistry::discover_sessions` do not exist.

- [x] **Step 3: Implement the minimal API**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteSupport {
    Supported,
    ReadOnly { reason: String },
    Conditional { reason: String },
}

#[derive(Debug, Clone)]
pub struct ProviderCapability {
    pub slug: String,
    pub alias: String,
    pub name: String,
    pub detection: DetectionResult,
    pub write_support: WriteSupport,
}

#[derive(Debug)]
pub struct DiscoveredSession {
    pub provider_slug: String,
    pub provider_alias: String,
    pub path: PathBuf,
    pub session: CanonicalSession,
}
```

Give `Provider::write_support` a default of `Supported`, add `ProviderRegistry::capabilities`, and add `ProviderRegistry::discover_sessions` that uses `list_sessions()` first, otherwise bounded `WalkDir` scanning. Parse each candidate with `read_session`, retain only non-empty canonical sessions, and return deterministic provider/path ordering. Keep diagnostics at provider/count/error-type level.

- [x] **Step 4: Add static provider overrides**

Override Antigravity as `ReadOnly` and OpenCode as `Conditional` with schema-check wording. Do not change writer behavior in this task.

- [x] **Step 5: Run tests**

Run the focused provider/discovery checks separately:
`cargo test --manifest-path vendor/casr/Cargo.toml --lib discovery::tests -- --nocapture`
and `cargo test --manifest-path vendor/casr/Cargo.toml --lib providers:: -- --nocapture`.

- [ ] **Step 6: Commit**

```powershell
git add vendor/casr/src/providers/mod.rs vendor/casr/src/discovery.rs vendor/casr/src/lib.rs
git commit -m "feat: expose CASR provider capabilities and discovery"
```

The source-of-truth checkout remains under `casr/` for upstream review, but the
outer cc2cx commit must include the synchronized `vendor/casr` files.

### Task 2: Add explicit source-path conversion API

**Files:**
- Modify: `casr/src/pipeline.rs`
- Modify: `casr/src/lib.rs`
- Test: `casr/src/pipeline.rs`

- [x] **Step 1: Write failing tests**

Add a temp-directory test that writes a synthetic source session, calls `convert_source_path("cod", source, "cc", opts)`, asserts the target provider and workspace override, and verifies a missing source path returns an error before any target file is created.

- [x] **Step 2: Run the focused tests**

Run `cargo test --manifest-path vendor/casr/Cargo.toml pipeline::tests::convert_source_path -- --nocapture`.
Expected: failure because the explicit API is absent.

- [x] **Step 3: Implement the API**

Add `ConversionPipeline::convert_source_path(source_alias, source_path, target_alias, opts)`. Resolve the source provider by alias, validate the file or virtual database path, read it, then route through the existing validation/budget/write/read-back/rollback pipeline without changing CLI `convert` semantics. Ensure `opts.source_hint` is not overwritten by untrusted renderer strings and preserve existing warnings.

- [x] **Step 4: Run regression tests**

Run `cargo test --manifest-path vendor/casr/Cargo.toml pipeline::tests -- --nocapture` and then the complete CASR suite.

- [ ] **Step 5: Commit**

```powershell
git add vendor/casr/src/pipeline.rs vendor/casr/src/lib.rs
git commit -m "feat: convert CASR sessions from explicit source paths"
```

Do not stage the original `casr/` checkout as an embedded git repository.

### Task 3: Add cc2cx CASR adapter and Tauri commands

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/session_transfer.rs`
- Modify: `src-tauri/src/session_manager/mod.rs`
- Modify: `src-tauri/src/commands/session_manager.rs`
- Test: `src-tauri/tests/session_transfer.rs`

- [x] **Step 1: Write failing adapter tests**

Cover provider ID mapping for all 17 aliases, rejection of empty/relative/missing source paths, workspace normalization, stable error codes, and filtering of credentials/message text from serialized errors.

- [x] **Step 2: Run tests to verify failure**

Run `cargo test --manifest-path src-tauri/Cargo.toml --test session_transfer -- --nocapture`.
Expected: compile failure because the adapter and path dependency are absent.

- [x] **Step 3: Add the CASR path dependency**

Add `casr = { package = "cross_agent_session_resumer", path = "../vendor/casr" }` to `src-tauri/Cargo.toml`, regenerate only the required lockfile entries, and declare `mod session_transfer`.

- [x] **Step 4: Implement DTOs and commands**

Implement `SessionTransferRequest`, `SessionTransferResult`, `SessionTransferTarget`, and a `{ code, message, details }` error envelope. Add `list_session_transfer_targets`, `transfer_session`, and `launch_transferred_session`. Use `spawn_blocking`, structured provider/session/workspace arguments, and a platform launcher registry; never accept a renderer shell command for the new commands.

- [x] **Step 5: Merge CASR discovery into session listing**

Call CASR discovery from `session_manager::scan_sessions`, map canonical sessions to `SessionMeta`, merge with the existing eight-provider results, and deduplicate by normalized path or provider/database/session identity. Preserve legacy `sqlite:` parsing for existing message/delete paths.

- [x] **Step 6: Run tests**

Run the focused adapter tests plus `cargo test --manifest-path src-tauri/Cargo.toml session_manager`.

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/commands/mod.rs src-tauri/src/commands/session_manager.rs src-tauri/src/session_manager/mod.rs src-tauri/src/session_transfer.rs src-tauri/tests/session_transfer.rs
git commit -m "feat: add CASR session transfer commands"
```

### Task 4: Add TypeScript API and transfer dialog

**Files:**
- Modify: `src/types.ts`
- Modify: `src/lib/api/sessions.ts`
- Modify: `src/components/sessions/SessionManagerPage.tsx`
- Create: `src/components/sessions/SessionTransferDialog.tsx`
- Test: `tests/components/SessionTransferDialog.test.tsx`

- [x] **Step 1: Write failing UI tests**

Test target rendering for all 17 providers, read-only/source-unsupported
disabling, directory selection, pending state, success details, and launch
action. Rollback behavior is tested at the CASR/Tauri boundary; the UI does
not currently expose a dedicated rollback panel.

- [x] **Step 2: Run tests to verify failure**

Run `pnpm exec vitest run tests/components/SessionTransferDialog.test.tsx`.
Expected: module/test failure because the API types and dialog are absent.

- [x] **Step 3: Implement typed API and dialog**

Add typed target/result DTOs and API methods. Build a focused dialog using the existing `Select`, `Button`, `Badge`, and directory picker APIs. Keep the source session read-only, surface lossy warnings, disable unsupported targets, and never display or execute a raw resume command as the primary action.

- [x] **Step 4: Wire the session manager**

Add the transfer action beside resume/delete, open the dialog for the selected session, invalidate the sessions query after success, and call `launch_transferred_session` from the success state.

- [x] **Step 5: Run tests and typecheck**

Run `pnpm exec vitest run tests/components/SessionTransferDialog.test.tsx` and
`pnpm build:renderer`.

- [ ] **Step 6: Commit**

```powershell
git add src/types.ts src/lib/api/sessions.ts src/components/sessions/SessionManagerPage.tsx src/components/sessions/SessionTransferDialog.tsx tests/components/SessionTransferDialog.test.tsx
git commit -m "feat: add session transfer dialog"
```

### Task 5: Provider matrix and end-to-end verification

**Files:**
- Create: `src-tauri/tests/session_transfer_matrix.rs`
- Modify: `docs/superpowers/specs/2026-09-15-casr-session-transfer-design.md`
- Create: `docs/cursor/session-transfer-support.md`

- [x] **Step 1: Add synthetic fixtures**

Create temporary, generated fixtures for every CASR provider; do not check in real sessions or credentials.

- [x] **Step 2: Verify capability and round-trip matrix**

Run the provider capability/launcher matrix, CASR fixture round-trip suites,
explicit read-only rejection for Antigravity, and OpenCode schema
refusal/conditional paths. The cc2cx matrix asserts ID/alias/name mapping,
static write boundaries, and structured argv; it does not execute a live
`transfer_session` for every provider.

- [x] **Step 3: Verify platform launch contracts**

On each supported target, assert executable resolution, argv construction, cwd propagation, missing-executable errors, and no shell interpolation.

- [x] **Step 4: Run full verification**

Run CASR tests, cc2cx Rust tests, frontend tests, `pnpm build:renderer`, and a
debug/release Tauri build as available. Record only aggregate counts and
non-sensitive paths.

- [x] **Step 5: Update support matrix**

Document the 17 providers, their write support, and known conditional limitations.

- [ ] **Step 6: Commit and publish**

Commit the matrix, implementation, and tests after the working-tree scope is
reviewed; publishing remains a separate release action.

## Verification Record (2026-09-17)

- CASR historical all-target record: `1188/1188` (611 library tests plus the integration binaries listed by Cargo); the current vendored library run is `615/615`.
- cc2cx fresh targeted suites: `cursor_adapter` 15/15, `cursor_backend` 30/30,
  `cursor_commands` 20/20, `cursor_protocol` 18/18, `cursor_provider` 34/34,
  `cursor_routes` 3/3, `cursor_settings` 7/7, `cursor_transport` 11/11,
  `session_transfer` 18/18, provider matrix 2/2, and library
  `session_manager` filter 89/89. The two non-interactive CA checks also
  passed; the Windows Root-store installation test is intentionally excluded
  from unattended evidence because it can invoke system certificate UI.
- Frontend targeted suites contain 14 transfer-dialog tests and 16
  session-manager tests; the fresh combined run passed `30/30`. The
  transfer-refresh test fixture includes both the source and target
  capabilities, matching the backend's all-provider capability contract.
  `pnpm typecheck` and the renderer build also passed.
- A previous full Tauri run recorded 2739 passed, 3 unrelated
  Windows/skill-environment failures, and 6 ignored; this record is not a
  claim that those environment failures are fixed.
- The formal unsigned NSIS build completed with exit code 0 using
  `pnpm tauri build --bundles nsis --no-sign --config
  '{"bundle":{"createUpdaterArtifacts":false}}'`. The installer is at
  `src-tauri/target/release/bundle/nsis/cc-launch_3.20.5_x64-setup.exe`,
  generated at `2026-09-17 03:34:37 +08:00`, with SHA-256
  `4D64E3A1619F6051C66368018A8C68F6AFB58EE7D97559553582140A0D539622`.
  It is unsigned because `TAURI_SIGNING_PRIVATE_KEY` is not present; updater
  artifacts were disabled for this installer-only build.
- The final commit/push remains intentionally pending.

Additional regression fixed after the initial verification: Cursor's native bubble format
collapses system/tool/other messages to assistant bubbles. Read-back verification now
applies that mapping only to the Cursor target instead of incorrectly rolling back valid
conversions. The regression is covered by
`pipeline::tests::cursor_readback_accepts_roles_collapsed_to_assistant`.

## Current limitations and security boundary

- The transfer dialog sends `force: false` by default. On `target_conflict` it
  exposes an explicit “overwrite and retry” action; only that user action sends
  `force: true`. The consent is cleared when the destination changes, and CASR
  uses atomic no-clobber publication so concurrent no-force writers cannot
  silently replace one another.
- Write support and automatic-launch support are separate target capabilities.
  Missing or unsupported launchers no longer turn a successful write into a
  failed transfer, and the UI only offers automatic opening for a currently
  resolvable structured launcher.
- CASR automatically rolls back an unverified write. A rollback failure is
  returned as the stable `rollback_failed` IPC error, but the current dialog
  renders the error message rather than a dedicated rollback-status view.
- cc2cx adapter errors and transfer logs avoid session bodies, credentials,
  raw commands, and secrets. CASR provider debug/trace code still emits some
  local paths, session IDs, and detection evidence; verbose diagnostics must
  therefore be treated as local, identifier-bearing logs rather than
  completely anonymous telemetry.
