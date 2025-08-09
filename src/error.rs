use chrono::ParseError as ChronoParseError;
use reqwest::StatusCode;
use std::path::PathBuf;
use thiserror::Error;
use url::Url;

pub type Result<T, E = MabelError> = std::result::Result<T, E>;

/// All fallible operations in mabel should return `Result<T, MabelError>`.
#[derive(Debug, Error)]
pub enum MabelError {
    // ------------------- Config / CLI -------------------
    #[error("missing required environment variable: {key}")]
    MissingEnv { key: &'static str },

    #[error("invalid configuration: {msg}")]
    Config { msg: String },

    // ------------------- I/O / filesystem -------------------
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("vault path not writable: {path}")]
    VaultNotWritable { path: PathBuf },

    #[error("template not found or unreadable: {path}")]
    TemplateMissing { path: PathBuf },

    // ------------------- HTTP / network -------------------
    #[error("HTTP request failed for {url}: {source}")]
    Http {
        url: Url,
        #[source]
        source: reqwest::Error,
    },

    #[error("unexpected HTTP status {status} from {url}{body_snip}")]
    HttpStatus {
        url: Url,
        status: StatusCode,
        /// Optional 1–2 KB snippet of the body for diagnostics.
        body_snip: String,
    },

    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    // ------------------- Parsing / formats -------------------
    #[error("XML parse error while reading {context}: {source}")]
    Xml {
        context: &'static str,
        #[source]
        source: quick_xml::Error,
    },

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    #[error("date/time parse error: {0}")]
    Chrono(#[from] ChronoParseError),

    #[error("templating error: {0}")]
    Template(#[from] tera::Error),

    // ------------------- Domain-specific -------------------
    #[error("invalid arXiv id or URL: {input}")]
    InvalidArxivId { input: String },

    #[error("extraction failed: {reason}")]
    Extraction { reason: String },

    #[error("GROBID returned malformed TEI: {reason}")]
    GrobidMalformed { reason: String },

    #[error("guardrail violation: {reason}")]
    Guardrail { reason: String },

    // ------------------- LLM backends -------------------
    #[cfg(feature = "openai")]
    #[error("OpenAI API error: {0}")]
    OpenAi(#[from] async_openai::error::OpenAIError),
}
