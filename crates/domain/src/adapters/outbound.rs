//! Domain-level outbound adapters.

use crate::AiPort;

pub type DynAiPort = Box<dyn AiPort>;

#[must_use]
pub fn boxed_ai_port(port: impl AiPort + 'static) -> DynAiPort {
    Box::new(port)
}
