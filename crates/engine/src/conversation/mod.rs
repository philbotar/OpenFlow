//! Conversation messages and transcript items for agent nodes.

use crate::tools::{ToolCall, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Serialized as `snake_case`; legacy `PascalCase` values remain accepted for saved run logs.
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    #[serde(alias = "System")]
    System,
    #[serde(alias = "Thinking")]
    Thinking,
    #[serde(alias = "User")]
    User,
    #[serde(alias = "Assistant")]
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMessageKind {
    NodeCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, rename = "streaming")]
    pub streaming: bool,
    #[serde(
        default,
        rename = "toolCallId",
        skip_serializing_if = "Option::is_none"
    )]
    pub tool_call_id: Option<String>,
    #[serde(
        default,
        rename = "messageKind",
        skip_serializing_if = "Option::is_none"
    )]
    pub message_kind: Option<ChatMessageKind>,
}

impl ChatMessage {
    #[must_use]
    pub fn text(role: ChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            id: None,
            streaming: false,
            tool_call_id: None,
            message_kind: None,
        }
    }

    #[must_use]
    pub fn streaming_assistant(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
            id: Some(id.into()),
            streaming: true,
            tool_call_id: None,
            message_kind: None,
        }
    }

    #[must_use]
    pub fn streaming_thinking(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Thinking,
            content: content.into(),
            id: Some(id.into()),
            streaming: true,
            tool_call_id: None,
            message_kind: None,
        }
    }

    #[must_use]
    pub fn tool_marker(tool_call_id: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Thinking,
            content: String::new(),
            id: None,
            streaming: false,
            tool_call_id: Some(tool_call_id.into()),
            message_kind: None,
        }
    }

    #[must_use]
    pub fn node_completed(summary: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: summary.into(),
            id: None,
            streaming: false,
            tool_call_id: None,
            message_kind: Some(ChatMessageKind::NodeCompleted),
        }
    }
}

/// JSON string of node output for the chat "Node completed" bubble (UI renders a tree).
#[must_use]
pub fn summary_from_node_output(output: &Value) -> Option<String> {
    if output.is_null() {
        return None;
    }
    serde_json::to_string(output).ok()
}

fn consume_tool_call_fence_block(content: &str) -> usize {
    const OPEN: &str = "```tool_call";

    if !content.starts_with(OPEN) {
        return 0;
    }

    let mut consumed = OPEN.len();

    if let Some(rest) = content.get(consumed..) {
        if let Some(stripped) = rest.strip_prefix("\r\n") {
            consumed += rest.len() - stripped.len();
        } else if let Some(stripped) = rest.strip_prefix('\n') {
            consumed += rest.len() - stripped.len();
        }
    }

    if let Some(rest) = content.get(consumed..) {
        if let Some(close) = rest.find("```") {
            return consumed + close + 3;
        }
    }

    content.len()
}

fn consume_tool_call_xml_block(content: &str) -> usize {
    const OPEN: &str = "<tool_call";
    const CLOSE: &str = "</tool_call>";

    if !content.starts_with(OPEN) {
        return 0;
    }

    content
        .find(CLOSE)
        .map_or(content.len(), |index| index + CLOSE.len())
}

fn strip_trailing_partial_tool_call_prefix(content: &str) -> String {
    const PREFIXES: [&str; 2] = ["```tool_call", "<tool_call"];

    for prefix in PREFIXES {
        for len in (1..prefix.len()).rev() {
            if let Some(partial) = prefix.get(..len) {
                if let Some(stripped) = content.strip_suffix(partial) {
                    return stripped.to_string();
                }
            }
        }
    }

    content.to_string()
}

/// Remove echoed tool-invocation markup while keeping any leading human text.
#[must_use]
pub fn strip_tool_call_markup(content: &str) -> String {
    let mut result = String::new();
    let mut rest = content;

    while !rest.is_empty() {
        let xml_index = rest.find("<tool_call");
        let fence_index = rest.find("```tool_call");

        let next = match (xml_index, fence_index) {
            (None, None) => {
                result.push_str(rest);
                break;
            }
            (Some(xml), None) => (xml, true),
            (None, Some(fence)) => (fence, false),
            (Some(xml), Some(fence)) => {
                if xml <= fence {
                    (xml, true)
                } else {
                    (fence, false)
                }
            }
        };

        let (start, is_xml) = next;
        result.push_str(&rest[..start]);
        let block = &rest[start..];

        let consumed = if is_xml {
            consume_tool_call_xml_block(block)
        } else {
            consume_tool_call_fence_block(block)
        };

        if consumed == 0 {
            result.push_str(rest);
            break;
        }

        rest = &rest[start + consumed..];
    }

    strip_trailing_partial_tool_call_prefix(&result)
        .trim()
        .to_string()
}

/// True when assistant text only echoes structured tool invocation markup.
#[must_use]
pub fn is_redundant_tool_call_markup(content: &str) -> bool {
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return false;
    }

    strip_tool_call_markup(trimmed).is_empty()
}

/// Drop or trim assistant text that duplicates structured tool calls in chat/transcript.
#[must_use]
pub fn filter_tool_turn_assistant_message(message: Option<String>) -> Option<String> {
    message
        .map(|content| strip_tool_call_markup(&content))
        .filter(|content| !content.trim().is_empty())
}

/// Whether `openflow_request_user_input` assistant text is a direct human-facing question.
#[must_use]
pub(crate) fn is_clarifying_question(message: &str) -> bool {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains('?') {
        return true;
    }
    let lower = trimmed.to_lowercase();
    [
        "which ",
        "what ",
        "when ",
        "where ",
        "who ",
        "how ",
        "should ",
        "can ",
        "could ",
        "would ",
        "do you ",
        "are you ",
        "is there ",
        "please choose",
        "please pick",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTranscriptItem {
    AssistantMessage { content: String },
    UserMessage { content: String },
    ToolCall { call: ToolCall },
    ToolResult { result: ToolResult },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test fixtures use unwrap for brevity")]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chat_message_serde_roundtrip() {
        let msg = ChatMessage::text(ChatRole::Thinking, "Preparing request...");
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);

        let marker = ChatMessage::tool_marker("call-1");
        let marker_json = serde_json::to_string(&marker).unwrap();
        assert!(marker_json.contains("\"toolCallId\":\"call-1\""));
        let marker_back: ChatMessage = serde_json::from_str(&marker_json).unwrap();
        assert_eq!(marker, marker_back);

        let completed = ChatMessage::node_completed("Shipped the summary.");
        let completed_json = serde_json::to_string(&completed).unwrap();
        assert!(completed_json.contains("\"messageKind\":\"node_completed\""));
        let completed_back: ChatMessage = serde_json::from_str(&completed_json).unwrap();
        assert_eq!(completed, completed_back);
    }

    #[test]
    fn summary_from_node_output_serializes_json() {
        assert_eq!(
            summary_from_node_output(&json!({"summary": "Done."})),
            Some(r#"{"summary":"Done."}"#.to_string())
        );
        assert_eq!(summary_from_node_output(&Value::Null), None);
    }

    #[test]
    fn redundant_tool_call_markup_detects_xml_echoes() {
        assert!(is_redundant_tool_call_markup(
            "<tool_call>\n<function=search>\n</function>\n</tool_call>"
        ));
        assert!(!is_redundant_tool_call_markup(
            "Let me search the repo for TODOs."
        ));
        assert!(!is_redundant_tool_call_markup(
            "I'll submit the result now.<tool_call><function=openflow_submit_node_output></function></tool_call>"
        ));
    }

    #[test]
    fn strip_tool_call_markup_keeps_leading_human_text() {
        assert_eq!(
            strip_tool_call_markup(
                "I'll capture the upstream message.<tool_call>\n<function=openflow_submit_node_output>\n</function>\n</tool_call>"
            ),
            "I'll capture the upstream message."
        );
        assert_eq!(
            strip_tool_call_markup("```tool_call\n<function=read>\n</function>\n```"),
            ""
        );
        assert_eq!(
            strip_tool_call_markup("Now searching.<tool_call>\n<function=search>\n"),
            "Now searching."
        );
        assert_eq!(strip_tool_call_markup("Planning.<tool_cal"), "Planning.");
        assert_eq!(strip_tool_call_markup("<tool"), "");
    }

    #[test]
    fn clarifying_question_detects_questions_and_rejects_preamble() {
        assert!(is_clarifying_question(
            "Should tool rows animate like Cursor's shimmer?"
        ));
        assert!(is_clarifying_question(
            "Which animation style do you prefer for the loading state?"
        ));
        assert!(!is_clarifying_question(
            "Let me check the existing animation patterns and the CSS custom properties used in the codebase:"
        ));
        assert!(!is_clarifying_question(
            "That's a pretty clear request! Let me make sure I have one detail right before submitting the brief:"
        ));
    }

    #[test]
    fn filter_tool_turn_assistant_message_keeps_human_text() {
        assert_eq!(
            filter_tool_turn_assistant_message(Some("Checking README.".to_string())),
            Some("Checking README.".to_string())
        );
        assert_eq!(
            filter_tool_turn_assistant_message(Some(
                "<tool_call><function=read></function></tool_call>".to_string()
            )),
            None
        );
        assert_eq!(
            filter_tool_turn_assistant_message(Some(
                "Preparing output.<tool_call><function=openflow_submit_node_output></function></tool_call>"
                    .to_string()
            )),
            Some("Preparing output.".to_string())
        );
    }

    #[test]
    fn chat_role_serializes_snake_case() {
        assert_eq!(
            serde_json::to_value(ChatRole::Assistant).unwrap(),
            json!("assistant")
        );
        assert_eq!(
            serde_json::from_value::<ChatRole>(json!("Assistant")).unwrap(),
            ChatRole::Assistant
        );
    }
}
