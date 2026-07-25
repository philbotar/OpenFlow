//! Lossless conversion between rig reasoning blocks and engine [`AgentReasoning`].

use engine::{AgentReasoning, AgentReasoningContent};
use rig_core::message::{Reasoning, ReasoningContent};

#[must_use]
pub fn rig_to_agent(reasoning: &Reasoning) -> AgentReasoning {
    AgentReasoning {
        id: reasoning.id.clone(),
        content: reasoning.content.iter().map(rig_content_to_agent).collect(),
    }
}

#[must_use]
pub fn agent_to_rig(reasoning: &AgentReasoning) -> Reasoning {
    let mut rig = Reasoning::new("");
    rig.id.clone_from(&reasoning.id);
    rig.content = reasoning.content.iter().map(agent_content_to_rig).collect();
    rig
}

/// Replace Rig's unsigned streaming-delta aggregate when the matching signed
/// final block follows it.
#[must_use]
pub fn coalesce_signed_stream_duplicates(
    reasoning: impl IntoIterator<Item = AgentReasoning>,
) -> Vec<AgentReasoning> {
    let mut normalized = Vec::new();
    for block in reasoning {
        let replaces_previous = block_signature(&block).is_some()
            && normalized.last().is_some_and(|previous| {
                block_signature(previous).is_none()
                    && previous.id == block.id
                    && block_text(previous) == block_text(&block)
            });
        if replaces_previous {
            let _ = normalized.pop();
        }
        normalized.push(block);
    }
    normalized
}

fn block_signature(reasoning: &AgentReasoning) -> Option<&str> {
    reasoning.content.iter().find_map(|content| match content {
        AgentReasoningContent::Text {
            signature: Some(signature),
            ..
        } => Some(signature.as_str()),
        AgentReasoningContent::Text {
            signature: None, ..
        }
        | AgentReasoningContent::Encrypted(_)
        | AgentReasoningContent::Redacted { .. }
        | AgentReasoningContent::Summary(_) => None,
    })
}

fn block_text(reasoning: &AgentReasoning) -> String {
    reasoning
        .content
        .iter()
        .filter_map(|content| match content {
            AgentReasoningContent::Text { text, .. } | AgentReasoningContent::Summary(text) => {
                Some(text.as_str())
            }
            AgentReasoningContent::Encrypted(_) | AgentReasoningContent::Redacted { .. } => None,
        })
        .collect()
}

fn rig_content_to_agent(content: &ReasoningContent) -> AgentReasoningContent {
    match content {
        ReasoningContent::Text { text, signature } => AgentReasoningContent::Text {
            text: text.clone(),
            signature: signature.clone(),
        },
        ReasoningContent::Encrypted(data) => AgentReasoningContent::Encrypted(data.clone()),
        ReasoningContent::Redacted { data } => {
            AgentReasoningContent::Redacted { data: data.clone() }
        }
        ReasoningContent::Summary(summary) => AgentReasoningContent::Summary(summary.clone()),
        _ => AgentReasoningContent::Text {
            text: String::new(),
            signature: None,
        },
    }
}

fn agent_content_to_rig(content: &AgentReasoningContent) -> ReasoningContent {
    match content {
        AgentReasoningContent::Text { text, signature } => ReasoningContent::Text {
            text: text.clone(),
            signature: signature.clone(),
        },
        AgentReasoningContent::Encrypted(data) => ReasoningContent::Encrypted(data.clone()),
        AgentReasoningContent::Redacted { data } => {
            ReasoningContent::Redacted { data: data.clone() }
        }
        AgentReasoningContent::Summary(summary) => ReasoningContent::Summary(summary.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_text_round_trips() {
        let rig = Reasoning::new_with_signature("think", Some("sig-1".into()));
        let agent = rig_to_agent(&rig);
        let back = agent_to_rig(&agent);
        assert_eq!(rig, back);
    }

    #[test]
    fn signature_only_round_trips() {
        let rig = Reasoning::new_with_signature("", Some("sig-only".into()));
        let agent = rig_to_agent(&rig);
        let back = agent_to_rig(&agent);
        assert_eq!(rig.first_signature(), back.first_signature());
    }
}
