/// The user-requested operation associated with a presentation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextAction {
    Translate,
    Proofread,
}

/// Source and target language labels displayed with a translation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguagePair {
    pub source: String,
    pub target: String,
}

/// Content displayed after a successful translation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationPresentation {
    pub original_text: String,
    pub language_pair: LanguagePair,
    pub translated_text: String,
}

/// Content displayed when proofreading produces a correction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofreadingPresentation {
    pub corrected_text: String,
    pub explanation: String,
}

/// User-facing information for a failed operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorPresentation {
    pub action: Option<TextAction>,
    pub title: String,
    pub message: String,
}

/// The complete presentation state owned by the Rust application layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentationState {
    Idle,
    Loading { action: TextAction },
    ProofreadingDisclosure,
    Translation(TranslationPresentation),
    Proofreading(ProofreadingPresentation),
    NoIssues,
    Error(ErrorPresentation),
}

impl PresentationState {
    /// Returns whether the popup should be visible for this state.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        !matches!(self, Self::Idle)
    }

    /// Returns the operation responsible for this state, when one is known.
    #[must_use]
    pub fn action(&self) -> Option<TextAction> {
        match self {
            Self::Idle => None,
            Self::Loading { action } => Some(*action),
            Self::Translation(_) => Some(TextAction::Translate),
            Self::ProofreadingDisclosure | Self::Proofreading(_) | Self::NoIssues => {
                Some(TextAction::Proofread)
            }
            Self::Error(error) => error.action,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ErrorPresentation, LanguagePair, PresentationState, ProofreadingPresentation, TextAction,
        TranslationPresentation,
    };

    #[test]
    fn idle_is_the_only_hidden_state() {
        assert!(!PresentationState::Idle.is_visible());

        let visible_states = [
            PresentationState::Loading {
                action: TextAction::Translate,
            },
            PresentationState::ProofreadingDisclosure,
            translation_state(),
            proofreading_state(),
            PresentationState::NoIssues,
            PresentationState::Error(ErrorPresentation {
                action: None,
                title: "Something went wrong".to_owned(),
                message: "Try again.".to_owned(),
            }),
        ];

        assert!(visible_states.iter().all(PresentationState::is_visible));
    }

    #[test]
    fn states_report_their_action_context() {
        let cases = [
            (PresentationState::Idle, None),
            (
                PresentationState::Loading {
                    action: TextAction::Proofread,
                },
                Some(TextAction::Proofread),
            ),
            (translation_state(), Some(TextAction::Translate)),
            (proofreading_state(), Some(TextAction::Proofread)),
            (PresentationState::NoIssues, Some(TextAction::Proofread)),
            (
                PresentationState::ProofreadingDisclosure,
                Some(TextAction::Proofread),
            ),
            (
                PresentationState::Error(ErrorPresentation {
                    action: Some(TextAction::Translate),
                    title: "Translation failed".to_owned(),
                    message: "Try again.".to_owned(),
                }),
                Some(TextAction::Translate),
            ),
        ];

        for (state, expected_action) in cases {
            assert_eq!(state.action(), expected_action);
        }
    }

    #[test]
    fn result_states_preserve_their_display_content() {
        let translation = translation_state();
        let proofreading = proofreading_state();

        assert_eq!(
            translation,
            PresentationState::Translation(TranslationPresentation {
                original_text: "Hallo".to_owned(),
                language_pair: LanguagePair {
                    source: "German".to_owned(),
                    target: "English".to_owned(),
                },
                translated_text: "Hello".to_owned(),
            })
        );
        assert_eq!(
            proofreading,
            PresentationState::Proofreading(ProofreadingPresentation {
                corrected_text: "This is correct.".to_owned(),
                explanation: "Added the missing verb.".to_owned(),
            })
        );
    }

    fn translation_state() -> PresentationState {
        PresentationState::Translation(TranslationPresentation {
            original_text: "Hallo".to_owned(),
            language_pair: LanguagePair {
                source: "German".to_owned(),
                target: "English".to_owned(),
            },
            translated_text: "Hello".to_owned(),
        })
    }

    fn proofreading_state() -> PresentationState {
        PresentationState::Proofreading(ProofreadingPresentation {
            corrected_text: "This is correct.".to_owned(),
            explanation: "Added the missing verb.".to_owned(),
        })
    }
}
