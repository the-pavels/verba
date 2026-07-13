//! OpenAI proofreading adapter for Verba.

mod client;
mod transport;

pub use client::{
    DEFAULT_MODEL, OPENAI_BASE_URL, OpenAiClient, OpenAiClientBuildError, OpenAiConfig,
    ResponsesApiRequest, ResponsesApiResponse,
};
