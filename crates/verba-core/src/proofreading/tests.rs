use std::sync::Arc;

use futures::executor::block_on;

use super::*;
use crate::testing::FakeProofreader;

#[test]
fn disclosure_acknowledgement_is_loaded_and_persisted_before_becoming_granted() {
    use std::sync::Mutex;

    struct MemoryConsentStore {
        acknowledged: bool,
        saves: Mutex<usize>,
        fail_save: bool,
    }

    impl ProofreadingConsentStore for MemoryConsentStore {
        fn load_acknowledged(&self) -> Result<bool, ProofreadingConsentStoreError> {
            Ok(self.acknowledged)
        }

        fn save_acknowledged(&self) -> Result<(), ProofreadingConsentStoreError> {
            if self.fail_save {
                return Err(ProofreadingConsentStoreError);
            }
            *self.saves.lock().unwrap() += 1;
            Ok(())
        }
    }

    let already_granted = Arc::new(MemoryConsentStore {
        acknowledged: true,
        saves: Mutex::new(0),
        fail_save: false,
    });
    let preferences = ProofreadingConsentPreferences::load(already_granted.clone()).unwrap();
    assert!(preferences.is_granted());
    preferences.grant().unwrap();
    assert_eq!(*already_granted.saves.lock().unwrap(), 0);

    let new_acknowledgement = Arc::new(MemoryConsentStore {
        acknowledged: false,
        saves: Mutex::new(0),
        fail_save: false,
    });
    let preferences = ProofreadingConsentPreferences::load(new_acknowledgement.clone()).unwrap();
    assert!(!preferences.is_granted());
    preferences.grant().unwrap();
    assert!(preferences.is_granted());
    assert_eq!(*new_acknowledgement.saves.lock().unwrap(), 1);

    let failing_store = Arc::new(MemoryConsentStore {
        acknowledged: false,
        saves: Mutex::new(0),
        fail_save: true,
    });
    let preferences = ProofreadingConsentPreferences::load(failing_store).unwrap();
    assert_eq!(preferences.grant(), Err(ProofreadingConsentStoreError));
    assert!(!preferences.is_granted());
}

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
    let correction = ProofreadingCorrection::new("This is correct.");
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
fn mechanical_policy_validation_preserves_unicode_whitespace_lines_and_markers() {
    let original = "\u{a0}\u{2003}> **This are wrong.**\r\n\r\n  1. Keep `code`.\u{202f}";
    let corrected = "\u{a0}\u{2003}> **This is wrong.**\r\n\r\n  1. Keep `code`.\u{202f}";

    let validation = evaluate_proofreading_policy(original, corrected);

    assert!(validation.outer_whitespace_preserved());
    assert!(validation.line_structure_preserved());
    assert!(validation.formatting_markers_preserved());
    assert_eq!(validation.first_violation(), None);
}

#[test]
fn mechanical_policy_validation_reports_each_enforced_invariant() {
    let cases = [
        (
            "\u{a0}This are wrong.\u{2003}",
            "This is wrong.\u{2003}",
            ProofreadingPolicyViolation::OuterWhitespace,
        ),
        (
            "First are wrong.\r\n\r\nSecond stays.",
            "First is wrong.\n\nSecond stays.",
            ProofreadingPolicyViolation::LineStructure,
        ),
        (
            "First are wrong.\n\nSecond stays.",
            "First is wrong.\nSecond stays.\nExtra text.",
            ProofreadingPolicyViolation::LineStructure,
        ),
        (
            "- **This are wrong.**",
            "* This is wrong.",
            ProofreadingPolicyViolation::FormattingMarkers,
        ),
        (
            "> Keep `code` here.",
            "Keep code here.",
            ProofreadingPolicyViolation::FormattingMarkers,
        ),
        (
            "```text\nThis are wrong.\n```",
            "````text\nThis is wrong.\n````",
            ProofreadingPolicyViolation::FormattingMarkers,
        ),
    ];

    for (original, corrected, expected) in cases {
        assert_eq!(
            evaluate_proofreading_policy(original, corrected).first_violation(),
            Some(expected)
        );
    }
}

#[test]
fn rejects_schema_valid_corrections_that_violate_mechanical_policy() {
    for (original, corrected, expected) in [
        (
            "  This are wrong.  ",
            "This is wrong.",
            ProofreadingPolicyViolation::OuterWhitespace,
        ),
        (
            "First are wrong.\n\nSecond stays.",
            "First is wrong.\nSecond stays.",
            ProofreadingPolicyViolation::LineStructure,
        ),
        (
            "- This item are wrong.",
            "This item is wrong.",
            ProofreadingPolicyViolation::FormattingMarkers,
        ),
    ] {
        let proofreader = Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::Corrected(
            ProofreadingCorrection::new(corrected),
        ))));

        assert_eq!(
            block_on(ProofreadText::new(proofreader).execute(
                original,
                ProofreadingConsent::Granted,
                &CancellationToken::default(),
            )),
            Err(ProofreadingFailure::PolicyViolation(expected))
        );
    }
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
fn character_boundaries_remain_inclusive_before_the_provider_call() {
    let proofreader = Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::NoIssues)));
    let use_case = ProofreadText::new(proofreader.clone());

    for character_count in [9_999, 10_000] {
        assert_eq!(
            block_on(use_case.execute(
                "a".repeat(character_count),
                ProofreadingConsent::Granted,
                &CancellationToken::default(),
            )),
            Ok(ProofreadingResult::NoIssues)
        );
    }
    assert_eq!(proofreader.requests().len(), 2);
}

#[test]
fn token_dense_input_is_rejected_before_network_use() {
    let proofreader = Arc::new(FakeProofreader::new(Ok(ProofreaderResponse::NoIssues)));
    let use_case = ProofreadText::new(proofreader.clone());
    let accepted = "🙂".repeat(2_500);
    let rejected = "🙂".repeat(2_501);

    assert_eq!(conservative_token_estimate(&accepted), 10_000);
    assert_eq!(
        block_on(use_case.execute(
            accepted,
            ProofreadingConsent::Granted,
            &CancellationToken::default(),
        )),
        Ok(ProofreadingResult::NoIssues)
    );
    assert_eq!(conservative_token_estimate(&rejected), 10_004);
    assert_eq!(
        block_on(use_case.execute(
            rejected,
            ProofreadingConsent::Granted,
            &CancellationToken::default(),
        )),
        Err(ProofreadingFailure::EstimatedInputTooLarge {
            maximum_estimated_tokens: 10_000,
            estimated_tokens: 10_004,
        })
    );
    assert_eq!(proofreader.requests().len(), 1);
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
    for correction in [
        ProofreadingCorrection::new(" \n"),
        ProofreadingCorrection::new("Original text"),
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
