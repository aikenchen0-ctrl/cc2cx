use cc_launch_lib::cursor::{
    adapter::{
        run_provider_stream, CursorAction, CursorField, CursorOutputEvent, CursorRequest,
        FieldDisposition, ProviderEvent, ProviderInvocation, ProviderStreamError,
        ProviderStreamOutcome, UnsupportedFeature,
    },
    protocol::{
        bidi::decode_append,
        proto::{agent::v1 as agent, aiserver::v1 as ai},
        run_sse::decode_response_body,
    },
    transport::TransportRegistry,
};
use futures::{stream, StreamExt};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn append_with_run_request(
    request_id: &str,
    seqno: i64,
    model_id: &str,
    conversation_id: &str,
    text: &str,
) -> ai::BidiAppendRequest {
    use prost::Message;

    let message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: model_id.into(),
                    ..Default::default()
                }),
                conversation_id: Some(conversation_id.into()),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: text.into(),
                                message_id: "message-fixture-1".into(),
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

    ai::BidiAppendRequest {
        request_id: Some(ai::BidiRequestId {
            request_id: request_id.into(),
        }),
        append_seqno: seqno,
        data: hex::encode(message.encode_to_vec()),
        ..Default::default()
    }
}

#[test]
fn cursor_request_preserves_protocol_identity_and_user_message() {
    let decoded = decode_append(&append_with_run_request(
        "request-fixture-1",
        7,
        "model-fixture-1",
        "conversation-fixture-1",
        "hello fixture",
    ))
    .expect("fixture must decode");

    let request = CursorRequest::from_decoded_append(decoded).expect("request contract");

    assert_eq!(request.request_id, "request-fixture-1");
    assert_eq!(request.append_seqno, 7);
    assert_eq!(request.model_id.as_deref(), Some("model-fixture-1"));
    assert_eq!(
        request.conversation_id.as_deref(),
        Some("conversation-fixture-1")
    );
    assert_eq!(request.user_message.as_deref(), Some("hello fixture"));
    assert_eq!(
        request.user_message_id.as_deref(),
        Some("message-fixture-1")
    );
    assert!(matches!(request.action, CursorAction::Run));
    assert!(
        request.field_dispositions.is_empty(),
        "a minimal text-only request must not report absent optional fields"
    );
}

#[test]
fn cursor_request_reports_only_present_unsupported_context() {
    use prost::Message;

    let message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-fixture-context".into(),
                    ..Default::default()
                }),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "hello context".into(),
                                message_id: "message-fixture-context".into(),
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
    let decoded = decode_append(&ai::BidiAppendRequest {
        request_id: Some(ai::BidiRequestId {
            request_id: "request-fixture-context".into(),
        }),
        data: hex::encode(message.encode_to_vec()),
        ..Default::default()
    })
    .expect("context fixture must decode");

    let request = CursorRequest::from_decoded_append(decoded).expect("request contract");
    assert!(
        request.field_dispositions.is_empty(),
        "fields not represented in the validated wire subset must not be guessed"
    );
}

#[test]
fn cursor_request_marks_present_history_images_mcp_and_reasoning_as_unimplemented() {
    use prost::Message;

    let message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-fixture-context".into(),
                    max_mode: true,
                    parameters: vec![agent::ModelParameterValue {
                        id: "reasoning_effort".into(),
                        value: "high".into(),
                    }],
                }),
                mcp_tools: Some(agent::McpTools {
                    mcp_tools: vec![agent::McpToolDefinition {
                        name: "fixture_tool".into(),
                        ..Default::default()
                    }],
                }),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "hello context".into(),
                                message_id: "message-fixture-context".into(),
                                selected_context: Some(agent::SelectedContext {
                                    selected_images: vec![agent::SelectedImage {
                                        uuid: "image-fixture".into(),
                                        mime_type: "image/png".into(),
                                        data: b"fixture-image".to_vec(),
                                        ..Default::default()
                                    }],
                                }),
                            }),
                            conversation_history: Some(agent::ConversationHistory {
                                messages: vec![agent::ConversationHistoryMessage {
                                    message: Some(
                                        agent::conversation_history_message::Message::User(
                                            agent::ConversationHistoryUserMessage {
                                                content: vec![agent::ConversationHistoryUserContent {
                                                    content: Some(
                                                        agent::conversation_history_user_content::Content::Text(
                                                            agent::ConversationHistoryTextContent {
                                                                text: "old fixture".into(),
                                                            },
                                                        ),
                                                    ),
                                                }],
                                            },
                                        ),
                                    ),
                                }],
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };
    let decoded = decode_append(&ai::BidiAppendRequest {
        request_id: Some(ai::BidiRequestId {
            request_id: "request-fixture-context".into(),
        }),
        data: hex::encode(message.encode_to_vec()),
        ..Default::default()
    })
    .expect("context fixture must decode");

    let request = CursorRequest::from_decoded_append(decoded).expect("request contract");
    assert_eq!(
        request.field_dispositions,
        vec![
            FieldDisposition::Dropped(CursorField::History),
            FieldDisposition::Unsupported(CursorField::Images),
            FieldDisposition::Unsupported(CursorField::McpTools),
            FieldDisposition::Dropped(CursorField::ReasoningEffort),
        ]
    );
}

#[test]
fn unsupported_cursor_actions_are_explicit() {
    let decoded = cc_launch_lib::cursor::protocol::bidi::DecodedAppend {
        request_id: "request-fixture-2".into(),
        seqno: 2,
        message: agent::AgentClientMessage {
            message: Some(agent::agent_client_message::Message::ExecClientMessage(
                agent::EmptyMessage {},
            )),
        },
    };

    let error = CursorRequest::from_decoded_append(decoded)
        .expect_err("exec action must not be silently treated as a user message");

    assert!(error.unsupported.contains(&UnsupportedFeature::Exec));
}

#[test]
fn provider_invocation_contains_only_normalized_request_fields() {
    let invocation = ProviderInvocation::from_request(CursorRequest {
        request_id: "request-fixture-3".into(),
        append_seqno: 3,
        conversation_id: Some("conversation-fixture-3".into()),
        model_id: Some("model-fixture-3".into()),
        user_message: Some("hello fixture".into()),
        user_message_id: Some("message-fixture-3".into()),
        action: CursorAction::Run,
        field_dispositions: vec![FieldDisposition::Dropped(CursorField::History)],
    })
    .expect("provider invocation");

    assert_eq!(invocation.request_id, "request-fixture-3");
    assert_eq!(invocation.model_id, "model-fixture-3");
    assert_eq!(
        invocation.conversation_id.as_deref(),
        Some("conversation-fixture-3")
    );
    assert_eq!(invocation.user_message.as_deref(), Some("hello fixture"));
    assert_eq!(
        invocation.user_message_id.as_deref(),
        Some("message-fixture-3")
    );
    assert_eq!(
        invocation.field_dispositions,
        vec![FieldDisposition::Dropped(CursorField::History)]
    );
}

#[test]
fn provider_invocation_rejects_missing_model_instead_of_guessing() {
    let request = CursorRequest {
        request_id: "request-fixture-4".into(),
        append_seqno: 4,
        conversation_id: None,
        model_id: None,
        user_message: Some("hello fixture".into()),
        user_message_id: Some("message-fixture-4".into()),
        action: CursorAction::Run,
        field_dispositions: Vec::new(),
    };

    let error = ProviderInvocation::from_request(request)
        .expect_err("provider model must be explicit in P2-0");
    assert_eq!(error.reason, "Cursor request model_id is required");
}

#[test]
fn provider_events_map_to_cursor_output_events_without_silent_loss() {
    assert_eq!(
        CursorOutputEvent::from_provider(&ProviderEvent::TextDelta("hello".into())),
        Some(CursorOutputEvent::TextDelta("hello".into()))
    );
    assert_eq!(
        CursorOutputEvent::from_provider(&ProviderEvent::ThinkingDelta("reason".into())),
        Some(CursorOutputEvent::ThinkingDelta("reason".into()))
    );
    assert_eq!(
        CursorOutputEvent::from_provider(&ProviderEvent::ToolCallStart {
            call_id: "call-fixture-1".into(),
            name: "tool_fixture".into(),
        }),
        Some(CursorOutputEvent::ToolCallStart {
            call_id: "call-fixture-1".into(),
            name: "tool_fixture".into(),
        })
    );
    assert_eq!(
        CursorOutputEvent::from_provider(&ProviderEvent::ToolCallArgumentsDelta {
            call_id: "call-fixture-1".into(),
            delta: "{}".into(),
        }),
        Some(CursorOutputEvent::Unsupported {
            feature: UnsupportedFeature::ToolArgumentsDelta,
        })
    );
    assert_eq!(
        CursorOutputEvent::from_provider(&ProviderEvent::Done),
        Some(CursorOutputEvent::TurnEnded)
    );
    assert_eq!(
        CursorOutputEvent::from_provider(&ProviderEvent::Heartbeat),
        Some(CursorOutputEvent::Heartbeat)
    );
    assert_eq!(
        CursorOutputEvent::from_provider(&ProviderEvent::Usage {
            input_tokens: 11,
            output_tokens: 7,
            cache_read_tokens: Some(3),
            cache_write_tokens: None,
            reasoning_tokens: Some(2),
        }),
        Some(CursorOutputEvent::Usage {
            input_tokens: 11,
            output_tokens: 7,
            cache_read_tokens: Some(3),
            cache_write_tokens: None,
            reasoning_tokens: Some(2),
        })
    );
    assert_eq!(
        CursorOutputEvent::from_provider(&ProviderEvent::Error {
            code: "upstream_unavailable".into(),
            message: "synthetic provider error".into(),
        }),
        Some(CursorOutputEvent::Error {
            code: "upstream_unavailable".into(),
            message: "synthetic provider error".into(),
        })
    );
}

fn invocation() -> ProviderInvocation {
    ProviderInvocation::from_request(CursorRequest {
        request_id: "request-fixture-stream".into(),
        append_seqno: 1,
        conversation_id: Some("conversation-fixture-stream".into()),
        model_id: Some("model-fixture-stream".into()),
        user_message: Some("hello stream".into()),
        user_message_id: Some("message-fixture-stream".into()),
        action: CursorAction::Run,
        field_dispositions: Vec::new(),
    })
    .expect("stream invocation")
}

#[tokio::test]
async fn synthetic_provider_emits_single_turn_stream() {
    let cancellation = CancellationToken::new();
    let events: Vec<_> =
        cc_launch_lib::cursor::adapter::SyntheticProvider::stream(invocation(), cancellation)
            .collect()
            .await;

    assert_eq!(
        events,
        vec![
            ProviderEvent::TextDelta("synthetic response".into()),
            ProviderEvent::Usage {
                input_tokens: 2,
                output_tokens: 2,
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
            },
            ProviderEvent::Done,
        ]
    );
}

#[tokio::test]
async fn synthetic_provider_honors_cancellation_before_emitting_events() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let events: Vec<_> =
        cc_launch_lib::cursor::adapter::SyntheticProvider::stream(invocation(), cancellation)
            .collect()
            .await;

    assert_eq!(
        events,
        vec![ProviderEvent::Error {
            code: "cancelled".into(),
            message: "Cursor request cancelled".into(),
        }]
    );
}

#[tokio::test]
async fn provider_stream_without_done_emits_a_truncated_terminal_error() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create("request-fixture-truncated")
        .expect("transport");
    let mut receiver = transport.subscribe();
    let provider = Box::pin(stream::iter(vec![Ok(ProviderEvent::TextDelta(
        "partial output".into(),
    ))]));

    let outcome = run_provider_stream(transport, provider, Duration::from_secs(1))
        .await
        .expect("truncated stream should be encoded, not crash the bridge");
    assert_eq!(outcome, ProviderStreamOutcome::Truncated);

    let mut encoded = Vec::new();
    while let Some(frame) = receiver.recv().await {
        encoded.extend_from_slice(&frame);
    }
    let frames = decode_response_body(&encoded).expect("terminal error frame");
    assert_eq!(frames.len(), 2);
    let terminal = frames.last().expect("terminal frame");
    let cc_launch_lib::cursor::protocol::run_sse::RunSseFrame::End { error: Some(error) } =
        terminal
    else {
        panic!("truncated provider stream must end with an error");
    };
    assert_eq!(error.code.as_deref(), Some("unavailable"));
    assert!(error
        .message
        .as_deref()
        .is_some_and(|message| message.contains("without a terminal Done event")));
}

#[tokio::test]
async fn provider_stream_error_emits_structured_terminal_error() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create("request-fixture-provider-error")
        .expect("transport");
    let mut receiver = transport.subscribe();
    let provider = Box::pin(stream::iter(vec![Err(ProviderStreamError {
        code: "upstream_unavailable".into(),
        message: "synthetic provider failure".into(),
    })]));

    let outcome = run_provider_stream(transport, provider, Duration::from_secs(1))
        .await
        .expect("provider error should be encoded");
    assert_eq!(outcome, ProviderStreamOutcome::Failed);

    let mut encoded = Vec::new();
    while let Some(frame) = receiver.recv().await {
        encoded.extend_from_slice(&frame);
    }
    let frames = decode_response_body(&encoded).expect("provider error frame");
    assert_eq!(frames.len(), 1);
    let terminal = frames.first().expect("terminal frame");
    let cc_launch_lib::cursor::protocol::run_sse::RunSseFrame::End { error: Some(error) } =
        terminal
    else {
        panic!("provider error must terminate the stream");
    };
    assert_eq!(error.code.as_deref(), Some("unavailable"));
    assert_eq!(error.message.as_deref(), Some("synthetic provider failure"));
}

#[tokio::test]
async fn provider_stream_done_emits_success_terminal_and_completed_outcome() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create("request-fixture-complete")
        .expect("transport");
    let mut receiver = transport.subscribe();
    let provider = Box::pin(stream::iter(vec![
        Ok(ProviderEvent::TextDelta("complete output".into())),
        Ok(ProviderEvent::Usage {
            input_tokens: 3,
            output_tokens: 2,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
        }),
        Ok(ProviderEvent::Done),
    ]));

    let outcome = run_provider_stream(transport, provider, Duration::from_secs(1))
        .await
        .expect("completed provider stream should be encoded");
    assert_eq!(outcome, ProviderStreamOutcome::Completed);

    let mut encoded = Vec::new();
    while let Some(frame) = receiver.recv().await {
        encoded.extend_from_slice(&frame);
    }
    let frames = decode_response_body(&encoded).expect("completed stream frames");
    assert_eq!(frames.len(), 3);
    assert!(matches!(
        frames.last(),
        Some(cc_launch_lib::cursor::protocol::run_sse::RunSseFrame::End { error: None })
    ));
}

#[tokio::test]
async fn provider_error_event_emits_structured_terminal_error() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create("request-fixture-provider-error-event")
        .expect("transport");
    let mut receiver = transport.subscribe();
    let provider = Box::pin(stream::iter(vec![Ok(ProviderEvent::Error {
        code: "provider_unavailable".into(),
        message: "synthetic provider event failure".into(),
    })]));

    let outcome = run_provider_stream(transport, provider, Duration::from_secs(1))
        .await
        .expect("provider error event should be encoded");
    assert_eq!(outcome, ProviderStreamOutcome::Failed);

    let mut encoded = Vec::new();
    while let Some(frame) = receiver.recv().await {
        encoded.extend_from_slice(&frame);
    }
    let frames = decode_response_body(&encoded).expect("provider error event frame");
    assert_eq!(frames.len(), 1);
    let Some(cc_launch_lib::cursor::protocol::run_sse::RunSseFrame::End { error: Some(error) }) =
        frames.first()
    else {
        panic!("provider error event must terminate the stream");
    };
    assert_eq!(error.code.as_deref(), Some("unavailable"));
    assert_eq!(
        error.message.as_deref(),
        Some("synthetic provider event failure")
    );
}

#[tokio::test]
async fn idle_provider_stream_emits_a_bounded_timeout_error() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create("request-fixture-idle")
        .expect("transport");
    let mut receiver = transport.subscribe();
    let provider = Box::pin(stream::pending::<Result<ProviderEvent, ProviderStreamError>>());

    let outcome = run_provider_stream(transport, provider, Duration::from_millis(10))
        .await
        .expect("idle timeout should be encoded");
    assert_eq!(outcome, ProviderStreamOutcome::IdleTimeout);

    let mut encoded = Vec::new();
    while let Some(frame) = receiver.recv().await {
        encoded.extend_from_slice(&frame);
    }
    let frames = decode_response_body(&encoded).expect("idle timeout frame");
    let terminal = frames.first().expect("terminal frame");
    let cc_launch_lib::cursor::protocol::run_sse::RunSseFrame::End { error: Some(error) } =
        terminal
    else {
        panic!("idle timeout must terminate the stream");
    };
    assert_eq!(error.code.as_deref(), Some("unavailable"));
    assert!(error
        .message
        .as_deref()
        .is_some_and(|message| message.contains("idle timeout")));
}

#[tokio::test]
async fn client_disconnect_cancels_provider_stream_without_emitting_after_close() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create("request-fixture-disconnect")
        .expect("transport");
    let mut receiver = transport.subscribe();
    let provider = Box::pin(stream::pending::<Result<ProviderEvent, ProviderStreamError>>());
    let task = tokio::spawn(run_provider_stream(
        transport.clone(),
        provider,
        Duration::ZERO,
    ));

    transport.disconnect();
    let outcome = tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("disconnect must stop the provider bridge")
        .expect("provider bridge task must not panic")
        .expect("disconnect should not be a protocol failure");
    assert_eq!(outcome, ProviderStreamOutcome::Cancelled);
    assert!(transport.cancellation_token().is_cancelled());
    assert!(receiver.recv().await.is_none());
}
