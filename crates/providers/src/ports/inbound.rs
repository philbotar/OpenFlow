//! Inbound ports for provider adapters.

use crate::AiClientConfig;
use domain::AiPort;

pub type BoxedAiPort = Box<dyn AiPort>;

pub trait ProviderFactoryPort {
    fn create(&self, config: AiClientConfig) -> BoxedAiPort;
}
