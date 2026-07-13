#[path = "support/client.rs"]
mod client;
#[path = "support/fixtures.rs"]
mod fixtures;
#[path = "support/mock_server.rs"]
mod mock_server;

use serde_json::{Value, json};
use verba_core::proofreading::{ProofreadingCorrection, ProofreadingResult};

use client::run_proofreading;
use fixtures::{ExpectedOutcome, FIXTURES};
use mock_server::{MockResponse, MockServer};

#[test]
fn preserves_curated_prompt_and_output_cases() {
    for fixture in FIXTURES {
        let response_payload = match fixture.expected {
            ExpectedOutcome::NoIssues => json!({
                "outcome": "no_issues",
                "corrected_text": null,
                "explanation": null
            }),
            ExpectedOutcome::Corrected { text, explanation } => json!({
                "outcome": "corrected",
                "corrected_text": text,
                "explanation": explanation
            }),
        };
        let server = MockServer::start(MockResponse::json(
            200,
            completed_response(response_payload),
        ));
        let result = run_proofreading(
            server.base_url(),
            "test-proofreading-model",
            "test-api-key",
            fixture.input,
        );
        let request = server.received();

        assert_eq!(request.method, "POST", "{} fixture failed", fixture.name);
        assert_eq!(
            request.path, "/v1/responses",
            "{} fixture failed",
            fixture.name
        );
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("Bearer test-api-key"),
            "{} fixture failed",
            fixture.name
        );
        assert_eq!(
            request.body["input"][1]["content"][0]["text"], fixture.input,
            "{} fixture changed before reaching the provider",
            fixture.name
        );
        match fixture.expected {
            ExpectedOutcome::NoIssues => assert_eq!(
                result,
                Ok(ProofreadingResult::NoIssues),
                "{} fixture failed",
                fixture.name
            ),
            ExpectedOutcome::Corrected { text, explanation } => assert_eq!(
                result,
                Ok(ProofreadingResult::Corrected(ProofreadingCorrection::new(
                    text,
                    explanation
                ))),
                "{} fixture failed",
                fixture.name
            ),
        }
    }
}

fn completed_response(payload: Value) -> Value {
    json!({
        "status": "completed",
        "output": [{
            "type": "message",
            "content": [{
                "type": "output_text",
                "text": serde_json::to_string(&payload).expect("payload should serialize")
            }]
        }]
    })
}
