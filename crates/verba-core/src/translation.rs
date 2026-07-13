use std::sync::Arc;

use crate::coordinator::CancellationToken;

pub const MAX_TRANSLATION_CHARACTERS: usize = 10_000;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LanguageIdentifier(String);

impl LanguageIdentifier {
    pub fn new(identifier: impl AsRef<str>) -> Result<Self, LanguageIdentifierError> {
        let identifier = identifier.as_ref().trim();
        let mut subtags = identifier.split('-');
        let Some(language) = subtags.next() else {
            return Err(LanguageIdentifierError::Invalid);
        };

        if !(2..=8).contains(&language.len())
            || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
            || !subtags.all(|subtag| {
                (1..=8).contains(&subtag.len())
                    && subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
        {
            return Err(LanguageIdentifierError::Invalid);
        }

        Ok(Self(identifier.to_ascii_lowercase()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanguageIdentifierError {
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationSettings {
    target_language: LanguageIdentifier,
}

impl TranslationSettings {
    #[must_use]
    pub const fn new(target_language: LanguageIdentifier) -> Self {
        Self { target_language }
    }

    #[must_use]
    pub const fn target_language(&self) -> &LanguageIdentifier {
        &self.target_language
    }

    pub fn set_target_language(&mut self, target_language: LanguageIdentifier) {
        self.target_language = target_language;
    }
}

impl Default for TranslationSettings {
    fn default() -> Self {
        Self::new(LanguageIdentifier("en".to_owned()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationRequest {
    text: String,
    target_language: LanguageIdentifier,
}

impl TranslationRequest {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn target_language(&self) -> &LanguageIdentifier {
        &self.target_language
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslatorResponse {
    source_language: LanguageIdentifier,
    translated_text: String,
}

impl TranslatorResponse {
    #[must_use]
    pub fn new(source_language: LanguageIdentifier, translated_text: impl Into<String>) -> Self {
        Self {
            source_language,
            translated_text: translated_text.into(),
        }
    }

    #[must_use]
    pub const fn source_language(&self) -> &LanguageIdentifier {
        &self.source_language
    }

    #[must_use]
    pub fn translated_text(&self) -> &str {
        &self.translated_text
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationResult {
    original_text: String,
    source_language: LanguageIdentifier,
    target_language: LanguageIdentifier,
    translated_text: String,
}

impl TranslationResult {
    #[must_use]
    pub fn original_text(&self) -> &str {
        &self.original_text
    }

    #[must_use]
    pub const fn source_language(&self) -> &LanguageIdentifier {
        &self.source_language
    }

    #[must_use]
    pub const fn target_language(&self) -> &LanguageIdentifier {
        &self.target_language
    }

    #[must_use]
    pub fn translated_text(&self) -> &str {
        &self.translated_text
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranslationFailure {
    EmptyInput,
    InputTooLong {
        maximum_characters: usize,
        actual_characters: usize,
    },
    SameLanguage {
        language: LanguageIdentifier,
    },
    UnsupportedPair {
        source_language: Option<LanguageIdentifier>,
        target_language: LanguageIdentifier,
    },
    Cancelled,
    InvalidResult,
    Failed,
}

pub trait Translator: Send + Sync {
    fn translate(
        &self,
        request: &TranslationRequest,
        cancellation: &CancellationToken,
    ) -> Result<TranslatorResponse, TranslationFailure>;
}

pub struct TranslateText {
    translator: Arc<dyn Translator>,
}

impl TranslateText {
    #[must_use]
    pub fn new(translator: Arc<dyn Translator>) -> Self {
        Self { translator }
    }

    pub fn execute(
        &self,
        text: impl Into<String>,
        settings: &TranslationSettings,
        cancellation: &CancellationToken,
    ) -> Result<TranslationResult, TranslationFailure> {
        if cancellation.is_cancelled() {
            return Err(TranslationFailure::Cancelled);
        }

        let text = text.into();
        if text.trim().is_empty() {
            return Err(TranslationFailure::EmptyInput);
        }

        let character_count = text.chars().count();
        if character_count > MAX_TRANSLATION_CHARACTERS {
            return Err(TranslationFailure::InputTooLong {
                maximum_characters: MAX_TRANSLATION_CHARACTERS,
                actual_characters: character_count,
            });
        }

        let request = TranslationRequest {
            text,
            target_language: settings.target_language.clone(),
        };
        let response = self.translator.translate(&request, cancellation)?;

        if cancellation.is_cancelled() {
            return Err(TranslationFailure::Cancelled);
        }
        if response.source_language == request.target_language {
            return Err(TranslationFailure::SameLanguage {
                language: response.source_language,
            });
        }
        if response.translated_text.trim().is_empty() {
            return Err(TranslationFailure::InvalidResult);
        }

        Ok(TranslationResult {
            original_text: request.text,
            source_language: response.source_language,
            target_language: request.target_language,
            translated_text: response.translated_text,
        })
    }
}

#[cfg(test)]
mod tests;
