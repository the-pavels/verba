use std::{fs, path::PathBuf, sync::Arc, time::Instant};

use futures::executor::block_on;
use serde::{Deserialize, Serialize};
use verba_core::{
    coordinator::CancellationToken,
    proofreading::{
        ProofreaderError, ProofreaderResponse, ProofreadingPolicyValidation,
        evaluate_proofreading_policy,
    },
};

use super::{build_request, decode_response};
use crate::{DEFAULT_MODEL, OPENAI_BASE_URL, OpenAiClient, OpenAiConfig, ResponsesApiResponse};

const CORPUS: &str = include_str!("../../tests/fixtures/proofreading-evaluation-v1.json");

#[test]
fn versioned_corpus_is_stable_bounded_and_release_gated() {
    use std::collections::HashSet;

    use verba_core::proofreading::MAX_PROOFREADING_CHARACTERS;

    let corpus: EvaluationCorpus =
        serde_json::from_str(CORPUS).expect("the versioned evaluation corpus should be valid");
    let mut identifiers = HashSet::new();

    assert_eq!(corpus.version, 1);
    assert_eq!(corpus.release_threshold, 0.9);
    assert!(corpus.cases.iter().any(|case| case.mandatory));
    for case in &corpus.cases {
        assert!(
            identifiers.insert(case.id.as_str()),
            "duplicate case identifier"
        );
        assert!(!case.id.trim().is_empty());
        assert!(
            case.input.materialize().chars().count() <= MAX_PROOFREADING_CHARACTERS,
            "{} exceeds the production input boundary",
            case.id
        );
    }
    for required_prefix in [
        "noop-",
        "grammar-",
        "mixed-script-",
        "outer-whitespace-",
        "blank-lines-",
        "markdown-list-",
        "quoted-text-",
        "markdown-code-",
        "prompt-injection-",
        "long-input-",
        "token-dense-",
        "refusal-probe-",
        "incomplete-probe-",
    ] {
        assert!(
            corpus
                .cases
                .iter()
                .any(|case| case.id.starts_with(required_prefix)),
            "missing {required_prefix} coverage"
        );
    }
}

#[test]
fn evaluation_report_records_usage_and_cost_without_text_content() {
    let original = "This are private synthetic text.";
    let corrected = "This is private synthetic text.";
    let case = EvaluationCase {
        id: "privacy-report-001".to_owned(),
        mandatory: true,
        input: EvaluationInput::Literal {
            text: original.to_owned(),
        },
        expected: ExpectedOutcome::Corrected {
            accepted_texts: vec![corrected.to_owned()],
        },
    };
    let response = ResponsesApiResponse::new(serde_json::json!({
        "status": "completed",
        "output": [{
            "type": "message",
            "content": [{
                "type": "output_text",
                "text": serde_json::to_string(&serde_json::json!({
                    "outcome": "corrected",
                    "corrected_text": corrected
                })).unwrap()
            }]
        }],
        "usage": {
            "input_tokens": 100,
            "output_tokens": 25
        }
    }));

    let report = evaluate_case(&case, original, Ok(response), 42, 2.0, 8.0);
    let encoded = serde_json::to_string(&report).unwrap();

    assert!(report.passed);
    assert_eq!(report.input_tokens, Some(100));
    assert_eq!(report.output_tokens, Some(25));
    assert!((report.cost_usd.unwrap() - 0.000_4).abs() < f64::EPSILON);
    assert!(!encoded.contains(original));
    assert!(!encoded.contains(corrected));
}

#[test]
#[ignore = "requires explicit paid live-evaluation opt-in and an OpenAI API key"]
fn production_model_meets_release_threshold() {
    require_opt_in();
    let api_key = required_environment("OPENAI_API_KEY");
    let report_path = PathBuf::from(required_environment("VERBA_EVAL_REPORT_PATH"));
    let input_price = price("VERBA_EVAL_INPUT_USD_PER_MILLION");
    let output_price = price("VERBA_EVAL_OUTPUT_USD_PER_MILLION");
    let corpus: EvaluationCorpus =
        serde_json::from_str(CORPUS).expect("the versioned evaluation corpus should be valid");
    let client = Arc::new(
        OpenAiClient::new(OpenAiConfig::new(OPENAI_BASE_URL, DEFAULT_MODEL))
            .expect("the production OpenAI configuration should be valid"),
    );

    let mut case_reports = Vec::with_capacity(corpus.cases.len());
    for case in &corpus.cases {
        let input = case.input.materialize();
        let started = Instant::now();
        let response = block_on(client.create_response(
            &api_key,
            build_request(&input),
            &CancellationToken::default(),
        ));
        let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        case_reports.push(evaluate_case(
            case,
            &input,
            response,
            latency_ms,
            input_price,
            output_price,
        ));
    }

    let passed = case_reports.iter().filter(|case| case.passed).count();
    let mandatory_passed = case_reports
        .iter()
        .filter(|case| case.mandatory)
        .all(|case| case.passed);
    let pass_rate = passed as f64 / case_reports.len() as f64;
    let total_cost_usd = case_reports.iter().filter_map(|case| case.cost_usd).sum();
    let report = EvaluationReport {
        corpus_version: corpus.version,
        model: DEFAULT_MODEL,
        release_threshold: corpus.release_threshold,
        mandatory_passed,
        passed_cases: passed,
        total_cases: case_reports.len(),
        pass_rate,
        total_cost_usd,
        cases: case_reports,
    };
    let encoded = serde_json::to_vec_pretty(&report).expect("evaluation report should serialize");
    fs::write(&report_path, encoded).expect("evaluation report should be writable");

    assert!(
        mandatory_passed && pass_rate >= corpus.release_threshold,
        "live evaluation did not meet the release gate; inspect the privacy-safe report at {}",
        report_path.display()
    );
}

fn evaluate_case(
    case: &EvaluationCase,
    input: &str,
    response: Result<ResponsesApiResponse, ProofreaderError>,
    latency_ms: u64,
    input_price: f64,
    output_price: f64,
) -> CaseReport {
    let usage = response.as_ref().ok().and_then(response_usage);
    let decoded = response.and_then(|response| decode_response(response, input));
    let (outcome, validation, reference_match) = match decoded {
        Ok(ProofreaderResponse::NoIssues) => (
            ObservedOutcome::NoIssues,
            Some(evaluate_proofreading_policy(input, input)),
            case.expected.accepts_no_issues(),
        ),
        Ok(ProofreaderResponse::Corrected(correction)) => {
            let corrected_text = correction.corrected_text();
            (
                ObservedOutcome::Corrected,
                Some(evaluate_proofreading_policy(input, corrected_text)),
                case.expected.accepts_correction(corrected_text),
            )
        }
        Err(error) => (
            ObservedOutcome::ProviderError(provider_error_code(error)),
            None,
            case.expected.accepts_error(error),
        ),
    };
    let mechanical_invariants_passed = validation
        .is_some_and(|validation| validation.first_violation().is_none())
        || matches!(outcome, ObservedOutcome::ProviderError(_));
    let usage_recorded = usage.is_some();
    let cost_usd = usage.map(|usage| {
        (usage.input_tokens as f64 * input_price + usage.output_tokens as f64 * output_price)
            / 1_000_000.0
    });

    CaseReport {
        id: case.id.clone(),
        mandatory: case.mandatory,
        passed: reference_match && mechanical_invariants_passed && usage_recorded,
        outcome,
        reference_output_match: reference_match,
        invariants: validation.map(InvariantReport::from),
        latency_ms,
        input_tokens: usage.map(|usage| usage.input_tokens),
        output_tokens: usage.map(|usage| usage.output_tokens),
        cost_usd,
    }
}

fn response_usage(response: &ResponsesApiResponse) -> Option<TokenUsage> {
    let usage = response.body().get("usage")?;
    Some(TokenUsage {
        input_tokens: usage.get("input_tokens")?.as_u64()?,
        output_tokens: usage.get("output_tokens")?.as_u64()?,
    })
}

fn require_opt_in() {
    assert_eq!(
        std::env::var("VERBA_RUN_LIVE_OPENAI_EVAL").as_deref(),
        Ok("1"),
        "set VERBA_RUN_LIVE_OPENAI_EVAL=1 to confirm the paid live evaluation"
    );
}

fn required_environment(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} should be configured"))
}

fn price(name: &str) -> f64 {
    let value = required_environment(name)
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("{name} should be a decimal price per million tokens"));
    assert!(value >= 0.0, "{name} cannot be negative");
    value
}

fn provider_error_code(error: ProofreaderError) -> &'static str {
    match error {
        ProofreaderError::MissingCredential => "missing_credential",
        ProofreaderError::Authentication => "authentication",
        ProofreaderError::RateLimited => "rate_limited",
        ProofreaderError::QuotaExceeded => "quota_exceeded",
        ProofreaderError::Offline => "offline",
        ProofreaderError::TimedOut => "timed_out",
        ProofreaderError::Refused => "refused",
        ProofreaderError::Incomplete => "incomplete",
        ProofreaderError::MalformedResponse => "malformed_response",
        ProofreaderError::ServiceUnavailable => "service_unavailable",
        ProofreaderError::Cancelled => "cancelled",
        ProofreaderError::Failed => "failed",
    }
}

#[derive(Deserialize)]
struct EvaluationCorpus {
    version: u32,
    release_threshold: f64,
    cases: Vec<EvaluationCase>,
}

#[derive(Deserialize)]
struct EvaluationCase {
    id: String,
    mandatory: bool,
    input: EvaluationInput,
    expected: ExpectedOutcome,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum EvaluationInput {
    Literal {
        text: String,
    },
    Repeat {
        prefix: String,
        unit: String,
        count: usize,
    },
}

impl EvaluationInput {
    fn materialize(&self) -> String {
        match self {
            Self::Literal { text } => text.clone(),
            Self::Repeat {
                prefix,
                unit,
                count,
            } => format!("{prefix}{}", unit.repeat(*count)),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ExpectedOutcome {
    NoIssues,
    Corrected { accepted_texts: Vec<String> },
    Valid,
    ValidOrRefused,
    ValidOrIncomplete,
}

impl ExpectedOutcome {
    fn accepts_no_issues(&self) -> bool {
        matches!(self, Self::NoIssues | Self::Valid)
    }

    fn accepts_correction(&self, corrected_text: &str) -> bool {
        match self {
            Self::Corrected { accepted_texts } => {
                accepted_texts.iter().any(|text| text == corrected_text)
            }
            Self::Valid | Self::ValidOrRefused | Self::ValidOrIncomplete => true,
            Self::NoIssues => false,
        }
    }

    fn accepts_error(&self, error: ProofreaderError) -> bool {
        matches!(
            (self, error),
            (Self::ValidOrRefused, ProofreaderError::Refused)
        ) || matches!(
            (self, error),
            (Self::ValidOrIncomplete, ProofreaderError::Incomplete)
        )
    }
}

#[derive(Clone, Copy)]
struct TokenUsage {
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Serialize)]
struct EvaluationReport {
    corpus_version: u32,
    model: &'static str,
    release_threshold: f64,
    mandatory_passed: bool,
    passed_cases: usize,
    total_cases: usize,
    pass_rate: f64,
    total_cost_usd: f64,
    cases: Vec<CaseReport>,
}

#[derive(Serialize)]
struct CaseReport {
    id: String,
    mandatory: bool,
    passed: bool,
    outcome: ObservedOutcome,
    reference_output_match: bool,
    invariants: Option<InvariantReport>,
    latency_ms: u64,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost_usd: Option<f64>,
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "code", rename_all = "snake_case")]
enum ObservedOutcome {
    NoIssues,
    Corrected,
    ProviderError(&'static str),
}

#[derive(Serialize)]
struct InvariantReport {
    outer_whitespace: bool,
    line_structure: bool,
    formatting_markers: bool,
}

impl From<ProofreadingPolicyValidation> for InvariantReport {
    fn from(validation: ProofreadingPolicyValidation) -> Self {
        Self {
            outer_whitespace: validation.outer_whitespace_preserved(),
            line_structure: validation.line_structure_preserved(),
            formatting_markers: validation.formatting_markers_preserved(),
        }
    }
}
