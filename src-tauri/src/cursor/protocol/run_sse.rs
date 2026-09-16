//! Cursor RunSSE request and Connect stream framing.

use bytes::Bytes;
use serde_json::Value;

use super::{
    connect::{self, END_STREAM_FLAG},
    proto::aiserver::v1 as ai,
};
use crate::cursor::error::{CursorError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSseError {
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunSseFrame {
    Data { flags: u8, payload: Bytes },
    End { error: Option<RunSseError> },
}

/// Decode the Connect-framed request body used by AgentService/RunSSE.
pub fn decode_request(body: &[u8]) -> Result<ai::BidiRequestId> {
    let request: ai::BidiRequestId = connect::decode_unary(body)?;
    if request.request_id.trim().is_empty() {
        return Err(CursorError::Protocol(
            "RunSSE request_id is required".to_string(),
        ));
    }
    Ok(request)
}

/// Decode all response frames and enforce that the terminal frame is final.
pub fn decode_response_body(body: &[u8]) -> Result<Vec<RunSseFrame>> {
    let frames = connect::decode_frames(body)?;
    let mut decoded = Vec::with_capacity(frames.len());
    let mut ended = false;

    for (flags, payload) in frames {
        if ended {
            return Err(CursorError::Protocol(
                "RunSSE frame received after terminal frame".to_string(),
            ));
        }
        if flags & END_STREAM_FLAG != 0 {
            decoded.push(RunSseFrame::End {
                error: parse_terminal_error(&payload)?,
            });
            ended = true;
        } else {
            decoded.push(RunSseFrame::Data { flags, payload });
        }
    }

    if !ended {
        return Err(CursorError::Protocol(
            "RunSSE response is missing terminal frame".to_string(),
        ));
    }
    Ok(decoded)
}

fn parse_terminal_error(payload: &[u8]) -> Result<Option<RunSseError>> {
    let value: Value = serde_json::from_slice(payload).map_err(|error| {
        CursorError::Protocol(format!("RunSSE terminal frame is not JSON: {error}"))
    })?;
    let Some(error) = value.get("error") else {
        return Ok(None);
    };
    let object = error.as_object().ok_or_else(|| {
        CursorError::Protocol("RunSSE terminal error must be a JSON object".to_string())
    })?;
    Ok(Some(RunSseError {
        code: object
            .get("code")
            .and_then(Value::as_str)
            .map(str::to_owned),
        message: object
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned),
    }))
}
