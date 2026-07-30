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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatAttachmentKind {
    Image,
    Document,
}

/// Durable attachment metadata. Storage paths and attachment bytes stay outside engine state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachmentRef {
    pub id: String,
    pub file_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub kind: ChatAttachmentKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ChatAttachmentRef>,
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
            attachments: Vec::new(),
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
            attachments: Vec::new(),
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
            attachments: Vec::new(),
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
            attachments: Vec::new(),
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
            attachments: Vec::new(),
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

/// Provider reasoning block preserved for multi-turn Claude thinking continuity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentReasoning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub content: Vec<AgentReasoningContent>,
}

/// Opaque reasoning payload from provider APIs (text, signature, encrypted, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentReasoningContent {
    Text {
        text: String,
        signature: Option<String>,
    },
    Encrypted(String),
    Redacted {
        data: String,
    },
    Summary(String),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AgentReasoningContentRef<'a> {
    Text {
        text: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<&'a str>,
    },
    Encrypted {
        data: &'a str,
    },
    Redacted {
        data: &'a str,
    },
    Summary {
        text: &'a str,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AgentReasoningContentValue {
    Text {
        text: String,
        #[serde(default)]
        signature: Option<String>,
    },
    Encrypted {
        data: String,
    },
    Redacted {
        data: String,
    },
    Summary {
        text: String,
    },
}

impl Serialize for AgentReasoningContent {
    fn serialize<__S>(&self, serializer: __S) -> Result<__S::Ok, __S::Error>
    where
        __S: serde::Serializer,
    {
        match self {
            Self::Text { text, signature } => AgentReasoningContentRef::Text {
                text,
                signature: signature.as_deref(),
            },
            Self::Encrypted(data) => AgentReasoningContentRef::Encrypted { data },
            Self::Redacted { data } => AgentReasoningContentRef::Redacted { data },
            Self::Summary(text) => AgentReasoningContentRef::Summary { text },
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AgentReasoningContent {
    fn deserialize<__D>(deserializer: __D) -> Result<Self, __D::Error>
    where
        __D: serde::Deserializer<'de>,
    {
        Ok(
            match AgentReasoningContentValue::deserialize(deserializer)? {
                AgentReasoningContentValue::Text { text, signature } => {
                    Self::Text { text, signature }
                }
                AgentReasoningContentValue::Encrypted { data } => Self::Encrypted(data),
                AgentReasoningContentValue::Redacted { data } => Self::Redacted { data },
                AgentReasoningContentValue::Summary { text } => Self::Summary(text),
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTranscriptItem {
    AssistantMessage {
        content: String,
    },
    UserMessage {
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<ChatAttachmentRef>,
    },
    Reasoning {
        reasoning: AgentReasoning,
    },
    ToolCall {
        call: ToolCall,
    },
    ToolResult {
        result: ToolResult,
    },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test fixtures use unwrap for brevity")]
mod tests {
    use super::*;
    use serde_json::json;

    fn image_attachment() -> ChatAttachmentRef {
        ChatAttachmentRef {
            id: "attachment-1".to_string(),
            file_name: "diagram.png".to_string(),
            media_type: "image/png".to_string(),
            size_bytes: 4,
            sha256: "abcd".to_string(),
            kind: ChatAttachmentKind::Image,
        }
    }

    #[test]
    fn legacy_user_messages_default_attachments_to_empty() {
        let message: ChatMessage = serde_json::from_value(json!({
            "role": "user",
            "content": "Legacy chat message",
            "streaming": false
        }))
        .unwrap();
        assert!(message.attachments.is_empty());

        let transcript: AgentTranscriptItem = serde_json::from_value(json!({
            "user_message": {
                "content": "Legacy transcript message"
            }
        }))
        .unwrap();
        assert_eq!(
            transcript,
            AgentTranscriptItem::UserMessage {
                content: "Legacy transcript message".to_string(),
                attachments: Vec::new(),
            }
        );
    }

    #[test]
    fn attachment_message_serde_roundtrip_uses_camel_case_metadata() {
        let message = ChatMessage {
            role: ChatRole::User,
            content: "Review this diagram".to_string(),
            id: None,
            streaming: false,
            tool_call_id: None,
            message_kind: None,
            attachments: vec![image_attachment()],
        };

        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(json["attachments"][0]["fileName"], "diagram.png");
        assert_eq!(json["attachments"][0]["mediaType"], "image/png");
        assert_eq!(json["attachments"][0]["sizeBytes"], 4);
        assert_eq!(json["attachments"][0]["kind"], "image");
        assert!(json["attachments"][0].get("file_name").is_none());
        assert_eq!(
            serde_json::from_value::<ChatMessage>(json).unwrap(),
            message
        );

        let transcript = AgentTranscriptItem::UserMessage {
            content: "Review this diagram".to_string(),
            attachments: vec![image_attachment()],
        };
        let json = serde_json::to_value(&transcript).unwrap();
        assert_eq!(
            serde_json::from_value::<AgentTranscriptItem>(json).unwrap(),
            transcript
        );
    }

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
    fn reasoning_content_serde_roundtrip() {
        let reasoning = AgentReasoning {
            id: Some("reasoning-1".to_string()),
            content: vec![
                AgentReasoningContent::Text {
                    text: "Inspecting the request.".to_string(),
                    signature: Some("signature".to_string()),
                },
                AgentReasoningContent::Encrypted("opaque".to_string()),
                AgentReasoningContent::Redacted {
                    data: "redacted".to_string(),
                },
                AgentReasoningContent::Summary("Choosing the next action.".to_string()),
            ],
        };

        let json = serde_json::to_string(&reasoning).unwrap();
        let back: AgentReasoning = serde_json::from_str(&json).unwrap();

        assert_eq!(reasoning, back);
        assert!(json.contains(r#""type":"encrypted","data":"opaque""#));
        assert!(json.contains(r#""type":"summary","text":"Choosing the next action.""#));
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
