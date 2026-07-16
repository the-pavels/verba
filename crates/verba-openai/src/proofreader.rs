use std::sync::Arc;

use verba_core::{
    coordinator::CancellationToken,
    proofreading::{
        Proofreader, ProofreaderError, ProofreaderResponse, ProofreadingPolicy,
        ProofreadingRequest, ProofreadingScope,
    },
};

use crate::{OpenAiClient, ResponsesApiRequest, ResponsesApiResponse};

mod request;
mod response;

use request::build_request;
use response::decode_response;

#[cfg(test)]
use request::{INSTRUCTIONS, MAX_OUTPUT_TOKENS};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiKeyProviderError {
    Missing,
    Unavailable,
}

pub trait ApiKeyProvider: Send + Sync {
    fn load_api_key(&self) -> Result<String, ApiKeyProviderError>;
}

pub struct OpenAiProofreader {
    client: Arc<dyn ResponsesClient>,
    api_key_provider: Arc<dyn ApiKeyProvider>,
}

impl OpenAiProofreader {
    #[must_use]
    pub fn new(client: Arc<OpenAiClient>, api_key_provider: Arc<dyn ApiKeyProvider>) -> Self {
        Self {
            client,
            api_key_provider,
        }
    }

    #[cfg(test)]
    fn with_client(
        client: Arc<dyn ResponsesClient>,
        api_key_provider: Arc<dyn ApiKeyProvider>,
    ) -> Self {
        Self {
            client,
            api_key_provider,
        }
    }
}

#[async_trait::async_trait]
impl Proofreader for OpenAiProofreader {
    async fn proofread(
        &self,
        request: &ProofreadingRequest,
        cancellation: &CancellationToken,
    ) -> Result<ProofreaderResponse, ProofreaderError> {
        if cancellation.is_cancelled() {
            return Err(ProofreaderError::Cancelled);
        }
        validate_policy(request.policy())?;

        let api_key = self
            .api_key_provider
            .load_api_key()
            .map_err(|error| match error {
                ApiKeyProviderError::Missing => ProofreaderError::MissingCredential,
                ApiKeyProviderError::Unavailable => ProofreaderError::Failed,
            })?;
        if cancellation.is_cancelled() {
            return Err(ProofreaderError::Cancelled);
        }

        let response = self
            .client
            .create_response(&api_key, build_request(request.text()), cancellation)
            .await?;
        decode_response(response, request.text())
    }
}

#[async_trait::async_trait]
trait ResponsesClient: Send + Sync {
    async fn create_response(
        &self,
        api_key: &str,
        request: ResponsesApiRequest,
        cancellation: &CancellationToken,
    ) -> Result<ResponsesApiResponse, ProofreaderError>;
}

#[async_trait::async_trait]
impl ResponsesClient for OpenAiClient {
    async fn create_response(
        &self,
        api_key: &str,
        request: ResponsesApiRequest,
        cancellation: &CancellationToken,
    ) -> Result<ResponsesApiResponse, ProofreaderError> {
        OpenAiClient::create_response(self, api_key, request, cancellation).await
    }
}

fn validate_policy(policy: ProofreadingPolicy) -> Result<(), ProofreaderError> {
    if policy.scope() != ProofreadingScope::SpellingAndGrammarOnly
        || !policy.preserves_language()
        || !policy.preserves_tone()
        || !policy.preserves_whitespace()
        || !policy.preserves_formatting()
    {
        return Err(ProofreaderError::Failed);
    }

    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod live_evaluation;
