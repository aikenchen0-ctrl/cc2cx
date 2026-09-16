//! Construction boundary between cc2cx provider records and Cursor providers.
//!
//! The factory deliberately accepts only an explicitly declared OpenAI Chat Completions
//! provider. It reuses cc2cx's existing adapter URL and authentication extraction so Cursor
//! does not grow a second credential format or silently reinterpret another protocol.

use crate::{
    app_config::AppType,
    database::Database,
    provider::Provider,
    proxy::providers::{get_adapter_for_provider_type, AuthStrategy, ProviderType},
};

use std::sync::Arc;

use super::{
    error::{CursorError, Result},
    provider::{
        AnthropicAuth, AnthropicMessagesConfig, AnthropicMessagesProvider, CursorModelPolicy,
        CursorProvider, OpenAiChatConfig, OpenAiChatProvider,
    },
};

/// Builds Cursor provider sources from the existing cc2cx provider record.
pub struct CursorProviderFactory;

impl CursorProviderFactory {
    /// Build a provider source from the database's selected provider for an app.
    ///
    /// The database lookup is intentionally kept at this boundary: Cursor selection is a
    /// read-only projection of the database's `is_current` marker and must not mutate the
    /// device-level settings cache when a stale local provider ID exists.
    pub fn from_current(
        database: &Database,
        app_type: &AppType,
    ) -> Result<Arc<dyn CursorProvider>> {
        let app_id = app_type.as_str();
        let provider_id =
            crate::settings::get_effective_current_provider_readonly(database, app_type)
                .map_err(|error| {
                    CursorError::Config(format!("读取 current provider 失败: {error}"))
                })?
                .ok_or_else(|| {
                    CursorError::Config(format!("未为应用 {app_id} 配置 current provider"))
                })?;
        let provider = database
            .get_provider_by_id(&provider_id, app_id)
            .map_err(|error| CursorError::Config(format!("读取 current provider 失败: {error}")))?
            .ok_or_else(|| {
                CursorError::Config(format!(
                    "current provider {provider_id} 在应用 {app_id} 中不存在"
                ))
            })?;

        Self::from_provider(&provider, app_type)
    }

    fn from_provider(provider: &Provider, app_type: &AppType) -> Result<Arc<dyn CursorProvider>> {
        let format = provider_format(provider);
        if format == "openai_chat" {
            return Ok(Arc::new(Self::openai_chat(provider, app_type)?));
        }
        if format.is_empty() || format == "anthropic" || format == "anthropic_messages" {
            return Ok(Arc::new(Self::anthropic_messages(provider, app_type)?));
        }
        Err(CursorError::Config(format!(
            "Cursor provider api_format 不支持: {format}"
        )))
    }

    pub fn anthropic_messages(
        provider: &Provider,
        app_type: &AppType,
    ) -> Result<AnthropicMessagesProvider> {
        if provider.is_codex_oauth() || provider.is_xai_oauth() || provider.is_github_copilot() {
            return Err(CursorError::Config(
                "Cursor provider 不支持托管 OAuth 或 Copilot 认证形态".to_string(),
            ));
        }
        let provider_type = ProviderType::from_app_type_and_config(app_type, provider)
            .ok_or_else(|| CursorError::Config("Cursor provider 类型无法确定".to_string()))?;
        if matches!(
            provider_type,
            ProviderType::Gemini
                | ProviderType::GeminiCli
                | ProviderType::GitHubCopilot
                | ProviderType::CodexOAuth
                | ProviderType::XaiOAuth
        ) {
            return Err(CursorError::Config(format!(
                "Cursor provider 类型 {} 不支持 Anthropic Messages",
                provider_type.as_str()
            )));
        }
        let adapter = get_adapter_for_provider_type(&provider_type);
        let base_url = adapter
            .extract_base_url(provider)
            .map_err(|_| CursorError::Config("Cursor provider 缺少 base_url 配置".to_string()))?;
        let auth = adapter.extract_auth(provider).ok_or_else(|| {
            CursorError::Config("Cursor provider 缺少 authentication 配置".to_string())
        })?;
        let auth_mode = match auth.strategy {
            AuthStrategy::ClaudeAuth | AuthStrategy::Bearer => AnthropicAuth::Bearer,
            AuthStrategy::Anthropic => AnthropicAuth::ApiKey,
            _ => {
                return Err(CursorError::Config(
                    "Cursor provider 的 authentication strategy 不支持 Anthropic Messages"
                        .to_string(),
                ))
            }
        };
        let is_full_url = provider
            .meta
            .as_ref()
            .and_then(|meta| meta.is_full_url)
            .unwrap_or(false);
        let request_url = if is_full_url {
            base_url.trim().to_string()
        } else {
            adapter.build_url(base_url.trim(), "/v1/messages")
        };
        validate_request_url(&request_url)?;
        let model_ids = cursor_model_ids(provider)?;
        let mut config = AnthropicMessagesConfig {
            request_url,
            api_key: auth.api_key,
            auth: auth_mode,
            ..Default::default()
        };
        config.timeout = OpenAiChatConfig::default().timeout;
        let model_policy = cursor_model_policy(provider)?;
        let source = AnthropicMessagesProvider::new_with_model_policy(config, model_policy)?
            .with_model_ids(model_ids);
        Ok(source)
    }

    /// Build an OpenAI-compatible Chat Completions source from a configured provider.
    ///
    /// `openai_chat` must be explicitly declared in provider metadata or the stored provider
    /// record. The general Claude resolver is intentionally not used here because it also
    /// interprets the legacy `openrouter_compat_mode` flag; that compatibility heuristic is not
    /// an explicit wire-format declaration and must not opt a provider into Cursor.
    pub fn openai_chat(provider: &Provider, app_type: &AppType) -> Result<OpenAiChatProvider> {
        ensure_openai_chat_format(provider)?;

        if provider.is_codex_oauth() || provider.is_xai_oauth() || provider.is_github_copilot() {
            return Err(CursorError::Config(
                "Cursor provider 不支持托管 OAuth 或 Copilot 认证形态".to_string(),
            ));
        }

        let provider_type = ProviderType::from_app_type_and_config(app_type, provider)
            .ok_or_else(|| CursorError::Config("Cursor provider 类型无法确定".to_string()))?;
        if matches!(
            provider_type,
            ProviderType::Gemini
                | ProviderType::GeminiCli
                | ProviderType::GitHubCopilot
                | ProviderType::CodexOAuth
                | ProviderType::XaiOAuth
        ) {
            return Err(CursorError::Config(format!(
                "Cursor provider 类型 {} 不支持 OpenAI Chat Completions",
                provider_type.as_str()
            )));
        }

        let adapter = get_adapter_for_provider_type(&provider_type);
        let base_url = adapter
            .extract_base_url(provider)
            .map_err(|_| CursorError::Config("Cursor provider 缺少 base_url 配置".to_string()))?;
        let base_url = base_url.trim();
        if base_url.is_empty() {
            return Err(CursorError::Config(
                "Cursor provider 缺少 base_url 配置".to_string(),
            ));
        }

        let auth = adapter.extract_auth(provider).ok_or_else(|| {
            CursorError::Config("Cursor provider 缺少 authentication 配置".to_string())
        })?;
        if auth.api_key.trim().is_empty() {
            return Err(CursorError::Config(
                "Cursor provider 缺少 authentication 配置".to_string(),
            ));
        }
        if !matches!(
            auth.strategy,
            AuthStrategy::Bearer | AuthStrategy::ClaudeAuth
        ) {
            return Err(CursorError::Config(
                "Cursor provider 的 authentication strategy 不支持 OpenAI Chat Completions"
                    .to_string(),
            ));
        }

        let is_full_url = provider
            .meta
            .as_ref()
            .and_then(|meta| meta.is_full_url)
            .unwrap_or(false);
        let request_url = if is_full_url {
            base_url.to_string()
        } else {
            adapter.build_url(base_url, "/v1/chat/completions")
        };
        validate_request_url(&request_url)?;

        let model_policy = cursor_model_policy(provider)?;
        OpenAiChatProvider::new_with_model_policy(
            OpenAiChatConfig {
                request_url,
                api_key: auth.api_key,
                custom_headers: Default::default(),
                timeout: OpenAiChatConfig::default().timeout,
            },
            model_policy,
        )
    }
}

fn provider_format(provider: &Provider) -> &str {
    provider
        .meta
        .as_ref()
        .and_then(|meta| meta.api_format.as_deref())
        .or_else(|| {
            provider
                .settings_config
                .get("api_format")
                .and_then(|value| value.as_str())
        })
        .unwrap_or("")
}

fn cursor_model_policy(provider: &Provider) -> Result<CursorModelPolicy> {
    let meta = provider.meta.as_ref();
    let routes = meta
        .map(|meta| {
            meta.cursor_model_routes
                .iter()
                .map(|(requested, upstream)| (requested.clone(), upstream.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let env = provider.settings_config.get("env");
    let mut known_models = Vec::new();
    for key in [
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        "ANTHROPIC_DEFAULT_FABLE_MODEL",
        "CLAUDE_CODE_SUBAGENT_MODEL",
    ] {
        if let Some(model) = env
            .and_then(|env| env.get(key))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|model| !model.is_empty())
        {
            known_models.push(model.to_owned());
        }
    }

    let default_model = meta
        .and_then(|meta| meta.cursor_default_model.clone())
        .or_else(|| {
            env.and_then(|env| env.get("ANTHROPIC_MODEL"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .map(str::to_owned)
        });

    CursorModelPolicy::strict(routes, known_models, default_model).map_err(CursorError::Config)
}

fn cursor_model_ids(provider: &Provider) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    if let Some(meta) = provider.meta.as_ref() {
        ids.extend(meta.cursor_model_routes.keys().cloned());
        ids.extend(meta.cursor_model_routes.values().cloned());
        if let Some(model) = meta.cursor_default_model.as_ref() {
            ids.push(model.clone());
        }
    }
    if let Some(env) = provider.settings_config.get("env") {
        for key in [
            "ANTHROPIC_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_FABLE_MODEL",
            "CLAUDE_CODE_SUBAGENT_MODEL",
        ] {
            if let Some(model) = env
                .get(key)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|model| !model.is_empty())
            {
                ids.push(model.to_owned());
            }
        }
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn ensure_openai_chat_format(provider: &Provider) -> Result<()> {
    let format = provider
        .meta
        .as_ref()
        .and_then(|meta| meta.api_format.as_deref())
        .or_else(|| {
            provider
                .settings_config
                .get("api_format")
                .and_then(|value| value.as_str())
        })
        .unwrap_or("");

    if format == "openai_chat" {
        Ok(())
    } else {
        Err(CursorError::Config(
            "Cursor provider 必须显式声明 api_format=openai_chat".to_string(),
        ))
    }
}

fn validate_request_url(request_url: &str) -> Result<()> {
    let parsed = url::Url::parse(request_url)
        .map_err(|_| CursorError::Config("Cursor provider 的 base_url 不是有效 URL".to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(CursorError::Config(
            "Cursor provider 的 base_url 必须是带主机的 HTTP(S) URL".to_string(),
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(CursorError::Config(
            "Cursor provider 的 base_url 不得包含 userinfo".to_string(),
        ));
    }
    Ok(())
}
