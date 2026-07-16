#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedText(String);

impl CapturedText {
    pub fn new(text: impl Into<String>) -> Result<Self, CaptureFailure> {
        let text = text.into();

        if text.trim().is_empty() {
            return Err(CaptureFailure::NoSelection);
        }

        Ok(Self(text))
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
    fn empty_and_whitespace_only_text_mean_no_selection() {
        for text in ["", "   ", "\n\t"] {
            assert_eq!(CapturedText::new(text), Err(CaptureFailure::NoSelection));
        }
    }
}
