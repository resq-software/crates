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

/// Read newline-delimited lines from `reader`, decoding invalid UTF-8 lossily
/// (U+FFFD) rather than aborting the stream. Non-empty lines are passed to
/// `on_line`; iteration stops on EOF, a genuine I/O error, or when `on_line`
/// returns `false`. Previously a single invalid-UTF-8 byte ended the whole log
/// stream because `BufRead::lines()` surfaces it as `Err(InvalidData)`.
fn for_each_line<R: BufRead>(mut reader: R, mut on_line: impl FnMut(&str) -> bool) {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            // EOF (Ok(0)) or a genuine I/O error both end the loop. A UTF-8
            // decode error does NOT reach here — read_until reads raw bytes and
            // decoding happens below via from_utf8_lossy, which is the fix.
            Ok(0) | Err(_) => break,
            Ok(_) => {
                while matches!(buf.last(), Some(b'\n' | b'\r')) {
                    buf.pop();
                }
                let line = String::from_utf8_lossy(&buf);
                if !line.trim().is_empty() && !on_line(line.as_ref()) {
                    break;
                }
            }
        }
    }
}

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
        for_each_line(reader, |l| tx.send(parse_line(l, &default_svc)).is_ok());
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
        for_each_line(reader, |l| tx.send(parse_line(l, &service_name)).is_ok());
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn for_each_line_survives_invalid_utf8() {
        // A middle line with invalid UTF-8 must be decoded lossily, not end the
        // stream — the whole point of the fix.
        let data: &[u8] = b"line one\n\xff\xfe not valid utf8\nline three\n";
        let mut got = Vec::new();
        for_each_line(Cursor::new(data), |l| {
            got.push(l.to_string());
            true
        });
        assert_eq!(
            got.len(),
            3,
            "invalid-UTF-8 line must not terminate the stream"
        );
        assert_eq!(got[0], "line one");
        assert!(
            got[2].contains("three"),
            "line after the bad one must still arrive"
        );
    }

    #[test]
    fn for_each_line_skips_blank_lines_and_trims_crlf() {
        let data: &[u8] = b"a\r\n\r\n   \r\nb\n";
        let mut got = Vec::new();
        for_each_line(Cursor::new(data), |l| {
            got.push(l.to_string());
            true
        });
        assert_eq!(got, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn for_each_line_stops_when_callback_returns_false() {
        let data: &[u8] = b"one\ntwo\nthree\n";
        let mut count = 0;
        for_each_line(Cursor::new(data), |_| {
            count += 1;
            count < 2 // stop after the second line
        });
        assert_eq!(count, 2, "iteration must stop when on_line returns false");
    }
}
