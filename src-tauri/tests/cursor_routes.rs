use cc_launch_lib::cursor::routes::{
    classify_request, is_cursor_host, RouteDecision, UnmatchedPathPolicy,
};

#[test]
fn cursor_hosts_are_case_insensitive_and_dot_tolerant() {
    assert!(is_cursor_host("API2.CURSOR.SH"));
    assert!(is_cursor_host("edge.example.cursor.sh."));
    assert!(!is_cursor_host("cursor.sh"));
    assert!(!is_cursor_host("api2.cursor.sh.attacker.example"));
}

#[test]
fn only_allowlisted_cursor_paths_are_local() {
    assert_eq!(
        classify_request(
            "api2.cursor.sh",
            "/aiserver.v1.BidiService/BidiAppend?x=1",
            UnmatchedPathPolicy::Passthrough,
        ),
        RouteDecision::Local
    );
    assert_eq!(
        classify_request(
            "api2.cursor.sh",
            "/agent.v1.AgentService/RunSSE",
            UnmatchedPathPolicy::Passthrough,
        ),
        RouteDecision::Local
    );
    assert_eq!(
        classify_request(
            "api2.cursor.sh",
            "/agent.v1.AgentService/Run",
            UnmatchedPathPolicy::Passthrough,
        ),
        RouteDecision::Local
    );
    assert_eq!(
        classify_request(
            "api2.cursor.sh",
            "/aiserver.v1.NetworkService/IsConnected",
            UnmatchedPathPolicy::Passthrough,
        ),
        RouteDecision::Local
    );
    assert_eq!(
        classify_request(
            "api2.cursor.sh",
            "/aiserver.v1.AiService/AvailableModels",
            UnmatchedPathPolicy::Passthrough,
        ),
        RouteDecision::Local
    );
    assert_eq!(
        classify_request(
            "api2.cursor.sh",
            "/agent.v1.AgentService/GetUsableModels",
            UnmatchedPathPolicy::Passthrough,
        ),
        RouteDecision::Local
    );
    for path in [
        "/aiserver.v1.AiService/GetDefaultModel",
        "/agent.v1.AgentService/GetDefaultModelForCli",
        "/aiserver.v1.AiService/GetDefaultModelForCli",
    ] {
        assert_eq!(
            classify_request("api2.cursor.sh", path, UnmatchedPathPolicy::Passthrough),
            RouteDecision::Local,
            "{path} must use the local model control plane"
        );
    }
    assert_eq!(
        classify_request(
            "agentn.global.api5.cursor.sh",
            "/agent.v1.AgentService/GetNewChatNudgeParameterizedModelPicker",
            UnmatchedPathPolicy::Reject,
        ),
        RouteDecision::Local
    );
    assert_eq!(
        classify_request(
            "api2.cursor.sh",
            "/unknown",
            UnmatchedPathPolicy::Passthrough,
        ),
        RouteDecision::Passthrough
    );
    assert_eq!(
        classify_request("api2.cursor.sh", "/unknown", UnmatchedPathPolicy::Reject,),
        RouteDecision::Reject
    );
}

#[test]
fn non_cursor_hosts_are_never_local() {
    assert_eq!(
        classify_request(
            "api.example.com",
            "/aiserver.v1.BidiService/BidiAppend",
            UnmatchedPathPolicy::Reject,
        ),
        RouteDecision::Passthrough
    );
}

#[test]
fn legacy_stream_methods_remain_upstream_until_an_adapter_exists() {
    for path in [
        "/aiserver.v1.AiService/StreamChat",
        "/aiserver.v1.AiService/StreamEdit",
        "/aiserver.v1.AiService/StreamReview",
    ] {
        assert_eq!(
            classify_request("api2.cursor.sh", path, UnmatchedPathPolicy::Passthrough),
            RouteDecision::Passthrough,
            "{path} must not enter the incomplete local adapter"
        );
    }
}
