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

//! Log sources: Docker Compose streaming and file tailing.

use std::io::BufRead;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::sync::mpsc;

use crate::parser::{parse_line, LogEntry};

/// Spawn a Docker Compose log stream, sending parsed entries to the channel.
///
/// Runs `docker compose logs -f --no-color` from the infra/docker directory.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn spawn_docker_source(
    project_root: PathBuf,
    service_filter: Option<String>,
    tx: mpsc::UnboundedSender<LogEntry>,
) -> std::io::Result<()> {
    let compose_dir = project_root.join("infra/docker");

    let mut cmd = std::process::Command::new("docker");
    cmd.args(["compose", "logs", "-f", "--no-color", "--tail", "200"]);

    if let Some(ref svc) = service_filter {
        cmd.arg(svc);
    }

    cmd.current_dir(&compose_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("no stdout"))?;

    let default_svc = service_filter.unwrap_or_else(|| "docker".to_string());

    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(l) if !l.trim().is_empty() => {
                    let entry = parse_line(&l, &default_svc);
                    if tx.send(entry).is_err() {
                        break;
                    }
                }
                Err(_) => break,
                _ => {}
            }
        }
        let _ = child.kill();
    });

    Ok(())
}

/// Spawn a file tail source, sending parsed entries to the channel.
///
/// Reads the last N lines of a file and then watches for new content.
pub(crate) fn spawn_file_source(path: PathBuf, tx: mpsc::UnboundedSender<LogEntry>) {
    let service_name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    std::thread::spawn(move || {
        // Read existing content
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.send(LogEntry {
                    timestamp: None,
                    level: crate::parser::LogLevel::Error,
                    service: service_name.clone(),
                    message: format!("Failed to open {}: {}", path.display(), e),
                    raw: String::new(),
                });
                return;
            }
        };

        let reader = std::io::BufReader::new(file);
        for line in reader.lines() {
            match line {
                Ok(l) if !l.trim().is_empty() => {
                    let entry = parse_line(&l, &service_name);
                    if tx.send(entry).is_err() {
                        return;
                    }
                }
                Err(_) => return,
                _ => {}
            }
        }
    });
}
