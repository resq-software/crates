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

//! Google Gemini API client.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::AiConfig;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerateRequest<'a> {
    system_instruction: SystemInstruction<'a>,
    contents: Vec<RequestContent<'a>>,
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct SystemInstruction<'a> {
    parts: Vec<TextPart<'a>>,
}

#[derive(Serialize)]
struct RequestContent<'a> {
    role: &'a str,
    parts: Vec<TextPart<'a>>,
}

#[derive(Serialize)]
struct TextPart<'a> {
    text: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig {
    max_output_tokens: u32,
}

#[derive(Deserialize)]
struct GenerateResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: CandidateContent,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Vec<CandidatePart>,
}

#[derive(Deserialize)]
struct CandidatePart {
    text: String,
}

/// Send a completion request to the Gemini API.
pub(crate) async fn complete(
    client: &reqwest::Client,
    config: &AiConfig,
    system: &str,
    user: &str,
) -> Result<String> {
    let base = config
        .base_url
        .as_deref()
        .unwrap_or("https://generativelanguage.googleapis.com");
    let url = format!("{base}/v1beta/models/{}:generateContent", config.model);

    let body = GenerateRequest {
        system_instruction: SystemInstruction {
            parts: vec![TextPart { text: system }],
        },
        contents: vec![RequestContent {
            role: "user",
            parts: vec![TextPart { text: user }],
        }],
        generation_config: GenerationConfig {
            max_output_tokens: config.max_tokens,
        },
    };

    let resp = client
        .post(&url)
        .header("x-goog-api-key", config.api_key())
        .header("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .json(&body)
        .send()
        .await
        .context("Failed to reach Gemini API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("Gemini API error ({status}): {text}");
    }

    let parsed: GenerateResponse = resp.json().await.context("Failed to parse response")?;
    parsed
        .candidates
        .into_iter()
        .next()
        .and_then(|c| c.content.parts.into_iter().next())
        .map(|p| p.text)
        .context("Empty response from Gemini")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_deserialization() {
        let json = r#"{"candidates":[{"content":{"parts":[{"text":"Hello!"}],"role":"model"}}]}"#;
        let resp: GenerateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.candidates[0].content.parts[0].text, "Hello!");
    }
}
