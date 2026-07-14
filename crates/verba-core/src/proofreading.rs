use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::coordinator::CancellationToken;

pub const MAX_PROOFREADING_CHARACTERS: usize = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofreadingConsent {
    NotGranted,
    Granted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofreadingConsentStoreError;

pub trait ProofreadingConsentStore: Send + Sync {
    fn load_acknowledged(&self) -> Result<bool, ProofreadingConsentStoreError>;

    fn save_acknowledged(&self) -> Result<(), ProofreadingConsentStoreError>;
}

pub trait ProofreadingConsentGate: Send + Sync {
    fn is_granted(&self) -> bool;

    fn grant(&self) -> Result<(), ProofreadingConsentStoreError>;
}

pub struct ProofreadingConsentPreferences {
    store: Arc<dyn ProofreadingConsentStore>,
    acknowledged: AtomicBool,
}

impl ProofreadingConsentPreferences {
    pub fn load(
        store: Arc<dyn ProofreadingConsentStore>,
    ) -> Result<Self, ProofreadingConsentStoreError> {
        let acknowledged = store.load_acknowledged()?;
        Ok(Self {
            store,
            acknowledged: AtomicBool::new(acknowledged),
        })
    }
}

impl ProofreadingConsentGate for ProofreadingConsentPreferences {
    fn is_granted(&self) -> bool {
        self.acknowledged.load(Ordering::Acquire)
    }

    fn grant(&self) -> Result<(), ProofreadingConsentStoreError> {
        if self.is_granted() {
            return Ok(());
        }

        self.store.save_acknowledged()?;
        self.acknowledged.store(true, Ordering::Release);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofreadingScope {
    SpellingAndGrammarOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofreadingPolicy {
    scope: ProofreadingScope,
    preserve_language: bool,
    preserve_tone: bool,
    preserve_whitespace: bool,
    preserve_formatting: bool,
}

impl ProofreadingPolicy {
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            scope: ProofreadingScope::SpellingAndGrammarOnly,
            preserve_language: true,
            preserve_tone: true,
            preserve_whitespace: true,
            preserve_formatting: true,
        }
    }

    #[must_use]
    pub const fn scope(&self) -> ProofreadingScope {
        self.scope
    }

    #[must_use]
    pub const fn preserves_language(&self) -> bool {
        self.preserve_language
    }

    #[must_use]
    pub const fn preserves_tone(&self) -> bool {
        self.preserve_tone
    }

    #[must_use]
    pub const fn preserves_whitespace(&self) -> bool {
        self.preserve_whitespace
    }

    #[must_use]
    pub const fn preserves_formatting(&self) -> bool {
        self.preserve_formatting
    }
}

impl Default for ProofreadingPolicy {
    fn default() -> Self {
        Self::strict()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofreadingRequest {
    text: String,
    policy: ProofreadingPolicy,
}

impl ProofreadingRequest {
    #[must_use]
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            policy: ProofreadingPolicy::strict(),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn policy(&self) -> ProofreadingPolicy {
        self.policy
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofreadingCorrection {
    corrected_text: String,
}

impl ProofreadingCorrection {
    #[must_use]
    pub fn new(corrected_text: impl Into<String>) -> Self {
        Self {
            corrected_text: corrected_text.into(),
        }
    }

    #[must_use]
    pub fn corrected_text(&self) -> &str {
        &self.corrected_text
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofreaderResponse {
    NoIssues,
    Corrected(ProofreadingCorrection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofreadingResult {
    NoIssues,
    Corrected(ProofreadingCorrection),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofreaderError {
    MissingCredential,
    Authentication,
    RateLimited,
    QuotaExceeded,
    Offline,
    TimedOut,
    Refused,
    Incomplete,
    MalformedResponse,
    ServiceUnavailable,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofreadingFailure {
    EmptyInput,
    InputTooLong {
        maximum_characters: usize,
        actual_characters: usize,
    },
    ConsentRequired,
    Cancelled,
    InvalidResult,
    Provider(ProofreaderError),
}

#[async_trait::async_trait]
pub trait Proofreader: Send + Sync {
    async fn proofread(
        &self,
        request: &ProofreadingRequest,
        cancellation: &CancellationToken,
    ) -> Result<ProofreaderResponse, ProofreaderError>;
}

pub struct ProofreadText {
    proofreader: Arc<dyn Proofreader>,
}

impl ProofreadText {
    #[must_use]
    pub fn new(proofreader: Arc<dyn Proofreader>) -> Self {
        Self { proofreader }
    }

    pub async fn execute(
        &self,
        text: impl Into<String>,
        consent: ProofreadingConsent,
        cancellation: &CancellationToken,
    ) -> Result<ProofreadingResult, ProofreadingFailure> {
        if cancellation.is_cancelled() {
            return Err(ProofreadingFailure::Cancelled);
        }

        let text = text.into();
        if text.trim().is_empty() {
            return Err(ProofreadingFailure::EmptyInput);
        }

        let character_count = text.chars().count();
        if character_count > MAX_PROOFREADING_CHARACTERS {
            return Err(ProofreadingFailure::InputTooLong {
                maximum_characters: MAX_PROOFREADING_CHARACTERS,
                actual_characters: character_count,
            });
        }

        if consent != ProofreadingConsent::Granted {
            return Err(ProofreadingFailure::ConsentRequired);
        }

        let request = ProofreadingRequest::new(text);
        let response = self
            .proofreader
            .proofread(&request, cancellation)
            .await
            .map_err(|error| match error {
                ProofreaderError::Cancelled => ProofreadingFailure::Cancelled,
                error => ProofreadingFailure::Provider(error),
            })?;

        if cancellation.is_cancelled() {
            return Err(ProofreadingFailure::Cancelled);
        }

        match response {
            ProofreaderResponse::NoIssues => Ok(ProofreadingResult::NoIssues),
            ProofreaderResponse::Corrected(correction) => {
                if correction.corrected_text.trim().is_empty()
                    || correction.corrected_text == request.text
                {
                    return Err(ProofreadingFailure::InvalidResult);
                }

                Ok(ProofreadingResult::Corrected(correction))
            }
        }
    }
}

#[cfg(test)]
mod tests;
