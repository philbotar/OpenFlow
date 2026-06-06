//! Inbound adapters for provider-facing entrypoints.

use crate::client::AiClient;
use crate::ports::inbound::{BoxedAiPort, ProviderFactoryPort};
use crate::AiClientConfig;

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultProviderFactory;

impl ProviderFactoryPort for DefaultProviderFactory {
	fn create(&self, config: AiClientConfig) -> BoxedAiPort {
		Box::new(AiClient::with_config(config))
	}
}