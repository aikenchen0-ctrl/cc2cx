//! OpenAI-compatible Chat Completions provider bridge for Cursor.
//!
//! This module owns only the provider wire boundary. It deliberately does not select a
//! configured product provider, execute tools, or expose protobuf types to the existing JSON
//! proxy. The first slice accepts a normalized [`ProviderInvocation`], sends a single user turn,
//! and converts an SSE response into the normalized [`ProviderEvent`] stream used by the Cursor
//! protocol adapter.

use std::{collections::BTreeMap, pin::Pin, time::Duration};

use async_stream::stream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use http::HeaderMap;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::adapter::{ProviderEvent, ProviderInvocation, ProviderStreamError};

fn is_loopback_host(host: Option<&str>) -> bool {
    matches!(host, Some("127.0.0.1" | "localhost" | "::1" | "[::1]"))
}

fn env_proxy_url(scheme: &str) -> Option<String> {
    let keys: &[&str] = if scheme.eq_ignore_ascii_case("https") {
        &[
            "HTTPS_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "all_proxy",
            "HTTP_PROXY",
            "http_proxy",
        ]
    } else {
        &[
            "HTTP_PROXY",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
            "HTTPS_PROXY",
            "https_proxy",
        ]
    };
    keys.iter()
        .find_map(|key| std::env::var(key).ok())
        .filter(|value| !value.trim().is_empty())
}

fn build_provider_http_client(timeout: Duration) -> super::error::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(timeout)
        .proxy(reqwest::Proxy::custom(|url| {
            if is_loopback_host(url.host_str()) {
                None
            } else {
                env_proxy_url(url.scheme())
            }
        }))
        .build()
        .map_err(|error| {
            super::error::CursorError::Config(format!(
                "构建 Cursor provider HTTP 客户端失败: {error}"
            ))
        })
}

/// Model selection policy applied to requests entering the production Cursor provider.
///
/// A Cursor model ID is a client-facing selector, not proof that the upstream accepts the same
/// string. Strict policies therefore require either an explicit alias, a declared upstream
/// model, or an explicit fallback. The low-level default remains passthrough for callers that
/// construct a provider directly; the database-backed factory always installs a strict policy.
#[derive(Clone, Debug, Default)]
pub struct CursorModelPolicy {
    routes: BTreeMap<String, String>,
    known_models: Vec<String>,
    default_model: Option<String>,
    reject_unknown: bool,
}

impl CursorModelPolicy {
    pub fn strict(
        routes: impl IntoIterator<Item = (String, String)>,
        known_models: impl IntoIterator<Item = String>,
        default_model: Option<String>,
    ) -> Result<Self, String> {
        let mut normalized_routes = BTreeMap::new();
        for (requested, upstream) in routes {
            let requested = requested.trim();
            let upstream = upstream.trim();
            if requested.is_empty() || upstream.is_empty() {
                return Err(
                    "Cursor model route must contain non-empty requested and upstream IDs".into(),
                );
            }
            normalized_routes.insert(requested.to_owned(), upstream.to_owned());
        }

        let default_model = default_model
            .map(|model| model.trim().to_owned())
            .filter(|model| !model.is_empty());
        let mut normalized_known = Vec::new();
        for model in known_models {
            let model = model.trim();
            if !model.is_empty() && !normalized_known.iter().any(|known| known == model) {
                normalized_known.push(model.to_owned());
            }
        }
        for model in normalized_routes.values() {
            if !normalized_known.iter().any(|known| known == model) {
                normalized_known.push(model.clone());
            }
        }
        if let Some(model) = default_model.as_ref() {
            if !normalized_known.iter().any(|known| known == model) {
                normalized_known.push(model.clone());
            }
        }

        Ok(Self {
            routes: normalized_routes,
            known_models: normalized_known,
            default_model,
            reject_unknown: true,
        })
    }

    pub fn passthrough() -> Self {
        Self::default()
    }

    fn resolve(
        &self,
        invocation: ProviderInvocation,
    ) -> Result<ResolvedProviderInvocation, ProviderStreamError> {
        let requested_model = invocation.model_id.trim().to_owned();
        if requested_model.is_empty() {
            return Err(provider_error(
                "provider_model",
                "Cursor request model_id is required",
            ));
        }

        let model_id = self
            .routes
            .get(&requested_model)
            .cloned()
            .or_else(|| {
                self.known_models
                    .iter()
                    .find(|model| model.as_str() == requested_model)
                    .cloned()
            })
            .or_else(|| self.default_model.clone())
            .or_else(|| (!self.reject_unknown).then(|| requested_model.clone()));

        let Some(model_id) = model_id else {
            return Err(provider_error(
                "provider_model",
                format!("Cursor requested model is not configured: {requested_model}"),
            ));
        };

        let mut invocation = invocation;
        invocation.model_id = model_id.clone();
        Ok(ResolvedProviderInvocation {
            requested_model,
            model_id,
            invocation,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProviderInvocation {
    pub requested_model: String,
    pub model_id: String,
    pub invocation: ProviderInvocation,
}

pub type ProviderStream =
    Pin<Box<dyn Stream<Item = Result<ProviderEvent, ProviderStreamError>> + Send>>;

/// Provider boundary consumed by the Cursor protocol backend.
///
/// Implementations own upstream authentication and network I/O. The backend only supplies the
/// normalized invocation and cancellation token, then forwards the resulting event stream to the
/// Cursor framing bridge.
pub trait CursorProvider: Send + Sync {
    fn stream(
        &self,
        invocation: ProviderInvocation,
        cancellation: CancellationToken,
    ) -> ProviderStream;

    /// Return client-visible model IDs for the local Cursor model catalog.
    ///
    /// Providers that do not expose a catalog keep the default empty response; this method is
    /// metadata-only and never returns credentials or endpoint details.
    fn model_ids(&self) -> Vec<String> {
        Vec::new()
    }

    /// Perform a bounded, bodyless startup probe. Implementations that do not expose a safe
    /// probe keep the default no-op behavior and remain observable through their first stream.
    fn preflight(
        &self,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), ProviderStreamError>> + Send + '_>>
    {
        Box::pin(async { Ok(()) })
    }
}

/// Configuration for one OpenAI-compatible Chat Completions endpoint.
///
/// `api_key` is kept in memory only by this adapter. Callers must obtain it from the existing
/// credential store and must not put it into fixtures or diagnostic output. An empty key is
/// allowed for local gateways that authenticate by another header.
#[derive(Clone, Debug)]
pub struct OpenAiChatConfig {
    pub request_url: String,
    pub api_key: String,
    pub custom_headers: HeaderMap,
    pub timeout: Duration,
}

impl Default for OpenAiChatConfig {
    fn default() -> Self {
        Self {
            request_url: "https://api.openai.com/v1/chat/completions".into(),
            api_key: String::new(),
            custom_headers: HeaderMap::new(),
            timeout: Duration::from_secs(120),
        }
    }
}

#[derive(Clone)]
pub struct OpenAiChatProvider {
    client: reqwest::Client,
    config: OpenAiChatConfig,
    model_policy: CursorModelPolicy,
}

impl std::fmt::Debug for OpenAiChatProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenAiChatProvider")
            .field("request_url", &self.config.request_url)
            .field("has_api_key", &(!self.config.api_key.is_empty()))
            .field("custom_header_count", &self.config.custom_headers.len())
            .field("timeout", &self.config.timeout)
            .field("model_route_count", &self.model_policy.routes.len())
            .field(
                "has_model_default",
                &self.model_policy.default_model.is_some(),
            )
            .finish()
    }
}

impl OpenAiChatProvider {
    pub fn new(config: OpenAiChatConfig) -> super::error::Result<Self> {
        Self::new_with_model_policy(config, CursorModelPolicy::passthrough())
    }

    pub fn new_with_model_policy(
        config: OpenAiChatConfig,
        model_policy: CursorModelPolicy,
    ) -> super::error::Result<Self> {
        let client = build_provider_http_client(config.timeout)?;
        Ok(Self {
            client,
            config,
            model_policy,
        })
    }

    pub fn model_ids(&self) -> Vec<String> {
        self.model_policy
            .known_models
            .iter()
            .cloned()
            .chain(self.model_policy.routes.keys().cloned())
            .collect()
    }

    /// Construct an adapter with a caller-owned client. This is useful for tests and for a
    /// future shared connection pool; the request/response contract remains identical.
    pub fn with_client(client: reqwest::Client, config: OpenAiChatConfig) -> Self {
        Self::with_client_and_model_policy(client, config, CursorModelPolicy::passthrough())
    }

    pub fn with_client_and_model_policy(
        client: reqwest::Client,
        config: OpenAiChatConfig,
        model_policy: CursorModelPolicy,
    ) -> Self {
        Self {
            client,
            config,
            model_policy,
        }
    }

    pub fn prepare_invocation(
        &self,
        invocation: ProviderInvocation,
    ) -> Result<ResolvedProviderInvocation, ProviderStreamError> {
        self.model_policy.resolve(invocation)
    }

    /// Probe endpoint reachability without sending a model request body.
    ///
    /// A 401/403 or server-side failure is surfaced before the Cursor proxy writes settings;
    /// other HTTP responses (including 404/405) prove that the endpoint is reachable and are
    /// accepted. Response bodies are deliberately never read.
    pub async fn preflight(&self) -> Result<(), ProviderStreamError> {
        let mut request = self
            .client
            .get(&self.config.request_url)
            .timeout(self.config.timeout.min(Duration::from_secs(8)))
            .headers(self.config.custom_headers.clone())
            .header(http::header::ACCEPT, "*/*")
            .header(http::header::ACCEPT_ENCODING, "identity");
        if !self.config.api_key.is_empty() {
            request = request.bearer_auth(&self.config.api_key);
        }

        let response = request.send().await.map_err(|error| {
            let code = if error.is_timeout() {
                "provider_timeout"
            } else {
                "provider_transport"
            };
            let message = if error.is_timeout() {
                "OpenAI-compatible provider startup probe timed out"
            } else {
                "OpenAI-compatible provider startup probe failed"
            };
            provider_error(code, message)
        })?;

        let status = response.status();
        if matches!(
            status,
            http::StatusCode::UNAUTHORIZED | http::StatusCode::FORBIDDEN
        ) {
            return Err(provider_error(
                "provider_auth",
                format!("OpenAI-compatible provider startup probe returned HTTP status {status}"),
            ));
        }
        if status.is_server_error() {
            return Err(provider_error(
                "provider_http",
                format!("OpenAI-compatible provider startup probe returned HTTP status {status}"),
            ));
        }
        Ok(())
    }

    pub fn stream(
        &self,
        invocation: ProviderInvocation,
        cancellation: CancellationToken,
    ) -> ProviderStream {
        let client = self.client.clone();
        let config = self.config.clone();
        let model_policy = self.model_policy.clone();
        Box::pin(stream! {
            if cancellation.is_cancelled() {
                return;
            }

            let resolved = match model_policy.resolve(invocation) {
                Ok(resolved) => resolved,
                Err(error) => {
                    yield Err(error);
                    return;
                }
            };
            let body = chat_request_body(&resolved.invocation, &resolved.model_id);
            let mut request = client
                .post(&config.request_url)
                .headers(config.custom_headers.clone())
                .header(http::header::ACCEPT, "text/event-stream")
                .header(http::header::CONTENT_TYPE, "application/json")
                .json(&body);
            if !config.api_key.is_empty() {
                request = request.bearer_auth(&config.api_key);
            }

            let response = tokio::select! {
                _ = cancellation.cancelled() => return,
                response = request.send() => response,
            };
            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    let code = if error.is_timeout() {
                        "provider_timeout"
                    } else {
                        "provider_transport"
                    };
                    let message = if error.is_timeout() {
                        "OpenAI-compatible provider request timed out"
                    } else {
                        "OpenAI-compatible provider transport failed"
                    };
                    yield Err(provider_error(
                        code,
                        message,
                    ));
                    return;
                }
            };

            let status = response.status();
            if !status.is_success() {
                let code = if matches!(status, http::StatusCode::UNAUTHORIZED | http::StatusCode::FORBIDDEN) {
                    "provider_auth"
                } else {
                    "provider_http"
                };
                yield Err(provider_error(
                    code,
                    format!("OpenAI-compatible provider returned HTTP status {status}"),
                ));
                return;
            }

            let mut body_stream = response.bytes_stream();
            let mut decoder = SseDecoder::default();
            let mut tools = BTreeMap::<usize, ToolState>::new();
            let mut done = false;
            while !done {
                let chunk = tokio::select! {
                    _ = cancellation.cancelled() => return,
                    chunk = body_stream.next() => chunk,
                };
                let Some(chunk) = chunk else { break };
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(error) => {
                        let code = if error.is_timeout() {
                            "provider_timeout"
                        } else {
                            "provider_transport"
                        };
                        let message = if error.is_timeout() {
                            "OpenAI-compatible provider stream timed out"
                        } else {
                            "OpenAI-compatible provider stream failed"
                        };
                        yield Err(provider_error(
                            code,
                            message,
                        ));
                        return;
                    }
                };
                let events = match decoder.feed(&chunk) {
                    Ok(events) => events,
                    Err(()) => {
                        yield Err(provider_error(
                            "provider_protocol",
                            "OpenAI-compatible provider returned invalid UTF-8 SSE data",
                        ));
                        return;
                    }
                };
                for data in events {
                    match parse_sse_data(&data, &mut tools) {
                        Ok(SseData::Events(events)) => {
                            for event in events {
                                yield Ok(event);
                            }
                        }
                        Ok(SseData::Done(events)) => {
                            for event in events {
                                yield Ok(event);
                            }
                            done = true;
                            break;
                        }
                        Err(error) => {
                            yield Err(error);
                            return;
                        }
                    }
                }
            }

            if !done {
                let tail = match decoder.finish() {
                    Ok(tail) => tail,
                    Err(()) => {
                        yield Err(provider_error(
                            "provider_protocol",
                            "OpenAI-compatible provider returned invalid UTF-8 SSE data",
                        ));
                        return;
                    }
                };
                if let Some(data) = tail {
                    match parse_sse_data(&data, &mut tools) {
                        Ok(SseData::Events(events)) | Ok(SseData::Done(events)) => {
                            for event in events {
                                yield Ok(event);
                            }
                            done = data.trim() == "[DONE]";
                        }
                        Err(error) => {
                            yield Err(error);
                            return;
                        }
                    }
                }
            }

            if done {
                yield Ok(ProviderEvent::Done);
            }
            // An EOF without [DONE] intentionally yields no terminal event. The Cursor bridge
            // turns that observable truncation into its structured unavailable terminal frame.
        })
    }
}

impl CursorProvider for OpenAiChatProvider {
    fn model_ids(&self) -> Vec<String> {
        OpenAiChatProvider::model_ids(self)
    }

    fn stream(
        &self,
        invocation: ProviderInvocation,
        cancellation: CancellationToken,
    ) -> ProviderStream {
        OpenAiChatProvider::stream(self, invocation, cancellation)
    }

    fn preflight(
        &self,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), ProviderStreamError>> + Send + '_>>
    {
        Box::pin(OpenAiChatProvider::preflight(self))
    }
}

/// Authentication modes supported by the Anthropic Messages API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnthropicAuth {
    ApiKey,
    Bearer,
}

/// Configuration for an Anthropic Messages endpoint.
///
/// `api_key` remains in memory only. The selected auth mode mirrors the existing Claude
/// provider semantics: `ANTHROPIC_API_KEY` uses `x-api-key`, while `ANTHROPIC_AUTH_TOKEN`
/// uses `Authorization: Bearer`.
#[derive(Clone, Debug)]
pub struct AnthropicMessagesConfig {
    pub request_url: String,
    pub api_key: String,
    pub auth: AnthropicAuth,
    pub custom_headers: HeaderMap,
    pub timeout: Duration,
    pub max_tokens: u64,
}

impl Default for AnthropicMessagesConfig {
    fn default() -> Self {
        Self {
            request_url: "https://api.anthropic.com/v1/messages".into(),
            api_key: String::new(),
            auth: AnthropicAuth::ApiKey,
            custom_headers: HeaderMap::new(),
            timeout: Duration::from_secs(120),
            max_tokens: 8192,
        }
    }
}

#[derive(Clone)]
pub struct AnthropicMessagesProvider {
    client: reqwest::Client,
    config: AnthropicMessagesConfig,
    model_ids: Vec<String>,
    model_policy: CursorModelPolicy,
}

impl std::fmt::Debug for AnthropicMessagesProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AnthropicMessagesProvider")
            .field("request_url", &self.config.request_url)
            .field("has_api_key", &(!self.config.api_key.is_empty()))
            .field("auth", &self.config.auth)
            .field("custom_header_count", &self.config.custom_headers.len())
            .field("timeout", &self.config.timeout)
            .field("max_tokens", &self.config.max_tokens)
            .finish()
    }
}

impl AnthropicMessagesProvider {
    pub fn new(config: AnthropicMessagesConfig) -> super::error::Result<Self> {
        Self::new_with_model_policy(config, CursorModelPolicy::passthrough())
    }

    pub fn new_with_model_policy(
        config: AnthropicMessagesConfig,
        model_policy: CursorModelPolicy,
    ) -> super::error::Result<Self> {
        let client = build_provider_http_client(config.timeout)?;
        Ok(Self {
            client,
            config,
            model_ids: Vec::new(),
            model_policy,
        })
    }

    pub fn with_client(client: reqwest::Client, config: AnthropicMessagesConfig) -> Self {
        Self {
            client,
            config,
            model_ids: Vec::new(),
            model_policy: CursorModelPolicy::passthrough(),
        }
    }

    pub fn with_model_ids(mut self, model_ids: Vec<String>) -> Self {
        self.model_ids = model_ids;
        self
    }

    pub fn stream(
        &self,
        invocation: ProviderInvocation,
        cancellation: CancellationToken,
    ) -> ProviderStream {
        let client = self.client.clone();
        let config = self.config.clone();
        let model_policy = self.model_policy.clone();
        Box::pin(stream! {
            if cancellation.is_cancelled() {
                return;
            }

            let resolved = match model_policy.resolve(invocation) {
                Ok(resolved) => resolved,
                Err(error) => {
                    yield Err(error);
                    return;
                }
            };
            let body = anthropic_request_body(&resolved.invocation, config.max_tokens);
            let mut headers = config.custom_headers.clone();
            headers.insert(http::header::ACCEPT, http::HeaderValue::from_static("text/event-stream"));
            headers.insert(http::header::CONTENT_TYPE, http::HeaderValue::from_static("application/json"));
            headers.insert(
                http::HeaderName::from_static("anthropic-version"),
                http::HeaderValue::from_static("2023-06-01"),
            );
            if !config.api_key.is_empty() {
                let header = match http::HeaderValue::from_str(&config.api_key) {
                    Ok(header) => header,
                    Err(_) => {
                        yield Err(provider_error(
                            "provider_auth",
                            "Anthropic provider authentication value is invalid",
                        ));
                        return;
                    }
                };
                match config.auth {
                    AnthropicAuth::ApiKey => {
                        headers.insert(http::HeaderName::from_static("x-api-key"), header);
                    }
                    AnthropicAuth::Bearer => {
                        let mut bearer = b"Bearer ".to_vec();
                        bearer.extend_from_slice(header.as_bytes());
                        let bearer = match http::HeaderValue::from_bytes(&bearer) {
                            Ok(value) => value,
                            Err(_) => {
                                yield Err(provider_error(
                                    "provider_auth",
                                    "Anthropic provider authentication value is invalid",
                                ));
                                return;
                            }
                        };
                        headers.insert(http::header::AUTHORIZATION, bearer);
                    }
                }
            }

            let request = client
                .post(&config.request_url)
                .headers(headers)
                .json(&body);
            let response = tokio::select! {
                _ = cancellation.cancelled() => return,
                response = request.send() => response,
            };
            let response = match response {
                Ok(response) => response,
                Err(_) => {
                    yield Err(provider_error(
                        "provider_transport",
                        "Anthropic provider transport failed",
                    ));
                    return;
                }
            };

            let status = response.status();
            if !status.is_success() {
                let code = if matches!(
                    status,
                    http::StatusCode::UNAUTHORIZED | http::StatusCode::FORBIDDEN
                ) {
                    "provider_auth"
                } else {
                    "provider_http"
                };
                yield Err(provider_error(
                    code,
                    format!("Anthropic provider returned HTTP status {status}"),
                ));
                return;
            }

            let mut body_stream = response.bytes_stream();
            let mut decoder = SseDecoder::default();
            let mut state = AnthropicStreamState::default();
            let mut tools = BTreeMap::<usize, AnthropicToolState>::new();
            let mut done = false;
            while !done {
                let chunk = tokio::select! {
                    _ = cancellation.cancelled() => return,
                    chunk = body_stream.next() => chunk,
                };
                let Some(chunk) = chunk else { break };
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(_) => {
                        yield Err(provider_error(
                            "provider_transport",
                            "Anthropic provider stream failed",
                        ));
                        return;
                    }
                };
                let events = match decoder.feed(&chunk) {
                    Ok(events) => events,
                    Err(()) => {
                        yield Err(provider_error(
                            "provider_protocol",
                            "Anthropic provider returned invalid UTF-8 SSE data",
                        ));
                        return;
                    }
                };
                for data in events {
                    match parse_anthropic_sse_data(&data, &mut state, &mut tools) {
                        Ok(AnthropicSseData::Events(events)) => {
                            for event in events {
                                yield Ok(event);
                            }
                        }
                        Ok(AnthropicSseData::Done(events)) => {
                            for event in events {
                                yield Ok(event);
                            }
                            done = true;
                            break;
                        }
                        Err(error) => {
                            yield Err(error);
                            return;
                        }
                    }
                }
            }

            if !done {
                let tail = match decoder.finish() {
                    Ok(tail) => tail,
                    Err(()) => {
                        yield Err(provider_error(
                            "provider_protocol",
                            "Anthropic provider returned invalid UTF-8 SSE data",
                        ));
                        return;
                    }
                };
                if let Some(data) = tail {
                    match parse_anthropic_sse_data(&data, &mut state, &mut tools) {
                        Ok(AnthropicSseData::Events(events)) | Ok(AnthropicSseData::Done(events)) => {
                            for event in events {
                                yield Ok(event);
                            }
                            done = data_type_is_message_stop(&data);
                        }
                        Err(error) => {
                            yield Err(error);
                            return;
                        }
                    }
                }
            }

            if done {
                yield Ok(ProviderEvent::Done);
            }
            // An EOF without `message_stop` intentionally yields no terminal event. The Cursor
            // bridge reports this as a truncated upstream stream.
        })
    }
}

impl CursorProvider for AnthropicMessagesProvider {
    fn model_ids(&self) -> Vec<String> {
        self.model_ids.clone()
    }

    fn stream(
        &self,
        invocation: ProviderInvocation,
        cancellation: CancellationToken,
    ) -> ProviderStream {
        AnthropicMessagesProvider::stream(self, invocation, cancellation)
    }
}

fn anthropic_request_body(invocation: &ProviderInvocation, max_tokens: u64) -> Value {
    json!({
        "model": invocation.model_id,
        "max_tokens": max_tokens,
        "messages": [{
            "role": "user",
            "content": invocation.user_message.as_deref().unwrap_or("")
        }],
        "stream": true
    })
}

#[derive(Default)]
struct AnthropicStreamState {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    has_usage: bool,
}

#[derive(Default)]
struct AnthropicToolState {
    call_id: String,
}

enum AnthropicSseData {
    Events(Vec<ProviderEvent>),
    Done(Vec<ProviderEvent>),
}

fn parse_anthropic_sse_data(
    data: &str,
    state: &mut AnthropicStreamState,
    tools: &mut BTreeMap<usize, AnthropicToolState>,
) -> Result<AnthropicSseData, ProviderStreamError> {
    let value: Value = serde_json::from_str(data).map_err(|_| {
        provider_error(
            "provider_protocol",
            "Anthropic provider returned invalid JSON in SSE data",
        )
    })?;
    if value.get("error").is_some_and(|error| !error.is_null())
        || value.get("type").and_then(Value::as_str) == Some("error")
    {
        return Err(provider_error(
            "provider_error_event",
            "Anthropic provider returned an error event",
        ));
    }

    let event_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    let mut events = Vec::new();
    match event_type {
        "message_start" => {
            if let Some(usage) = value.pointer("/message/usage") {
                update_anthropic_usage(state, usage, false);
            }
        }
        "content_block_start" => {
            let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            if let Some(content) = value.get("content_block") {
                if content.get("type").and_then(Value::as_str) == Some("tool_use") {
                    let call_id = content
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    let name = content
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    if !call_id.is_empty() && !name.is_empty() {
                        tools.insert(
                            index,
                            AnthropicToolState {
                                call_id: call_id.clone(),
                            },
                        );
                        events.push(ProviderEvent::ToolCallStart { call_id, name });
                    }
                }
            }
        }
        "content_block_delta" => {
            let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let delta = value.get("delta").unwrap_or(&Value::Null);
            match delta.get("type").and_then(Value::as_str) {
                Some("text_delta") => {
                    if let Some(text) = delta
                        .get("text")
                        .and_then(Value::as_str)
                        .filter(|text| !text.is_empty())
                    {
                        events.push(ProviderEvent::TextDelta(text.to_owned()));
                    }
                }
                Some("thinking_delta") => {
                    if let Some(text) = delta
                        .get("thinking")
                        .and_then(Value::as_str)
                        .filter(|text| !text.is_empty())
                    {
                        events.push(ProviderEvent::ThinkingDelta(text.to_owned()));
                    }
                }
                Some("input_json_delta") => {
                    if let Some(tool) = tools.get(&index) {
                        if let Some(partial_json) = delta
                            .get("partial_json")
                            .and_then(Value::as_str)
                            .filter(|value| !value.is_empty())
                        {
                            events.push(ProviderEvent::ToolCallArgumentsDelta {
                                call_id: tool.call_id.clone(),
                                delta: partial_json.to_owned(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        "message_delta" => {
            if let Some(usage) = value.get("usage") {
                update_anthropic_usage(state, usage, true);
                events.push(ProviderEvent::Usage {
                    input_tokens: state.input_tokens,
                    output_tokens: state.output_tokens,
                    cache_read_tokens: state.cache_read_tokens,
                    cache_write_tokens: state.cache_write_tokens,
                    reasoning_tokens: None,
                });
            }
        }
        "message_stop" => return Ok(AnthropicSseData::Done(events)),
        _ => {}
    }

    Ok(AnthropicSseData::Events(events))
}

fn update_anthropic_usage(state: &mut AnthropicStreamState, usage: &Value, output: bool) {
    if output {
        if let Some(tokens) = usage.get("output_tokens").and_then(Value::as_u64) {
            state.output_tokens = tokens;
        }
    } else if let Some(tokens) = usage.get("input_tokens").and_then(Value::as_u64) {
        state.input_tokens = tokens;
    }
    if let Some(tokens) = usage.get("cache_read_input_tokens").and_then(Value::as_u64) {
        state.cache_read_tokens = Some(tokens);
    }
    if let Some(tokens) = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
    {
        state.cache_write_tokens = Some(tokens);
    }
    state.has_usage = true;
}

fn data_type_is_message_stop(data: &str) -> bool {
    serde_json::from_str::<Value>(data)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(Value::as_str)
                .map(|kind| kind == "message_stop")
        })
        .unwrap_or(false)
}

fn chat_request_body(invocation: &ProviderInvocation, model_id: &str) -> Value {
    json!({
        "model": model_id,
        "messages": [{
            "role": "user",
            "content": invocation.user_message.as_deref().unwrap_or("")
        }],
        "stream": true,
        "stream_options": {"include_usage": true}
    })
}

fn provider_error(code: impl Into<String>, message: impl Into<String>) -> ProviderStreamError {
    ProviderStreamError {
        code: code.into(),
        message: message.into(),
    }
}

#[derive(Default)]
struct ToolState {
    call_id: String,
    name: String,
    pending_arguments: String,
    started: bool,
}

enum SseData {
    Events(Vec<ProviderEvent>),
    Done(Vec<ProviderEvent>),
}

fn parse_sse_data(
    data: &str,
    tools: &mut BTreeMap<usize, ToolState>,
) -> Result<SseData, ProviderStreamError> {
    if data.trim() == "[DONE]" {
        let events = flush_tools(tools)?;
        return Ok(SseData::Done(events));
    }

    let value: Value = serde_json::from_str(data).map_err(|_| {
        provider_error(
            "provider_protocol",
            "OpenAI-compatible provider returned invalid JSON in SSE data",
        )
    })?;
    if value.get("error").is_some_and(|error| !error.is_null())
        || value.get("type").and_then(Value::as_str) == Some("error")
    {
        return Err(provider_error(
            "provider_error_event",
            "OpenAI-compatible provider returned an error event",
        ));
    }

    let mut events = Vec::new();
    if let Some(usage) = value.get("usage").filter(|value| !value.is_null()) {
        events.push(parse_usage(usage)?);
    }
    if let Some(choices) = value.get("choices").and_then(Value::as_array) {
        for choice in choices {
            let index = choice.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let delta = choice.get("delta").unwrap_or(&Value::Null);
            if let Some(text) = delta
                .get("reasoning_content")
                .or_else(|| delta.get("reasoning"))
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
            {
                events.push(ProviderEvent::ThinkingDelta(text.to_owned()));
            }
            if let Some(text) = delta
                .get("content")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
            {
                events.push(ProviderEvent::TextDelta(text.to_owned()));
            }
            if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                for (position, tool) in tool_calls.iter().enumerate() {
                    let tool_index = tool
                        .get("index")
                        .and_then(Value::as_u64)
                        .map(|value| value as usize)
                        .unwrap_or(position);
                    let state = tools.entry(tool_index).or_default();
                    let was_started = state.started;
                    if let Some(id) = tool.get("id").and_then(Value::as_str) {
                        merge_fragment(&mut state.call_id, id);
                    }
                    if let Some(name) = tool
                        .get("function")
                        .and_then(|function| function.get("name"))
                        .and_then(Value::as_str)
                    {
                        merge_fragment(&mut state.name, name);
                    }
                    if let Some(arguments) = tool
                        .get("function")
                        .and_then(|function| function.get("arguments"))
                        .and_then(Value::as_str)
                        .filter(|arguments| !arguments.is_empty())
                    {
                        state.pending_arguments.push_str(arguments);
                    }
                    if !state.started && !state.call_id.is_empty() && !state.name.is_empty() {
                        events.push(ProviderEvent::ToolCallStart {
                            call_id: state.call_id.clone(),
                            name: state.name.clone(),
                        });
                        state.started = true;
                        if !state.pending_arguments.is_empty() {
                            events.push(ProviderEvent::ToolCallArgumentsDelta {
                                call_id: state.call_id.clone(),
                                delta: std::mem::take(&mut state.pending_arguments),
                            });
                        }
                    } else if was_started && !state.pending_arguments.is_empty() {
                        events.push(ProviderEvent::ToolCallArgumentsDelta {
                            call_id: state.call_id.clone(),
                            delta: std::mem::take(&mut state.pending_arguments),
                        });
                    }
                }
            }
            // `finish_reason` is intentionally observed but not copied into this first Cursor
            // event contract, whose `Done` variant has no reason field. The provider still
            // requires the explicit `[DONE]` marker before emitting `Done`.
            let _ = index;
        }
    }
    Ok(SseData::Events(events))
}

fn flush_tools(
    tools: &mut BTreeMap<usize, ToolState>,
) -> Result<Vec<ProviderEvent>, ProviderStreamError> {
    let mut events = Vec::new();
    for state in tools.values_mut() {
        if state.started {
            continue;
        }
        if state.call_id.is_empty() || state.name.is_empty() {
            return Err(provider_error(
                "provider_protocol",
                "OpenAI-compatible provider tool call is missing id or name",
            ));
        }
        events.push(ProviderEvent::ToolCallStart {
            call_id: state.call_id.clone(),
            name: state.name.clone(),
        });
        state.started = true;
        if !state.pending_arguments.is_empty() {
            events.push(ProviderEvent::ToolCallArgumentsDelta {
                call_id: state.call_id.clone(),
                delta: std::mem::take(&mut state.pending_arguments),
            });
        }
    }
    Ok(events)
}

fn merge_fragment(target: &mut String, fragment: &str) {
    if target == fragment || target.ends_with(fragment) {
        return;
    }
    if fragment.starts_with(target.as_str()) {
        *target = fragment.to_owned();
    } else {
        target.push_str(fragment);
    }
}

fn parse_usage(value: &Value) -> Result<ProviderEvent, ProviderStreamError> {
    let input_tokens = value
        .get("prompt_tokens")
        .or_else(|| value.get("input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = value
        .get("completion_tokens")
        .or_else(|| value.get("output_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read_tokens = value
        .get("cache_read_tokens")
        .or_else(|| value.get("prompt_cache_hit_tokens"))
        .and_then(Value::as_u64);
    let cache_write_tokens = value
        .get("cache_write_tokens")
        .or_else(|| value.get("prompt_cache_miss_tokens"))
        .and_then(Value::as_u64);
    let reasoning_tokens = value.get("reasoning_tokens").and_then(Value::as_u64);
    Ok(ProviderEvent::Usage {
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        reasoning_tokens,
    })
}

#[derive(Default)]
struct SseDecoder {
    buffer: Vec<u8>,
}

impl SseDecoder {
    fn feed(&mut self, chunk: &Bytes) -> Result<Vec<String>, ()> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some((index, delimiter_len)) = find_event_delimiter(&self.buffer) {
            let frame = self.buffer.drain(..index).collect::<Vec<_>>();
            self.buffer.drain(..delimiter_len);
            if let Some(data) = decode_sse_frame(&frame)? {
                events.push(data);
            }
        }
        Ok(events)
    }

    fn finish(&mut self) -> Result<Option<String>, ()> {
        if self.buffer.is_empty() {
            return Ok(None);
        }
        let frame = std::mem::take(&mut self.buffer);
        decode_sse_frame(&frame)
    }
}

fn find_event_delimiter(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer.windows(2).position(|window| window == b"\n\n");
    let crlf = buffer.windows(4).position(|window| window == b"\r\n\r\n");
    match (lf, crlf) {
        (Some(lf), Some(crlf)) if crlf < lf => Some((crlf, 4)),
        (Some(lf), _) => Some((lf, 2)),
        (None, Some(crlf)) => Some((crlf, 4)),
        (None, None) => None,
    }
}

fn decode_sse_frame(frame: &[u8]) -> Result<Option<String>, ()> {
    let text = std::str::from_utf8(frame).map_err(|_| ())?;
    let mut data_lines = Vec::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.strip_prefix(' ').unwrap_or(value));
        }
    }
    if data_lines.is_empty() {
        Ok(None)
    } else {
        Ok(Some(data_lines.join("\n")))
    }
}
