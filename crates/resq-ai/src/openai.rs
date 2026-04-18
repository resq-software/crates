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

//! OpenAI-compatible Chat Completions API client.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::AiConfig;

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    max_completion_tokens: u32,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

/// Send a completion request to an OpenAI-compatible endpoint.
pub(crate) async fn complete(
    client: &reqwest::Client,
    config: &AiConfig,
    system: &str,
    user: &str,
) -> Result<String> {
    let base = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com");
    let url = format!("{base}/v1/chat/completions");

    let body = ChatRequest {
        model: &config.model,
        max_completion_tokens: config.max_tokens,
        messages: vec![
            ChatMessage {
                role: "system",
                content: system,
            },
            ChatMessage {
                role: "user",
                content: user,
            },
        ],
    };

    let resp = client
        .post(&url)
        .bearer_auth(config.api_key())
        .header("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .json(&body)
        .send()
        .await
        .context("Failed to reach OpenAI API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("OpenAI API error ({status}): {text}");
    }

    let parsed: ChatResponse = resp.json().await.context("Failed to parse response")?;
    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .context("Empty response from OpenAI")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_deserialization() {
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"Hello!"}}]}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices[0].message.content, "Hello!");
    }
}
