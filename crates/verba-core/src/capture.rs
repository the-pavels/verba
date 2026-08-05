#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedText {
    text: String,
    language_detection_context: Option<String>,
}

impl CapturedText {
    pub fn new(text: impl Into<String>) -> Result<Self, CaptureFailure> {
        let text = text.into();

        if text.trim().is_empty() {
            return Err(CaptureFailure::NoSelection);
        }

        Ok(Self {
            text,
            language_detection_context: None,
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn language_detection_context(&self) -> Option<&str> {
        self.language_detection_context.as_deref()
    }

    #[must_use]
    pub fn with_language_detection_context(mut self, context: impl Into<String>) -> Self {
        let context = context.into();
        if !context.trim().is_empty() && context != self.text {
            self.language_detection_context = Some(context);
        }
        self
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.text
    }

    #[must_use]
    pub fn into_parts(self) -> (String, Option<String>) {
        (self.text, self.language_detection_context)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureFailure {
    NoSelection,
    TimedOut,
    PermissionDenied,
    SecureField,
    FieldSecurityUnavailable,
    UnsupportedContent,
    ClipboardUnavailable,
    Cancelled,
}

pub trait TextCapture: Send + Sync {
    fn capture(&self) -> Result<CapturedText, CaptureFailure>;

    fn capture_with_language_detection_context(&self) -> Result<CapturedText, CaptureFailure> {
        self.capture()
    }
}

#[cfg(test)]
mod tests {
    use super::{CaptureFailure, CapturedText};

    #[test]
    fn captured_text_preserves_the_original_selection() {
        let text = "  First line\nSecond line  ";
        let captured = CapturedText::new(text).expect("non-empty text should be accepted");

        assert_eq!(captured.as_str(), text);
        assert_eq!(captured.into_string(), text);
    }

    #[test]
    fn language_context_is_optional_and_never_replaces_the_selection() {
        let captured = CapturedText::new("bergen")
            .unwrap()
            .with_language_detection_context("Rega muss sie bergen");

        assert_eq!(captured.as_str(), "bergen");
        assert_eq!(
            captured.language_detection_context(),
            Some("Rega muss sie bergen")
        );
        assert_eq!(
            captured.into_parts(),
            ("bergen".to_owned(), Some("Rega muss sie bergen".to_owned()))
        );
    }

    #[test]
    fn empty_and_whitespace_only_text_mean_no_selection() {
        for text in ["", "   ", "\n\t"] {
            assert_eq!(CapturedText::new(text), Err(CaptureFailure::NoSelection));
        }
    }
}
