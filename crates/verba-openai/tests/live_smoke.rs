use std::sync::Arc;

use futures::executor::block_on;
use verba_core::{
    coordinator::CancellationToken,
    proofreading::{ProofreadText, ProofreadingConsent, ProofreadingResult},
};
use verba_openai::{
    ApiKeyProvider, ApiKeyProviderError, DEFAULT_MODEL, OpenAiClient, OpenAiConfig,
    OpenAiProofreader,
};

#[test]
#[ignore = "requires VERBA_RUN_LIVE_OPENAI_TEST=1 and OPENAI_API_KEY"]
fn live_responses_api_smoke_test() {
    assert_eq!(
        std::env::var("VERBA_RUN_LIVE_OPENAI_TEST").as_deref(),
        Ok("1"),
        "set VERBA_RUN_LIVE_OPENAI_TEST=1 to confirm the paid live request"
    );
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY should be configured");
    let client = Arc::new(
        OpenAiClient::new(OpenAiConfig::new(DEFAULT_MODEL))
            .expect("production client configuration should be valid"),
    );
    let proofreader = Arc::new(OpenAiProofreader::new(
        client,
        Arc::new(StaticApiKeyProvider(api_key)),
    ));
    let result = block_on(ProofreadText::new(proofreader).execute(
        "This sentence is grammatically correct.",
        ProofreadingConsent::Granted,
        &CancellationToken::default(),
    ))
    .expect("live proofreading request should succeed");

    assert!(matches!(
        result,
        ProofreadingResult::NoIssues | ProofreadingResult::Corrected(_)
    ));
}

struct StaticApiKeyProvider(String);

impl ApiKeyProvider for StaticApiKeyProvider {
    fn load_api_key(&self) -> Result<String, ApiKeyProviderError> {
        Ok(self.0.clone())
    }
}
