use std::sync::Arc;

use futures::{
    executor::block_on,
    future::{Either, select},
};
use verba_core::{
    coordinator::{
        CancellationToken, ProcessingFailure, ProcessingOutcome, ProcessingRequest,
        TextActionProcessor,
    },
    presentation::{LanguagePair, ProofreadingPresentation, TextAction, TranslationPresentation},
    proofreading::{
        ProofreadText, Proofreader, ProofreadingConsent, ProofreadingFailure, ProofreadingResult,
    },
    translation::{TranslateText, TranslationFailure, TranslationPreferences},
};

use crate::translation::{ForeignTranslator, NativeTranslator};

pub(crate) struct ApplicationProcessor {
    translation: TranslateText,
    translation_preferences: Arc<TranslationPreferences>,
    proofreading: ProofreadText,
}

impl ApplicationProcessor {
    pub(crate) fn new(
        translator: Arc<dyn NativeTranslator>,
        translation_preferences: Arc<TranslationPreferences>,
        proofreader: Arc<dyn Proofreader>,
    ) -> Self {
        Self {
            translation: TranslateText::new(Arc::new(ForeignTranslator::new(translator))),
            translation_preferences,
            proofreading: ProofreadText::new(proofreader),
        }
    }

    fn translate(
        &self,
        text: String,
        cancellation: &CancellationToken,
    ) -> Result<ProcessingOutcome, ProcessingFailure> {
        let settings = self.translation_preferences.settings();
        let result = block_on(async {
            let translation = Box::pin(self.translation.execute(text, &settings, cancellation));
            let cancelled = Box::pin(cancellation.cancelled());

            match select(translation, cancelled).await {
                Either::Left((result, _)) => result,
                Either::Right(((), _)) => Err(TranslationFailure::Cancelled),
            }
        })
        .map_err(translation_processing_failure)?;

        Ok(ProcessingOutcome::Translation(TranslationPresentation {
            original_text: result.original_text().to_owned(),
            language_pair: LanguagePair {
                source: result.source_language().as_str().to_owned(),
                target: result.target_language().as_str().to_owned(),
            },
            translated_text: result.translated_text().to_owned(),
        }))
    }

    fn proofread(
        &self,
        text: String,
        cancellation: &CancellationToken,
    ) -> Result<ProcessingOutcome, ProcessingFailure> {
        let original_text = text.clone();
        let result = block_on(async {
            let proofreading = Box::pin(self.proofreading.execute(
                text,
                ProofreadingConsent::Granted,
                cancellation,
            ));
            let cancelled = Box::pin(cancellation.cancelled());

            match select(proofreading, cancelled).await {
                Either::Left((result, _)) => result,
                Either::Right(((), _)) => Err(ProofreadingFailure::Cancelled),
            }
        })
        .map_err(proofreading_processing_failure)?;

        Ok(match result {
            ProofreadingResult::NoIssues => ProcessingOutcome::NoIssues,
            ProofreadingResult::Corrected(correction) => {
                ProcessingOutcome::Proofreading(ProofreadingPresentation {
                    original_text,
                    corrected_text: correction.corrected_text().to_owned(),
                    explanation: correction.explanation().to_owned(),
                })
            }
        })
    }
}

impl TextActionProcessor for ApplicationProcessor {
    fn process(
        &self,
        request: ProcessingRequest,
        cancellation: &CancellationToken,
    ) -> Result<ProcessingOutcome, ProcessingFailure> {
        if cancellation.is_cancelled() {
            return Err(ProcessingFailure::Cancelled);
        }

        let text = request.text.into_string();
        match request.action {
            TextAction::Translate => self.translate(text, cancellation),
            TextAction::Proofread => self.proofread(text, cancellation),
        }
    }
}

fn translation_processing_failure(failure: TranslationFailure) -> ProcessingFailure {
    match failure {
        TranslationFailure::Cancelled => ProcessingFailure::Cancelled,
        TranslationFailure::InvalidResult => ProcessingFailure::InvalidOutput,
        TranslationFailure::EmptyInput => ProcessingFailure::EmptyInput,
        TranslationFailure::InputTooLong { .. } => ProcessingFailure::InputTooLong,
        TranslationFailure::SameLanguage { .. } => ProcessingFailure::SameLanguage,
        TranslationFailure::Failed => ProcessingFailure::Failed,
        TranslationFailure::UnsupportedPair { .. } => ProcessingFailure::UnsupportedConfiguration,
    }
}

fn proofreading_processing_failure(failure: ProofreadingFailure) -> ProcessingFailure {
    match failure {
        ProofreadingFailure::Cancelled => ProcessingFailure::Cancelled,
        ProofreadingFailure::InvalidResult => ProcessingFailure::InvalidOutput,
        ProofreadingFailure::ConsentRequired => ProcessingFailure::UnsupportedConfiguration,
        ProofreadingFailure::InputTooLong { .. } => ProcessingFailure::InputTooLong,
        ProofreadingFailure::Provider(error) => ProcessingFailure::ProofreadingProvider(error),
        ProofreadingFailure::EmptyInput => ProcessingFailure::EmptyInput,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::{NativeTranslationError, NativeTranslationRequest, NativeTranslationResponse};
    use verba_core::proofreading::{
        ProofreaderError, ProofreaderResponse, ProofreadingCorrection, ProofreadingRequest,
    };
    use verba_core::translation::{
        LanguageIdentifier, TranslationSettingsStore, TranslationSettingsStoreError,
    };

    struct FakeNativeTranslator {
        requests: Mutex<Vec<NativeTranslationRequest>>,
    }

    struct MemorySettingsStore;

    struct FakeProofreader {
        result: Mutex<Option<Result<ProofreaderResponse, ProofreaderError>>>,
        requests: Mutex<Vec<String>>,
    }

    impl FakeProofreader {
        fn new(result: Result<ProofreaderResponse, ProofreaderError>) -> Self {
            Self {
                result: Mutex::new(Some(result)),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl Proofreader for FakeProofreader {
        async fn proofread(
            &self,
            request: &ProofreadingRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ProofreaderResponse, ProofreaderError> {
            self.requests
                .lock()
                .unwrap()
                .push(request.text().to_owned());
            self.result
                .lock()
                .unwrap()
                .take()
                .expect("proofreader should be called once")
        }
    }

    impl TranslationSettingsStore for MemorySettingsStore {
        fn load_target_language(
            &self,
        ) -> Result<Option<LanguageIdentifier>, TranslationSettingsStoreError> {
            Ok(None)
        }

        fn save_target_language(
            &self,
            _target_language: &LanguageIdentifier,
        ) -> Result<(), TranslationSettingsStoreError> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl NativeTranslator for FakeNativeTranslator {
        async fn translate(
            &self,
            request: NativeTranslationRequest,
        ) -> Result<NativeTranslationResponse, NativeTranslationError> {
            self.requests.lock().unwrap().push(request);
            Ok(NativeTranslationResponse {
                source_language_identifier: "de".to_owned(),
                translated_text: "Hello".to_owned(),
            })
        }
    }

    #[test]
    fn translates_with_the_native_adapter() {
        let translator = Arc::new(FakeNativeTranslator {
            requests: Mutex::new(Vec::new()),
        });
        let preferences =
            Arc::new(TranslationPreferences::load(Arc::new(MemorySettingsStore)).unwrap());
        let processor = ApplicationProcessor::new(
            translator.clone(),
            preferences.clone(),
            Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::NoIssues))),
        );

        let first_outcome = processor
            .translate("Hallo".to_owned(), &CancellationToken::default())
            .unwrap();

        preferences
            .set_supported_targets([language("en"), language("fr")])
            .unwrap();
        preferences.set_target_language(language("fr")).unwrap();
        let second_outcome = processor
            .translate("Hallo".to_owned(), &CancellationToken::default())
            .unwrap();

        assert_eq!(
            first_outcome,
            ProcessingOutcome::Translation(TranslationPresentation {
                original_text: "Hallo".to_owned(),
                language_pair: LanguagePair {
                    source: "de".to_owned(),
                    target: "en".to_owned(),
                },
                translated_text: "Hello".to_owned(),
            })
        );
        assert!(matches!(
            second_outcome,
            ProcessingOutcome::Translation(TranslationPresentation {
                language_pair: LanguagePair { ref target, .. },
                ..
            }) if target == "fr"
        ));
        assert_eq!(
            translator.requests.lock().unwrap().as_slice(),
            &[
                NativeTranslationRequest {
                    text: "Hallo".to_owned(),
                    target_language_identifier: "en".to_owned(),
                },
                NativeTranslationRequest {
                    text: "Hallo".to_owned(),
                    target_language_identifier: "fr".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn proofreading_renders_corrected_and_no_issues_outcomes() {
        let correction =
            ProofreadingCorrection::new("This is correct.", "Fixed subject-verb agreement.");
        let proofreader = Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::Corrected(
            correction,
        ))));
        let processor = test_processor(proofreader.clone());

        assert_eq!(
            processor.proofread(
                "This are correct.".to_owned(),
                &CancellationToken::default()
            ),
            Ok(ProcessingOutcome::Proofreading(ProofreadingPresentation {
                original_text: "This are correct.".to_owned(),
                corrected_text: "This is correct.".to_owned(),
                explanation: "Fixed subject-verb agreement.".to_owned(),
            }))
        );
        assert_eq!(
            proofreader.requests.lock().unwrap().as_slice(),
            ["This are correct."]
        );

        let processor = test_processor(Arc::new(FakeProofreader::new(Ok(
            ProofreaderResponse::NoIssues,
        ))));
        assert_eq!(
            processor.proofread("Looks good.".to_owned(), &CancellationToken::default()),
            Ok(ProcessingOutcome::NoIssues)
        );
    }

    #[test]
    fn proofreading_preserves_typed_provider_failures_for_user_recovery() {
        for error in [
            ProofreaderError::MissingCredential,
            ProofreaderError::Authentication,
            ProofreaderError::RateLimited,
            ProofreaderError::QuotaExceeded,
            ProofreaderError::Offline,
            ProofreaderError::TimedOut,
            ProofreaderError::Refused,
            ProofreaderError::Incomplete,
            ProofreaderError::MalformedResponse,
            ProofreaderError::ServiceUnavailable,
            ProofreaderError::Failed,
        ] {
            let processor = test_processor(Arc::new(FakeProofreader::new(Err(error))));
            assert_eq!(
                processor.proofread("Text".to_owned(), &CancellationToken::default()),
                Err(ProcessingFailure::ProofreadingProvider(error))
            );
        }
    }

    #[test]
    fn unsupported_pairs_point_to_configuration() {
        assert_eq!(
            translation_processing_failure(TranslationFailure::UnsupportedPair {
                source_language: Some(language("de")),
                target_language: language("ga"),
            }),
            ProcessingFailure::UnsupportedConfiguration
        );
    }

    #[test]
    fn preserves_recoverable_translation_failure_categories() {
        let cases = [
            (
                TranslationFailure::EmptyInput,
                ProcessingFailure::EmptyInput,
            ),
            (
                TranslationFailure::InputTooLong {
                    maximum_characters: 10_000,
                    actual_characters: 10_001,
                },
                ProcessingFailure::InputTooLong,
            ),
            (
                TranslationFailure::SameLanguage {
                    language: language("de"),
                },
                ProcessingFailure::SameLanguage,
            ),
            (
                TranslationFailure::InvalidResult,
                ProcessingFailure::InvalidOutput,
            ),
            (TranslationFailure::Failed, ProcessingFailure::Failed),
            (TranslationFailure::Cancelled, ProcessingFailure::Cancelled),
        ];

        for (failure, expected) in cases {
            assert_eq!(translation_processing_failure(failure), expected);
        }
    }

    #[test]
    fn preserves_recoverable_proofreading_failure_categories() {
        let cases = [
            (
                ProofreadingFailure::EmptyInput,
                ProcessingFailure::EmptyInput,
            ),
            (
                ProofreadingFailure::InputTooLong {
                    maximum_characters: 10_000,
                    actual_characters: 10_001,
                },
                ProcessingFailure::InputTooLong,
            ),
            (
                ProofreadingFailure::InvalidResult,
                ProcessingFailure::InvalidOutput,
            ),
            (
                ProofreadingFailure::ConsentRequired,
                ProcessingFailure::UnsupportedConfiguration,
            ),
            (ProofreadingFailure::Cancelled, ProcessingFailure::Cancelled),
        ];

        for (failure, expected) in cases {
            assert_eq!(proofreading_processing_failure(failure), expected);
        }
    }

    fn language(identifier: &str) -> LanguageIdentifier {
        LanguageIdentifier::new(identifier).unwrap()
    }

    fn test_processor(proofreader: Arc<dyn Proofreader>) -> ApplicationProcessor {
        let preferences =
            Arc::new(TranslationPreferences::load(Arc::new(MemorySettingsStore)).unwrap());
        ApplicationProcessor::new(
            Arc::new(FakeNativeTranslator {
                requests: Mutex::new(Vec::new()),
            }),
            preferences,
            proofreader,
        )
    }
}
