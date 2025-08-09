//! src/config.rs
//! Load and validate runtime configuration for mabel.
//!
//! Priority: CLI flags > .env > defaults.

use crate::{MabelError, Result};
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Duration as StdDuration,
};
use url::Url;

/// What LLM backend to use.
#[derive(Clone, Debug)]
pub enum LlmBackend {
    OpenAi {
        api_key: String,
        model: String, // e.g., "gpt-4o-mini"
        max_tokens: u32,
        temperature: f32,
    },
    Ollama {
        host: Url,     // e.g., http://localhost:11434
        model: String, // e.g., "llama3:8b-instruct"
        max_tokens: u32,
        temperature: f32,
    },
}

/// Output style preset for the note.
#[derive(Clone, Debug)]
pub enum Mode {
    Concise, // short abstract + bullets
    Study,   // longer method/results/glossary
}

#[derive(Clone, Debug)]
pub struct Config {
    // Obsidian
    pub vault_path: PathBuf,  // absolute, validated
    pub vault_subdir: String, // e.g., "Papers"
    pub copy_pdf_into_vault: bool,

    // Cache & IO
    pub cache_dir: PathBuf,   // e.g., ~/.mabel/papers
    pub overwrite_note: bool, // if false, append/update

    // LLM
    pub llm: LlmBackend,

    // Extraction
    pub grobid_url: Option<Url>, // if None -> fallback extractor

    // HTTP/runtime
    pub http_timeout: StdDuration,
    pub http_retries: u32,
    pub rate_limit_per_min: u32,

    // Rendering
    pub template_path: PathBuf, // templates/paper_note.md.tera
    pub mode: Mode,
}

impl Config {
    /// Build from CLI flags + env; do path and permission checks.
    pub fn load(cli: &crate::cli::Cli) -> Result<Self> {
        // Load .env first (no error if absent).
        let _ = dotenvy::dotenv();

        // ---- Obsidian vault ----
        let vault_path = cli
            .vault_path
            .clone()
            .or_else(|| env::var("OBSIDIAN_VAULT_PATH").ok().map(PathBuf::from))
            .ok_or(MabelError::MissingEnv {
                key: "OBSIDIAN_VAULT_PATH",
            })?;
        let vault_path = expand_path(&vault_path);

        ensure_dir_exists(&vault_path).map_err(|e| MabelError::Io {
            path: vault_path.clone(),
            source: e,
        })?;
        ensure_writable(&vault_path).map_err(|_| MabelError::VaultNotWritable {
            path: vault_path.clone(),
        })?;

        let vault_subdir = cli
            .vault_subdir
            .clone()
            .or_else(|| env::var("OBSIDIAN_SUBDIR").ok())
            .unwrap_or_else(|| "Papers".to_string());

        let copy_pdf_into_vault = cli.copy_pdf_into_vault || env_bool("MABEL_COPY_PDF", false);

        // ---- Cache ----
        let cache_dir = cli
            .cache_dir
            .clone()
            .or_else(|| env::var("MABEL_CACHE_DIR").ok().map(PathBuf::from))
            .unwrap_or_else(|| default_cache_dir());
        let cache_dir = expand_path(&cache_dir);
        ensure_dir_exists(&cache_dir).map_err(|e| MabelError::Io {
            path: cache_dir.clone(),
            source: e,
        })?;

        let overwrite_note = cli.overwrite || env_bool("MABEL_OVERWRITE_NOTE", false);

        // ---- LLM backend selection ----
        let llm = if cli.ollama {
            let host = cli
                .ollama_host
                .clone()
                .or_else(|| env::var("OLLAMA_HOST").ok())
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            let host = Url::parse(&host)?;
            let model = cli
                .model
                .clone()
                .or_else(|| env::var("OLLAMA_MODEL").ok())
                .unwrap_or_else(|| "llama3:8b-instruct".to_string());
            LlmBackend::Ollama {
                host,
                model,
                max_tokens: env_u32("MABEL_MAX_TOKENS", 800),
                temperature: env_f32("MABEL_TEMPERATURE", 0.2),
            }
        } else {
            let api_key = cli
                .openai_key
                .clone()
                .or_else(|| env::var("OPENAI_API_KEY").ok())
                .ok_or(MabelError::MissingEnv { key: "OPENAI_API_KEY" })?;
            let model = cli
                .model
                .clone()
                .or_else(|| env::var("OPENAI_MODEL").ok())
                .unwrap_or_else(|| "gpt-4o-mini".to_string());
            LlmBackend::OpenAi {
                api_key,
                model,
                max_tokens: env_u32("MABEL_MAX_TOKENS", 800),
                temperature: env_f32("MABEL_TEMPERATURE", 0.2),
            }
        };

        // ---- Extraction (GROBID optional) ----
        let grobid_url = cli
            .grobid_url
            .clone()
            .or_else(|| env::var("GROBID_URL").ok())
            .map(|s| Url::parse(&s))
            .transpose()?;

        // ---- HTTP/runtime ----
        let http_timeout = StdDuration::from_secs(env_u64("MABEL_HTTP_TIMEOUT_SECS", 20));
        let http_retries = env_u32("MABEL_HTTP_RETRIES", 2);
        let rate_limit_per_min = env_u32("MABEL_RATE_PER_MIN", 30);

        // ---- Rendering ----
        let template_path = cli
            .template
            .clone()
            .unwrap_or_else(|| PathBuf::from("templates/paper_note.md.tera"));
        let template_path = expand_path(&template_path);

        let mode = match cli.mode.as_deref() {
            | Some("study") => Mode::Study,
            | _ => Mode::Concise,
        };

        Ok(Self {
            vault_path,
            vault_subdir,
            copy_pdf_into_vault,
            cache_dir,
            overwrite_note,
            llm,
            grobid_url,
            http_timeout,
            http_retries,
            rate_limit_per_min,
            template_path,
            mode,
        })
    }

    /// Full path inside the vault where notes should be written.
    pub fn vault_notes_dir(&self) -> PathBuf {
        self.vault_path.join(&self.vault_subdir)
    }

    /// Cache path for a given arXiv ID’s PDF.
    pub fn cached_pdf_path(&self, arxiv_id: &str) -> PathBuf {
        self.cache_dir.join("papers").join(format!("{arxiv_id}.pdf"))
    }
}

// ---------- helpers ----------

fn expand_path(p: &Path) -> PathBuf {
    // support "~" in env/CLI paths
    let s = p.to_string_lossy();
    PathBuf::from(shellexpand::tilde(&s).into_owned())
}

fn ensure_dir_exists(dir: &Path) -> std::io::Result<()> {
    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

fn ensure_writable(dir: &Path) -> std::io::Result<()> {
    let test = dir.join(".mabel_write_check");
    let mut f = OpenOptions::new().create(true).write(true).open(&test)?;
    f.write_all(b"ok")?;
    let _ = fs::remove_file(test);
    Ok(())
}

fn default_cache_dir() -> PathBuf {
    // ~/.mabel
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".mabel")
}

fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(default)
}
fn env_u32(key: &str, default: u32) -> u32 {
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
fn env_f32(key: &str, default: f32) -> f32 {
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
