# cc-launch Branding and Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the Tauri application and cc-boot integration as a new, three-platform `cc-launch` application whose own state is isolated from cc-switch.

**Architecture:** Centralize the new application storage identity in existing Rust path helpers, then let database, settings, skills, backup and sync code continue consuming those helpers. Configure bundle metadata and cc-boot from explicit `CC_LAUNCH_*` constants; retain protocol-internal proxy markers unless they are part of the new sync namespace.

**Tech Stack:** Tauri 2, Rust, rusqlite, React/TypeScript, pnpm/Vite/Vitest, WiX, Flatpak, GitHub Actions.

---

### Task 1: Establish the new Rust application-data contract

**Files:**
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/database/mod.rs`
- Modify: `src-tauri/tests/support.rs`
- Test: `src-tauri/src/config.rs`
- Test: `src-tauri/src/database/tests.rs`

- [ ] **Step 1: Add failing path tests beside the existing config tests**

```rust
#[test]
#[serial]
fn app_config_dir_uses_cc_launch_directory() {
    let home = tempfile::tempdir().unwrap();
    std::env::set_var("CC_LAUNCH_TEST_HOME", home.path());
    assert_eq!(get_app_config_dir(), home.path().join(".cc-launch"));
}
```

Also extend the test-home guard to save, set and restore `CC_LAUNCH_TEST_HOME`; do not use `CC_SWITCH_TEST_HOME` for new application tests.

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml app_config_dir_uses_cc_launch_directory`

Expected: FAIL because the existing helper uses `.cc-switch` and only reads `CC_SWITCH_TEST_HOME`.

- [ ] **Step 3: Replace the application-owned constants and remove the legacy fallback**

At the top of `config.rs`, define and use:

```rust
pub const APP_DATA_DIR_NAME: &str = ".cc-launch";
pub const APP_DATABASE_FILE_NAME: &str = "cc-launch.db";
pub const TEST_HOME_ENV: &str = "CC_LAUNCH_TEST_HOME";
```

Make `get_home_dir()` read only `TEST_HOME_ENV` before `dirs::home_dir()`. Make `get_app_config_dir()` return the explicit override or `get_home_dir().join(APP_DATA_DIR_NAME)`; delete the Windows `HOME/.cc-switch` fallback block. In `Database::init`, construct the database path using `APP_DATABASE_FILE_NAME`.

- [ ] **Step 4: Update test cleanup and verify the new files are isolated**

In `src-tauri/tests/support.rs`, replace `.cc-switch` with `.cc-launch` and replace `CC_SWITCH_TEST_HOME` with `CC_LAUNCH_TEST_HOME`. Add a database test that initializes `Database::init()` with a temporary test home and asserts that only `.cc-launch/cc-launch.db` exists while `.cc-switch/cc-switch.db` does not.

- [ ] **Step 5: Run the focused Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml app_config_dir_uses_cc_launch_directory database`

Expected: PASS.

- [ ] **Step 6: Commit the isolated data contract**

```powershell
git add src-tauri/src/config.rs src-tauri/src/database/mod.rs src-tauri/tests/support.rs
git commit -m "feat: isolate cc-launch application data"
```

### Task 2: Move owned storage, backup, and sync defaults to cc-launch

**Files:**
- Modify: `src-tauri/src/app_config.rs`
- Modify: `src-tauri/src/services/skill.rs`
- Modify: `src-tauri/src/services/webdav_sync.rs`
- Modify: `src-tauri/src/services/s3_sync.rs`
- Modify: `src-tauri/src/services/sync_protocol.rs`
- Modify: `src-tauri/src/commands/webdav_sync.rs`
- Modify: `src-tauri/src/commands/s3_sync.rs`
- Test: each modified Rust module’s existing test section

- [ ] **Step 1: Change failing default-value assertions**

Replace expected `cc-switch-sync` values with `cc-launch-sync` in WebDAV, S3 and command persistence tests. Add this protocol assertion:

```rust
#[test]
fn manifest_uses_cc_launch_format() {
    let manifest = manifest_with(
        PROTOCOL_FORMAT,
        PROTOCOL_VERSION,
        Some(DB_COMPAT_VERSION),
    );
    assert_eq!(manifest.format, "cc-launch-webdav-sync");
}
```

- [ ] **Step 2: Run the sync-focused tests and verify failures**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sync_protocol webdav_sync s3_sync`

Expected: FAIL only where old defaults or old protocol format are asserted.

- [ ] **Step 3: Use named application constants for the new defaults**

Add constants in the owning modules:

```rust
pub(crate) const DEFAULT_SYNC_REMOTE_ROOT: &str = "cc-launch-sync";
pub(crate) const PROTOCOL_FORMAT: &str = "cc-launch-webdav-sync";
```

Make `Default` implementations for `WebDavSyncSettings` and `S3SyncSettings` use `DEFAULT_SYNC_REMOTE_ROOT`; replace literal default-root test fixtures with the constant where visibility permits. Keep custom `remote_root` values unchanged. Update user-facing path comments in `app_config.rs` and `skill.rs` to `~/.cc-launch` without renaming the persisted enum variant `SkillStorageLocation::CcSwitch`, because it is a schema value rather than display branding.

- [ ] **Step 4: Make format incompatibility explicit**

Keep `validate_manifest_compat()` exact-match behavior. Add a test passing `"cc-switch-webdav-sync"` and assert it returns the existing `sync.manifest_format_incompatible` error. Do not add a read fallback for old manifests.

- [ ] **Step 5: Run the focused Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sync_protocol webdav_sync s3_sync skill`

Expected: PASS.

- [ ] **Step 6: Commit the owned storage and sync defaults**

```powershell
git add src-tauri/src/app_config.rs src-tauri/src/services/skill.rs src-tauri/src/services/webdav_sync.rs src-tauri/src/services/s3_sync.rs src-tauri/src/services/sync_protocol.rs src-tauri/src/commands/webdav_sync.rs src-tauri/src/commands/s3_sync.rs
git commit -m "feat: namespace cc-launch sync storage"
```

### Task 3: Rename the desktop application identity and deep link

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/tauri.windows.conf.json`
- Modify: `src-tauri/Info.plist`
- Modify: `src-tauri/wix/per-user-main.wxs`
- Modify: `src-tauri/tests/deeplink_import.rs`
- Modify: `src-tauri/src/deeplink/tests.rs`

- [ ] **Step 1: Change deep-link tests to the new public scheme**

Replace test input URLs such as:

```rust
let url = "ccswitch://v1/import?resource=provider&app=claude&name=DeepLink%20Claude&endpoint=https%3A%2F%2Fapi.example.com%2Fv1&apiKey=sk-test-claude-key&model=claude-sonnet-4";
```

with `cclaunch://v1/import?resource=provider&app=claude&name=DeepLink%20Claude&endpoint=https%3A%2F%2Fapi.example.com%2Fv1&apiKey=sk-test-claude-key&model=claude-sonnet-4`. Add a parser rejection test for `ccswitch://` if the parser validates schemes. Do not alter payload fields or import behavior.

- [ ] **Step 2: Run deep-link tests and verify configuration remains old**

Run: `cargo test --manifest-path src-tauri/Cargo.toml deeplink`

Expected: tests exercising registration/configuration fail until the scheme is changed.

- [ ] **Step 3: Apply matching identity changes**

Set the package and Cargo names to `cc-launch`; set the Rust library name to `cc_launch_lib` and update all integration-test imports accordingly. In Tauri configuration set:

```json
"productName": "cc-launch",
"identifier": "com.cclaunch.desktop",
"schemes": ["cclaunch"]
```

Set the Windows override title to `cc-launch`; update WiX product, manufacturer-visible fields, executable/component references and install directory to the new binary name. Update `Info.plist` URL name and scheme to `cc-launch` and `cclaunch`.

- [ ] **Step 4: Run type and Rust deep-link verification**

Run: `pnpm typecheck; cargo test --manifest-path src-tauri/Cargo.toml deeplink_import`

Expected: both commands PASS.

- [ ] **Step 5: Commit desktop identity changes**

```powershell
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json src-tauri/tauri.windows.conf.json src-tauri/Info.plist src-tauri/wix/per-user-main.wxs src-tauri/tests/deeplink_import.rs src-tauri/src/deeplink/tests.rs
git commit -m "feat: rename desktop identity to cc-launch"
```

### Task 4: Rename cc-boot handoff integration and its tests

**Files:**
- Modify: `integrations/cc-boot/package.json`
- Modify: `integrations/cc-boot/src/constants.ts`
- Modify: `integrations/cc-boot/src/handoff/deep-link.ts`
- Modify: `integrations/cc-boot/src/handoff/detector.ts`
- Modify: `integrations/cc-boot/src/handoff/installer.ts`
- Modify: `integrations/cc-boot/src/proxy/detector.ts`
- Modify: `integrations/cc-boot/src/i18n/locales/en/common.ts`
- Modify: `integrations/cc-boot/src/i18n/locales/zh-CN/common.ts`
- Test: `integrations/cc-boot/tests/handoff/deep-link.test.ts`
- Test: `integrations/cc-boot/tests/handoff/detector.test.ts`

- [ ] **Step 1: Update failing handoff expectations**

Change deep-link expectations to `cclaunch://`. Extend detector tests with Windows, macOS, AppImage, Deb/RPM, and Flatpak expectations whose paths/package IDs contain `cc-launch` or `com.cclaunch.desktop`.

- [ ] **Step 2: Run cc-boot tests and verify they fail**

Run: `pnpm --dir integrations/cc-boot test -- --runInBand`

Expected: deep-link and detector expectations fail against CC Switch constants.

- [ ] **Step 3: Replace the handoff constants atomically**

In `constants.ts`, define:

```ts
export const CC_LAUNCH_APP_ID = 'com.cclaunch.desktop'
export const CC_LAUNCH_BREW_CASK = 'cc-launch'
export const CC_LAUNCH_DEEP_LINK_PREFIX = 'cclaunch://'
```

Use these names in installer, detector, deep-link and proxy detector code. Change user-facing localization values to `cc-launch`. Update package description and keywords. Keep an external repository or release URL unchanged until a real cc-launch release repository is configured; expose a clearly named release-base constant rather than inventing a GitHub repository.

- [ ] **Step 4: Run cc-boot checks**

Run: `pnpm --dir integrations/cc-boot typecheck; pnpm --dir integrations/cc-boot test; pnpm --dir integrations/cc-boot build`

Expected: all commands PASS.

- [ ] **Step 5: Commit cc-boot integration changes**

```powershell
git add integrations/cc-boot
git commit -m "feat: hand off cc-boot to cc-launch"
```

### Task 5: Update Linux metadata, release assets, and CI naming

**Files:**
- Rename: `flatpak/com.ccswitch.desktop.yml` to `flatpak/com.cclaunch.desktop.yml`
- Rename: `flatpak/com.ccswitch.desktop.metainfo.xml` to `flatpak/com.cclaunch.desktop.metainfo.xml`
- Rename: `flatpak/com.ccswitch.desktop.desktop` to `flatpak/com.cclaunch.desktop.desktop`
- Modify: `flatpak/README.md`
- Modify: `.github/workflows/release.yml`
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/wsl2-nightly.yml`

- [ ] **Step 1: Update metadata references and add a validation search**

Change Flatpak application IDs, desktop executable references and visible names to `com.cclaunch.desktop` and `cc-launch`. Update release asset glob patterns from `cc-switch.exe` to `cc-launch.exe`. Keep WSL test-directory names as `cc-switch-*` only if they are purely temporary fixture labels; rename them when they are treated as emitted artifact names.

Add this CI verification step after dependency setup:

```yaml
- name: Verify package identity
  shell: bash
  run: |
    test -f src-tauri/tauri.conf.json
    grep -q '"identifier": "com.cclaunch.desktop"' src-tauri/tauri.conf.json
    grep -q '"productName": "cc-launch"' src-tauri/tauri.conf.json
```

- [ ] **Step 2: Validate metadata before changing release behavior**

Run: `rg -n 'com\.ccswitch\.desktop|ccswitch://|cc-switch\.exe' flatpak src-tauri .github/workflows`

Expected: only explicitly retained historical/protocol references remain; no packaging or registration reference remains.

- [ ] **Step 3: Add target-platform packaging verification**

Ensure the existing Windows, macOS and Linux release jobs each invoke `pnpm tauri build` on their native runner and upload platform-native artifacts. Do not add cross-compilation from Windows. Make the artifact-selection code fail with an explicit `throw`/`exit 1` when the expected `cc-launch` output is absent.

- [ ] **Step 4: Commit packaging and workflow metadata**

```powershell
git add flatpak .github/workflows/release.yml .github/workflows/ci.yml .github/workflows/wsl2-nightly.yml
git commit -m "ci: publish cc-launch platform artifacts"
```

### Task 6: Update owned documentation and run final verification

**Files:**
- Modify: `README.cc-father.md`
- Modify: `README.md`
- Modify: `README_ZH.md`
- Modify: `README_DE.md`
- Modify: `README_JA.md`
- Modify: `CONTRIBUTING.md`
- Modify: `首次启动.md`

- [ ] **Step 1: Update only owned product references**

Change names, local data paths, deep links, build output paths and three-platform install instructions to `cc-launch`. Preserve GitHub URLs, external sponsor tracking IDs, historical release references and promo codes unless their owner supplies a replacement URL/code.

- [ ] **Step 2: Run static identity scans**

Run:

```powershell
rg -n 'ccswitch://|com\.ccswitch\.desktop|\.cc-switch|cc-switch\.db|cc-switch-sync|cc-switch-webdav-sync' src-tauri integrations/cc-boot flatpak .github README*.md CONTRIBUTING.md 首次启动.md
```

Expected: no old value in application identity, local application data, deep-link registration, sync namespace, Flatpak metadata, or package artifact selection. Review every remaining occurrence as a deliberately retained protocol marker, test fixture, external URL, or third-party code.

- [ ] **Step 3: Run the complete local verification set**

Run:

```powershell
pnpm typecheck
pnpm test:unit
pnpm --dir integrations/cc-boot typecheck
pnpm --dir integrations/cc-boot test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm build
```

Expected: PASS. Record any Windows symlink-permission failures separately as environment failures, with the exact test names; do not mark the feature as verified until all non-environment failures are fixed.

- [ ] **Step 4: Inspect Windows output names**

Run: `Get-ChildItem src-tauri\target\release\bundle -Recurse -File | Select-Object FullName`

Expected: Windows installer and executable names use `cc-launch`; no generated cc-switch package is selected for release.

- [ ] **Step 5: Commit documentation and verification updates**

```powershell
git add README*.md CONTRIBUTING.md 首次启动.md
git commit -m "docs: document cc-launch build and data paths"
```

## Spec Coverage Review

- New application identity and deep link: Task 3.
- New local data, database, backup and skills boundary: Tasks 1 and 2.
- New WebDAV/S3 namespace and rejected old manifest: Task 2.
- cc-boot installation, detection, handoff and tests: Task 4.
- Windows, macOS and Linux package metadata and native-runner verification: Task 5.
- User documentation plus full validation: Task 6.

No task reads, migrates, writes, or deletes old cc-switch application data.
