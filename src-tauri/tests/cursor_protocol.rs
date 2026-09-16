use bytes::Bytes;
use cc_launch_lib::cursor::adapter::{CursorOutputEvent, UnsupportedFeature};
use cc_launch_lib::cursor::protocol::{
    bidi::{decode_append, DecodedAppend},
    connect::{
        decode_frames, decode_unary, encode_end_stream, encode_message, encode_output_event,
        CursorOutputEncoder, EncodedCursorFrame,
    },
    proto::{agent::v1 as agent, aiserver::v1 as ai},
    run_sse::{decode_request, decode_response_body, RunSseFrame},
};
use prost::Message;

#[test]
fn connect_unary_round_trip_preserves_bidi_append_envelope() {
    let request = ai::BidiAppendRequest {
        request_id: Some(ai::BidiRequestId {
            request_id: "request-1".into(),
        }),
        append_seqno: 4,
        data: "0102".into(),
        ..Default::default()
    };

    let framed = encode_message(&request).expect("encode Connect frame");
    assert_eq!(&framed[..5], &[0, 0, 0, 0, request.encoded_len() as u8]);

    let decoded: ai::BidiAppendRequest = decode_unary(&framed).expect("decode Connect frame");
    assert_eq!(decoded, request);
}

#[test]
fn bidi_append_decodes_run_request_model_and_conversation() {
    let agent_message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "cc2cx-test-model".into(),
                    ..Default::default()
                }),
                conversation_id: Some("conversation-1".into()),
                ..Default::default()
            },
        )),
    };
    let request = ai::BidiAppendRequest {
        request_id: Some(ai::BidiRequestId {
            request_id: "request-1".into(),
        }),
        append_seqno: 9,
        data: hex::encode(agent_message.encode_to_vec()),
        ..Default::default()
    };

    let decoded = decode_append(&request).expect("decode BidiAppend");

    assert_eq!(decoded.request_id, "request-1");
    assert_eq!(decoded.seqno, 9);
    assert_eq!(decoded.model_id(), Some("cc2cx-test-model"));
    assert_eq!(decoded.conversation_id(), Some("conversation-1"));
    assert!(matches!(decoded, DecodedAppend { .. }));
}

#[test]
fn bidi_append_rejects_binary_payload_until_supported() {
    let request = ai::BidiAppendRequest {
        request_id: Some(ai::BidiRequestId {
            request_id: "request-1".into(),
        }),
        data_binary: vec![1, 2, 3],
        ..Default::default()
    };

    let error = decode_append(&request).expect_err("binary payload must be rejected");
    assert!(error.to_string().contains("data_binary"));
}

#[test]
fn bidi_append_extracts_user_message_from_run_request() {
    let agent_message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "hello from fixture".into(),
                                message_id: "message-1".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };
    let request = ai::BidiAppendRequest {
        request_id: Some(ai::BidiRequestId {
            request_id: "request-1".into(),
        }),
        data: hex::encode(agent_message.encode_to_vec()),
        ..Default::default()
    };

    let decoded = decode_append(&request).expect("decode user message fixture");

    assert_eq!(decoded.user_message_text(), Some("hello from fixture"));
    assert_eq!(decoded.user_message_id(), Some("message-1"));
}

#[test]
fn run_sse_request_decodes_connect_framed_request_id() {
    let body = hex::decode("00000000070a0572756e2d31").expect("valid request fixture");

    let request = decode_request(&body).expect("decode RunSSE request");

    assert_eq!(request.request_id, "run-1");
}

#[test]
fn run_sse_response_preserves_data_and_success_terminal_frame() {
    let body = hex::decode("000000000361626302000000027b7d").expect("valid stream fixture");

    let frames = decode_response_body(&body).expect("decode RunSSE response");

    assert_eq!(
        frames,
        vec![
            RunSseFrame::Data {
                flags: 0,
                payload: b"abc".as_slice().into(),
            },
            RunSseFrame::End { error: None },
        ]
    );
}

#[test]
fn run_sse_response_extracts_error_from_terminal_json_frame() {
    let body = hex::decode(
        "02000000377b226572726f72223a7b22636f6465223a22756e617661696c61626c65222c226d657373616765223a226f7665726c6f61646564227d7d",
    )
    .expect("valid error stream fixture");

    let frames = decode_response_body(&body).expect("decode RunSSE error response");

    assert_eq!(
        frames,
        vec![RunSseFrame::End {
            error: Some(cc_launch_lib::cursor::protocol::run_sse::RunSseError {
                code: Some("unavailable".into()),
                message: Some("overloaded".into()),
            }),
        }]
    );
}

#[test]
fn run_sse_response_rejects_data_after_terminal_frame() {
    let body = hex::decode("02000000027b7d000000000161").expect("valid malformed fixture");

    let error = decode_response_body(&body).expect_err("terminal frame must be final");

    assert!(error.to_string().contains("after terminal"));
}

#[test]
fn cursor_text_delta_encodes_as_a_connect_data_frame() {
    let frame = encode_output_event(&CursorOutputEvent::TextDelta("hello output".into()))
        .expect("encode text output");

    let EncodedCursorFrame::Data(frame) = frame else {
        panic!("text output must be a data frame");
    };
    let [(flags, ref payload)] = decode_frames(&frame).expect("decode text output frame")[..]
    else {
        panic!("text output must contain exactly one frame");
    };
    assert_eq!(flags, 0);

    let message =
        agent::AgentServerMessage::decode(payload.as_ref()).expect("decode server message");
    let Some(agent::agent_server_message::Message::InteractionUpdate(update)) = message.message
    else {
        panic!("expected interaction update");
    };
    assert!(matches!(
        update.message,
        Some(agent::interaction_update::Message::TextDelta(
            agent::TextDeltaUpdate { text, is_server_notice: false }
        )) if text == "hello output"
    ));
}

#[test]
fn cursor_thinking_usage_and_heartbeat_keep_their_wire_presence() {
    let events = [
        CursorOutputEvent::ThinkingDelta("reasoning output".into()),
        CursorOutputEvent::Heartbeat,
    ];

    for event in events {
        let EncodedCursorFrame::Data(frame) = encode_output_event(&event).expect("encode event")
        else {
            panic!("non-error output must be a data frame");
        };
        let [(flags, ref payload)] = decode_frames(&frame).expect("decode event frame")[..] else {
            panic!("each event must contain exactly one frame");
        };
        assert_eq!(flags, 0);
        let message =
            agent::AgentServerMessage::decode(payload.as_ref()).expect("decode event message");
        let Some(agent::agent_server_message::Message::InteractionUpdate(update)) = message.message
        else {
            panic!("expected interaction update");
        };
        match event {
            CursorOutputEvent::ThinkingDelta(text) => assert!(matches!(
                update.message,
                Some(agent::interaction_update::Message::ThinkingDelta(
                    agent::ThinkingDeltaUpdate { text: actual, thinking_style: Some(style) }
                )) if actual == text && style == agent::ThinkingStyle::Default as i32
            )),
            CursorOutputEvent::Heartbeat => assert!(matches!(
                update.message,
                Some(agent::interaction_update::Message::Heartbeat(
                    agent::HeartbeatUpdate {}
                ))
            )),
            _ => unreachable!(),
        }
    }
}

#[test]
fn cursor_usage_is_buffered_until_turn_end_and_then_closes_successfully() {
    let mut encoder = CursorOutputEncoder::default();
    let usage = CursorOutputEvent::Usage {
        input_tokens: 11,
        output_tokens: 7,
        cache_read_tokens: Some(3),
        cache_write_tokens: None,
        reasoning_tokens: Some(2),
    };
    assert!(encoder
        .encode_event(&usage)
        .expect("buffer usage")
        .is_empty());

    let frames = encoder
        .encode_event(&CursorOutputEvent::TurnEnded)
        .expect("encode turn end");
    assert_eq!(frames.len(), 2);
    let EncodedCursorFrame::Data(frame) = &frames[0] else {
        panic!("turn end must begin with a data frame");
    };
    let [(flags, ref payload)] = decode_frames(frame).expect("decode turn frame")[..] else {
        panic!("turn data must contain exactly one frame");
    };
    assert_eq!(flags, 0);
    let message = agent::AgentServerMessage::decode(payload.as_ref()).expect("decode turn message");
    let Some(agent::agent_server_message::Message::InteractionUpdate(update)) = message.message
    else {
        panic!("expected interaction update");
    };
    assert!(matches!(
        update.message,
        Some(agent::interaction_update::Message::TurnEnded(
            agent::TurnEndedUpdate {
                input_tokens: Some(11),
                output_tokens: Some(7),
                cache_read_tokens: Some(3),
                cache_write_tokens: None,
                reasoning_tokens: Some(2),
            }
        ))
    ));
    assert!(matches!(frames[1], EncodedCursorFrame::End(_)));
}

#[test]
fn cursor_usage_merges_partial_buckets_without_erasing_prior_values() {
    let mut encoder = CursorOutputEncoder::default();
    let initial = CursorOutputEvent::Usage {
        input_tokens: 11,
        output_tokens: 7,
        cache_read_tokens: Some(3),
        cache_write_tokens: Some(4),
        reasoning_tokens: Some(2),
    };
    let partial = CursorOutputEvent::Usage {
        input_tokens: 12,
        output_tokens: 8,
        cache_read_tokens: None,
        cache_write_tokens: Some(5),
        reasoning_tokens: None,
    };

    assert!(encoder
        .encode_event(&initial)
        .expect("buffer initial usage")
        .is_empty());
    assert!(encoder
        .encode_event(&partial)
        .expect("merge partial usage")
        .is_empty());

    let frames = encoder
        .encode_event(&CursorOutputEvent::TurnEnded)
        .expect("encode merged turn end");
    let EncodedCursorFrame::Data(frame) = &frames[0] else {
        panic!("turn end must begin with a data frame");
    };
    let [(flags, ref payload)] = decode_frames(frame).expect("decode merged turn frame")[..] else {
        panic!("turn data must contain exactly one frame");
    };
    assert_eq!(flags, 0);
    let message = agent::AgentServerMessage::decode(payload.as_ref()).expect("decode turn message");
    let Some(agent::agent_server_message::Message::InteractionUpdate(update)) = message.message
    else {
        panic!("expected interaction update");
    };
    assert!(matches!(
        update.message,
        Some(agent::interaction_update::Message::TurnEnded(
            agent::TurnEndedUpdate {
                input_tokens: Some(12),
                output_tokens: Some(8),
                cache_read_tokens: Some(3),
                cache_write_tokens: Some(5),
                reasoning_tokens: Some(2),
            }
        ))
    ));
}

#[test]
fn cursor_tool_start_uses_a_compatibility_placeholder_without_side_effects() {
    let frame = encode_output_event(&CursorOutputEvent::ToolCallStart {
        call_id: "call-output-1".into(),
        name: "shell".into(),
    })
    .expect("encode tool start");
    let EncodedCursorFrame::Data(frame) = frame else {
        panic!("tool start must be a data frame");
    };
    let [(flags, ref payload)] = decode_frames(&frame).expect("decode tool frame")[..] else {
        panic!("tool start must contain exactly one frame");
    };
    assert_eq!(flags, 0);
    let message = agent::AgentServerMessage::decode(payload.as_ref()).expect("decode tool message");
    let Some(agent::agent_server_message::Message::InteractionUpdate(update)) = message.message
    else {
        panic!("expected interaction update");
    };
    let Some(agent::interaction_update::Message::PartialToolCall(partial)) = update.message else {
        panic!("expected partial tool call");
    };
    assert_eq!(partial.call_id, "call-output-1");
    assert!(partial.args_text_delta.is_empty());
    let Some(tool) = partial.tool_call else {
        panic!("compatibility placeholder is required");
    };
    assert_eq!(tool.tool_call_id.as_deref(), Some("call-output-1"));
    assert!(matches!(
        tool.tool,
        Some(agent::tool_call::Tool::McpToolCall(agent::McpToolCall {
            args: Some(agent::McpArgs {
                name,
                tool_call_id,
                provider_identifier,
                tool_name,
                server_identifier,
            }),
            description: Some(description),
            ..
        })) if name == "shell"
            && tool_call_id == "call-output-1"
            && provider_identifier == "cc2cx-compat"
            && tool_name == "shell"
            && server_identifier == "cc2cx-compat"
            && description == "shell"
    ));
}

#[test]
fn cursor_error_output_is_a_structured_terminal_frame() {
    let frame = encode_output_event(&CursorOutputEvent::Error {
        code: "upstream_unavailable".into(),
        message: "provider unavailable".into(),
    })
    .expect("encode error output");
    let EncodedCursorFrame::End(frame) = frame else {
        panic!("error output must terminate the stream");
    };
    let [(flags, ref payload)] = decode_frames(&frame).expect("decode error frame")[..] else {
        panic!("error output must contain exactly one frame");
    };
    assert_eq!(flags, 0x02);
    let json: serde_json::Value = serde_json::from_slice(payload).expect("terminal JSON");
    assert_eq!(json["error"]["code"], "unavailable");
    assert_eq!(json["error"]["message"], "provider unavailable");
}

#[test]
fn unsupported_output_is_a_structured_invalid_argument_terminal_frame() {
    let frame = encode_output_event(&CursorOutputEvent::Unsupported {
        feature: UnsupportedFeature::ToolArgumentsDelta,
    })
    .expect("encode unsupported output");
    let EncodedCursorFrame::End(frame) = frame else {
        panic!("unsupported output must terminate the stream");
    };
    let [(flags, ref payload)] = decode_frames(&frame).expect("decode unsupported frame")[..]
    else {
        panic!("unsupported output must contain exactly one frame");
    };
    assert_eq!(flags, 0x02);
    let json: serde_json::Value = serde_json::from_slice(payload).expect("terminal JSON");
    assert_eq!(json["error"]["code"], "invalid_argument");
    assert!(json["error"]["message"]
        .as_str()
        .expect("error message")
        .contains("ToolArgumentsDelta"));
}

#[test]
fn normal_cursor_completion_uses_the_empty_success_terminal_frame() {
    assert_eq!(
        encode_end_stream(),
        Bytes::from_static(&[2, 0, 0, 0, 2, b'{', b'}'])
    );
}

#[test]
fn output_encoder_finishes_with_turn_end_before_success_terminal() {
    let mut encoder = CursorOutputEncoder::default();
    let frames = encoder.finish().expect("finish output stream");

    assert_eq!(frames.len(), 2);
    let EncodedCursorFrame::Data(frame) = &frames[0] else {
        panic!("finish must publish a turn-ended data frame first");
    };
    let [(flags, ref payload)] = decode_frames(frame).expect("decode finish data")[..] else {
        panic!("finish data must contain one frame");
    };
    assert_eq!(flags, 0);
    let message =
        agent::AgentServerMessage::decode(payload.as_ref()).expect("decode finish message");
    let Some(agent::agent_server_message::Message::InteractionUpdate(update)) = message.message
    else {
        panic!("finish must publish interaction update");
    };
    assert!(matches!(
        update.message,
        Some(agent::interaction_update::Message::TurnEnded(
            agent::TurnEndedUpdate {
                input_tokens: None,
                output_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
            }
        ))
    ));
    assert!(matches!(frames[1], EncodedCursorFrame::End(_)));
}

#[test]
fn cancelled_provider_output_uses_the_connect_canceled_code() {
    let frame = encode_output_event(&CursorOutputEvent::Error {
        code: "cancelled".into(),
        message: "Cursor request cancelled".into(),
    })
    .expect("encode cancellation output");
    let EncodedCursorFrame::End(frame) = frame else {
        panic!("cancellation must terminate the stream");
    };
    let [(flags, ref payload)] = decode_frames(&frame).expect("decode cancellation frame")[..]
    else {
        panic!("cancellation must contain one frame");
    };
    assert_eq!(flags, 0x02);
    let json: serde_json::Value = serde_json::from_slice(payload).expect("terminal JSON");
    assert_eq!(json["error"]["code"], "canceled");
}
