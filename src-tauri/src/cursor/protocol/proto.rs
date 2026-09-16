//! Minimal protobuf wire subset copied from the validated Cursor schema.

pub mod aiserver {
    pub mod v1 {
        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct IsConnectedResponse {}

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct GetServerConfigResponse {
            #[prost(string, tag = "6")]
            pub config_version: String,
            #[prost(int32, tag = "7")]
            pub http2_config: i32,
            #[prost(bool, optional, tag = "28")]
            pub cli_sandbox_default_enabled: Option<bool>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct GetDefaultModelResponse {
            #[prost(string, tag = "1")]
            pub model: String,
            #[prost(string, tag = "2")]
            pub thinking_model: String,
            #[prost(bool, tag = "3")]
            pub max_mode: bool,
            #[prost(string, tag = "4")]
            pub next_default_set_date: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct AvailableModelsResponse {
            #[prost(string, repeated, tag = "1")]
            pub model_names: Vec<String>,
            #[prost(message, repeated, tag = "2")]
            pub models: Vec<AvailableModel>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct AvailableModel {
            #[prost(string, tag = "1")]
            pub name: String,
            #[prost(bool, tag = "2")]
            pub default_on: bool,
            #[prost(bool, optional, tag = "5")]
            pub supports_agent: Option<bool>,
            #[prost(string, optional, tag = "18")]
            pub server_model_name: Option<String>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct GetUsableModelsResponse {
            #[prost(message, repeated, tag = "1")]
            pub models: Vec<ModelDetails>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ModelDetails {
            #[prost(string, tag = "1")]
            pub model_id: String,
            #[prost(message, optional, tag = "2")]
            pub thinking_details: Option<ThinkingDetails>,
            #[prost(bool, tag = "7")]
            pub max_mode: bool,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ThinkingDetails {}

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct BidiRequestId {
            #[prost(string, tag = "1")]
            pub request_id: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct BidiAppendRequest {
            #[prost(string, tag = "1")]
            pub data: String,
            #[prost(message, optional, tag = "2")]
            pub request_id: Option<BidiRequestId>,
            #[prost(int64, tag = "3")]
            pub append_seqno: i64,
            #[prost(bytes = "vec", tag = "4")]
            pub data_binary: Vec<u8>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct BidiAppendResponse {}
    }
}

pub mod agent {
    pub mod v1 {
        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct AgentClientMessage {
            #[prost(
                oneof = "agent_client_message::Message",
                tags = "1, 2, 3, 4, 5, 6, 7, 8"
            )]
            pub message: Option<agent_client_message::Message>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct GetDefaultModelForCliResponse {
            #[prost(message, optional, tag = "1")]
            pub model: Option<ModelDetails>,
        }

        pub mod agent_client_message {
            use super::EmptyMessage;

            #[derive(Clone, PartialEq, ::prost::Oneof)]
            pub enum Message {
                #[prost(message, tag = "1")]
                RunRequest(super::AgentRunRequest),
                #[prost(message, tag = "2")]
                ExecClientMessage(EmptyMessage),
                #[prost(message, tag = "5")]
                ExecClientControlMessage(EmptyMessage),
                #[prost(message, tag = "3")]
                KvClientMessage(EmptyMessage),
                #[prost(message, tag = "4")]
                ConversationAction(super::ConversationAction),
                #[prost(message, tag = "6")]
                InteractionResponse(EmptyMessage),
                #[prost(message, tag = "7")]
                ClientHeartbeat(EmptyMessage),
                #[prost(message, tag = "8")]
                PrewarmRequest(EmptyMessage),
            }
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct EmptyMessage {}

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct AgentRunRequest {
            #[prost(message, optional, tag = "2")]
            pub action: Option<ConversationAction>,
            #[prost(message, optional, tag = "3")]
            pub model_details: Option<ModelDetails>,
            #[prost(message, optional, tag = "4")]
            pub mcp_tools: Option<McpTools>,
            #[prost(message, optional, tag = "9")]
            pub requested_model: Option<RequestedModel>,
            #[prost(string, optional, tag = "5")]
            pub conversation_id: Option<String>,
            #[prost(message, optional, tag = "6")]
            pub mcp_file_system_options: Option<McpFileSystemOptions>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationAction {
            #[prost(oneof = "conversation_action::Action", tags = "1, 3")]
            pub action: Option<conversation_action::Action>,
        }

        pub mod conversation_action {
            #[derive(Clone, PartialEq, ::prost::Oneof)]
            pub enum Action {
                #[prost(message, tag = "1")]
                UserMessageAction(super::UserMessageAction),
                #[prost(message, tag = "3")]
                CancelAction(super::CancelAction),
            }
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct CancelAction {
            #[prost(string, tag = "1")]
            pub reason: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct UserMessageAction {
            #[prost(message, optional, tag = "1")]
            pub user_message: Option<UserMessage>,
            #[prost(message, optional, tag = "2")]
            pub request_context: Option<RequestContext>,
            #[prost(message, optional, tag = "7")]
            pub conversation_history: Option<ConversationHistory>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct UserMessage {
            #[prost(string, tag = "1")]
            pub text: String,
            #[prost(string, tag = "2")]
            pub message_id: String,
            #[prost(message, optional, tag = "3")]
            pub selected_context: Option<SelectedContext>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct RequestedModel {
            #[prost(string, tag = "1")]
            pub model_id: String,
            #[prost(bool, tag = "2")]
            pub max_mode: bool,
            #[prost(message, repeated, tag = "3")]
            pub parameters: Vec<ModelParameterValue>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ModelParameterValue {
            #[prost(string, tag = "1")]
            pub id: String,
            #[prost(string, tag = "2")]
            pub value: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ModelDetails {
            #[prost(string, tag = "1")]
            pub model_id: String,
            #[prost(message, optional, tag = "2")]
            pub thinking_details: Option<ThinkingDetails>,
            #[prost(bool, tag = "7")]
            pub max_mode: bool,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ThinkingDetails {}

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistory {
            #[prost(message, repeated, tag = "1")]
            pub messages: Vec<ConversationHistoryMessage>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryMessage {
            #[prost(oneof = "conversation_history_message::Message", tags = "1, 2, 3")]
            pub message: Option<conversation_history_message::Message>,
        }

        pub mod conversation_history_message {
            #[derive(Clone, PartialEq, ::prost::Oneof)]
            pub enum Message {
                #[prost(message, tag = "1")]
                User(super::ConversationHistoryUserMessage),
                #[prost(message, tag = "2")]
                Assistant(super::ConversationHistoryAssistantMessage),
                #[prost(message, tag = "3")]
                Tool(super::ConversationHistoryToolMessage),
            }
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryUserMessage {
            #[prost(message, repeated, tag = "1")]
            pub content: Vec<ConversationHistoryUserContent>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryUserContent {
            #[prost(oneof = "conversation_history_user_content::Content", tags = "1, 2")]
            pub content: Option<conversation_history_user_content::Content>,
        }

        pub mod conversation_history_user_content {
            #[derive(Clone, PartialEq, ::prost::Oneof)]
            pub enum Content {
                #[prost(message, tag = "1")]
                Text(super::ConversationHistoryTextContent),
                #[prost(message, tag = "2")]
                Image(super::ConversationHistoryImageContent),
            }
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryAssistantMessage {
            #[prost(message, repeated, tag = "1")]
            pub content: Vec<ConversationHistoryAssistantContent>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryAssistantContent {
            #[prost(
                oneof = "conversation_history_assistant_content::Content",
                tags = "1, 2, 3, 4"
            )]
            pub content: Option<conversation_history_assistant_content::Content>,
        }

        pub mod conversation_history_assistant_content {
            #[derive(Clone, PartialEq, ::prost::Oneof)]
            pub enum Content {
                #[prost(message, tag = "1")]
                Text(super::ConversationHistoryTextContent),
                #[prost(message, tag = "2")]
                Reasoning(super::ConversationHistoryReasoningContent),
                #[prost(message, tag = "3")]
                RedactedReasoning(super::ConversationHistoryRedactedReasoningContent),
                #[prost(message, tag = "4")]
                ToolCall(super::ConversationHistoryToolCall),
            }
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryToolMessage {
            #[prost(string, tag = "1")]
            pub tool_call_id: String,
            #[prost(string, tag = "2")]
            pub tool_name: String,
            #[prost(message, repeated, tag = "3")]
            pub content: Vec<ConversationHistoryToolResultContent>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryToolResultContent {
            #[prost(
                oneof = "conversation_history_tool_result_content::Content",
                tags = "1, 2"
            )]
            pub content: Option<conversation_history_tool_result_content::Content>,
        }

        pub mod conversation_history_tool_result_content {
            #[derive(Clone, PartialEq, ::prost::Oneof)]
            pub enum Content {
                #[prost(message, tag = "1")]
                Text(super::ConversationHistoryTextContent),
                #[prost(message, tag = "2")]
                Image(super::ConversationHistoryImageContent),
            }
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryTextContent {
            #[prost(string, tag = "1")]
            pub text: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryImageContent {
            #[prost(string, tag = "1")]
            pub data: String,
            #[prost(string, optional, tag = "2")]
            pub mime_type: Option<String>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryReasoningContent {
            #[prost(string, tag = "1")]
            pub text: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryRedactedReasoningContent {
            #[prost(string, tag = "1")]
            pub data: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ConversationHistoryToolCall {
            #[prost(string, tag = "1")]
            pub tool_call_id: String,
            #[prost(string, tag = "2")]
            pub tool_name: String,
            #[prost(string, tag = "3")]
            pub args_json: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct SelectedContext {
            #[prost(message, repeated, tag = "1")]
            pub selected_images: Vec<SelectedImage>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct SelectedImage {
            #[prost(string, tag = "2")]
            pub uuid: String,
            #[prost(string, tag = "3")]
            pub path: String,
            #[prost(string, tag = "7")]
            pub mime_type: String,
            #[prost(bytes = "vec", tag = "1")]
            pub blob_id: Vec<u8>,
            #[prost(bytes = "vec", tag = "8")]
            pub data: Vec<u8>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct McpTools {
            #[prost(message, repeated, tag = "1")]
            pub mcp_tools: Vec<McpToolDefinition>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct McpToolDefinition {
            #[prost(string, tag = "1")]
            pub name: String,
            #[prost(string, tag = "2")]
            pub description: String,
            #[prost(string, tag = "4")]
            pub provider_identifier: String,
            #[prost(string, tag = "5")]
            pub tool_name: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct RequestContext {
            #[prost(message, repeated, tag = "7")]
            pub tools: Vec<McpToolDefinition>,
            #[prost(message, optional, tag = "23")]
            pub mcp_file_system_options: Option<McpFileSystemOptions>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct McpFileSystemOptions {
            #[prost(bool, tag = "1")]
            pub enabled: bool,
            #[prost(message, repeated, tag = "3")]
            pub mcp_descriptors: Vec<McpDescriptor>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct McpDescriptor {
            #[prost(string, tag = "1")]
            pub server_name: String,
            #[prost(string, tag = "2")]
            pub server_identifier: String,
            #[prost(message, repeated, tag = "5")]
            pub tools: Vec<McpToolDefinition>,
        }

        // The server-side subset mirrors the captured agent.v1 field numbers. Keeping this
        // subset local avoids pulling the full generated Cursor schema into the product while
        // preserving the wire contract for the first provider adapter slice.
        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct AgentServerMessage {
            #[prost(oneof = "agent_server_message::Message", tags = "1")]
            pub message: Option<agent_server_message::Message>,
            #[prost(message, optional, tag = "8")]
            pub ttft_breakdown: Option<TtftBreakdown>,
        }

        pub mod agent_server_message {
            #[derive(Clone, PartialEq, ::prost::Oneof)]
            pub enum Message {
                #[prost(message, tag = "1")]
                InteractionUpdate(super::InteractionUpdate),
            }
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct InteractionUpdate {
            #[prost(oneof = "interaction_update::Message", tags = "1, 2, 4, 5, 7, 13, 14")]
            pub message: Option<interaction_update::Message>,
        }

        pub mod interaction_update {
            #[derive(Clone, PartialEq, ::prost::Oneof)]
            pub enum Message {
                #[prost(message, tag = "1")]
                TextDelta(super::TextDeltaUpdate),
                #[prost(message, tag = "2")]
                ToolCallStarted(super::ToolCallStartedUpdate),
                #[prost(message, tag = "4")]
                ThinkingDelta(super::ThinkingDeltaUpdate),
                #[prost(message, tag = "5")]
                ThinkingCompleted(super::ThinkingCompletedUpdate),
                #[prost(message, tag = "7")]
                PartialToolCall(super::PartialToolCallUpdate),
                #[prost(message, tag = "13")]
                Heartbeat(super::HeartbeatUpdate),
                #[prost(message, tag = "14")]
                TurnEnded(super::TurnEndedUpdate),
            }
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct TextDeltaUpdate {
            #[prost(string, tag = "1")]
            pub text: String,
            #[prost(bool, tag = "2")]
            pub is_server_notice: bool,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ThinkingDeltaUpdate {
            #[prost(string, tag = "1")]
            pub text: String,
            #[prost(enumeration = "ThinkingStyle", optional, tag = "2")]
            pub thinking_style: Option<i32>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ThinkingCompletedUpdate {
            #[prost(int32, tag = "1")]
            pub thinking_duration_ms: i32,
        }

        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration,
        )]
        #[repr(i32)]
        pub enum ThinkingStyle {
            Unspecified = 0,
            Default = 1,
            Codex = 2,
            Gpt5 = 3,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct PartialToolCallUpdate {
            #[prost(string, tag = "1")]
            pub call_id: String,
            #[prost(message, optional, tag = "2")]
            pub tool_call: Option<ToolCall>,
            #[prost(string, tag = "3")]
            pub args_text_delta: String,
            #[prost(string, tag = "4")]
            pub model_call_id: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ToolCallStartedUpdate {
            #[prost(string, tag = "1")]
            pub call_id: String,
            #[prost(message, optional, tag = "2")]
            pub tool_call: Option<ToolCall>,
            #[prost(string, tag = "3")]
            pub model_call_id: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct HeartbeatUpdate {}

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct TurnEndedUpdate {
            #[prost(int64, optional, tag = "1")]
            pub input_tokens: Option<i64>,
            #[prost(int64, optional, tag = "2")]
            pub output_tokens: Option<i64>,
            #[prost(int64, optional, tag = "3")]
            pub cache_read_tokens: Option<i64>,
            #[prost(int64, optional, tag = "4")]
            pub cache_write_tokens: Option<i64>,
            #[prost(int64, optional, tag = "5")]
            pub reasoning_tokens: Option<i64>,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct ToolCall {
            #[prost(string, optional, tag = "57")]
            pub tool_call_id: Option<String>,
            #[prost(oneof = "tool_call::Tool", tags = "15")]
            pub tool: Option<tool_call::Tool>,
        }

        pub mod tool_call {
            #[derive(Clone, PartialEq, ::prost::Oneof)]
            pub enum Tool {
                #[prost(message, tag = "15")]
                McpToolCall(super::McpToolCall),
            }
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct McpToolCall {
            #[prost(message, optional, tag = "1")]
            pub args: Option<McpArgs>,
            #[prost(message, optional, tag = "2")]
            pub result: Option<EmptyMessage>,
            #[prost(string, optional, tag = "3")]
            pub description: Option<String>,
        }

        // Only scalar fields needed by the compatibility placeholder are represented here.
        // The omitted map and approval messages remain unknown fields at the wire boundary.
        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct McpArgs {
            #[prost(string, tag = "1")]
            pub name: String,
            #[prost(string, tag = "3")]
            pub tool_call_id: String,
            #[prost(string, tag = "4")]
            pub provider_identifier: String,
            #[prost(string, tag = "5")]
            pub tool_name: String,
            #[prost(string, tag = "9")]
            pub server_identifier: String,
        }

        #[derive(Clone, PartialEq, ::prost::Message)]
        pub struct TtftBreakdown {
            #[prost(double, tag = "1")]
            pub server_first_token_ms: f64,
            #[prost(double, tag = "2")]
            pub pre_stream_setup_ms: f64,
            #[prost(double, tag = "3")]
            pub wait_for_first_event_ms: f64,
            #[prost(double, optional, tag = "4")]
            pub provider_ttft_ms: Option<f64>,
            #[prost(double, tag = "5")]
            pub slow_pool_wait_ms: f64,
        }
    }
}
