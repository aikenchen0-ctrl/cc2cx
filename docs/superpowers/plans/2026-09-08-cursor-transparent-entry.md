# Cursor Transparent Entry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route real Cursor NodeService connections for explicitly managed Cursor hosts through a reversible local fake-IP/TLS entry into the existing Cursor backend and configured Provider.

**Architecture:** Add a Windows-only, loopback-bound transparent entry beside the existing explicit HTTP MITM. A Hosts transaction maps only allowlisted Cursor names to loopback fake IPs; the entry accepts TCP 443, validates SNI, terminates TLS with the existing CA, and forwards HTTP/2/Connect requests into the existing backend router. Existing protocol, Transport, Provider, and database Harness remain the single business-logic path.

**Tech Stack:** Rust, Tokio, `tokio-rustls`, `rustls`, `rcgen`, `hudsucker` where reusable, Windows Hosts file, CryptoAPI CA material, Axum backend, existing integration tests.

---

## File Map

- Create: `src-tauri/src/cursor/hosts.rs` — hash-checked CC2CX Hosts transaction and rollback.
- Create: `src-tauri/src/cursor/fake_ip.rs` — deterministic loopback allocation and hostname mapping.
- Create: `src-tauri/src/cursor/transparent.rs` — Windows TCP/TLS listener lifecycle and SNI validation.
- Modify: `src-tauri/src/cursor/mod.rs` — export the new modules.
- Modify: `src-tauri/src/cursor/harness.rs` — start/stop the transparent entry transactionally with backend and settings.
- Modify: `src-tauri/src/cursor/settings.rs` — include `cursor.general.disableHttp2` in the reversible managed settings set.
- Modify: `src-tauri/src/cursor/profile.rs` — keep explicit proxy launch support available for diagnostics only.
- Modify: `src-tauri/src/commands/cursor.rs` and `src-tauri/src/lib.rs` — expose transparent-entry status and lifecycle only through the existing Cursor integration commands.
- Create: `src-tauri/tests/cursor_hosts.rs`, `src-tauri/tests/cursor_fake_ip.rs`, `src-tauri/tests/cursor_transparent.rs` — focused regression and integration tests.
- Modify: `docs/cursor/baseline.md` and `docs/cursor/p2-contract.md` — record the new boundary and test evidence without credentials.

## Task 1: Hosts Transaction

**Files:** Create `src-tauri/src/cursor/hosts.rs`; create `src-tauri/tests/cursor_hosts.rs`.

- [ ] **Step 1: Write failing tests** for inserting one marked block, preserving unrelated lines, idempotent enable, refusing restore after an external edit, and exact restore after an unchanged transaction.

```rust
#[test]
fn hosts_transaction_is_idempotent_and_restores_exact_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hosts");
    let original = b"127.0.0.1 localhost\n# user entry\n";
    std::fs::write(&path, original).unwrap();
    let tx = enable(&path, &[("127.0.0.2".parse().unwrap(), "api2.cursor.sh")]).unwrap();
    let applied = std::fs::read_to_string(&path).unwrap();
    let tx2 = enable(&path, &[("127.0.0.2".parse().unwrap(), "api2.cursor.sh")]).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), applied);
    assert_eq!(tx.applied_sha256(), tx2.applied_sha256());
    restore(&tx).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn hosts_transaction_refuses_to_overwrite_external_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hosts");
    std::fs::write(&path, "127.0.0.1 localhost\n").unwrap();
    let tx = enable(&path, &[("127.0.0.2".parse().unwrap(), "api2.cursor.sh")]).unwrap();
    std::fs::write(&path, "127.0.0.1 localhost\n# external edit\n").unwrap();
    let error = restore(&tx).unwrap_err();
    assert!(error.to_string().contains("外部修改"));
}
```

- [ ] **Step 2: Run the focused test and verify it fails.**

Run: `cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_hosts -- --nocapture`

Expected: FAIL because `HostsTransaction` does not exist.

- [ ] **Step 3: Implement the minimal API.** Define:

```rust
pub struct HostsTransaction { path: PathBuf, original: Vec<u8>, applied_sha256: String }
pub fn enable(path: &Path, entries: &[(IpAddr, &str)]) -> Result<HostsTransaction>
pub fn restore(transaction: &HostsTransaction) -> Result<()>
pub fn recover(path: &Path, backup: &Path) -> Result<()>
```

Use a fixed `# BEGIN CC2CX CURSOR` / `# END CC2CX CURSOR` marker, reject duplicate managed hostnames, write atomically, and compare SHA-256 before restore. Never rewrite bytes outside the managed block.

- [ ] **Step 4: Run the focused test and verify it passes.**

Run: `cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_hosts -- --nocapture`

- [ ] **Step 5: Commit.** `git add src-tauri/src/cursor/hosts.rs src-tauri/tests/cursor_hosts.rs && git commit -m "feat: add reversible cursor hosts transaction"`

## Task 2: Fake-IP Mapping

**Files:** Create `src-tauri/src/cursor/fake_ip.rs`; create `src-tauri/tests/cursor_fake_ip.rs`.

- [ ] **Step 1: Write failing tests** for deterministic allocation from `127.0.0.2`, release/reuse, duplicate hostname rejection, non-loopback rejection, and unknown fake-IP lookup failure.

```rust
#[test]
fn allocator_maps_and_releases_cursor_hosts() {
    let mut map = FakeIpMap::allocate(&["api2.cursor.sh".into(), "api3.cursor.sh".into()]).unwrap();
    assert_eq!(map.address("API2.CURSOR.SH"), Some("127.0.0.2".parse().unwrap()));
    assert_eq!(map.hostname("127.0.0.3".parse().unwrap()), Some("api3.cursor.sh"));
    map.release("api2.cursor.sh");
    assert_eq!(map.address("api2.cursor.sh"), None);
}

#[test]
fn allocator_rejects_non_loopback_or_unknown_addresses() {
    let map = FakeIpMap::allocate(&["api2.cursor.sh".into()]).unwrap();
    assert!(map.hostname("10.0.0.1".parse().unwrap()).is_none());
    assert!(map.hostname("127.0.0.254".parse().unwrap()).is_none());
}
```

- [ ] **Step 2: Run and verify failure.** `cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_fake_ip -- --nocapture`

- [ ] **Step 3: Implement the minimal API.** Define:

```rust
pub struct FakeIpMap { /* hostname and IP maps */ }
pub fn allocate(hosts: &[String]) -> Result<FakeIpMap>
impl FakeIpMap {
    pub fn address(&self, hostname: &str) -> Option<Ipv4Addr>
    pub fn hostname(&self, address: Ipv4Addr) -> Option<&str>
    pub fn entries(&self) -> Vec<(Ipv4Addr, String)>
}
```

Restrict the pool to `127.0.0.2..=127.0.0.254`, normalize hostnames case-insensitively, and keep the map process-local.

- [ ] **Step 4: Run and verify pass.** `cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_fake_ip -- --nocapture`

- [ ] **Step 5: Commit.** `git add src-tauri/src/cursor/fake_ip.rs src-tauri/tests/cursor_fake_ip.rs && git commit -m "feat: add cursor fake ip mapping"`

## Task 3: Transparent TLS Entry

**Files:** Create `src-tauri/src/cursor/transparent.rs`; create `src-tauri/tests/cursor_transparent.rs`; modify `src-tauri/src/cursor/mod.rs`.

- [ ] **Step 1: Write failing tests** for loopback-only binding, SNI-to-fake-IP matching, mismatch rejection, unsupported ALPN rejection, and clean shutdown of active connections.

```rust
#[tokio::test]
async fn transparent_entry_rejects_sni_not_matching_fake_ip() {
    let (mut runtime, ca, fake_ips) = start_test_runtime().await.unwrap();
    let result = tls_connect(runtime.address_for("api2.cursor.sh"), "api3.cursor.sh", &ca).await;
    assert!(result.unwrap_err().to_string().contains("SNI"));
    runtime.stop().await;
}

#[tokio::test]
async fn transparent_entry_shuts_down_without_leaking_tasks() {
    let (mut runtime, _ca, _fake_ips) = start_test_runtime().await.unwrap();
    let address = runtime.address_for("api2.cursor.sh");
    runtime.stop().await;
    assert!(TcpStream::connect(address).await.is_err());
}
```

- [ ] **Step 2: Run and verify failure.** `cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_transparent -- --nocapture`

- [ ] **Step 3: Implement the minimal listener.** Define:

```rust
pub struct TransparentRuntime { /* listener, stop signal, join handle */ }
pub async fn start(
    ca: LoadedCa,
    fake_ips: Arc<FakeIpMap>,
    backend: SocketAddr,
) -> Result<TransparentRuntime>
impl TransparentRuntime { pub fn address(&self) -> SocketAddr; pub async fn stop(&mut self); }
```

Bind one listener per allocated fake IP (`fake_ip:443`) so the OS routes connections for `127.0.0.2`, `127.0.0.3`, and so on into the correct runtime; tests inject a non-privileged port and the runtime exposes every bound address. Read ClientHello SNI before certificate selection, require the SNI to map to the destination fake IP, use a per-host rcgen leaf signed by the existing CA, advertise `h2`, and fail closed for missing/mismatched SNI or unsupported ALPN. Do not forward raw TLS bytes to a Provider.

- [ ] **Step 4: Run and verify pass.** `cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_transparent -- --nocapture`

- [ ] **Step 5: Commit.** `git add src-tauri/src/cursor/transparent.rs src-tauri/src/cursor/mod.rs src-tauri/tests/cursor_transparent.rs && git commit -m "feat: add transparent cursor tls entry"`

## Task 4: Backend and Harness Integration

**Files:** Modify `src-tauri/src/cursor/harness.rs`, `src-tauri/src/cursor/settings.rs`, `src-tauri/src/commands/cursor.rs`, `src-tauri/src/lib.rs`; modify `src-tauri/tests/cursor_commands.rs` and `src-tauri/tests/cursor_settings.rs`.

- [ ] **Step 1: Write failing lifecycle tests** covering start order, preflight rollback, stop order, stale transaction recovery, and status fields for Hosts/fake-IP/transparent listener.

```rust
#[tokio::test]
async fn harness_rolls_back_hosts_when_backend_preflight_fails() { /* no settings/hosts residue */ }

#[tokio::test]
async fn harness_status_exposes_transparent_entry_state() {
    let status = harness_with_test_provider().start().await.unwrap();
    assert!(status.transparent_entry.is_some());
    assert!(!status.fake_ip_entries.is_empty());
    assert!(status.managed_hosts);
}
```

- [ ] **Step 2: Run and verify failure.** `cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_commands -- --nocapture`

- [ ] **Step 3: Implement transaction order.** Extend `CursorHarnessStatus` with `transparent_entry`, `managed_hosts`, and `fake_ip_entries`. In `start()`: validate CA, allocate fake IPs, bind all transparent listeners, request the required Windows elevation for the Hosts transaction, write Hosts, run Provider preflight, then apply reversible Cursor settings. On any error, stop listeners, restore Hosts, restore settings, and release maps. In `stop()`, stop accepting new connections, stop backend, restore settings, restore Hosts, then clear status.

- [ ] **Step 4: Add `cursor.general.disableHttp2=true` to the managed settings transaction** and ensure restore preserves a user-owned prior value.

- [ ] **Step 5: Run the focused tests and the existing Cursor suite.**

Run: `cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_commands --test cursor_settings --test cursor_routes -- --test-threads=1`

- [ ] **Step 6: Commit.** `git add src-tauri/src/cursor src-tauri/src/commands/cursor.rs src-tauri/src/lib.rs src-tauri/tests/cursor_commands.rs src-tauri/tests/cursor_settings.rs && git commit -m "feat: integrate transparent cursor entry with harness"`

## Task 5: Real Cursor Evidence Runner

**Files:** Modify `src-tauri/examples/cursor_e2e_lab.rs`; create `src-tauri/tests/cursor_e2e_contract.rs`; modify `docs/cursor/baseline.md`.

- [ ] **Step 1: Write the runner contract test** asserting that the runner requires an isolated profile, emits only PID/ports/host/path metadata, and refuses to run without an explicit CA state or test-only override.

- [ ] **Step 2: Run and verify failure.** `cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_e2e_contract -- --nocapture`

- [ ] **Step 3: Update the runner** to use the transparent Harness path, print the fake-IP map without credentials, and retain `CC2CX_E2E_SKIP_CA_CHECK=1` as a test-only diagnostic switch. The runner must never touch the default Cursor profile or install directory.

- [ ] **Step 4: Build and launch the runner.**

Run: `cargo build --locked --manifest-path src-tauri/Cargo.toml --example cursor_e2e_lab`

Run: `$env:CC2CX_E2E_CA_DIR='C:\Users\血饮\.cc-launch\cursor-ca'; $env:CC2CX_E2E_SKIP_CA_CHECK='1'; & src-tauri\target\debug\examples\cursor_e2e_lab.exe`

Expected: Cursor launches with a unique profile, Hosts transaction, fake-IP entry, backend port, and transparent listener port.

- [ ] **Step 5: Verify real traffic.** Send `CC2CX_E2E_OK` in the isolated window, then assert all of the following from process/network/backend evidence: NodeService connects to loopback fake-IP/443; `AgentService` or the current protocol equivalent reaches the local backend; Provider health records a successful request; no credential or prompt body appears in logs.

- [ ] **Step 6: Commit.** `git add src-tauri/examples/cursor_e2e_lab.rs src-tauri/tests/cursor_e2e_contract.rs docs/cursor/baseline.md && git commit -m "test: verify cursor transparent entry e2e"`

## Task 6: Documentation and Full Verification

**Files:** Modify `docs/cursor/baseline.md`, `docs/cursor/p2-contract.md`, `README.md` only where the existing Cursor integration section requires factual updates.

- [ ] **Step 1: Update documentation** with the actual Windows prerequisites, exact rollback behavior, supported hosts, and the distinction between `Auto` official routing and cc2cx Provider routing.

- [ ] **Step 2: Run formatting and static checks.**

Run: `cargo fmt --all -- --check`

Run: `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

- [ ] **Step 3: Run the complete relevant test set.**

Run: `$env:CARGO_BUILD_JOBS='1'; cargo test --locked --manifest-path src-tauri/Cargo.toml --test cursor_ca --test cursor_adapter --test cursor_backend --test cursor_commands --test cursor_protocol --test cursor_provider --test cursor_routes --test cursor_settings --test cursor_transport -- --test-threads=1`

- [ ] **Step 4: Run `git diff --check` and inspect all changed files** for credentials, Cookies, prompt bodies, private keys, machine-wide bindings, or forbidden Cursor patch logic.

- [ ] **Step 5: Commit final documentation and verification evidence.** `git add docs/cursor README.md && git commit -m "docs: document cursor transparent entry verification"`

## Safety Gates

- No task may add Cursor installation patching, membership/plan fabrication, account pools, storage injection, or modification of Cursor's account database.
- No task may bind beyond loopback or change system DNS globally without a separate approved design.
- Hosts writes must use an explicit elevated operation and must fail closed when elevation is unavailable; never silently edit a different Hosts file.
- Every failure path must restore settings and Hosts before returning an error.
- Real E2E success requires backend/Provider evidence; a successful official Cursor response is insufficient.
