//! Connect protocol envelope encoding and decoding.

use bytes::{BufMut, Bytes, BytesMut};
use prost::Message;
use serde::Serialize;

use super::super::error::{CursorError, Result};
use crate::cursor::{
    adapter::{CursorOutputEvent, UnsupportedFeature},
    protocol::proto::agent::v1 as agent,
};

pub const END_STREAM_FLAG: u8 = 0x02;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectCode {
    Canceled,
    InvalidArgument,
    NotFound,
    Unavailable,
    Internal,
}

impl ConnectCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Canceled => "canceled",
            Self::InvalidArgument => "invalid_argument",
            Self::NotFound => "not_found",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConnectErrorDetail {
    #[serde(rename = "type")]
    pub type_name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectStreamError {
    pub code: ConnectCode,
    pub message: String,
    pub details: Vec<ConnectErrorDetail>,
}

#[derive(Serialize)]
struct EndStreamResponse<'a> {
    error: WireError<'a>,
}

#[derive(Serialize)]
struct WireError<'a> {
    code: &'static str,
    #[serde(skip_serializing_if = "str::is_empty")]
    message: &'a str,
    #[serde(skip_serializing_if = "details_are_empty")]
    details: &'a [ConnectErrorDetail],
}

fn details_are_empty(details: &&[ConnectErrorDetail]) -> bool {
    details.is_empty()
}

/// A single encoded Connect response frame.
///
/// Data frames carry an `AgentServerMessage`; the terminal frame carries the Connect JSON
/// status object and is intentionally kept separate from protobuf data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodedCursorFrame {
    Data(Bytes),
    End(Bytes),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PendingUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
}

/// Encodes the first provider event slice while preserving usage until turn completion.
///
/// Provider usage callbacks are commonly cumulative. The latest value wins for each bucket;
/// values are emitted once in `TurnEnded`, followed by the Connect terminal frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CursorOutputEncoder {
    usage: PendingUsage,
    ended: bool,
}

impl CursorOutputEncoder {
    pub fn encode_event(&mut self, event: &CursorOutputEvent) -> Result<Vec<EncodedCursorFrame>> {
        if self.ended {
            return Err(CursorError::Protocol(
                "Cursor output received after terminal frame".into(),
            ));
        }

        match event {
            CursorOutputEvent::Usage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_write_tokens,
                reasoning_tokens,
            } => {
                self.usage.input_tokens = Some(*input_tokens);
                self.usage.output_tokens = Some(*output_tokens);
                merge_usage_bucket(&mut self.usage.cache_read_tokens, *cache_read_tokens);
                merge_usage_bucket(&mut self.usage.cache_write_tokens, *cache_write_tokens);
                merge_usage_bucket(&mut self.usage.reasoning_tokens, *reasoning_tokens);
                Ok(Vec::new())
            }
            CursorOutputEvent::TurnEnded => {
                self.ended = true;
                let message = turn_ended_message(&self.usage);
                Ok(vec![
                    EncodedCursorFrame::Data(encode_message(&message)?),
                    EncodedCursorFrame::End(encode_end_stream()),
                ])
            }
            CursorOutputEvent::Error { code, message } => {
                self.ended = true;
                Ok(vec![EncodedCursorFrame::End(encode_output_error(
                    code, message,
                )?)])
            }
            CursorOutputEvent::Unsupported { feature } => {
                self.ended = true;
                Ok(vec![EncodedCursorFrame::End(encode_unsupported_error(
                    feature,
                )?)])
            }
            _ => Ok(vec![encode_output_event(event)?]),
        }
    }

    /// Closes a stream that ended without a provider `Done` event.
    pub fn finish(&mut self) -> Result<Vec<EncodedCursorFrame>> {
        if self.ended {
            return Ok(Vec::new());
        }
        self.ended = true;
        let message = turn_ended_message(&self.usage);
        Ok(vec![
            EncodedCursorFrame::Data(encode_message(&message)?),
            EncodedCursorFrame::End(encode_end_stream()),
        ])
    }
}

fn merge_usage_bucket(current: &mut Option<u64>, update: Option<u64>) {
    if update.is_some() {
        *current = update;
    }
}

/// Encode one non-usage output event as a Connect frame.
///
/// Usage requires [`CursorOutputEncoder`] so it cannot be accidentally discarded. A standalone
/// `TurnEnded` event is encoded as a data frame without token buckets; callers that own a stream
/// should use `CursorOutputEncoder` to append the terminal frame atomically.
pub fn encode_output_event(event: &CursorOutputEvent) -> Result<EncodedCursorFrame> {
    let message = match event {
        CursorOutputEvent::TextDelta(text) => interaction_message(
            agent::interaction_update::Message::TextDelta(agent::TextDeltaUpdate {
                text: text.clone(),
                is_server_notice: false,
            }),
        ),
        CursorOutputEvent::ThinkingDelta(text) => interaction_message(
            agent::interaction_update::Message::ThinkingDelta(agent::ThinkingDeltaUpdate {
                text: text.clone(),
                thinking_style: Some(agent::ThinkingStyle::Default as i32),
            }),
        ),
        CursorOutputEvent::ToolCallStart { call_id, name } => interaction_message(
            agent::interaction_update::Message::PartialToolCall(agent::PartialToolCallUpdate {
                call_id: call_id.clone(),
                tool_call: Some(compatibility_tool_call(call_id, name)),
                args_text_delta: String::new(),
                model_call_id: String::new(),
            }),
        ),
        CursorOutputEvent::Heartbeat => interaction_message(
            agent::interaction_update::Message::Heartbeat(agent::HeartbeatUpdate {}),
        ),
        CursorOutputEvent::TurnEnded => interaction_message(
            agent::interaction_update::Message::TurnEnded(agent::TurnEndedUpdate {
                input_tokens: None,
                output_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
            }),
        ),
        CursorOutputEvent::Usage { .. } => {
            return Err(CursorError::Protocol(
                "Cursor usage requires a stateful output encoder".into(),
            ))
        }
        CursorOutputEvent::Error { code, message } => {
            return Ok(EncodedCursorFrame::End(encode_output_error(code, message)?))
        }
        CursorOutputEvent::Unsupported { feature } => {
            return Ok(EncodedCursorFrame::End(encode_unsupported_error(feature)?))
        }
    };
    Ok(EncodedCursorFrame::Data(encode_message(&message)?))
}

fn interaction_message(message: agent::interaction_update::Message) -> agent::AgentServerMessage {
    agent::AgentServerMessage {
        message: Some(agent::agent_server_message::Message::InteractionUpdate(
            agent::InteractionUpdate {
                message: Some(message),
            },
        )),
        ttft_breakdown: None,
    }
}

fn turn_ended_message(usage: &PendingUsage) -> agent::AgentServerMessage {
    interaction_message(agent::interaction_update::Message::TurnEnded(
        agent::TurnEndedUpdate {
            input_tokens: usage.input_tokens.map(|value| value as i64),
            output_tokens: usage.output_tokens.map(|value| value as i64),
            cache_read_tokens: usage.cache_read_tokens.map(|value| value as i64),
            cache_write_tokens: usage.cache_write_tokens.map(|value| value as i64),
            reasoning_tokens: usage.reasoning_tokens.map(|value| value as i64),
        },
    ))
}

fn compatibility_tool_call(call_id: &str, name: &str) -> agent::ToolCall {
    agent::ToolCall {
        tool_call_id: Some(call_id.to_owned()),
        tool: Some(agent::tool_call::Tool::McpToolCall(agent::McpToolCall {
            args: Some(agent::McpArgs {
                name: name.to_owned(),
                tool_call_id: call_id.to_owned(),
                provider_identifier: "cc2cx-compat".into(),
                tool_name: name.to_owned(),
                server_identifier: "cc2cx-compat".into(),
            }),
            result: None,
            description: Some(name.to_owned()),
        })),
    }
}

fn encode_output_error(code: &str, message: &str) -> Result<Bytes> {
    encode_error_end_stream(&ConnectStreamError {
        code: connect_code_for_output(code),
        message: message.to_owned(),
        details: Vec::new(),
    })
}

fn encode_unsupported_error(feature: &UnsupportedFeature) -> Result<Bytes> {
    encode_error_end_stream(&ConnectStreamError {
        code: ConnectCode::InvalidArgument,
        message: format!("Cursor output feature is unsupported: {feature:?}"),
        details: Vec::new(),
    })
}

fn connect_code_for_output(code: &str) -> ConnectCode {
    let normalized = code.trim().to_ascii_lowercase();
    if normalized == "cancelled" || normalized == "canceled" || normalized == "cancel" {
        ConnectCode::Canceled
    } else if normalized == "not_found" || normalized == "notfound" {
        ConnectCode::NotFound
    } else if normalized == "invalid_argument"
        || normalized == "invalid_invocation"
        || normalized == "unsupported"
    {
        ConnectCode::InvalidArgument
    } else if normalized == "unavailable"
        || normalized.starts_with("upstream_")
        || normalized.starts_with("provider_")
    {
        ConnectCode::Unavailable
    } else {
        ConnectCode::Internal
    }
}

pub fn encode_message<M: Message>(message: &M) -> Result<Bytes> {
    let len = message.encoded_len();
    let len = u32::try_from(len)
        .map_err(|_| CursorError::Protocol("Connect payload exceeds u32 length".into()))?;
    let mut output = BytesMut::with_capacity(5 + len as usize);
    output.put_u8(0);
    output.put_u32(len);
    message
        .encode(&mut output)
        .map_err(|error| CursorError::Protocol(format!("protobuf encode failed: {error}")))?;
    Ok(output.freeze())
}

pub fn encode_end_stream() -> Bytes {
    encode_end_stream_payload(b"{}")
}

pub fn encode_error_end_stream(error: &ConnectStreamError) -> Result<Bytes> {
    let payload = serde_json::to_vec(&EndStreamResponse {
        error: WireError {
            code: error.code.as_str(),
            message: &error.message,
            details: &error.details,
        },
    })
    .map_err(|error| CursorError::Protocol(format!("Connect error encode failed: {error}")))?;
    Ok(encode_end_stream_payload(&payload))
}

fn encode_end_stream_payload(payload: &[u8]) -> Bytes {
    let len =
        u32::try_from(payload.len()).expect("Connect terminal payload length must fit in a u32");
    let mut output = BytesMut::with_capacity(5 + payload.len());
    output.put_u8(END_STREAM_FLAG);
    output.put_u32(len);
    output.extend_from_slice(payload);
    output.freeze()
}

pub fn decode_unary<M: Message + Default>(body: &[u8]) -> Result<M> {
    if body.len() >= 5 {
        let flags = body[0];
        let length = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
        if flags & END_STREAM_FLAG == 0 && length == body.len() - 5 {
            return M::decode(&body[5..]).map_err(|error| {
                CursorError::Protocol(format!("protobuf decode failed: {error}"))
            });
        }
    }
    M::decode(body)
        .map_err(|error| CursorError::Protocol(format!("protobuf decode failed: {error}")))
}

pub fn decode_frames(mut body: &[u8]) -> Result<Vec<(u8, Bytes)>> {
    let mut frames = Vec::new();
    while !body.is_empty() {
        if body.len() < 5 {
            return Err(CursorError::Protocol("truncated Connect envelope".into()));
        }
        let flags = body[0];
        let length = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
        body = &body[5..];
        if body.len() < length {
            return Err(CursorError::Protocol("truncated Connect payload".into()));
        }
        frames.push((flags, Bytes::copy_from_slice(&body[..length])));
        body = &body[length..];
    }
    Ok(frames)
}
