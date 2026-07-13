#[path = "support/client.rs"]
mod client;

use verba_core::proofreading::ProofreadingResult;
use verba_openai::{DEFAULT_MODEL, OPENAI_BASE_URL};

use client::run_proofreading;

#[test]
#[ignore = "requires VERBA_RUN_LIVE_OPENAI_TEST=1 and OPENAI_API_KEY"]
fn live_responses_api_smoke_test() {
    assert_eq!(
        std::env::var("VERBA_RUN_LIVE_OPENAI_TEST").as_deref(),
        Ok("1"),
        "set VERBA_RUN_LIVE_OPENAI_TEST=1 to confirm the paid live request"
    );
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY should be configured");
    let result = run_proofreading(
        OPENAI_BASE_URL,
        DEFAULT_MODEL,
        &api_key,
        "This sentence is grammatically correct.",
    )
    .expect("live proofreading request should succeed");

    assert!(matches!(
        result,
        ProofreadingResult::NoIssues | ProofreadingResult::Corrected(_)
    ));
}
