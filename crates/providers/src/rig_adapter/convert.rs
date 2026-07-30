//! `AgentRequest` → rig `CompletionRequest` translation.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "rig migration: wired when AiClient switches to rig_adapter"
    )
)]

use std::collections::BTreeMap;

use crate::mapping::{all_tool_specs, build_node_context, ToolSpec};
use crate::rig_adapter::reasoning_convert;
use base64::Engine as _;
use engine::{
    AgentError, AgentReasoning, AgentRequest, AgentTranscriptItem, ChatAttachmentKind,
    ChatAttachmentRef,
};
use rig_core::completion::CompletionRequest;
use rig_core::message::{
    AssistantContent, Document, DocumentMediaType, DocumentSourceKind, ImageMediaType, Message,
    ToolCall as RigToolCall, ToolChoice, ToolFunction, ToolResultContent, UserContent,
};
use rig_core::OneOrMany;

pub fn to_completion_request(request: &AgentRequest) -> Result<CompletionRequest, AgentError> {
    let node_context = build_node_context(request);
    let mut history: Vec<Message> = vec![rig_user_message(
        request,
        &node_context,
        &request.entrypoint_attachments,
    )?];
    let mut index = 0;
    while index < request.transcript.len() {
        match &request.transcript[index] {
            AgentTranscriptItem::UserMessage {
                content,
                attachments,
            } => {
                history.push(rig_user_message(request, content, attachments)?);
                index += 1;
            }
            AgentTranscriptItem::AssistantMessage { content } => {
                history.push(Message::assistant(content.clone()));
                index += 1;
            }
            AgentTranscriptItem::Reasoning { reasoning } => {
                let mut reasoning_blocks = vec![reasoning.clone()];
                index += 1;
                while let Some(AgentTranscriptItem::Reasoning { reasoning }) =
                    request.transcript.get(index)
                {
                    reasoning_blocks.push(reasoning.clone());
                    index += 1;
                }
                let reasoning_blocks =
                    reasoning_convert::coalesce_signed_stream_duplicates(reasoning_blocks);
                if matches!(
                    request.transcript.get(index),
                    Some(AgentTranscriptItem::ToolCall { .. })
                ) {
                    let consumed = push_tool_turn(
                        &mut history,
                        &request.transcript[index..],
                        &reasoning_blocks,
                    );
                    index += consumed;
                } else {
                    for block in reasoning_blocks {
                        let content = OneOrMany::one(AssistantContent::Reasoning(
                            reasoning_convert::agent_to_rig(&block),
                        ));
                        history.push(Message::Assistant { id: None, content });
                    }
                }
            }
            AgentTranscriptItem::ToolCall { .. } | AgentTranscriptItem::ToolResult { .. } => {
                let consumed = push_tool_turn(&mut history, &request.transcript[index..], &[]);
                index += consumed;
            }
        }
    }
    let chat_history = OneOrMany::many(history).map_err(|_| {
        AgentError::Permanent("provider request contains no user messages".to_string())
    })?;
    Ok(CompletionRequest {
        model: Some(request.model.clone()),
        preamble: Some(request.system_content()),
        chat_history,
        documents: Vec::new(),
        tools: all_tool_specs(request).into_iter().map(rig_tool).collect(),
        temperature: None,
        max_tokens: None,
        tool_choice: Some(ToolChoice::Required),
        additional_params: None,
        output_schema: None,
    })
}

fn rig_user_message(
    request: &AgentRequest,
    text: &str,
    attachments: &[ChatAttachmentRef],
) -> Result<Message, AgentError> {
    let mut content = Vec::with_capacity(usize::from(!text.is_empty()) + attachments.len());
    if !text.is_empty() {
        content.push(UserContent::text(text));
    }
    for attachment in attachments {
        content.push(rig_attachment(request, attachment)?);
    }
    if content.is_empty() {
        content.push(UserContent::text(""));
    }
    let content = OneOrMany::many(content).map_err(|_| {
        AgentError::Permanent("provider user message contains no content".to_string())
    })?;
    Ok(Message::User { content })
}

fn rig_attachment(
    request: &AgentRequest,
    attachment: &ChatAttachmentRef,
) -> Result<UserContent, AgentError> {
    let resolved = request
        .resolved_attachments
        .get(&attachment.id)
        .ok_or_else(|| {
            AgentError::Permanent(format!(
                "attachment `{}` ({}) was not resolved",
                attachment.file_name, attachment.id
            ))
        })?;
    match attachment.kind {
        ChatAttachmentKind::Image => {
            let media_type = match attachment.media_type.as_str() {
                "image/jpeg" => ImageMediaType::JPEG,
                "image/png" => ImageMediaType::PNG,
                "image/gif" => ImageMediaType::GIF,
                "image/webp" => ImageMediaType::WEBP,
                _ => return Err(unsupported_attachment_media_type(attachment)),
            };
            Ok(UserContent::image_base64(
                base64::engine::general_purpose::STANDARD.encode(&resolved.bytes),
                Some(media_type),
                None,
            ))
        }
        ChatAttachmentKind::Document if attachment.media_type == "application/pdf" => {
            Ok(UserContent::Document(Document {
                data: DocumentSourceKind::Base64(
                    base64::engine::general_purpose::STANDARD.encode(&resolved.bytes),
                ),
                media_type: Some(DocumentMediaType::PDF),
                additional_params: None,
            }))
        }
        ChatAttachmentKind::Document if is_utf8_document_media_type(&attachment.media_type) => {
            let document = std::str::from_utf8(&resolved.bytes).map_err(|_| {
                AgentError::Permanent(format!(
                    "attachment `{}` is not valid UTF-8",
                    attachment.file_name
                ))
            })?;
            Ok(UserContent::text(format!(
                "Attachment: {}\n{document}",
                attachment.file_name
            )))
        }
        ChatAttachmentKind::Document => Err(unsupported_attachment_media_type(attachment)),
    }
}

fn is_utf8_document_media_type(media_type: &str) -> bool {
    matches!(
        media_type,
        "text/plain"
            | "text/markdown"
            | "text/csv"
            | "application/json"
            | "text/html"
            | "text/css"
            | "application/javascript"
            | "text/x-python"
    )
}

fn unsupported_attachment_media_type(attachment: &ChatAttachmentRef) -> AgentError {
    AgentError::Permanent(format!(
        "attachment `{}` uses unsupported media type `{}`",
        attachment.file_name, attachment.media_type
    ))
}

fn rig_tool(spec: ToolSpec) -> rig_core::completion::ToolDefinition {
    rig_core::completion::ToolDefinition {
        name: spec.name,
        description: spec.description,
        parameters: spec.parameters,
    }
}

/// Consume one contiguous run of tool calls/results. Emits a single assistant
/// message carrying every call, then a single user message carrying every
/// result in call order. Bedrock requires all `toolResults` for a `toolUse`
/// batch in the one next user message ("Expected `toolResult` blocks at
/// `messages.N.content`"); rig's `OpenAI` adapters re-split that message into one
/// tool-role message per result, which is the shape strict OpenAI-compatible
/// providers demand.
fn push_tool_turn(
    history: &mut Vec<Message>,
    items: &[AgentTranscriptItem],
    leading_reasoning: &[AgentReasoning],
) -> usize {
    let mut calls: Vec<engine::ToolCall> = Vec::new();
    let mut results_by_id: BTreeMap<String, engine::ToolResult> = BTreeMap::new();
    let mut consumed = 0;
    for item in items {
        match item {
            AgentTranscriptItem::ToolCall { call } => calls.push(call.clone()),
            AgentTranscriptItem::ToolResult { result } => {
                results_by_id.insert(result.tool_call_id.clone(), result.clone());
            }
            _ => break,
        }
        consumed += 1;
    }
    let mut contents: Vec<AssistantContent> = leading_reasoning
        .iter()
        .map(|block| AssistantContent::Reasoning(reasoning_convert::agent_to_rig(block)))
        .collect();
    contents.extend(calls.iter().map(|call| {
        let rig_call = RigToolCall::new(
            call.id.clone(),
            ToolFunction {
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            },
        );
        AssistantContent::ToolCall(match &call.provider_call_id {
            Some(provider_call_id) => rig_call.with_call_id(provider_call_id.clone()),
            None => rig_call,
        })
    }));
    if let Ok(content) = OneOrMany::many(contents) {
        history.push(Message::Assistant { id: None, content });
    }
    let result_contents: Vec<UserContent> = calls
        .iter()
        .map(|call| match results_by_id.remove(&call.id) {
            Some(result) => rig_tool_result(
                result.tool_call_id,
                call.provider_call_id.as_deref(),
                result.content,
            ),
            // A call with no recorded result (interrupted batch) must still be
            // answered or strict providers reject the whole transcript.
            None => rig_tool_result(
                call.id.clone(),
                call.provider_call_id.as_deref(),
                "Tool execution was interrupted before a result was produced.".to_string(),
            ),
        })
        .collect();
    if let Ok(content) = OneOrMany::many(result_contents) {
        history.push(Message::User { content });
    }
    // Orphan results with no matching call in this run (e.g. truncated
    // checkpoints): degrade to plain user text rather than sending an
    // unanswerable tool_result.
    for result in results_by_id.into_values() {
        history.push(Message::user(result.content));
    }
    consumed
}

fn rig_tool_result(id: String, provider_call_id: Option<&str>, content: String) -> UserContent {
    let content = OneOrMany::one(ToolResultContent::text(content));
    match provider_call_id {
        Some(provider_call_id) => {
            UserContent::tool_result_with_call_id(id, provider_call_id.to_string(), content)
        }
        None => UserContent::tool_result(id, content),
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "unit tests assert message shapes with expect/panic"
)]
mod tests {
    use super::*;
    use crate::mapping::SUBMIT_OUTPUT_TOOL;
    use engine::{
        ChatAttachmentKind, ChatAttachmentRef, NodeId, ResolvedChatAttachment,
        ToolCall as EngineToolCall, WorkflowId,
    };
    use rig_core::message::{
        DocumentMediaType, DocumentSourceKind, ImageMediaType, Text, ToolResultContent, UserContent,
    };
    use serde_json::json;

    fn request_with_transcript(transcript: Vec<AgentTranscriptItem>) -> AgentRequest {
        AgentRequest {
            workflow_id: WorkflowId("wf-1".into()),
            node_id: NodeId("n1".into()),
            node_label: "Node".into(),
            model: "claude-sonnet-4-6".into(),
            provider_id: None,
            system_messages: vec!["sys-a".into(), "sys-b".into()],
            task_prompt: "do the thing".into(),
            input: json!({"k": "v"}),
            output_schema: json!({"type": "object", "properties": {"r": {"type": "string"}}}),
            tool_config: engine::NodeToolConfig::default(),
            available_tools: Vec::new(),
            transcript,
            entrypoint_attachments: Vec::new(),
            resolved_attachments: BTreeMap::default(),
            model_attempt: 1,
            reasoning_effort: None,
            reasoning_budget_tokens: None,
            fast_mode: false,
            tool_access_policy: engine::ToolAccessPolicy::Execution,
            allow_user_input: true,
            conversation_mode: false,
        }
    }

    fn tc(id: &str, name: &str) -> AgentTranscriptItem {
        AgentTranscriptItem::ToolCall {
            call: EngineToolCall {
                id: id.into(),
                provider_call_id: None,
                name: name.into(),
                arguments: json!({}),
            },
        }
    }

    fn tr(id: &str, content: &str) -> AgentTranscriptItem {
        AgentTranscriptItem::ToolResult {
            result: engine::ToolResult {
                tool_call_id: id.into(),
                tool_name: "read".into(),
                content: content.into(),
                is_error: false,
                artifact_ids: Vec::new(),
                output_meta: None,
            },
        }
    }

    fn attachment(
        id: &str,
        file_name: &str,
        media_type: &str,
        kind: ChatAttachmentKind,
    ) -> ChatAttachmentRef {
        ChatAttachmentRef {
            id: id.into(),
            file_name: file_name.into(),
            media_type: media_type.into(),
            size_bytes: 3,
            sha256: "fixture".into(),
            kind,
        }
    }

    #[test]
    fn entrypoint_image_is_mapped_after_node_context_text() {
        let mut request = request_with_transcript(Vec::new());
        request.entrypoint_attachments = vec![attachment(
            "image-1",
            "diagram.png",
            "image/png",
            ChatAttachmentKind::Image,
        )];
        request.resolved_attachments.insert(
            "image-1".into(),
            ResolvedChatAttachment {
                bytes: vec![1, 2, 3],
            },
        );

        let completion = to_completion_request(&request).expect("image should map");
        let Message::User { content } = completion.chat_history.first() else {
            panic!("expected initial user message");
        };
        let items: Vec<_> = content.iter().collect();
        assert!(matches!(items[0], UserContent::Text(_)));
        assert!(matches!(
            items[1],
            UserContent::Image(image)
                if image.media_type == Some(ImageMediaType::PNG)
                    && image.data == DocumentSourceKind::Base64("AQID".into())
        ));
    }

    #[test]
    fn historical_user_message_keeps_text_and_attachment_ref_order_in_one_message() {
        let image = attachment(
            "image-1",
            "diagram.webp",
            "image/webp",
            ChatAttachmentKind::Image,
        );
        let document = attachment(
            "document-1",
            "brief.pdf",
            "application/pdf",
            ChatAttachmentKind::Document,
        );
        let transcript = vec![AgentTranscriptItem::UserMessage {
            content: "Compare these.".into(),
            attachments: vec![image, document],
        }];
        let mut request = request_with_transcript(transcript);
        request.resolved_attachments.insert(
            "image-1".into(),
            ResolvedChatAttachment {
                bytes: vec![1, 2, 3],
            },
        );
        request.resolved_attachments.insert(
            "document-1".into(),
            ResolvedChatAttachment {
                bytes: b"%PDF-1".to_vec(),
            },
        );

        let completion = to_completion_request(&request).expect("attachments should map");
        let messages: Vec<_> = completion.chat_history.iter().collect();
        assert_eq!(messages.len(), 2);
        let Message::User { content } = messages[1] else {
            panic!("expected historical user message");
        };
        let items: Vec<_> = content.iter().collect();
        assert!(matches!(
            items[0],
            UserContent::Text(Text { text, .. }) if text == "Compare these."
        ));
        assert!(matches!(
            items[1],
            UserContent::Image(image) if image.media_type == Some(ImageMediaType::WEBP)
        ));
        assert!(matches!(
            items[2],
            UserContent::Document(document)
                if document.media_type == Some(DocumentMediaType::PDF)
                    && document.data == DocumentSourceKind::Base64("JVBERi0x".into())
        ));
    }

    #[test]
    fn utf8_documents_become_named_text_blocks_including_json() {
        let transcript = vec![AgentTranscriptItem::UserMessage {
            content: String::new(),
            attachments: vec![
                attachment(
                    "markdown-1",
                    "notes.md",
                    "text/markdown",
                    ChatAttachmentKind::Document,
                ),
                attachment(
                    "json-1",
                    "data.json",
                    "application/json",
                    ChatAttachmentKind::Document,
                ),
            ],
        }];
        let mut request = request_with_transcript(transcript);
        request.resolved_attachments.insert(
            "markdown-1".into(),
            ResolvedChatAttachment {
                bytes: b"# Notes".to_vec(),
            },
        );
        request.resolved_attachments.insert(
            "json-1".into(),
            ResolvedChatAttachment {
                bytes: br#"{"answer":42}"#.to_vec(),
            },
        );

        let completion = to_completion_request(&request).expect("documents should map");
        let messages: Vec<_> = completion.chat_history.iter().collect();
        let Message::User { content } = messages[1] else {
            panic!("expected historical user message");
        };
        let text: Vec<_> = content
            .iter()
            .map(|item| match item {
                UserContent::Text(Text { text, .. }) => text.as_str(),
                other => panic!("expected named text attachment, got {other:?}"),
            })
            .collect();
        assert_eq!(
            text,
            vec![
                "Attachment: notes.md\n# Notes",
                "Attachment: data.json\n{\"answer\":42}",
            ]
        );
    }

    #[test]
    fn missing_resolved_attachment_fails_before_transport() {
        let mut request = request_with_transcript(Vec::new());
        request.entrypoint_attachments = vec![attachment(
            "missing-1",
            "missing.png",
            "image/png",
            ChatAttachmentKind::Image,
        )];

        let error = to_completion_request(&request).expect_err("missing bytes must fail");
        assert!(matches!(
            error,
            engine::AgentError::Permanent(message)
                if message.contains("missing.png") && message.contains("not resolved")
        ));
    }

    #[test]
    fn unsupported_attachment_media_type_fails_before_transport() {
        let mut request = request_with_transcript(Vec::new());
        request.entrypoint_attachments = vec![attachment(
            "unsupported-1",
            "photo.heic",
            "image/heic",
            ChatAttachmentKind::Image,
        )];
        request.resolved_attachments.insert(
            "unsupported-1".into(),
            ResolvedChatAttachment {
                bytes: vec![1, 2, 3],
            },
        );

        let error = to_completion_request(&request).expect_err("unsupported MIME must fail");
        assert!(matches!(
            error,
            engine::AgentError::Permanent(message)
                if message.contains("photo.heic") && message.contains("image/heic")
        ));
    }

    #[test]
    fn maps_system_messages_and_task_prompt() {
        let req = to_completion_request(&request_with_transcript(Vec::new()))
            .expect("request should map");
        assert_eq!(req.preamble.as_deref(), Some("sys-a\n\nsys-b"));
        let first = req.chat_history.first();
        assert!(matches!(first, Message::User { .. }));
        let Message::User { content } = first else {
            panic!("expected user message");
        };
        let UserContent::Text(Text { text, .. }) = content.first() else {
            panic!("expected text user content");
        };
        assert!(text.contains("do the thing"));
    }

    #[test]
    fn always_includes_submit_output_tool_and_requires_tool_choice() {
        let req = to_completion_request(&request_with_transcript(Vec::new()))
            .expect("request should map");
        assert!(req.tools.iter().any(|t| t.name == SUBMIT_OUTPUT_TOOL));
        assert_eq!(req.tool_choice, Some(ToolChoice::Required));
    }

    #[test]
    fn transcript_tool_call_and_result_stay_paired() {
        let transcript = vec![
            AgentTranscriptItem::ToolCall {
                call: EngineToolCall {
                    id: "c1".into(),
                    provider_call_id: None,
                    name: "search".into(),
                    arguments: json!({"q": "x"}),
                },
            },
            AgentTranscriptItem::ToolResult {
                result: engine::ToolResult {
                    tool_call_id: "c1".into(),
                    tool_name: "search".into(),
                    content: "found".into(),
                    is_error: false,
                    artifact_ids: Vec::new(),
                    output_meta: None,
                },
            },
        ];
        let req = to_completion_request(&request_with_transcript(transcript))
            .expect("request should map");
        let msgs: Vec<_> = req.chat_history.iter().collect();
        assert_eq!(msgs.len(), 3);
        assert!(matches!(msgs[0], Message::User { .. }));
        assert!(matches!(msgs[1], Message::Assistant { .. }));
        assert!(matches!(msgs[2], Message::User { .. }));
        let Message::Assistant { content, .. } = msgs[1] else {
            panic!("expected assistant tool-call message");
        };
        assert!(matches!(
            content.first(),
            AssistantContent::ToolCall(call) if call.function.name == "search"
        ));
        let Message::User { content } = msgs[2] else {
            panic!("expected user tool-result message");
        };
        assert!(matches!(
            content.first(),
            UserContent::ToolResult(result) if result.id == "c1"
                && result.content.first() == ToolResultContent::text("found")
        ));
    }

    #[test]
    fn multi_call_batch_becomes_one_assistant_message_then_results_in_call_order() {
        let transcript = vec![
            tc("c1", "read"),
            tc("c2", "read"),
            tr("c2", "two"),
            tr("c1", "one"),
        ];
        let req = to_completion_request(&request_with_transcript(transcript))
            .expect("request should map");
        let msgs: Vec<_> = req.chat_history.iter().collect();
        // [node context, assistant(c1+c2), one user message with both results].
        // Bedrock requires every toolResult for a toolUse batch in the single
        // next user message; splitting them across messages is rejected with
        // "Expected toolResult blocks at messages.N.content".
        assert_eq!(msgs.len(), 3);
        let Message::Assistant { content, .. } = msgs[1] else {
            panic!("expected assistant tool-call message");
        };
        let ids: Vec<_> = content
            .iter()
            .map(|c| match c {
                AssistantContent::ToolCall(call) => call.id.clone(),
                other => panic!("expected tool call content, got {other:?}"),
            })
            .collect();
        assert_eq!(
            ids,
            vec!["c1", "c2"],
            "all calls in one assistant message, call order"
        );
        let Message::User { content } = msgs[2] else {
            panic!("expected user tool-result message");
        };
        let results: Vec<_> = content
            .iter()
            .map(|c| match c {
                UserContent::ToolResult(result) => (result.id.clone(), result.content.first()),
                other => panic!("expected tool result content, got {other:?}"),
            })
            .collect();
        assert_eq!(
            results,
            vec![
                ("c1".to_string(), ToolResultContent::text("one")),
                ("c2".to_string(), ToolResultContent::text("two")),
            ],
            "all results in one user message, call order"
        );
    }

    #[test]
    fn missing_result_is_synthesized_so_no_call_goes_unanswered() {
        let transcript = vec![tc("c1", "bash"), tc("c2", "read"), tr("c2", "two")];
        let req = to_completion_request(&request_with_transcript(transcript))
            .expect("request should map");
        let msgs: Vec<_> = req.chat_history.iter().collect();
        assert_eq!(msgs.len(), 3);
        let Message::User { content } = msgs[2] else {
            panic!("expected tool-result message");
        };
        let ids: Vec<_> = content
            .iter()
            .map(|c| match c {
                UserContent::ToolResult(result) => result.id.clone(),
                other => panic!("expected tool result content, got {other:?}"),
            })
            .collect();
        assert_eq!(ids, vec!["c1", "c2"], "synthesized result for c1 included");
    }

    #[test]
    fn reasoning_precedes_tool_calls_in_one_assistant_message() {
        let reasoning = engine::AgentReasoning {
            id: None,
            content: vec![engine::AgentReasoningContent::Text {
                text: "think".into(),
                signature: Some("sig".into()),
            }],
        };
        let transcript = vec![
            AgentTranscriptItem::Reasoning { reasoning },
            tc("c1", "search"),
            tr("c1", "found"),
        ];
        let req = to_completion_request(&request_with_transcript(transcript))
            .expect("request should map");
        let msgs: Vec<_> = req.chat_history.iter().collect();
        let Message::Assistant { content, .. } = msgs[1] else {
            panic!("expected assistant message");
        };
        assert!(matches!(content.first(), AssistantContent::Reasoning(_)));
        assert!(matches!(
            content.last(),
            AssistantContent::ToolCall(call) if call.id == "c1"
        ));
    }

    #[test]
    fn replay_replaces_unsigned_reasoning_delta_with_signed_final_block() {
        let reasoning = |signature| engine::AgentReasoning {
            id: None,
            content: vec![engine::AgentReasoningContent::Text {
                text: "checking the request".into(),
                signature,
            }],
        };
        let transcript = vec![
            AgentTranscriptItem::Reasoning {
                reasoning: reasoning(None),
            },
            AgentTranscriptItem::Reasoning {
                reasoning: reasoning(Some("bedrock-signature".into())),
            },
            tc("c1", "search"),
            tr("c1", "found"),
        ];

        let req = to_completion_request(&request_with_transcript(transcript))
            .expect("request should map");
        let msgs: Vec<_> = req.chat_history.iter().collect();
        let Message::Assistant { content, .. } = msgs[1] else {
            panic!("expected assistant message");
        };
        assert_eq!(content.len(), 2);
        assert!(matches!(
            content.first(),
            AssistantContent::Reasoning(reasoning)
                if reasoning.first_signature() == Some("bedrock-signature")
        ));
    }
}
