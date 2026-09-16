//! Internal Cursor-to-provider contract.
//!
//! This module deliberately stops at normalized values. It does not perform network I/O,
//! select a provider, execute tools, or expose Cursor protobuf types to the existing JSON
//! gateway. Those concerns belong to later P2 slices.

use super::protocol::{
    bidi::DecodedAppend,
    connect::{CursorOutputEncoder, EncodedCursorFrame},
    proto::agent::v1::{self as agent, agent_client_message::Message},
};
use super::transport::TransportHandle;
use crate::cursor::error::{CursorError, Result as CursorResult};
use futures::{stream, Stream, StreamExt};
use std::pin::Pin;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CursorAction {
    Run,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnsupportedFeature {
    Exec,
    ExecControl,
    Kv,
    ConversationAction,
    InteractionResponse,
    ClientHeartbeat,
    Prewarm,
    ToolArgumentsDelta,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CursorField {
    History,
    Images,
    McpTools,
    ReasoningEffort,
    Cancellation,
    SideEffectRetry,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldDisposition {
    Dropped(CursorField),
    Unsupported(CursorField),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorRequest {
    pub request_id: String,
    pub append_seqno: i64,
    pub conversation_id: Option<String>,
    pub model_id: Option<String>,
    pub user_message: Option<String>,
    pub user_message_id: Option<String>,
    pub action: CursorAction,
    pub field_dispositions: Vec<FieldDisposition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorRequestError {
    pub unsupported: Vec<UnsupportedFeature>,
}

impl CursorRequest {
    pub fn from_decoded_append(decoded: DecodedAppend) -> Result<Self, CursorRequestError> {
        let message = decoded.message.message.as_ref();
        let Some(message) = message else {
            return Err(CursorRequestError {
                unsupported: vec![UnsupportedFeature::ConversationAction],
            });
        };

        let Message::RunRequest(run) = message else {
            return Err(CursorRequestError {
                unsupported: vec![unsupported_message(message)],
            });
        };

        Ok(Self {
            request_id: decoded.request_id,
            append_seqno: decoded.seqno,
            conversation_id: run
                .conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            model_id: model_id(run),
            user_message: user_message(run).map(|value| value.text.clone()),
            user_message_id: user_message(run).map(|value| value.message_id.clone()),
            action: CursorAction::Run,
            field_dispositions: p2_field_dispositions(run),
        })
    }
}

fn p2_field_dispositions(run: &agent::AgentRunRequest) -> Vec<FieldDisposition> {
    let mut dispositions = Vec::new();
    let user_action = run
        .action
        .as_ref()
        .and_then(|action| action.action.as_ref())
        .and_then(|action| match action {
            agent::conversation_action::Action::UserMessageAction(action) => Some(action),
            agent::conversation_action::Action::CancelAction(_) => None,
        });

    if user_action
        .and_then(|action| action.conversation_history.as_ref())
        .is_some()
    {
        dispositions.push(FieldDisposition::Dropped(CursorField::History));
    }

    if user_action
        .and_then(|action| action.user_message.as_ref())
        .and_then(|message| message.selected_context.as_ref())
        .is_some_and(|context| !context.selected_images.is_empty())
    {
        dispositions.push(FieldDisposition::Unsupported(CursorField::Images));
    }

    let request_context_has_tools = user_action
        .and_then(|action| action.request_context.as_ref())
        .is_some_and(|context| {
            !context.tools.is_empty()
                || context
                    .mcp_file_system_options
                    .as_ref()
                    .is_some_and(|options| !options.mcp_descriptors.is_empty())
        });
    let run_has_tools = run
        .mcp_tools
        .as_ref()
        .is_some_and(|tools| !tools.mcp_tools.is_empty())
        || run
            .mcp_file_system_options
            .as_ref()
            .is_some_and(|options| !options.mcp_descriptors.is_empty());
    if request_context_has_tools || run_has_tools {
        dispositions.push(FieldDisposition::Unsupported(CursorField::McpTools));
    }

    let has_reasoning_parameters = run
        .requested_model
        .as_ref()
        .is_some_and(|model| model.max_mode || !model.parameters.is_empty())
        || run
            .model_details
            .as_ref()
            .is_some_and(|model| model.max_mode || model.thinking_details.is_some());
    if has_reasoning_parameters {
        dispositions.push(FieldDisposition::Dropped(CursorField::ReasoningEffort));
    }

    dispositions
}

fn model_id(run: &agent::AgentRunRequest) -> Option<String> {
    run.requested_model
        .as_ref()
        .map(|model| model.model_id.as_str())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            run.model_details
                .as_ref()
                .map(|model| model.model_id.as_str())
                .filter(|value| !value.trim().is_empty())
        })
        .map(str::trim)
        .map(str::to_owned)
}

fn user_message(run: &agent::AgentRunRequest) -> Option<&agent::UserMessage> {
    let action = run.action.as_ref()?.action.as_ref()?;
    let agent::conversation_action::Action::UserMessageAction(action) = action else {
        return None;
    };
    action.user_message.as_ref()
}

fn unsupported_message(message: &Message) -> UnsupportedFeature {
    match message {
        Message::RunRequest(_) => unreachable!("run requests are handled before classification"),
        Message::ExecClientMessage(_) => UnsupportedFeature::Exec,
        Message::ExecClientControlMessage(_) => UnsupportedFeature::ExecControl,
        Message::KvClientMessage(_) => UnsupportedFeature::Kv,
        Message::ConversationAction(_) => UnsupportedFeature::ConversationAction,
        Message::InteractionResponse(_) => UnsupportedFeature::InteractionResponse,
        Message::ClientHeartbeat(_) => UnsupportedFeature::ClientHeartbeat,
        Message::PrewarmRequest(_) => UnsupportedFeature::Prewarm,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderInvocation {
    pub request_id: String,
    pub conversation_id: Option<String>,
    pub model_id: String,
    pub user_message: Option<String>,
    pub user_message_id: Option<String>,
    pub field_dispositions: Vec<FieldDisposition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderInvocationError {
    pub reason: &'static str,
}

pub type SyntheticProviderStream = Pin<Box<dyn Stream<Item = ProviderEvent> + Send>>;

/// Normalized provider stream errors. The bridge never exposes the provider's wire error type
/// to Cursor; only a classified code and display-safe message cross this boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderStreamError {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderStreamOutcome {
    Completed,
    Failed,
    Truncated,
    IdleTimeout,
    Cancelled,
}

pub struct SyntheticProvider;

impl SyntheticProvider {
    pub fn stream(
        invocation: ProviderInvocation,
        cancellation: CancellationToken,
    ) -> SyntheticProviderStream {
        if invocation.model_id.trim().is_empty() {
            return Box::pin(stream::once(async {
                ProviderEvent::Error {
                    code: "invalid_invocation".into(),
                    message: "Cursor request model_id is required".into(),
                }
            }));
        }

        let events = vec![
            ProviderEvent::TextDelta("synthetic response".into()),
            ProviderEvent::Usage {
                input_tokens: 2,
                output_tokens: 2,
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
            },
            ProviderEvent::Done,
        ];
        Box::pin(stream::unfold(
            (events.into_iter(), cancellation, false),
            |(mut events, cancellation, cancelled_emitted)| async move {
                if cancelled_emitted {
                    return None;
                }
                if cancellation.is_cancelled() {
                    return Some((
                        ProviderEvent::Error {
                            code: "cancelled".into(),
                            message: "Cursor request cancelled".into(),
                        },
                        (Vec::new().into_iter(), cancellation, true),
                    ));
                }
                events
                    .next()
                    .map(|event| (event, (events, cancellation, false)))
            },
        ))
    }
}

/// Run a normalized provider stream through the replayable Cursor transport.
///
/// This is the only P2 boundary between a provider implementation and Cursor framing. It owns
/// idle detection, cancellation, terminal-frame policy, and the mapping from provider events to
/// `AgentServerMessage` data frames. A zero idle timeout disables the timer for local providers
/// whose own stream already has a deadline.
pub async fn run_provider_stream<S>(
    transport: TransportHandle,
    provider: S,
    idle_timeout: Duration,
) -> CursorResult<ProviderStreamOutcome>
where
    S: Stream<Item = Result<ProviderEvent, ProviderStreamError>> + Send,
{
    let cancellation = transport.cancellation_token();
    tokio::pin!(provider);
    let mut encoder = CursorOutputEncoder::default();

    loop {
        if cancellation.is_cancelled() || transport.is_disconnected() {
            return Ok(ProviderStreamOutcome::Cancelled);
        }

        let next = if idle_timeout.is_zero() {
            tokio::select! {
                _ = cancellation.cancelled() => return Ok(ProviderStreamOutcome::Cancelled),
                item = provider.next() => item,
            }
        } else {
            tokio::select! {
                _ = cancellation.cancelled() => return Ok(ProviderStreamOutcome::Cancelled),
                item = provider.next() => item,
                _ = tokio::time::sleep(idle_timeout) => {
                    let frames = encoder.encode_event(&CursorOutputEvent::Error {
                        code: "upstream_unavailable".into(),
                        message: format!(
                            "Cursor provider idle timeout after {} ms",
                            idle_timeout.as_millis()
                        ),
                    })?;
                    let _ = publish_encoded_frames(&transport, frames)?;
                    return Ok(terminal_outcome(&transport, ProviderStreamOutcome::IdleTimeout));
                }
            }
        };

        if cancellation.is_cancelled() || transport.is_disconnected() {
            return Ok(ProviderStreamOutcome::Cancelled);
        }

        let Some(item) = next else {
            let frames = encoder.encode_event(&CursorOutputEvent::Error {
                code: "upstream_unavailable".into(),
                message: "Cursor provider stream ended without a terminal Done event".into(),
            })?;
            let _ = publish_encoded_frames(&transport, frames)?;
            return Ok(terminal_outcome(
                &transport,
                ProviderStreamOutcome::Truncated,
            ));
        };

        let event = match item {
            Ok(event) => event,
            Err(error) => {
                let frames = encoder.encode_event(&CursorOutputEvent::Error {
                    code: error.code,
                    message: error.message,
                })?;
                let _ = publish_encoded_frames(&transport, frames)?;
                return Ok(terminal_outcome(&transport, ProviderStreamOutcome::Failed));
            }
        };
        let completed = matches!(event, ProviderEvent::Done);
        let output = CursorOutputEvent::from_provider(&event)
            .ok_or_else(|| CursorError::Protocol("provider emitted an unmapped event".into()))?;
        let frames = encoder.encode_event(&output)?;
        if publish_encoded_frames(&transport, frames)? {
            let outcome = if completed {
                ProviderStreamOutcome::Completed
            } else {
                ProviderStreamOutcome::Failed
            };
            return Ok(terminal_outcome(&transport, outcome));
        }
    }
}

fn terminal_outcome(
    transport: &TransportHandle,
    outcome: ProviderStreamOutcome,
) -> ProviderStreamOutcome {
    if transport.is_disconnected() || transport.cancellation_token().is_cancelled() {
        ProviderStreamOutcome::Cancelled
    } else {
        outcome
    }
}

/// Run the P2 synthetic provider through the same bridge used by future real providers.
///
/// This intentionally remains a testable single-turn bridge: it does not select a real
/// provider, execute tools, or retry requests with side effects.
pub async fn run_synthetic_stream(
    transport: TransportHandle,
    invocation: ProviderInvocation,
) -> CursorResult<()> {
    let cancellation = transport.cancellation_token();
    let provider = SyntheticProvider::stream(invocation, cancellation)
        .map(Ok::<ProviderEvent, ProviderStreamError>);
    run_provider_stream(transport, provider, Duration::ZERO)
        .await
        .map(|_| ())
}

fn publish_encoded_frames(
    transport: &TransportHandle,
    frames: Vec<EncodedCursorFrame>,
) -> CursorResult<bool> {
    for frame in frames {
        match frame {
            EncodedCursorFrame::Data(frame) => {
                if !transport.emit_frame(frame) {
                    return Ok(true);
                }
            }
            EncodedCursorFrame::End(frame) => {
                let _ = transport.finish(frame);
                return Ok(true);
            }
        }
    }
    Ok(false)
}

impl ProviderInvocation {
    pub fn from_request(request: CursorRequest) -> Result<Self, ProviderInvocationError> {
        let Some(model_id) = request
            .model_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Err(ProviderInvocationError {
                reason: "Cursor request model_id is required",
            });
        };

        Ok(Self {
            request_id: request.request_id,
            conversation_id: request.conversation_id,
            model_id: model_id.to_owned(),
            user_message: request.user_message,
            user_message_id: request.user_message_id,
            field_dispositions: request.field_dispositions,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderEvent {
    TextDelta(String),
    ThinkingDelta(String),
    ToolCallStart {
        call_id: String,
        name: String,
    },
    ToolCallArgumentsDelta {
        call_id: String,
        delta: String,
    },
    Heartbeat,
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: Option<u64>,
        cache_write_tokens: Option<u64>,
        reasoning_tokens: Option<u64>,
    },
    Done,
    Error {
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CursorOutputEvent {
    TextDelta(String),
    ThinkingDelta(String),
    ToolCallStart {
        call_id: String,
        name: String,
    },
    Heartbeat,
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: Option<u64>,
        cache_write_tokens: Option<u64>,
        reasoning_tokens: Option<u64>,
    },
    TurnEnded,
    Error {
        code: String,
        message: String,
    },
    Unsupported {
        feature: UnsupportedFeature,
    },
}

impl CursorOutputEvent {
    pub fn from_provider(event: &ProviderEvent) -> Option<Self> {
        Some(match event {
            ProviderEvent::TextDelta(text) => Self::TextDelta(text.clone()),
            ProviderEvent::ThinkingDelta(text) => Self::ThinkingDelta(text.clone()),
            ProviderEvent::ToolCallStart { call_id, name } => Self::ToolCallStart {
                call_id: call_id.clone(),
                name: name.clone(),
            },
            ProviderEvent::ToolCallArgumentsDelta { .. } => Self::Unsupported {
                feature: UnsupportedFeature::ToolArgumentsDelta,
            },
            ProviderEvent::Heartbeat => Self::Heartbeat,
            ProviderEvent::Usage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_write_tokens,
                reasoning_tokens,
            } => Self::Usage {
                input_tokens: *input_tokens,
                output_tokens: *output_tokens,
                cache_read_tokens: *cache_read_tokens,
                cache_write_tokens: *cache_write_tokens,
                reasoning_tokens: *reasoning_tokens,
            },
            ProviderEvent::Done => Self::TurnEnded,
            ProviderEvent::Error { code, message } => Self::Error {
                code: code.clone(),
                message: message.clone(),
            },
        })
    }
}
