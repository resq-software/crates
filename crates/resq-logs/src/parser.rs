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

//! Log line parser supporting JSON structured, Docker Compose prefix,
//! `RUST_LOG`, and plain text formats.

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Severity level of a log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Parse a level string (case-insensitive).
    pub(crate) fn from_str_loose(s: &str) -> Self {
        if s.eq_ignore_ascii_case("ERROR")
            || s.eq_ignore_ascii_case("ERR")
            || s.eq_ignore_ascii_case("FATAL")
            || s.eq_ignore_ascii_case("CRITICAL")
        {
            Self::Error
        } else if s.eq_ignore_ascii_case("WARN") || s.eq_ignore_ascii_case("WARNING") {
            Self::Warn
        } else if s.eq_ignore_ascii_case("INFO") {
            Self::Info
        } else if s.eq_ignore_ascii_case("DEBUG") || s.eq_ignore_ascii_case("DBG") {
            Self::Debug
        } else if s.eq_ignore_ascii_case("TRACE") {
            Self::Trace
        } else {
            Self::Info
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A parsed log entry.
#[derive(Debug, Clone)]
pub(crate) struct LogEntry {
    pub timestamp: Option<DateTime<Utc>>,
    pub level: LogLevel,
    pub service: String,
    pub message: String,
    pub raw: String,
}

/// JSON structured log (common format).
#[derive(Debug, Deserialize)]
struct JsonLog {
    #[serde(alias = "lvl", alias = "severity")]
    level: Option<String>,
    #[serde(alias = "msg", alias = "message")]
    #[serde(default)]
    msg: String,
    #[serde(alias = "time", alias = "ts", alias = "@timestamp")]
    timestamp: Option<String>,
    #[serde(alias = "service", alias = "component")]
    service: Option<String>,
}

/// Parse a raw log line into a structured `LogEntry`.
///
/// Attempts formats in order:
/// 1. Docker Compose prefix (`service-name  | ...`)
/// 2. JSON structured log
/// 3. `RUST_LOG` format (`2026-01-01T00:00:00Z INFO module: msg`)
/// 4. Plain text fallback
pub(crate) fn parse_line(raw: &str, default_service: &str) -> LogEntry {
    let trimmed = raw.trim();

    // 1. Docker Compose prefix: "service-name  | rest of line"
    if let Some((svc, rest)) = try_docker_prefix(trimmed) {
        // The rest might be JSON or plain text
        if let Some(mut entry) = try_json(rest) {
            if entry.service.is_empty() {
                entry.service = svc;
            }
            entry.raw = raw.to_string();
            return entry;
        }
        if let Some(mut entry) = try_rust_log(rest) {
            if entry.service.is_empty() {
                entry.service = svc;
            }
            entry.raw = raw.to_string();
            return entry;
        }
        return LogEntry {
            timestamp: None,
            level: guess_level(rest),
            service: svc,
            message: rest.to_string(),
            raw: raw.to_string(),
        };
    }

    // 2. JSON structured
    if let Some(entry) = try_json(trimmed) {
        return LogEntry {
            raw: raw.to_string(),
            ..entry
        };
    }

    // 3. RUST_LOG format
    if let Some(entry) = try_rust_log(trimmed) {
        return LogEntry {
            raw: raw.to_string(),
            ..entry
        };
    }

    // 4. Fallback
    LogEntry {
        timestamp: None,
        level: guess_level(trimmed),
        service: default_service.to_string(),
        message: trimmed.to_string(),
        raw: raw.to_string(),
    }
}

/// Try to extract `service | rest` from Docker Compose output.
fn try_docker_prefix(line: &str) -> Option<(String, &str)> {
    // Docker compose format: "container-name  | message"
    let pipe_pos = line.find(" | ")?;
    let svc = line[..pipe_pos].trim();
    // Service names are alphanumeric + hyphens, no spaces
    if svc.is_empty() || svc.contains(' ') {
        return None;
    }
    let rest = &line[pipe_pos + 3..];
    // Strip "resq-" prefix if present
    let svc = svc.strip_prefix("resq-").unwrap_or(svc);
    Some((svc.to_string(), rest))
}

/// Try parsing as JSON structured log.
fn try_json(line: &str) -> Option<LogEntry> {
    if !line.starts_with('{') {
        return None;
    }
    let parsed: JsonLog = serde_json::from_str(line).ok()?;
    let level = parsed
        .level
        .as_deref()
        .map_or(LogLevel::Info, LogLevel::from_str_loose);
    let timestamp = parsed
        .timestamp
        .as_deref()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Some(LogEntry {
        timestamp,
        level,
        service: parsed.service.unwrap_or_default(),
        message: parsed.msg,
        raw: line.to_string(),
    })
}

/// Try parsing `RUST_LOG` format: `2026-01-01T00:00:00Z INFO module::path: message`
fn try_rust_log(line: &str) -> Option<LogEntry> {
    // Expect ISO timestamp at the start
    if line.len() < 25 || !line.as_bytes()[4].is_ascii_punctuation() {
        return None;
    }
    let ts_end = line.find(' ')?;
    let ts_str = &line[..ts_end];
    let ts = DateTime::parse_from_rfc3339(ts_str)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))?;

    let rest = line[ts_end + 1..].trim_start();
    let level_end = rest.find(' ').unwrap_or(rest.len());
    let level = LogLevel::from_str_loose(&rest[..level_end]);
    let after_level = rest[level_end..].trim_start();

    // Module path before colon
    let (service, message) = if let Some(colon_pos) = after_level.find(": ") {
        let module = &after_level[..colon_pos];
        let msg = &after_level[colon_pos + 2..];
        (module.to_string(), msg.to_string())
    } else {
        (String::new(), after_level.to_string())
    };

    Some(LogEntry {
        timestamp: Some(ts),
        level,
        service,
        message,
        raw: line.to_string(),
    })
}

/// Guess log level from keywords in the line.
fn guess_level(line: &str) -> LogLevel {
    let upper = line.to_ascii_uppercase();
    if upper.contains("ERROR") || upper.contains("FATAL") || upper.contains("PANIC") {
        LogLevel::Error
    } else if upper.contains("WARN") {
        LogLevel::Warn
    } else if upper.contains("DEBUG") {
        LogLevel::Debug
    } else if upper.contains("TRACE") {
        LogLevel::Trace
    } else {
        LogLevel::Info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_prefix() {
        let line = "resq-coordination-hce  | Server started on port 3001";
        let entry = parse_line(line, "unknown");
        assert_eq!(entry.service, "coordination-hce");
        assert_eq!(entry.message, "Server started on port 3001");
    }

    #[test]
    fn test_json_log() {
        let line =
            r#"{"level":"error","msg":"connection refused","timestamp":"2026-02-09T12:00:00Z"}"#;
        let entry = parse_line(line, "test");
        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.message, "connection refused");
    }

    #[test]
    fn test_plain_fallback() {
        let line = "Something happened with an ERROR here";
        let entry = parse_line(line, "default-svc");
        assert_eq!(entry.service, "default-svc");
        assert_eq!(entry.level, LogLevel::Error);
    }

    #[test]
    fn test_rust_log_format() {
        let line = "2026-01-15T10:30:00Z INFO resq_worker::queue: Processing job #1234";
        let entry = parse_line(line, "fallback");
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.service, "resq_worker::queue");
        assert_eq!(entry.message, "Processing job #1234");
        assert!(entry.timestamp.is_some());
    }

    #[test]
    fn test_json_with_aliases() {
        // Uses "lvl" and "msg" aliases instead of "level" and "message"
        let line = r#"{"lvl":"warn","msg":"disk almost full","service":"db"}"#;
        let entry = parse_line(line, "test");
        assert_eq!(entry.level, LogLevel::Warn);
        assert_eq!(entry.message, "disk almost full");
        assert_eq!(entry.service, "db");
    }

    #[test]
    fn test_json_with_severity_alias() {
        let line = r#"{"severity":"debug","message":"query plan","component":"postgres"}"#;
        let entry = parse_line(line, "test");
        assert_eq!(entry.level, LogLevel::Debug);
        assert_eq!(entry.service, "postgres");
    }

    #[test]
    fn test_empty_line() {
        let entry = parse_line("", "svc");
        assert_eq!(entry.service, "svc");
        assert_eq!(entry.level, LogLevel::Info);
    }

    #[test]
    fn test_malformed_json() {
        // Starts with { but isn't valid JSON — should fall through to plain text
        let line = r"{ broken json";
        let entry = parse_line(line, "svc");
        assert_eq!(entry.service, "svc");
        assert_eq!(entry.level, LogLevel::Info);
    }

    #[test]
    fn test_docker_prefix_with_json_body() {
        let line =
            r#"resq-api  | {"level":"error","msg":"timeout","timestamp":"2026-03-01T00:00:00Z"}"#;
        let entry = parse_line(line, "unknown");
        assert_eq!(entry.service, "api");
        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.message, "timeout");
    }

    #[test]
    fn test_guess_level_keywords() {
        assert_eq!(guess_level("FATAL: system crash"), LogLevel::Error);
        assert_eq!(guess_level("PANIC at the disco"), LogLevel::Error);
        assert_eq!(guess_level("WARNING: low memory"), LogLevel::Warn);
        assert_eq!(guess_level("DEBUG: variable x=5"), LogLevel::Debug);
        assert_eq!(guess_level("TRACE entering fn"), LogLevel::Trace);
        assert_eq!(guess_level("normal message"), LogLevel::Info);
    }

    #[test]
    fn test_level_parsing_case_insensitive() {
        let line = r#"{"level":"ERROR","msg":"fail"}"#;
        let entry = parse_line(line, "test");
        assert_eq!(entry.level, LogLevel::Error);
    }
}
