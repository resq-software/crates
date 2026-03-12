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

//! Docker operations for `ResQ` deployment.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;
use tokio::sync::mpsc;

/// Container status from `docker compose ps`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ContainerStatus {
    pub service: String,
    pub state: String,
    pub status: String,
}

/// Parsed output of `docker compose ps --format json`.
#[derive(Debug, Deserialize)]
struct ComposePs {
    #[serde(alias = "Service")]
    service: String,
    #[serde(alias = "State")]
    state: String,
    #[serde(alias = "Status")]
    status: String,
}

/// Get compose directory and build the compose file arguments.
#[must_use]
fn compose_dir(project_root: &Path) -> PathBuf {
    project_root.join("infra/docker")
}

#[must_use]
fn compose_files(env: &str) -> Vec<String> {
    let mut files = vec!["-f".into(), "docker-compose.yml".into()];
    match env {
        "dev" => {
            files.extend_from_slice(&["-f".into(), "docker-compose.dev.yml".into()]);
        },
        "prod" => {
            files.extend_from_slice(&["-f".into(), "docker-compose.prod.yml".into()]);
        },
        _ => {},
    }
    files
}

/// Get the status of all containers.
pub fn get_status(project_root: &Path, env: &str) -> Vec<ContainerStatus> {
    let dir = compose_dir(project_root);
    let mut args = vec!["compose".to_string()];
    args.extend(compose_files(env));
    args.extend(["ps".into(), "--format".into(), "json".into()]);

    let output = Command::new("docker")
        .args(&args)
        .current_dir(&dir)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // Each line is a JSON object
            stdout
                .lines()
                .filter_map(|line| {
                    let parsed: ComposePs = serde_json::from_str(line).ok()?;
                    Some(ContainerStatus {
                        service: parsed.service,
                        state: parsed.state,
                        status: parsed.status,
                    })
                })
                .collect()
        },
        Err(_) => Vec::new(),
    }
}

/// Run a docker compose action, streaming stdout/stderr to the channel.
pub fn run_action(
    project_root: &Path,
    env: &str,
    action: &str,
    service: Option<&str>,
    tx: mpsc::UnboundedSender<String>,
) -> Result<(), String> {
    let dir = compose_dir(project_root);
    let mut args = vec!["compose".to_string()];
    args.extend(compose_files(env));

    match action {
        "build" => {
            args.push("build".into());
            if let Some(svc) = service {
                args.push(svc.into());
            }
        },
        "up" => {
            args.extend(["up".into(), "-d".into(), "--build".into()]);
            if let Some(svc) = service {
                args.push(svc.into());
            }
        },
        "down" => {
            args.push("down".into());
        },
        "restart" => {
            args.push("restart".into());
            if let Some(svc) = service {
                args.push(svc.into());
            }
        },
        "logs" => {
            args.extend(["logs".into(), "-f".into(), "--tail".into(), "100".into()]);
            if let Some(svc) = service {
                args.push(svc.into());
            }
        },
        _ => return Err(format!("Unknown action: {action}")),
    }

    let _ = tx.send(format!("$ docker {}", args.join(" ")));

    let mut child = Command::new("docker")
        .args(&args)
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn: {e}"))?;

    // Stream stdout
    if let Some(stdout) = child.stdout.take() {
        spawn_output_stream(stdout, tx.clone(), None);
    }

    // Stream stderr
    if let Some(stderr) = child.stderr.take() {
        spawn_output_stream(stderr, tx.clone(), Some("[stderr] "));
    }

    // Wait for completion in background
    std::thread::spawn(move || match child.wait() {
        Ok(status) => {
            let _ = tx.send(format!(
                "--- Process exited with {} ---",
                status.code().unwrap_or(-1)
            ));
        },
        Err(e) => {
            let _ = tx.send(format!("--- Process error: {e} ---"));
        },
    });

    Ok(())
}

/// Spawn a thread that reads lines from a pipe and sends them to a channel.
pub fn spawn_output_stream<R: std::io::Read + Send + 'static>(
    pipe: R,
    tx: mpsc::UnboundedSender<String>,
    prefix: Option<&'static str>,
) {
    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(pipe);
        for line in reader.lines().map_while(Result::ok) {
            let msg = match prefix {
                Some(p) => format!("{p}{line}"),
                None => line,
            };
            if tx.send(msg).is_err() {
                break;
            }
        }
    });
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_dir_returns_infra_docker() {
        let root = Path::new("/home/user/resQ");
        assert_eq!(
            compose_dir(root),
            PathBuf::from("/home/user/resQ/infra/docker")
        );
    }

    #[test]
    fn compose_files_dev_includes_dev_override() {
        let files = compose_files("dev");
        assert_eq!(
            files,
            vec!["-f", "docker-compose.yml", "-f", "docker-compose.dev.yml"]
        );
    }

    #[test]
    fn compose_files_prod_includes_prod_override() {
        let files = compose_files("prod");
        assert_eq!(
            files,
            vec!["-f", "docker-compose.yml", "-f", "docker-compose.prod.yml"]
        );
    }

    #[test]
    fn compose_files_staging_is_base_only() {
        let files = compose_files("staging");
        assert_eq!(files, vec!["-f", "docker-compose.yml"]);
    }

    #[test]
    fn compose_files_unknown_env_is_base_only() {
        let files = compose_files("test");
        assert_eq!(files, vec!["-f", "docker-compose.yml"]);
    }
}
