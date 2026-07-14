use verba_core::presentation as core;

/// The operation associated with a Swift-facing presentation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum PresentationAction {
    Translate,
    Proofread,
}

/// The recovery control rendered for a Swift-facing error state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum RecoveryActionViewModel {
    Retry,
    OpenSettings,
    GrantAccessibility,
    ChangeLanguage,
    Dismiss,
}

/// An immutable source and target language pair for Swift.
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct LanguagePairViewModel {
    pub source: String,
    pub target: String,
}

/// An immutable snapshot of the presentation state consumed by Swift.
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum PresentationViewModel {
    Idle,
    Loading {
        action: PresentationAction,
    },
    ProofreadingDisclosure,
    Translation {
        original_text: String,
        language_pair: LanguagePairViewModel,
        translated_text: String,
    },
    Proofreading {
        original_text: String,
        corrected_text: String,
        explanation: String,
    },
    NoIssues,
    Error {
        action: Option<PresentationAction>,
        title: String,
        message: String,
        recovery: RecoveryActionViewModel,
        diagnostic_code: String,
    },
}

/// Returns the presentation state used when the application starts.
#[uniffi::export]
pub fn initial_presentation() -> PresentationViewModel {
    core::PresentationState::Idle.into()
}

impl From<core::TextAction> for PresentationAction {
    fn from(action: core::TextAction) -> Self {
        match action {
            core::TextAction::Translate => Self::Translate,
            core::TextAction::Proofread => Self::Proofread,
        }
    }
}

impl From<PresentationAction> for core::TextAction {
    fn from(action: PresentationAction) -> Self {
        match action {
            PresentationAction::Translate => Self::Translate,
            PresentationAction::Proofread => Self::Proofread,
        }
    }
}

impl From<core::RecoveryAction> for RecoveryActionViewModel {
    fn from(action: core::RecoveryAction) -> Self {
        match action {
            core::RecoveryAction::Retry => Self::Retry,
            core::RecoveryAction::OpenSettings => Self::OpenSettings,
            core::RecoveryAction::GrantAccessibility => Self::GrantAccessibility,
            core::RecoveryAction::ChangeLanguage => Self::ChangeLanguage,
            core::RecoveryAction::Dismiss => Self::Dismiss,
        }
    }
}

impl From<core::LanguagePair> for LanguagePairViewModel {
    fn from(pair: core::LanguagePair) -> Self {
        Self {
            source: pair.source,
            target: pair.target,
        }
    }
}

impl From<core::PresentationState> for PresentationViewModel {
    fn from(state: core::PresentationState) -> Self {
        match state {
            core::PresentationState::Idle => Self::Idle,
            core::PresentationState::Loading { action } => Self::Loading {
                action: action.into(),
            },
            core::PresentationState::ProofreadingDisclosure => Self::ProofreadingDisclosure,
            core::PresentationState::Translation(translation) => Self::Translation {
                original_text: translation.original_text,
                language_pair: translation.language_pair.into(),
                translated_text: translation.translated_text,
            },
            core::PresentationState::Proofreading(proofreading) => Self::Proofreading {
                original_text: proofreading.original_text,
                corrected_text: proofreading.corrected_text,
                explanation: proofreading.explanation,
            },
            core::PresentationState::NoIssues => Self::NoIssues,
            core::PresentationState::Error(error) => Self::Error {
                action: error.action.map(Into::into),
                title: error.title,
                message: error.message,
                recovery: error.recovery.into(),
                diagnostic_code: error.diagnostic_code,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LanguagePairViewModel, PresentationAction, PresentationViewModel, RecoveryActionViewModel,
    };
    use verba_core::presentation::{
        ErrorPresentation, LanguagePair, PresentationState, ProofreadingPresentation,
        RecoveryAction, TextAction, TranslationPresentation,
    };

    #[test]
    fn converts_every_core_presentation_variant() {
        let cases = [
            (PresentationState::Idle, PresentationViewModel::Idle),
            (
                PresentationState::Loading {
                    action: TextAction::Translate,
                },
                PresentationViewModel::Loading {
                    action: PresentationAction::Translate,
                },
            ),
            (
                PresentationState::ProofreadingDisclosure,
                PresentationViewModel::ProofreadingDisclosure,
            ),
            (
                PresentationState::Translation(TranslationPresentation {
                    original_text: "Hallo".to_owned(),
                    language_pair: LanguagePair {
                        source: "German".to_owned(),
                        target: "English".to_owned(),
                    },
                    translated_text: "Hello".to_owned(),
                }),
                PresentationViewModel::Translation {
                    original_text: "Hallo".to_owned(),
                    language_pair: LanguagePairViewModel {
                        source: "German".to_owned(),
                        target: "English".to_owned(),
                    },
                    translated_text: "Hello".to_owned(),
                },
            ),
            (
                PresentationState::Proofreading(ProofreadingPresentation {
                    original_text: "This correct.".to_owned(),
                    corrected_text: "This is correct.".to_owned(),
                    explanation: "Added the missing verb.".to_owned(),
                }),
                PresentationViewModel::Proofreading {
                    original_text: "This correct.".to_owned(),
                    corrected_text: "This is correct.".to_owned(),
                    explanation: "Added the missing verb.".to_owned(),
                },
            ),
            (PresentationState::NoIssues, PresentationViewModel::NoIssues),
            (
                PresentationState::Error(ErrorPresentation {
                    action: Some(TextAction::Proofread),
                    title: "Proofreading failed".to_owned(),
                    message: "Try again.".to_owned(),
                    recovery: RecoveryAction::Retry,
                    diagnostic_code: "proofreading.failed".to_owned(),
                }),
                PresentationViewModel::Error {
                    action: Some(PresentationAction::Proofread),
                    title: "Proofreading failed".to_owned(),
                    message: "Try again.".to_owned(),
                    recovery: RecoveryActionViewModel::Retry,
                    diagnostic_code: "proofreading.failed".to_owned(),
                },
            ),
        ];

        for (core_state, expected_view_model) in cases {
            assert_eq!(PresentationViewModel::from(core_state), expected_view_model);
        }
    }

    #[test]
    fn initial_presentation_is_idle() {
        assert_eq!(super::initial_presentation(), PresentationViewModel::Idle);
    }
}
