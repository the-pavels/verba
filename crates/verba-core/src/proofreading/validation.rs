#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofreadingPolicyViolation {
    OuterWhitespace,
    LineStructure,
    FormattingMarkers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofreadingPolicyValidation {
    outer_whitespace_preserved: bool,
    line_structure_preserved: bool,
    formatting_markers_preserved: bool,
}

impl ProofreadingPolicyValidation {
    #[must_use]
    pub const fn outer_whitespace_preserved(self) -> bool {
        self.outer_whitespace_preserved
    }

    #[must_use]
    pub const fn line_structure_preserved(self) -> bool {
        self.line_structure_preserved
    }

    #[must_use]
    pub const fn formatting_markers_preserved(self) -> bool {
        self.formatting_markers_preserved
    }

    #[must_use]
    pub const fn first_violation(self) -> Option<ProofreadingPolicyViolation> {
        if !self.outer_whitespace_preserved {
            return Some(ProofreadingPolicyViolation::OuterWhitespace);
        }
        if !self.line_structure_preserved {
            return Some(ProofreadingPolicyViolation::LineStructure);
        }
        if !self.formatting_markers_preserved {
            return Some(ProofreadingPolicyViolation::FormattingMarkers);
        }
        None
    }
}

#[must_use]
pub fn evaluate_proofreading_policy(
    original_text: &str,
    corrected_text: &str,
) -> ProofreadingPolicyValidation {
    ProofreadingPolicyValidation {
        outer_whitespace_preserved: outer_whitespace(original_text)
            == outer_whitespace(corrected_text),
        line_structure_preserved: line_break_signature(original_text)
            == line_break_signature(corrected_text)
            && blank_line_signature(original_text) == blank_line_signature(corrected_text),
        formatting_markers_preserved: formatting_marker_signature(original_text)
            == formatting_marker_signature(corrected_text),
    }
}

fn outer_whitespace(text: &str) -> (&str, &str) {
    let leading_end = text
        .char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .map_or(text.len(), |(index, _)| index);
    let trailing_start = text
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())
        .map_or(0, |(index, character)| index + character.len_utf8());
    (&text[..leading_end], &text[trailing_start..])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LineBreak {
    LineFeed,
    CarriageReturn,
    CarriageReturnLineFeed,
}

fn line_break_signature(text: &str) -> Vec<LineBreak> {
    let bytes = text.as_bytes();
    let mut signature = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                signature.push(LineBreak::CarriageReturnLineFeed);
                index += 2;
            }
            b'\r' => {
                signature.push(LineBreak::CarriageReturn);
                index += 1;
            }
            b'\n' => {
                signature.push(LineBreak::LineFeed);
                index += 1;
            }
            _ => index += 1,
        }
    }
    signature
}

fn logical_lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' || bytes[index] == b'\n' {
            lines.push(&text[start..index]);
            if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index += 1;
            }
            start = index + 1;
        }
        index += 1;
    }
    lines.push(&text[start..]);
    lines
}

fn blank_line_signature(text: &str) -> Vec<bool> {
    logical_lines(text)
        .into_iter()
        .map(|line| line.trim().is_empty())
        .collect()
}

fn formatting_marker_signature(text: &str) -> Vec<Vec<String>> {
    logical_lines(text)
        .into_iter()
        .map(line_formatting_markers)
        .collect()
}

fn line_formatting_markers(line: &str) -> Vec<String> {
    let mut markers = Vec::new();
    if let Some(prefix) = line_prefix_marker(line) {
        markers.push(format!("prefix:{prefix}"));
    }

    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let marker = bytes[index];
        if marker == b'`' || marker == b'*' || marker == b'_' || marker == b'~' {
            let start = index;
            while bytes.get(index) == Some(&marker) {
                index += 1;
            }
            let length = index - start;
            if marker == b'`' || length >= 2 {
                markers.push(format!("inline:{}", &line[start..index]));
            }
        } else {
            index += 1;
        }
    }
    markers
}

fn line_prefix_marker(line: &str) -> Option<&str> {
    let indent_end = line
        .char_indices()
        .find(|(_, character)| !matches!(character, ' ' | '\t'))
        .map_or(line.len(), |(index, _)| index);
    let content = &line[indent_end..];

    if content.starts_with("```") || content.starts_with("~~~") {
        let marker = content.as_bytes()[0];
        let marker_end = content
            .as_bytes()
            .iter()
            .take_while(|byte| **byte == marker)
            .count();
        return Some(&line[..indent_end + marker_end]);
    }

    if content.starts_with('>') {
        let marker_end = content
            .char_indices()
            .find(|(_, character)| !matches!(character, '>' | ' ' | '\t'))
            .map_or(content.len(), |(index, _)| index);
        return Some(&line[..indent_end + marker_end]);
    }

    let bytes = content.as_bytes();
    if matches!(bytes.first(), Some(b'-' | b'*' | b'+'))
        && matches!(bytes.get(1), Some(b' ' | b'\t'))
    {
        return Some(&line[..indent_end + 1]);
    }

    let digit_end = bytes
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digit_end > 0
        && matches!(bytes.get(digit_end), Some(b'.' | b')'))
        && matches!(bytes.get(digit_end + 1), Some(b' ' | b'\t'))
    {
        return Some(&line[..indent_end + digit_end + 1]);
    }

    None
}
