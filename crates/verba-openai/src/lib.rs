//! OpenAI proofreading adapter for Verba.

mod client;
mod connection;
mod proofreader;
mod transport;

pub use client::{
    DEFAULT_MODEL, OPENAI_BASE_URL, OpenAiClient, OpenAiClientBuildError, OpenAiConfig,
    ReasoningEffort, ResponsesApiRequest, ResponsesApiResponse,
};
pub use connection::OpenAiConnectionTester;
pub use proofreader::{ApiKeyProvider, ApiKeyProviderError, OpenAiProofreader};

pub const PROOFREADING_REQUEST_POLICY_VERSION: &str = "verba-proofreading-2026-07-16-v2";
pub const PROOFREADING_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::Low;
pub const PROOFREADING_MAX_OUTPUT_TOKENS: u32 = 16_384;
pub const CONNECTION_TEST_MAX_OUTPUT_TOKENS: u32 = 256;
