use bytes::Bytes;
use cc_launch_lib::cursor::{
    adapter::{CursorAction, CursorRequest, ProviderInvocation},
    protocol::connect::{decode_frames, END_STREAM_FLAG},
    transport::{TransportError, TransportRegistry},
};
use prost::Message;

fn invocation() -> ProviderInvocation {
    ProviderInvocation::from_request(CursorRequest {
        request_id: "request-runtime".into(),
        append_seqno: 0,
        conversation_id: Some("conversation-runtime".into()),
        model_id: Some("model-runtime".into()),
        user_message: Some("hello runtime".into()),
        user_message_id: Some("message-runtime".into()),
        action: CursorAction::Run,
        field_dispositions: Vec::new(),
    })
    .expect("runtime invocation")
}

#[test]
fn request_id_stays_bound_to_one_transport_until_replaced() {
    let registry = TransportRegistry::new();

    let first = registry
        .get_or_create("request-sticky")
        .expect("create transport");
    let second = registry
        .get_or_create("request-sticky")
        .expect("reuse transport");

    assert_eq!(first.request_id(), "request-sticky");
    assert_eq!(first.generation(), second.generation());
    assert_eq!(
        registry.current("request-sticky").unwrap().generation(),
        first.generation()
    );
}

#[test]
fn append_sequence_releases_buffered_payloads_in_order() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create_for_append("request-seq")
        .expect("create transport");

    assert!(transport
        .append(2, Bytes::from_static(b"two"))
        .expect("future append is buffered")
        .is_empty());
    assert_eq!(
        transport
            .append(0, Bytes::from_static(b"zero"))
            .expect("initial append is ready"),
        vec![(0, Bytes::from_static(b"zero"))]
    );
    assert_eq!(
        transport
            .append(1, Bytes::from_static(b"one"))
            .expect("second append releases buffered payload"),
        vec![
            (1, Bytes::from_static(b"one")),
            (2, Bytes::from_static(b"two"))
        ]
    );
    let duplicate = transport
        .append(1, Bytes::from_static(b"duplicate"))
        .expect_err("duplicate append must be rejected");
    assert_eq!(
        duplicate,
        TransportError::Sequence {
            expected: 3,
            received: 1,
        }
    );
}

#[test]
fn subscription_replays_history_then_receives_new_frames_without_a_gap() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create("request-replay")
        .expect("create transport");
    transport.emit_frame(Bytes::from_static(b"history"));

    let mut receiver = transport.subscribe();
    transport.emit_frame(Bytes::from_static(b"live"));

    assert_eq!(
        receiver.blocking_recv(),
        Some(Bytes::from_static(b"history"))
    );
    assert_eq!(receiver.blocking_recv(), Some(Bytes::from_static(b"live")));
}

#[test]
fn terminal_frame_is_replayed_and_closes_output() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create("request-terminal")
        .expect("create transport");
    let mut receiver = transport.subscribe();

    assert!(transport.finish(Bytes::from_static(b"terminal")));
    assert!(!transport.emit_frame(Bytes::from_static(b"after-terminal")));
    assert_eq!(
        receiver.blocking_recv(),
        Some(Bytes::from_static(b"terminal"))
    );
    assert_eq!(receiver.blocking_recv(), None);
    assert!(transport.is_terminal());

    let mut late_receiver = transport.subscribe();
    assert_eq!(
        late_receiver.blocking_recv(),
        Some(Bytes::from_static(b"terminal"))
    );
    assert_eq!(late_receiver.blocking_recv(), None);
    assert_eq!(
        transport
            .append(0, Bytes::from_static(b"after-terminal"))
            .expect_err("terminal transport must reject append"),
        TransportError::Closed
    );
}

#[test]
fn disconnect_cancels_the_request_and_closes_subscribers() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create("request-cancel")
        .expect("create transport");
    let cancellation = transport.cancellation_token();
    let mut receiver = transport.subscribe();

    transport.disconnect();

    assert!(cancellation.is_cancelled());
    assert!(transport.is_disconnected());
    assert_eq!(receiver.blocking_recv(), None);
    assert_eq!(
        transport
            .append(0, Bytes::from_static(b"after-disconnect"))
            .expect_err("disconnected transport must reject append"),
        TransportError::Closed
    );
    assert!(!transport.finish(Bytes::from_static(b"late-terminal")));
}

#[test]
fn stale_generation_cleanup_cannot_remove_a_replacement_transport() {
    let registry = TransportRegistry::new();
    let old = registry
        .get_or_create_for_append("request-generation")
        .expect("create first generation");
    let old_generation = old.generation();
    old.disconnect();

    let replacement = registry
        .get_or_create_for_append("request-generation")
        .expect("create replacement generation");
    assert_ne!(replacement.generation(), old_generation);

    assert!(!registry.remove_if_current("request-generation", old_generation));
    assert_eq!(
        registry
            .current("request-generation")
            .expect("replacement remains registered")
            .generation(),
        replacement.generation()
    );
    replacement.disconnect();
    assert!(registry.remove_if_current("request-generation", replacement.generation()));
    assert!(registry.current("request-generation").is_none());
}

#[test]
fn append_sequence_rejects_negative_numbers() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create_for_append("request-negative")
        .expect("create transport");

    assert_eq!(
        transport
            .append(-1, Bytes::from_static(b"negative"))
            .expect_err("negative append must be rejected"),
        TransportError::Sequence {
            expected: 0,
            received: -1,
        }
    );
}

#[test]
fn append_sequence_rejects_i64_max_before_it_can_wrap() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create_for_append("request-overflow")
        .expect("create transport");

    assert_eq!(
        transport
            .append(i64::MAX, Bytes::from_static(b"overflow"))
            .expect_err("maximum sequence must not be accepted"),
        TransportError::SequenceExhausted
    );
}

#[test]
fn registry_canonicalizes_and_limits_request_ids() {
    let registry = TransportRegistry::new();
    let trimmed = registry
        .get_or_create("  request-canonical  ")
        .expect("surrounding whitespace is canonicalized");
    assert_eq!(trimmed.request_id(), "request-canonical");
    assert_eq!(
        registry
            .get_or_create("request-canonical")
            .expect("canonical ID reuses transport")
            .generation(),
        trimmed.generation()
    );

    let too_long = "x".repeat(257);
    assert_eq!(
        registry.get_or_create(&too_long).unwrap_err(),
        TransportError::InvalidRequestId
    );
}

#[test]
fn live_transport_cannot_be_removed_by_generation_cleanup() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create("request-live")
        .expect("create transport");

    assert!(!registry.remove_if_current("request-live", transport.generation()));
    assert!(registry.current("request-live").is_some());
}

#[tokio::test]
async fn synthetic_provider_stream_is_published_as_replayable_cursor_frames() {
    let registry = TransportRegistry::new();
    let transport = registry
        .get_or_create_for_append("request-runtime")
        .expect("create runtime transport");
    let mut receiver = transport.subscribe();

    cc_launch_lib::cursor::adapter::run_synthetic_stream(transport.clone(), invocation())
        .await
        .expect("synthetic stream should complete");

    let text_frame = receiver.recv().await.expect("text frame");
    let text_payload = decode_frames(&text_frame)
        .expect("decode text frame")
        .pop()
        .expect("one text frame")
        .1;
    let text_message =
        cc_launch_lib::cursor::protocol::proto::agent::v1::AgentServerMessage::decode(text_payload)
            .expect("decode text message");
    assert!(matches!(
        text_message.message,
        Some(
            cc_launch_lib::cursor::protocol::proto::agent::v1::agent_server_message::Message::InteractionUpdate(
                cc_launch_lib::cursor::protocol::proto::agent::v1::InteractionUpdate {
                    message: Some(
                        cc_launch_lib::cursor::protocol::proto::agent::v1::interaction_update::Message::TextDelta(_)
                    )
                }
            )
        )
    ));

    let turn_end_frame = receiver.recv().await.expect("turn ended frame");
    assert_eq!(
        decode_frames(&turn_end_frame)
            .expect("decode turn ended")
            .len(),
        1
    );

    let terminal_frame = receiver.recv().await.expect("terminal frame");
    assert_eq!(terminal_frame[0] & END_STREAM_FLAG, END_STREAM_FLAG);
    assert_eq!(receiver.recv().await, None);
    assert!(transport.is_terminal());

    let mut replay = transport.subscribe();
    assert!(replay.recv().await.is_some());
    assert!(replay.recv().await.is_some());
    assert!(replay.recv().await.is_some());
    assert_eq!(replay.recv().await, None);
}
