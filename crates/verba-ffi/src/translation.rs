use std::{error::Error, fmt, sync::Arc};

use verba_core::{
    coordinator::CancellationToken,
    translation::{
        LanguageIdentifier, TranslationFailure, TranslationRequest, Translator, TranslatorResponse,
    },
};

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct NativeTranslationRequest {
    pub text: String,
    pub target_language_identifier: String,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct NativeTranslationResponse {
    pub source_language_identifier: String,
    pub translated_text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Error)]
pub enum NativeTranslationError {
    UnsupportedPair,
    UnableToIdentifyLanguage,
    Cancelled,
    Failed,
}

impl fmt::Display for NativeTranslationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnsupportedPair => "unsupported language pair",
            Self::UnableToIdentifyLanguage => "unable to identify source language",
            Self::Cancelled => "translation cancelled",
            Self::Failed => "translation failed",
        };
        formatter.write_str(message)
    }
}

impl Error for NativeTranslationError {}

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait NativeTranslator: Send + Sync {
    async fn translate(
        &self,
        request: NativeTranslationRequest,
    ) -> Result<NativeTranslationResponse, NativeTranslationError>;
}

pub(crate) struct ForeignTranslator {
    translator: Arc<dyn NativeTranslator>,
}

impl ForeignTranslator {
    pub(crate) fn new(translator: Arc<dyn NativeTranslator>) -> Self {
        Self { translator }
    }
}

#[async_trait::async_trait]
impl Translator for ForeignTranslator {
    async fn translate(
        &self,
        request: &TranslationRequest,
        _cancellation: &CancellationToken,
    ) -> Result<TranslatorResponse, TranslationFailure> {
        let response = self
            .translator
            .translate(NativeTranslationRequest {
                text: request.text().to_owned(),
                target_language_identifier: request.target_language().as_str().to_owned(),
            })
            .await
            .map_err(|error| match error {
                NativeTranslationError::UnsupportedPair => TranslationFailure::UnsupportedPair {
                    source_language: None,
                    target_language: request.target_language().clone(),
                },
                NativeTranslationError::Cancelled => TranslationFailure::Cancelled,
                NativeTranslationError::UnableToIdentifyLanguage
                | NativeTranslationError::Failed => TranslationFailure::Failed,
            })?;
        let source_language = LanguageIdentifier::new(response.source_language_identifier)
            .map_err(|_| TranslationFailure::InvalidResult)?;

        Ok(TranslatorResponse::new(
            source_language,
            response.translated_text,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use verba_core::translation::{TranslateText, TranslationSettings};

    use super::*;

    struct FakeNativeTranslator {
        result: Result<NativeTranslationResponse, NativeTranslationError>,
        requests: Mutex<Vec<NativeTranslationRequest>>,
    }

    #[async_trait::async_trait]
    impl NativeTranslator for FakeNativeTranslator {
        async fn translate(
            &self,
            request: NativeTranslationRequest,
        ) -> Result<NativeTranslationResponse, NativeTranslationError> {
            self.requests.lock().unwrap().push(request);
            self.result.clone()
        }
    }

    #[test]
    fn foreign_translator_converts_requests_and_responses() {
        let native = Arc::new(FakeNativeTranslator {
            result: Ok(NativeTranslationResponse {
                source_language_identifier: "de".to_owned(),
                translated_text: "Hello".to_owned(),
            }),
            requests: Mutex::new(Vec::new()),
        });
        let translator = Arc::new(ForeignTranslator::new(native.clone()));
        let use_case = TranslateText::new(translator);

        let response = futures::executor::block_on(use_case.execute(
            "Hallo",
            &TranslationSettings::default(),
            &CancellationToken::default(),
        ))
        .unwrap();

        assert_eq!(response.source_language().as_str(), "de");
        assert_eq!(response.translated_text(), "Hello");
        assert_eq!(
            native.requests.lock().unwrap().as_slice(),
            &[NativeTranslationRequest {
                text: "Hallo".to_owned(),
                target_language_identifier: "en".to_owned(),
            }]
        );
    }
}
