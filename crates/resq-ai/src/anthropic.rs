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

//! Anthropic Messages API client.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::AiConfig;

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<Message<'a>>,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(default)]
    text: Option<String>,
}

/// Send a completion request to the Anthropic Messages API.
pub(crate) async fn complete(
    client: &reqwest::Client,
    config: &AiConfig,
    system: &str,
    user: &str,
) -> Result<String> {
    let base = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.anthropic.com");
    let url = format!("{base}/v1/messages");

    let body = MessagesRequest {
        model: &config.model,
        max_tokens: config.max_tokens,
        system,
        messages: vec![Message {
            role: "user",
            content: user,
        }],
    };

    let resp = client
        .post(&url)
        .header("x-api-key", config.api_key())
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .json(&body)
        .send()
        .await
        .context("Failed to reach Anthropic API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("Anthropic API error ({status}): {text}");
    }

    let parsed: MessagesResponse = resp.json().await.context("Failed to parse response")?;
    parsed
        .content
        .into_iter()
        .find_map(|c| c.text)
        .context("No text content in Anthropic response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serialization() {
        let req = MessagesRequest {
            model: "claude-sonnet-4-20250514",
            max_tokens: 1024,
            system: "You are helpful.",
            messages: vec![Message {
                role: "user",
                content: "Hello",
            }],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("claude-sonnet"));
        assert!(json.contains("\"role\":\"user\""));
    }

    #[test]
    fn response_deserialization() {
        let json = r#"{"content":[{"type":"text","text":"Hello back!"}]}"#;
        let resp: MessagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content[0].text.as_deref(), Some("Hello back!"));
    }
}
