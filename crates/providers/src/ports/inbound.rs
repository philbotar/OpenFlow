//! Inbound ports for provider adapters.

use crate::AiClientConfig;
use workflow_core::AiPort;

pub type BoxedAiPort = Box<dyn AiPort>;

pub trait ProviderFactoryPort {
    fn create(&self, config: AiClientConfig) -> BoxedAiPort;
}
