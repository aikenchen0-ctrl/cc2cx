//! Decoding and extracting the first validated Cursor BidiAppend request.

use prost::Message;

use super::{
    connect,
    proto::{agent::v1 as agent, aiserver::v1 as ai},
};
use crate::cursor::error::{CursorError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAppend {
    pub request_id: String,
    pub seqno: i64,
    pub message: agent::AgentClientMessage,
}

impl DecodedAppend {
    pub fn model_id(&self) -> Option<&str> {
        let agent::agent_client_message::Message::RunRequest(request) =
            self.message.message.as_ref()?
        else {
            return None;
        };
        request
            .requested_model
            .as_ref()
            .map(|model| model.model_id.as_str())
            .filter(|model| !model.is_empty())
            .or_else(|| {
                request
                    .model_details
                    .as_ref()
                    .map(|model| model.model_id.as_str())
                    .filter(|model| !model.is_empty())
            })
    }

    pub fn conversation_id(&self) -> Option<&str> {
        let agent::agent_client_message::Message::RunRequest(request) =
            self.message.message.as_ref()?
        else {
            return None;
        };
        request.conversation_id.as_deref()
    }

    pub fn user_message_text(&self) -> Option<&str> {
        self.user_message().map(|message| message.text.as_str())
    }

    pub fn user_message_id(&self) -> Option<&str> {
        self.user_message()
            .map(|message| message.message_id.as_str())
    }

    fn user_message(&self) -> Option<&agent::UserMessage> {
        let agent::agent_client_message::Message::RunRequest(request) =
            self.message.message.as_ref()?
        else {
            return None;
        };
        let action = request.action.as_ref()?.action.as_ref()?;
        let agent::conversation_action::Action::UserMessageAction(action) = action else {
            return None;
        };
        action.user_message.as_ref()
    }
}

pub fn decode_append(request: &ai::BidiAppendRequest) -> Result<DecodedAppend> {
    let request_id = request
        .request_id
        .as_ref()
        .map(|id| id.request_id.as_str())
        .filter(|id| !id.is_empty())
        .ok_or_else(|| CursorError::Protocol("BidiAppend request_id is required".into()))?;
    if !request.data_binary.is_empty() {
        return Err(CursorError::Protocol(
            "BidiAppend data_binary is not part of the captured protocol".into(),
        ));
    }
    if request.data.is_empty() {
        return Err(CursorError::Protocol(
            "BidiAppend contains no AgentClientMessage".into(),
        ));
    }
    let payload = hex::decode(&request.data)
        .map_err(|error| CursorError::Protocol(format!("invalid BidiAppend hex: {error}")))?;
    let message = agent::AgentClientMessage::decode(payload.as_slice()).map_err(|error| {
        CursorError::Protocol(format!("AgentClientMessage decode failed: {error}"))
    })?;
    Ok(DecodedAppend {
        request_id: request_id.into(),
        seqno: request.append_seqno,
        message,
    })
}

pub fn decode_connect_body(body: &[u8]) -> Result<ai::BidiAppendRequest> {
    connect::decode_unary(body)
}
