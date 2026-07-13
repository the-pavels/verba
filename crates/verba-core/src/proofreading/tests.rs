use std::sync::Arc;

use futures::executor::block_on;

use super::*;
use crate::testing::FakeProofreader;

#[test]
fn strict_policy_preserves_every_proofreading_invariant() {
    let policy = ProofreadingPolicy::strict();

    assert_eq!(policy.scope(), ProofreadingScope::SpellingAndGrammarOnly);
    assert!(policy.preserves_language());
    assert!(policy.preserves_tone());
    assert!(policy.preserves_whitespace());
    assert!(policy.preserves_formatting());
}

#[test]
fn returns_a_validated_correction_and_records_the_strict_request() {
    let correction = ProofreadingCorrection::new("This is correct.", "Fixed the verb form.");
    let proofreader = Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::Corrected(
        correction.clone(),
    ))));
    let use_case = ProofreadText::new(proofreader.clone());

    let result = block_on(use_case.execute(
        "This are correct.",
        ProofreadingConsent::Granted,
        &CancellationToken::default(),
    ));

    assert_eq!(result, Ok(ProofreadingResult::Corrected(correction)));
    let requests = proofreader.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].text(), "This are correct.");
    assert_eq!(requests[0].policy(), ProofreadingPolicy::strict());
}

#[test]
fn keeps_no_issues_separate_from_corrected_text() {
    let proofreader = Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::NoIssues)));
    let use_case = ProofreadText::new(proofreader);

    assert_eq!(
        block_on(use_case.execute(
            "This is correct.",
            ProofreadingConsent::Granted,
            &CancellationToken::default(),
        )),
        Ok(ProofreadingResult::NoIssues)
    );
}

#[test]
fn requires_consent_without_exposing_text_to_the_provider() {
    let proofreader = Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::NoIssues)));
    let use_case = ProofreadText::new(proofreader.clone());

    assert_eq!(
        block_on(use_case.execute(
            "Private selected text",
            ProofreadingConsent::NotGranted,
            &CancellationToken::default(),
        )),
        Err(ProofreadingFailure::ConsentRequired)
    );
    assert!(proofreader.requests().is_empty());
}

#[test]
fn rejects_empty_and_oversized_input_without_calling_the_provider() {
    let proofreader = Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::NoIssues)));
    let use_case = ProofreadText::new(proofreader.clone());

    for text in ["", " \n\t"] {
        assert_eq!(
            block_on(use_case.execute(
                text,
                ProofreadingConsent::Granted,
                &CancellationToken::default(),
            )),
            Err(ProofreadingFailure::EmptyInput)
        );
    }

    let oversized = "é".repeat(MAX_PROOFREADING_CHARACTERS + 1);
    assert_eq!(
        block_on(use_case.execute(
            oversized,
            ProofreadingConsent::Granted,
            &CancellationToken::default(),
        )),
        Err(ProofreadingFailure::InputTooLong {
            maximum_characters: MAX_PROOFREADING_CHARACTERS,
            actual_characters: MAX_PROOFREADING_CHARACTERS + 1,
        })
    );
    assert!(proofreader.requests().is_empty());
}

#[test]
fn skips_the_provider_when_the_request_is_already_cancelled() {
    let proofreader = Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::NoIssues)));
    let use_case = ProofreadText::new(proofreader.clone());
    let cancellation = CancellationToken::default();
    cancellation.cancel();

    assert_eq!(
        block_on(use_case.execute("Selected text", ProofreadingConsent::Granted, &cancellation,)),
        Err(ProofreadingFailure::Cancelled)
    );
    assert!(proofreader.requests().is_empty());
}

#[test]
fn reports_cancellation_that_happens_during_the_provider_call() {
    let proofreader = Arc::new(CancellingProofreader);
    let use_case = ProofreadText::new(proofreader);

    assert_eq!(
        block_on(use_case.execute(
            "Selected text",
            ProofreadingConsent::Granted,
            &CancellationToken::default(),
        )),
        Err(ProofreadingFailure::Cancelled)
    );
}

#[test]
fn maps_provider_cancellation_and_preserves_typed_provider_errors() {
    for (error, expected) in [
        (ProofreaderError::Cancelled, ProofreadingFailure::Cancelled),
        (
            ProofreaderError::Authentication,
            ProofreadingFailure::Provider(ProofreaderError::Authentication),
        ),
        (
            ProofreaderError::RateLimited,
            ProofreadingFailure::Provider(ProofreaderError::RateLimited),
        ),
        (
            ProofreaderError::QuotaExceeded,
            ProofreadingFailure::Provider(ProofreaderError::QuotaExceeded),
        ),
    ] {
        let proofreader = Arc::new(FakeProofreader::new(Err(error)));
        let use_case = ProofreadText::new(proofreader);

        assert_eq!(
            block_on(use_case.execute(
                "Selected text",
                ProofreadingConsent::Granted,
                &CancellationToken::default(),
            )),
            Err(expected)
        );
    }
}

#[test]
fn rejects_invalid_corrections() {
    let too_long = "x".repeat(MAX_PROOFREADING_EXPLANATION_CHARACTERS + 1);

    for correction in [
        ProofreadingCorrection::new(" \n", "Fixed an error."),
        ProofreadingCorrection::new("Original text", "Fixed an error."),
        ProofreadingCorrection::new("Corrected text", " \n"),
        ProofreadingCorrection::new("Corrected text", too_long),
    ] {
        let proofreader = Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::Corrected(
            correction,
        ))));
        let use_case = ProofreadText::new(proofreader);

        assert_eq!(
            block_on(use_case.execute(
                "Original text",
                ProofreadingConsent::Granted,
                &CancellationToken::default(),
            )),
            Err(ProofreadingFailure::InvalidResult)
        );
    }
}

struct CancellingProofreader;

#[async_trait::async_trait]
impl Proofreader for CancellingProofreader {
    async fn proofread(
        &self,
        _request: &ProofreadingRequest,
        cancellation: &CancellationToken,
    ) -> Result<ProofreaderResponse, ProofreaderError> {
        cancellation.cancel();
        Ok(ProofreaderResponse::NoIssues)
    }
}
