//! Cursor host/path routing policy.
//!
//! This module deliberately contains no proxy implementation. Keeping the policy pure makes
//! it possible to test that unrelated traffic is never routed into the Cursor adapter.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnmatchedPathPolicy {
    Passthrough,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDecision {
    Local,
    Passthrough,
    Reject,
}

const LOCAL_PATHS: &[&str] = &[
    "/agent.v1.AgentService/Run",
    "/agent.v1.AgentService/RunSSE",
    "/agent.v1.AgentService/GetNewChatNudgeParameterizedModelPicker",
    "/aiserver.v1.BidiService/BidiAppend",
    "/aiserver.v1.NetworkService/IsConnected",
    "/aiserver.v1.AiService/AvailableModels",
    "/agent.v1.AgentService/GetUsableModels",
    "/aiserver.v1.AiService/GetUsableModels",
    "/aiserver.v1.AiService/GetDefaultModel",
    "/agent.v1.AgentService/GetDefaultModelForCli",
    "/aiserver.v1.AiService/GetDefaultModelForCli",
    "/aiserver.v1.ServerConfigService/GetServerConfig",
    "/aiserver.v1.AiService/GetServerConfig",
];

pub fn is_cursor_host(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    matches!(host.as_str(), "api2.cursor.sh" | "api3.cursor.sh") || host.ends_with(".cursor.sh")
}

pub fn is_local_path(path: &str) -> bool {
    let path = path.split(['?', '#']).next().unwrap_or(path);
    LOCAL_PATHS.contains(&path)
}

pub fn classify_request(host: &str, path: &str, unmatched: UnmatchedPathPolicy) -> RouteDecision {
    if !is_cursor_host(host) {
        return RouteDecision::Passthrough;
    }
    if is_local_path(path) {
        RouteDecision::Local
    } else {
        match unmatched {
            UnmatchedPathPolicy::Passthrough => RouteDecision::Passthrough,
            UnmatchedPathPolicy::Reject => RouteDecision::Reject,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_matching_ignores_query_and_fragment() {
        assert!(is_local_path("/agent.v1.AgentService/RunSSE?x=1#fragment"));
    }
}
