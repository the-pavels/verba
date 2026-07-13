use std::sync::Arc;

use super::*;
use crate::testing::FakeTranslator;

#[test]
fn language_identifiers_are_normalized_and_validated() {
    assert_eq!(language(" ZH-Hans ").as_str(), "zh-hans");

    for invalid in ["", "e", "en_US", "en-", "12", "languagecode"] {
        assert_eq!(
            LanguageIdentifier::new(invalid),
            Err(LanguageIdentifierError::Invalid)
        );
    }
}

#[test]
fn target_language_settings_default_to_english_and_can_change() {
    let mut settings = TranslationSettings::default();
    assert_eq!(settings.target_language(), &language("en"));

    settings.set_target_language(language("de"));
    assert_eq!(settings.target_language(), &language("de"));
}

#[test]
fn translates_valid_text_and_preserves_the_request_context() {
    let translator = Arc::new(FakeTranslator::new(Ok(response("de", "Hello"))));
    let use_case = TranslateText::new(translator.clone());
    let settings = TranslationSettings::default();

    let result = use_case
        .execute(" Hallo\n", &settings, &CancellationToken::default())
        .unwrap();

    assert_eq!(result.original_text(), " Hallo\n");
    assert_eq!(result.source_language(), &language("de"));
    assert_eq!(result.target_language(), &language("en"));
    assert_eq!(result.translated_text(), "Hello");
    assert_eq!(
        translator.requests(),
        vec![TranslationRequest {
            text: " Hallo\n".to_owned(),
            target_language: language("en"),
        }]
    );
}

#[test]
fn rejects_empty_input_without_calling_the_translator() {
    let translator = Arc::new(FakeTranslator::new(Ok(response("de", "unused"))));
    let use_case = TranslateText::new(translator.clone());

    for text in ["", "  \n\t"] {
        assert_eq!(
            use_case.execute(
                text,
                &TranslationSettings::default(),
                &CancellationToken::default()
            ),
            Err(TranslationFailure::EmptyInput)
        );
    }

    assert!(translator.requests().is_empty());
}

#[test]
fn rejects_oversized_input_by_character_count() {
    let translator = Arc::new(FakeTranslator::new(Ok(response("de", "unused"))));
    let use_case = TranslateText::new(translator.clone());
    let text = "é".repeat(MAX_TRANSLATION_CHARACTERS + 1);

    assert_eq!(
        use_case.execute(
            text,
            &TranslationSettings::default(),
            &CancellationToken::default()
        ),
        Err(TranslationFailure::InputTooLong {
            maximum_characters: MAX_TRANSLATION_CHARACTERS,
            actual_characters: MAX_TRANSLATION_CHARACTERS + 1,
        })
    );
    assert!(translator.requests().is_empty());
}

#[test]
fn reports_same_language_results() {
    let translator = Arc::new(FakeTranslator::new(Ok(response("en", "Hello"))));
    let use_case = TranslateText::new(translator);

    assert_eq!(
        use_case.execute(
            "Hello",
            &TranslationSettings::default(),
            &CancellationToken::default()
        ),
        Err(TranslationFailure::SameLanguage {
            language: language("en"),
        })
    );
}

#[test]
fn skips_translation_when_the_request_is_already_cancelled() {
    let translator = Arc::new(FakeTranslator::new(Ok(response("de", "unused"))));
    let use_case = TranslateText::new(translator.clone());
    let cancellation = CancellationToken::default();
    cancellation.cancel();

    assert_eq!(
        use_case.execute("Hallo", &TranslationSettings::default(), &cancellation),
        Err(TranslationFailure::Cancelled)
    );
    assert!(translator.requests().is_empty());
}

#[test]
fn preserves_unsupported_pair_details_from_the_translator() {
    let failure = TranslationFailure::UnsupportedPair {
        source_language: Some(language("ga")),
        target_language: language("en"),
    };
    let translator = Arc::new(FakeTranslator::new(Err(failure.clone())));
    let use_case = TranslateText::new(translator);

    assert_eq!(
        use_case.execute(
            "Dia dhuit",
            &TranslationSettings::default(),
            &CancellationToken::default()
        ),
        Err(failure)
    );
}

#[test]
fn rejects_an_empty_translator_result() {
    let translator = Arc::new(FakeTranslator::new(Ok(response("de", " \n"))));
    let use_case = TranslateText::new(translator);

    assert_eq!(
        use_case.execute(
            "Hallo",
            &TranslationSettings::default(),
            &CancellationToken::default()
        ),
        Err(TranslationFailure::InvalidResult)
    );
}

fn language(identifier: &str) -> LanguageIdentifier {
    LanguageIdentifier::new(identifier).unwrap()
}

fn response(source_language: &str, translated_text: &str) -> TranslatorResponse {
    TranslatorResponse::new(language(source_language), translated_text)
}
