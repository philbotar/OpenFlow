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
