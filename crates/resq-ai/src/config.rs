/*
 * Copyright 2026 ResQ
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Config cascade: env vars -> `<project>/.resq/ai.toml` -> `~/.resq/ai.toml`
//!
//! Most-specific wins: project-local config overrides home config.

use crate::provider::Provider;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::PathBuf;
use std::{env, fmt, fs};

/// AI configuration.
pub struct AiConfig {
    /// Selected provider.
    pub provider: Provider,
    /// Model identifier.
    pub model: String,
    /// API key (private — use `api_key()` accessor).
    api_key: String,
    /// Base URL override.
    pub base_url: Option<String>,
    /// Max tokens in response.
    pub max_tokens: u32,
    /// HTTP request timeout in seconds.
    pub timeout_secs: u64,
}

impl AiConfig {
    /// Access the API key.
    #[must_use]
    pub fn api_key(&self) -> &str {
        &self.api_key
    }
}

impl fmt::Debug for AiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiConfig")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("api_key", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("max_tokens", &self.max_tokens)
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

/// TOML file schema.
#[derive(Deserialize, Default)]
struct FileConfig {
    provider: Option<Provider>,
    model: Option<String>,
    base_url: Option<String>,
    max_tokens: Option<u32>,
    timeout_secs: Option<u64>,
}

/// Load config with cascade: env vars -> ~/.resq/ai.toml -> .resq/ai.toml.
///
/// # Errors
///
/// Returns an error if no API key is found for the selected provider.
pub fn load_config() -> Result<AiConfig> {
    let home_cfg = load_toml_config(home_config_path());
    let project_cfg = load_toml_config(project_config_path());

    let provider = match env::var("RESQ_AI_PROVIDER") {
        Ok(s) => match s.to_lowercase().as_str() {
            "anthropic" => Provider::Anthropic,
            "openai" => Provider::OpenAi,
            "gemini" => Provider::Gemini,
            other => bail!("Unknown RESQ_AI_PROVIDER={other:?}. Use: anthropic, openai, gemini"),
        },
        Err(_) => project_cfg
            .provider
            .or(home_cfg.provider)
            .unwrap_or(Provider::Anthropic),
    };

    let model = env::var("RESQ_AI_MODEL")
        .ok()
        .or(project_cfg.model)
        .or(home_cfg.model)
        .unwrap_or_else(|| provider.default_model().to_string());

    let api_key = env::var(provider.api_key_env_var()).with_context(|| {
        format!(
            "No API key found. Set {} environment variable.",
            provider.api_key_env_var()
        )
    })?;

    if api_key.is_empty() {
        bail!(
            "{} is set but empty. Provide a valid API key.",
            provider.api_key_env_var()
        );
    }

    let base_url = env::var("RESQ_AI_BASE_URL")
        .ok()
        .or(project_cfg.base_url)
        .or(home_cfg.base_url);

    if let Some(ref url_str) = base_url {
        let parsed = reqwest::Url::parse(url_str)
            .with_context(|| format!("base_url is not a valid URL: {url_str:?}"))?;
        if parsed.scheme() != "https" {
            bail!(
                "base_url must use HTTPS to protect the API key (got scheme {:?})",
                parsed.scheme()
            );
        }
    }

    let max_tokens = project_cfg
        .max_tokens
        .or(home_cfg.max_tokens)
        .unwrap_or(1024);

    let timeout_secs = project_cfg
        .timeout_secs
        .or(home_cfg.timeout_secs)
        .unwrap_or(30);

    Ok(AiConfig {
        provider,
        model,
        api_key,
        base_url,
        max_tokens,
        timeout_secs,
    })
}

fn home_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".resq").join("ai.toml"))
}

fn project_config_path() -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?;
    for ancestor in cwd.ancestors() {
        let candidate = ancestor.join(".resq").join("ai.toml");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn load_toml_config(path: Option<PathBuf>) -> FileConfig {
    let Some(p) = path else {
        return FileConfig::default();
    };
    let content = match fs::read_to_string(&p) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return FileConfig::default(),
        Err(e) => {
            eprintln!("Warning: could not read {}: {e}", p.display());
            return FileConfig::default();
        }
    };
    match toml::from_str(&content) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Warning: invalid TOML in {}: {e}", p.display());
            FileConfig::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serialize env-mutating tests to prevent races under parallel test harness.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn debug_redacts_api_key() {
        let cfg = AiConfig {
            provider: Provider::Anthropic,
            model: "test".to_string(),
            api_key: "test-placeholder-value".to_string(),
            base_url: None,
            max_tokens: 1024,
            timeout_secs: 30,
        };
        let debug_str = format!("{cfg:?}");
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("test-placeholder"));
    }

    #[test]
    fn load_config_fails_without_api_key() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // Save and clear all possible keys
        let saved: Vec<(&str, Option<String>)> = [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "GEMINI_API_KEY",
            "RESQ_AI_PROVIDER",
            "RESQ_AI_MODEL",
            "RESQ_AI_BASE_URL",
        ]
        .iter()
        .map(|k| (*k, env::var(k).ok()))
        .collect();

        for (k, _) in &saved {
            env::remove_var(k);
        }

        let result = load_config();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("ANTHROPIC_API_KEY"));

        // Restore
        for (k, v) in saved {
            match v {
                Some(val) => env::set_var(k, val),
                None => env::remove_var(k),
            }
        }
    }

    #[test]
    fn load_config_with_env_vars() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // Save originals
        let saved: Vec<(&str, Option<String>)> =
            ["RESQ_AI_PROVIDER", "OPENAI_API_KEY", "RESQ_AI_MODEL"]
                .iter()
                .map(|k| (*k, env::var(k).ok()))
                .collect();

        env::set_var("RESQ_AI_PROVIDER", "openai");
        env::set_var("OPENAI_API_KEY", "test-placeholder-value");
        env::set_var("RESQ_AI_MODEL", "gpt-4o-mini");

        let cfg = load_config().unwrap();
        assert_eq!(cfg.provider, Provider::OpenAi);
        assert_eq!(cfg.model, "gpt-4o-mini");
        assert_eq!(cfg.api_key(), "test-placeholder-value");
        assert_eq!(cfg.max_tokens, 1024);

        // Restore
        for (k, v) in saved {
            match v {
                Some(val) => env::set_var(k, val),
                None => env::remove_var(k),
            }
        }
    }

    #[test]
    fn provider_defaults() {
        assert_eq!(
            Provider::Anthropic.default_model(),
            "claude-sonnet-4-20250514"
        );
        assert_eq!(Provider::OpenAi.default_model(), "gpt-4o");
        assert_eq!(Provider::Gemini.default_model(), "gemini-2.0-flash");
    }
}
