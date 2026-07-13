use std::sync::Arc;

use futures::executor::block_on;
use verba_core::{
    coordinator::CancellationToken,
    proofreading::{ProofreadText, ProofreadingConsent, ProofreadingFailure, ProofreadingResult},
};
use verba_openai::{
    ApiKeyProvider, ApiKeyProviderError, OpenAiClient, OpenAiConfig, OpenAiProofreader,
};

pub fn run_proofreading(
    base_url: &str,
    model: &str,
    api_key: &str,
    text: &str,
) -> Result<ProofreadingResult, ProofreadingFailure> {
    let client = Arc::new(
        OpenAiClient::new(OpenAiConfig::new(base_url, model))
            .expect("test client configuration should be valid"),
    );
    let proofreader = Arc::new(OpenAiProofreader::new(
        client,
        Arc::new(StaticApiKeyProvider(api_key.to_owned())),
    ));

    block_on(ProofreadText::new(proofreader).execute(
        text,
        ProofreadingConsent::Granted,
        &CancellationToken::default(),
    ))
}

struct StaticApiKeyProvider(String);

impl ApiKeyProvider for StaticApiKeyProvider {
    fn load_api_key(&self) -> Result<String, ApiKeyProviderError> {
        Ok(self.0.clone())
    }
}
