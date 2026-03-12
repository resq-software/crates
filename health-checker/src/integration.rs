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

//! Integration test runner for `ResQ`.

use std::process::Command;
use std::time::Instant;

/// Status of an integration test.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum TestStatus {
    Running,
    Passed,
    Failed,
    Skipped,
}

/// Result of an integration test execution.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub duration_ms: u64,
    pub output: String,
}

/// Run a shell script as an integration test.
#[allow(dead_code)]
pub(crate) fn run_test_script(script_path: &str) -> TestResult {
    let start = Instant::now();
    let name = std::path::Path::new(script_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    match Command::new("bash").arg(script_path).output() {
        Ok(output) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let status = if output.status.success() {
                TestStatus::Passed
            } else {
                TestStatus::Failed
            };

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined_output = format!("STDOUT:\n{stdout}\n\nSTDERR:\n{stderr}");

            TestResult {
                name,
                status,
                duration_ms,
                output: combined_output,
            }
        },
        Err(e) => TestResult {
            name,
            status: TestStatus::Failed,
            duration_ms: start.elapsed().as_millis() as u64,
            output: format!("Failed to execute script: {e}"),
        },
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_equality() {
        assert_eq!(TestStatus::Passed, TestStatus::Passed);
        assert_ne!(TestStatus::Passed, TestStatus::Failed);
        assert_ne!(TestStatus::Running, TestStatus::Skipped);
    }

    #[test]
    fn test_result_construction() {
        let result = TestResult {
            name: "smoke_test.sh".to_string(),
            status: TestStatus::Passed,
            duration_ms: 42,
            output: "STDOUT:\nok\n\nSTDERR:\n".to_string(),
        };
        assert_eq!(result.name, "smoke_test.sh");
        assert_eq!(result.status, TestStatus::Passed);
        assert_eq!(result.duration_ms, 42);
    }

    #[test]
    fn script_name_extraction() {
        let path = std::path::Path::new("/project/tests/scripts/smoke_test.sh");
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(name, "smoke_test.sh");
    }
}
