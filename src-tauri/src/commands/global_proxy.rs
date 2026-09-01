//! 全局出站代理相关命令
//!
//! 提供获取、设置和测试全局代理的 Tauri 命令。

use crate::proxy::http_client;
use crate::store::AppState;
use serde::Serialize;
use std::fs;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 获取全局代理 URL
///
/// 返回当前配置的代理 URL，null 表示直连。
#[tauri::command]
pub fn get_global_proxy_url(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let result = state.db.get_global_proxy_url().map_err(|e| e.to_string())?;
    log::debug!(
        "[GlobalProxy] [GP-010] Read from database: {}",
        result
            .as_ref()
            .map(|u| http_client::mask_url(u))
            .unwrap_or_else(|| "None".to_string())
    );
    Ok(result)
}

/// 设置全局代理 URL
///
/// - 传入非空字符串：启用代理
/// - 传入空字符串：清除代理（直连）
///
/// 执行顺序：先验证 → 写 DB → 再应用
/// 这样确保 DB 写失败时不会出现运行态与持久化不一致的问题
#[tauri::command]
pub fn set_global_proxy_url(state: tauri::State<'_, AppState>, url: String) -> Result<(), String> {
    // 调试：显示接收到的 URL 信息（不包含敏感内容）
    let has_auth = url.contains('@') && (url.starts_with("http://") || url.starts_with("socks"));
    log::debug!(
        "[GlobalProxy] [GP-011] Received URL: length={}, has_auth={}",
        url.len(),
        has_auth
    );

    let url_opt = if url.trim().is_empty() {
        None
    } else {
        Some(url.as_str())
    };

    // 1. 先验证代理配置是否有效（不应用）
    http_client::validate_proxy(url_opt)?;

    // 2. 验证成功后保存到数据库
    state
        .db
        .set_global_proxy_url(url_opt)
        .map_err(|e| e.to_string())?;

    // 3. DB 写入成功后再应用到运行态
    http_client::apply_proxy(url_opt)?;

    log::info!(
        "[GlobalProxy] [GP-009] Configuration updated: {}",
        url_opt
            .map(http_client::mask_url)
            .unwrap_or_else(|| "direct connection".to_string())
    );

    Ok(())
}

/// 代理测试结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    /// 是否连接成功
    pub success: bool,
    /// 延迟（毫秒）
    pub latency_ms: u64,
    /// 错误信息
    pub error: Option<String>,
}

/// 测试代理连接
///
/// 通过指定的代理 URL 发送测试请求，返回连接结果和延迟。
/// 使用多个测试目标，任一成功即认为代理可用。
#[tauri::command]
pub async fn test_proxy_url(url: String) -> Result<ProxyTestResult, String> {
    if url.trim().is_empty() {
        return Err("Proxy URL is empty".to_string());
    }

    let start = Instant::now();

    // 构建带代理的临时客户端
    let proxy = reqwest::Proxy::all(&url).map_err(|e| format!("Invalid proxy URL: {e}"))?;

    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(std::time::Duration::from_secs(10))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build client: {e}"))?;

    // 使用多个测试目标，提高兼容性
    // 优先使用 httpbin（专门用于 HTTP 测试），回退到其他公共端点
    let test_urls = [
        "https://chatgpt.com/backend-api/codex/responses",
        "https://api.openai.com/v1/models",
        "https://httpbin.org/get",
        "https://www.google.com",
        "https://api.anthropic.com",
    ];

    let mut last_error = None;

    for test_url in test_urls {
        match client.head(test_url).send().await {
            Ok(resp) => {
                let latency = start.elapsed().as_millis() as u64;
                log::debug!(
                    "[GlobalProxy] Test successful: {} -> {} via {} ({}ms)",
                    http_client::mask_url(&url),
                    test_url,
                    resp.status(),
                    latency
                );
                return Ok(ProxyTestResult {
                    success: true,
                    latency_ms: latency,
                    error: None,
                });
            }
            Err(e) => {
                log::debug!("[GlobalProxy] Test to {test_url} failed: {e}");
                last_error = Some(e);
            }
        }
    }

    // 所有测试目标都失败
    let latency = start.elapsed().as_millis() as u64;
    let error_msg = last_error
        .map(|e| e.to_string())
        .unwrap_or_else(|| "All test targets failed".to_string());

    log::debug!(
        "[GlobalProxy] Test failed: {} -> {} ({}ms)",
        http_client::mask_url(&url),
        error_msg,
        latency
    );

    Ok(ProxyTestResult {
        success: false,
        latency_ms: latency,
        error: Some(error_msg),
    })
}

/// 获取当前出站代理状态
///
/// 返回当前是否启用了出站代理以及代理 URL。
#[tauri::command]
pub fn get_upstream_proxy_status() -> UpstreamProxyStatus {
    let url = http_client::get_current_proxy_url();
    UpstreamProxyStatus {
        enabled: url.is_some(),
        proxy_url: url,
    }
}

/// 出站代理状态信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamProxyStatus {
    /// 是否启用代理
    pub enabled: bool,
    /// 代理 URL
    pub proxy_url: Option<String>,
}

/// 检测到的代理信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedProxy {
    /// 代理 URL
    pub url: String,
    /// 代理类型 (http/socks5)
    pub proxy_type: String,
    /// 端口
    pub port: u16,
}

/// 常见代理端口配置
/// 格式：(端口, 主要类型, 是否同时支持 http 和 socks5)
/// 对于 mixed 端口，会同时返回两种协议供用户选择
const PROXY_PORTS: &[(u16, &str, bool)] = &[
    (7890, "http", true),     // Clash (mixed mode)
    (7891, "socks5", false),  // Clash SOCKS only
    (1080, "socks5", false),  // 通用 SOCKS5
    (8080, "http", false),    // 通用 HTTP
    (8888, "http", false),    // Charles/Fiddler
    (3128, "http", false),    // Squid
    (10808, "socks5", false), // V2Ray SOCKS
    (10809, "http", false),   // V2Ray HTTP
];

/// 扫描本地代理
///
/// 检测常见端口是否有代理服务在运行。
/// 使用异步任务避免阻塞 UI 线程。
#[tauri::command]
pub async fn scan_local_proxies() -> Vec<DetectedProxy> {
    // 使用 spawn_blocking 避免阻塞主线程
    tokio::task::spawn_blocking(|| {
        let mut found = Vec::new();

        for &(port, primary_type, is_mixed) in PROXY_PORTS {
            let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
            if TcpStream::connect_timeout(&addr.into(), Duration::from_millis(100)).is_ok() {
                // 添加主要类型
                found.push(DetectedProxy {
                    url: format!("{primary_type}://127.0.0.1:{port}"),
                    proxy_type: primary_type.to_string(),
                    port,
                });
                // 对于 mixed 端口，同时添加另一种协议
                if is_mixed {
                    let alt_type = if primary_type == "http" {
                        "socks5"
                    } else {
                        "http"
                    };
                    found.push(DetectedProxy {
                        url: format!("{alt_type}://127.0.0.1:{port}"),
                        proxy_type: alt_type.to_string(),
                        port,
                    });
                }
            }
        }

        found
    })
    .await
    .unwrap_or_default()
}

const CODEX_PROXY_BEGIN: &str = "# BEGIN CC2CX CODEX PROXY";
const CODEX_PROXY_END: &str = "# END CC2CX CODEX PROXY";

fn codex_proxy_env_lines(proxy_url: &str) -> Result<String, String> {
    if proxy_url
        .chars()
        .any(|character| matches!(character, '\r' | '\n' | '"'))
    {
        return Err("代理地址包含不能写入 .env 的字符".to_string());
    }
    let proxy_url = proxy_url.trim();
    let parsed = url::Url::parse(proxy_url).map_err(|error| format!("代理地址无效: {error}"))?;
    if parsed.host_str().is_none() || parsed.port().is_none() {
        return Err("代理地址必须包含主机和端口".to_string());
    }
    let scheme = parsed.scheme().to_ascii_lowercase();
    let masked_url = http_client::mask_url(proxy_url);
    match scheme.as_str() {
        "http" | "https" => Ok(format!(
            "HTTP_PROXY=\"{proxy_url}\"\nHTTPS_PROXY=\"{proxy_url}\"\nNO_PROXY=\"localhost,127.0.0.1,::1\""
        )),
        "socks5" | "socks5h" => Ok(format!(
            "ALL_PROXY=\"{proxy_url}\"\nNO_PROXY=\"localhost,127.0.0.1,::1\""
        )),
        _ => Err(format!("不支持的 Codex 代理协议: {masked_url}")),
    }
}

fn remove_codex_proxy_env(existing: &str) -> Result<String, String> {
    let mut output = String::with_capacity(existing.len());
    let mut in_managed_block = false;
    let mut found_begin = false;
    let mut found_end = false;
    for line in existing.split_inclusive('\n') {
        if line.trim() == CODEX_PROXY_BEGIN {
            if in_managed_block || found_begin {
                return Err("Codex .env 包含重复的 CC2CX 代理块".to_string());
            }
            in_managed_block = true;
            found_begin = true;
            continue;
        }
        if line.trim() == CODEX_PROXY_END {
            if !in_managed_block {
                return Err("Codex .env 的 CC2CX 代理结束标记不匹配".to_string());
            }
            in_managed_block = false;
            found_end = true;
            continue;
        }
        if !in_managed_block {
            output.push_str(line);
        }
    }
    if in_managed_block || (found_begin != found_end) {
        return Err("Codex .env 的 CC2CX 代理块不完整".to_string());
    }
    Ok(output)
}

fn upsert_codex_proxy_env(existing: &str, proxy_url: &str) -> Result<String, String> {
    let lines = codex_proxy_env_lines(proxy_url)?;
    let cleaned = remove_codex_proxy_env(existing)?;
    let newline = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut output = cleaned;
    if !output.is_empty() && !output.ends_with('\n') {
        output.push_str(newline);
    }
    output.push_str(CODEX_PROXY_BEGIN);
    output.push_str(newline);
    output.push_str(&lines.replace('\n', newline));
    output.push_str(newline);
    output.push_str(CODEX_PROXY_END);
    output.push_str(newline);
    Ok(output)
}

fn codex_env_path() -> PathBuf {
    crate::codex_config::get_codex_config_dir().join(".env")
}

fn codex_env_backup_path(path: &Path) -> PathBuf {
    path.with_file_name(".env.cc2cx.bak")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProxyEnvStatus {
    pub enabled: bool,
    pub path: String,
    pub proxy_url: Option<String>,
    pub backup_path: Option<String>,
    pub port_reachable: Option<bool>,
    pub env_txt_detected: bool,
}

fn read_codex_env(path: &Path) -> Result<String, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err("Codex .env 是符号链接，CC2CX 不会自动修改".to_string());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(format!("读取 Codex .env 元数据失败: {error}")),
    }
    fs::read_to_string(path).map_err(|error| format!("Codex .env 不是有效的 UTF-8 文本: {error}"))
}

fn codex_managed_proxy_url(content: &str) -> Option<String> {
    let mut in_managed_block = false;
    for line in content.lines() {
        if line.trim() == CODEX_PROXY_BEGIN {
            in_managed_block = true;
            continue;
        }
        if line.trim() == CODEX_PROXY_END {
            break;
        }
        if !in_managed_block {
            continue;
        }
        if let Some(value) = line
            .strip_prefix("HTTP_PROXY=\"")
            .or_else(|| line.strip_prefix("ALL_PROXY=\""))
            .and_then(|value| value.strip_suffix('"'))
        {
            return Some(value.to_string());
        }
    }
    None
}

fn proxy_port_reachable(proxy_url: &str) -> Option<bool> {
    let parsed = url::Url::parse(proxy_url).ok()?;
    let host = parsed.host_str()?;
    if !matches!(host, "127.0.0.1" | "localhost" | "::1") {
        return None;
    }
    let port = parsed.port_or_known_default()?;
    let address = format!("{host}:{port}");
    let addresses = std::net::ToSocketAddrs::to_socket_addrs(&address).ok()?;
    Some(
        addresses.into_iter().any(|address| {
            TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok()
        }),
    )
}

fn read_codex_proxy_env_status() -> Result<CodexProxyEnvStatus, String> {
    let path = codex_env_path();
    read_codex_proxy_env_status_at(&path)
}

fn read_codex_proxy_env_status_at(path: &Path) -> Result<CodexProxyEnvStatus, String> {
    let content = read_codex_env(&path)?;
    remove_codex_proxy_env(&content)?;
    let enabled = content.contains(CODEX_PROXY_BEGIN) && content.contains(CODEX_PROXY_END);
    let raw_proxy_url = enabled.then(|| codex_managed_proxy_url(&content)).flatten();
    if enabled && raw_proxy_url.is_none() {
        return Err("Codex .env 的 CC2CX 代理块缺少代理地址".to_string());
    }
    let port_reachable = raw_proxy_url.as_deref().and_then(proxy_port_reachable);
    let proxy_url = raw_proxy_url.as_deref().map(http_client::mask_url);
    Ok(CodexProxyEnvStatus {
        enabled,
        path: path.to_string_lossy().into_owned(),
        proxy_url,
        backup_path: enabled.then(|| codex_env_backup_path(&path).to_string_lossy().into_owned()),
        port_reachable,
        env_txt_detected: path.with_file_name(".env.txt").is_file(),
    })
}

fn write_codex_proxy_env(proxy_url: Option<&str>) -> Result<CodexProxyEnvStatus, String> {
    let path = codex_env_path();
    write_codex_proxy_env_at(&path, proxy_url)
}

fn write_codex_proxy_env_at(
    path: &Path,
    proxy_url: Option<&str>,
) -> Result<CodexProxyEnvStatus, String> {
    let existing = read_codex_env(&path)?;
    let updated = match proxy_url.filter(|url| !url.trim().is_empty()) {
        Some(proxy_url) => upsert_codex_proxy_env(&existing, proxy_url)?,
        None => remove_codex_proxy_env(&existing)?,
    };
    if updated == existing {
        return read_codex_proxy_env_status_at(path);
    }
    if path.exists() {
        let backup = codex_env_backup_path(&path);
        crate::config::atomic_write_private(&backup, existing.as_bytes())
            .map_err(|error| format!("备份 Codex .env 失败: {error}"))?;
    }
    crate::config::atomic_write_private(&path, updated.as_bytes())
        .map_err(|error| format!("写入 Codex .env 失败: {error}"))?;
    read_codex_proxy_env_status_at(path)
}

#[tauri::command]
pub fn get_codex_proxy_env_status() -> Result<CodexProxyEnvStatus, String> {
    read_codex_proxy_env_status()
}

#[tauri::command]
pub fn set_codex_proxy_env(proxy_url: String) -> Result<CodexProxyEnvStatus, String> {
    let proxy_url = proxy_url.trim();
    let proxy_url = (!proxy_url.is_empty()).then_some(proxy_url);
    if let Some(proxy_url) = proxy_url {
        codex_proxy_env_lines(proxy_url)?;
    }
    let status = write_codex_proxy_env(proxy_url)?;
    log::info!(
        "[GlobalProxy] Codex .env proxy {}: {}",
        if status.enabled {
            "enabled"
        } else {
            "disabled"
        },
        http_client::mask_url(status.proxy_url.as_deref().unwrap_or("direct"))
    );
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_proxy_env_rejects_unsupported_schemes() {
        assert!(codex_proxy_env_lines("http://127.0.0.1:7890").is_ok());
        assert!(codex_proxy_env_lines("socks5h://127.0.0.1:1080").is_ok());
        assert!(codex_proxy_env_lines("ftp://127.0.0.1:21").is_err());
        assert!(codex_proxy_env_lines("http://127.0.0.1").is_err());
        assert!(codex_proxy_env_lines("http://127.0.0.1:7890\nINJECTED=value").is_err());
    }

    #[test]
    fn codex_proxy_env_upsert_preserves_user_values_and_replaces_managed_block() {
        let existing = "CUSTOM=value\n\n# BEGIN CC2CX CODEX PROXY\nHTTP_PROXY=\"http://old:1\"\n# END CC2CX CODEX PROXY\n";
        let updated = upsert_codex_proxy_env(existing, "http://127.0.0.1:7890")
            .expect("proxy block should be generated");

        assert!(updated.contains("CUSTOM=value"));
        assert!(updated.contains("HTTP_PROXY=\"http://127.0.0.1:7890\""));
        assert!(!updated.contains("http://old:1"));
        assert_eq!(updated.matches("BEGIN CC2CX CODEX PROXY").count(), 1);
    }

    #[test]
    fn codex_proxy_env_remove_keeps_unmanaged_content() {
        let existing = "CUSTOM=value\n# BEGIN CC2CX CODEX PROXY\nALL_PROXY=\"socks5://127.0.0.1:1080\"\n# END CC2CX CODEX PROXY\n";
        let cleaned = remove_codex_proxy_env(existing).expect("managed block should be removed");

        assert_eq!(cleaned.trim(), "CUSTOM=value");
        assert!(!cleaned.contains("CC2CX CODEX PROXY"));
    }

    #[test]
    fn codex_proxy_status_reads_only_the_managed_block() {
        let content = "HTTP_PROXY=\"http://user-owned:9000\"\n# BEGIN CC2CX CODEX PROXY\nHTTP_PROXY=\"http://127.0.0.1:7890\"\n# END CC2CX CODEX PROXY\n";
        assert_eq!(
            codex_managed_proxy_url(content).as_deref(),
            Some("http://127.0.0.1:7890")
        );
    }

    #[test]
    fn codex_proxy_env_preserves_crlf_outside_the_managed_block() {
        let existing = "CUSTOM=one\r\nOTHER=two\r\n";
        let updated = upsert_codex_proxy_env(existing, "http://127.0.0.1:7890")
            .expect("enable proxy with CRLF");
        assert!(updated.starts_with("CUSTOM=one\r\nOTHER=two\r\n"));
        assert!(!updated.replace("\r\n", "").contains('\n'));
        let cleaned = remove_codex_proxy_env(&updated).expect("remove managed block");
        assert_eq!(cleaned, existing);
    }

    #[test]
    fn codex_proxy_file_roundtrip_backs_up_and_preserves_user_content() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let codex_dir = directory.path().join(".codex");
        fs::create_dir_all(&codex_dir).expect("create Codex directory");
        let path = codex_dir.join(".env");
        fs::write(&path, "CUSTOM=value\n").expect("write existing env");

        let enabled =
            write_codex_proxy_env_at(&path, Some("http://127.0.0.1:7890")).expect("enable proxy");
        assert!(enabled.enabled);
        assert_eq!(
            fs::read_to_string(codex_env_backup_path(&path)).expect("read backup"),
            "CUSTOM=value\n"
        );
        let enabled_content = fs::read_to_string(&path).expect("read enabled env");
        assert!(enabled_content.contains("CUSTOM=value"));
        assert!(enabled_content.contains(CODEX_PROXY_BEGIN));

        let disabled = write_codex_proxy_env_at(&path, None).expect("disable proxy");
        assert!(!disabled.enabled);
        assert_eq!(
            fs::read_to_string(&path).expect("read cleaned env").trim(),
            "CUSTOM=value"
        );
    }
}
