# 旧 CC Switch 一键迁移实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a one-click legacy CC Switch database migration while preserving the existing SQL import fallback.

**Architecture:** Add a backend command that opens the fixed legacy SQLite database read-only, snapshots it through SQLite Backup into memory, then reuses the existing guarded SQL staging/import path. Update the Agent install panel to call this command when a legacy database is detected and keep manual SQL selection for fallback.

**Tech Stack:** Rust/Tauri, rusqlite Backup API, existing database restore lock, React/Vitest.

---

### Task 1: Snapshot and import legacy SQLite safely

**Files:**
- Modify: `src-tauri/src/database/backup.rs`
- Modify: `src-tauri/src/commands/misc.rs`
- Test: existing Rust unit modules in the same files

- [x] Write failing tests for fixed legacy path and read-only snapshot import.
- [x] Run the focused tests and confirm the missing helper/command fails.
- [x] Implement `Database::import_legacy_database` using read-only `Connection` + SQLite Backup + existing staged SQL import.
- [x] Add the fixed `legacy_cc_switch_database_path` helper and Tauri command wrapper.
- [x] Run focused Rust tests.

### Task 2: Register the command and wire the API

**Files:**
- Modify: `src-tauri/src/commands/import_export.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api/legacyMigration.ts`

- [x] Add `migrate_legacy_cc_switch` under the existing restore lock and register it in Tauri invoke handlers.
- [x] Add the typed frontend API method.
- [x] Run TypeScript checks.

### Task 3: Make the Agent panel one-click with fallback

**Files:**
- Modify: `src/components/agent-install/AgentInstallPanel.tsx`
- Test: `tests/components/AgentInstallPanel.test.tsx`

- [x] Add a failing test that a detected database calls `migrate`, not a file dialog.
- [x] Implement one-click label/message and preserve manual SQL import.
- [x] Run the panel test suite.

### Task 4: Verification and documentation

**Files:**
- Modify: `docs/superpowers/specs/2026-09-13-legacy-cc-switch-auto-migration-design.md`

- [x] Run rustfmt, focused Rust tests, frontend tests, typecheck, and diff checks.
- [x] Run the full Rust suite serially and record unrelated environment failures.
- [x] Commit implementation and verification evidence separately from unrelated worktree changes.
