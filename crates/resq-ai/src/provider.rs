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

//! LLM provider abstraction and dispatch.

use serde::Deserialize;

/// Supported LLM providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// Anthropic Claude API
    Anthropic,
    /// OpenAI-compatible Chat Completions API
    #[serde(alias = "openai")]
    OpenAi,
    /// Google Gemini API
    Gemini,
}

impl Provider {
    /// Default model for this provider.
    #[must_use]
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Anthropic => "claude-sonnet-4-20250514",
            Self::OpenAi => "gpt-4o",
            Self::Gemini => "gemini-2.0-flash",
        }
    }

    /// Default base URL for this provider.
    #[must_use]
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com",
            Self::OpenAi => "https://api.openai.com",
            Self::Gemini => "https://generativelanguage.googleapis.com",
        }
    }

    /// Environment variable name for the API key.
    #[must_use]
    pub fn api_key_env_var(&self) -> &'static str {
        match self {
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::OpenAi => "OPENAI_API_KEY",
            Self::Gemini => "GEMINI_API_KEY",
        }
    }
}

/// Placeholder — full dispatch comes later.
pub async fn complete(
    _config: &super::AiConfig,
    _system: &str,
    _user: &str,
) -> anyhow::Result<String> {
    anyhow::bail!("not yet implemented")
}
