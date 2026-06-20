use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A greeting record storing the message, optional recipient, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Greeting {
    pub id: Uuid,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient_name: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,
}

impl Greeting {
    /// Create a new greeting with the given message, optional recipient name, and optional source IP.
    pub fn new(message: String, recipient_name: Option<String>, source_ip: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            message,
            recipient_name,
            created_at: Utc::now(),
            source_ip,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_greeting_has_unique_id() {
        let g1 = Greeting::new("Hi".into(), None, None);
        let g2 = Greeting::new("Hi".into(), None, None);
        assert_ne!(g1.id, g2.id);
    }

    #[test]
    fn new_greeting_sets_timestamp() {
        let before = Utc::now();
        let greeting = Greeting::new("Hello".into(), Some("World".into()), Some("127.0.0.1".into()));
        let after = Utc::now();

        assert_eq!(greeting.message, "Hello");
        assert_eq!(greeting.recipient_name.as_deref(), Some("World"));
        assert_eq!(greeting.source_ip.as_deref(), Some("127.0.0.1"));
        assert!(greeting.created_at >= before);
        assert!(greeting.created_at <= after);
    }

    #[test]
    fn new_greeting_optional_fields_are_none() {
        let greeting = Greeting::new("Hey".into(), None, None);
        assert!(greeting.recipient_name.is_none());
        assert!(greeting.source_ip.is_none());
    }

    #[test]
    fn greeting_serializes_to_json() {
        let greeting = Greeting::new("Hello".into(), Some("Alice".into()), None);
        let json = serde_json::to_string(&greeting).unwrap();
        assert!(json.contains("\"message\":\"Hello\""));
        assert!(json.contains("\"recipient_name\":\"Alice\""));
    }

    #[test]
    fn greeting_deserializes_from_json() {
        let greeting = Greeting::new("Hi".into(), None, None);
        let json = serde_json::to_string(&greeting).unwrap();
        let decoded: Greeting = serde_json::from_str(&json).unwrap();
        assert_eq!(greeting, decoded);
    }
}
