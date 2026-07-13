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
    translation::{TranslateText, TranslationFailure, TranslationSettings},
};

use crate::translation::{ForeignTranslator, NativeTranslator};

pub(crate) struct ApplicationProcessor {
    translation: TranslateText,
    translation_settings: TranslationSettings,
}

impl ApplicationProcessor {
    pub(crate) fn new(translator: Arc<dyn NativeTranslator>) -> Self {
        Self {
            translation: TranslateText::new(Arc::new(ForeignTranslator::new(translator))),
            translation_settings: TranslationSettings::default(),
        }
    }

    fn translate(
        &self,
        text: String,
        cancellation: &CancellationToken,
    ) -> Result<ProcessingOutcome, ProcessingFailure> {
        let result = block_on(async {
            let translation = Box::pin(self.translation.execute(
                text,
                &self.translation_settings,
                cancellation,
            ));
            let cancelled = Box::pin(cancellation.cancelled());

            match select(translation, cancelled).await {
                Either::Left((result, _)) => result,
                Either::Right(((), _)) => Err(TranslationFailure::Cancelled),
            }
        })
        .map_err(processing_failure)?;

        Ok(ProcessingOutcome::Translation(TranslationPresentation {
            original_text: result.original_text().to_owned(),
            language_pair: LanguagePair {
                source: result.source_language().as_str().to_owned(),
                target: result.target_language().as_str().to_owned(),
            },
            translated_text: result.translated_text().to_owned(),
        }))
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
            TextAction::Proofread => Ok(proofreading_preview(text)),
        }
    }
}

fn processing_failure(failure: TranslationFailure) -> ProcessingFailure {
    match failure {
        TranslationFailure::Cancelled => ProcessingFailure::Cancelled,
        TranslationFailure::InvalidResult => ProcessingFailure::InvalidOutput,
        TranslationFailure::EmptyInput
        | TranslationFailure::InputTooLong { .. }
        | TranslationFailure::SameLanguage { .. }
        | TranslationFailure::UnsupportedPair { .. }
        | TranslationFailure::Failed => ProcessingFailure::Failed,
    }
}

fn proofreading_preview(text: String) -> ProcessingOutcome {
    ProcessingOutcome::Proofreading(ProofreadingPresentation {
        corrected_text: text,
        explanation: "Proofreading preview".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::{NativeTranslationError, NativeTranslationRequest, NativeTranslationResponse};

    struct FakeNativeTranslator {
        requests: Mutex<Vec<NativeTranslationRequest>>,
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
        let processor = ApplicationProcessor::new(translator.clone());

        let outcome = processor
            .translate("Hallo".to_owned(), &CancellationToken::default())
            .unwrap();

        assert_eq!(
            outcome,
            ProcessingOutcome::Translation(TranslationPresentation {
                original_text: "Hallo".to_owned(),
                language_pair: LanguagePair {
                    source: "de".to_owned(),
                    target: "en".to_owned(),
                },
                translated_text: "Hello".to_owned(),
            })
        );
        assert_eq!(
            translator.requests.lock().unwrap().as_slice(),
            &[NativeTranslationRequest {
                text: "Hallo".to_owned(),
                target_language_identifier: "en".to_owned(),
            }]
        );
    }

    #[test]
    fn proofreading_keeps_its_preview_processor() {
        assert!(matches!(
            proofreading_preview("Text".to_owned()),
            ProcessingOutcome::Proofreading(_)
        ));
    }
}
