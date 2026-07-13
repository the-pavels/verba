use serde::Deserialize;
use verba_core::proofreading::{
    MAX_PROOFREADING_EXPLANATION_CHARACTERS, ProofreaderError, ProofreaderResponse,
    ProofreadingCorrection,
};

use crate::ResponsesApiResponse;

pub(super) fn decode_response(
    response: ResponsesApiResponse,
    original_text: &str,
) -> Result<ProofreaderResponse, ProofreaderError> {
    let response = serde_json::from_value::<ResponseEnvelope>(response.into_body())
        .map_err(|_| ProofreaderError::MalformedResponse)?;

    match response.status {
        ResponseStatus::Completed => {}
        ResponseStatus::Incomplete | ResponseStatus::Queued | ResponseStatus::InProgress => {
            return Err(ProofreaderError::Incomplete);
        }
        ResponseStatus::Cancelled => return Err(ProofreaderError::Cancelled),
        ResponseStatus::Failed | ResponseStatus::Unknown => return Err(ProofreaderError::Failed),
    }

    let mut output_text = None;
    for output in response.output {
        let OutputItem::Message { content } = output else {
            continue;
        };

        for content in content {
            match content {
                OutputContent::Refusal => return Err(ProofreaderError::Refused),
                OutputContent::OutputText { text } => {
                    if output_text.replace(text).is_some() {
                        return Err(ProofreaderError::MalformedResponse);
                    }
                }
                OutputContent::Other => {}
            }
        }
    }

    let output_text = output_text.ok_or(ProofreaderError::MalformedResponse)?;
    let payload = serde_json::from_str::<ProofreadingPayload>(&output_text)
        .map_err(|_| ProofreaderError::MalformedResponse)?;
    match (payload.outcome, payload.corrected_text, payload.explanation) {
        (ProofreadingOutcome::NoIssues, None, None) => Ok(ProofreaderResponse::NoIssues),
        (ProofreadingOutcome::Corrected, Some(corrected_text), Some(explanation))
            if !corrected_text.trim().is_empty()
                && corrected_text != original_text
                && !explanation.trim().is_empty()
                && explanation.chars().count() <= MAX_PROOFREADING_EXPLANATION_CHARACTERS =>
        {
            Ok(ProofreaderResponse::Corrected(ProofreadingCorrection::new(
                corrected_text,
                explanation,
            )))
        }
        _ => Err(ProofreaderError::MalformedResponse),
    }
}

#[derive(Deserialize)]
struct ResponseEnvelope {
    status: ResponseStatus,
    #[serde(default)]
    output: Vec<OutputItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ResponseStatus {
    Completed,
    Incomplete,
    Queued,
    InProgress,
    Failed,
    Cancelled,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputItem {
    Message {
        #[serde(default)]
        content: Vec<OutputContent>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputContent {
    OutputText {
        text: String,
    },
    Refusal,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofreadingPayload {
    outcome: ProofreadingOutcome,
    corrected_text: Option<String>,
    explanation: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProofreadingOutcome {
    NoIssues,
    Corrected,
}
