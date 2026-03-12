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

//! Kubernetes operations for `ResQ` deployment.

use std::path::Path;
use std::process::{Command, Stdio};

use tokio::sync::mpsc;

/// Get pod status via `kubectl get pods` and map to `ContainerStatus`.
pub fn get_status(env: &str) -> Vec<crate::docker::ContainerStatus> {
    let namespace = format!("resq-{env}");
    let output = Command::new("kubectl")
        .args(["get", "pods", "-n", &namespace, "--no-headers"])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 3 {
                        return None;
                    }
                    let name = parts[0];
                    let ready = parts[1];
                    let status = parts[2];

                    // Simple heuristic to match service name
                    let service = [
                        "infrastructure-api",
                        "coordination-hce",
                        "intelligence-pdie",
                        "web-dashboard",
                    ]
                    .iter()
                    .find(|&s| name.starts_with(s))
                    .map_or_else(|| "unknown".to_string(), |&s| s.to_string());

                    Some(crate::docker::ContainerStatus {
                        service,
                        state: status.to_lowercase(), // e.g. "Running" -> "running"
                        status: format!("Ready: {ready}"),
                    })
                })
                .collect()
        },
        Err(_) => Vec::new(),
    }
}

/// Run a kubectl action, streaming output to the channel.
pub fn run_action(
    project_root: &Path,
    env: &str,
    action: &str,
    service: Option<&str>,
    tx: mpsc::UnboundedSender<String>,
) -> Result<(), String> {
    let overlay_dir = project_root.join("infra/k8s/overlays").join(env);
    let namespace = format!("resq-{env}");

    let (cmd, args): (&str, Vec<String>) = match action {
        "deploy" => {
            let dir_str = overlay_dir
                .to_str()
                .ok_or("invalid overlay path")?
                .to_string();
            ("kubectl", vec!["apply".into(), "-k".into(), dir_str])
        },
        "destroy" => {
            let dir_str = overlay_dir
                .to_str()
                .ok_or("invalid overlay path")?
                .to_string();
            (
                "kubectl",
                vec![
                    "delete".into(),
                    "-k".into(),
                    dir_str,
                    "--ignore-not-found".into(),
                ],
            )
        },
        "status" => (
            "kubectl",
            vec![
                "get".into(),
                "pods".into(),
                "-n".into(),
                namespace,
                "-o".into(),
                "wide".into(),
            ],
        ),
        "logs" => {
            let svc = service.ok_or("Service name required for logs")?;
            (
                "kubectl",
                vec![
                    "logs".into(),
                    "-f".into(),
                    format!("deployment/{svc}"),
                    "-n".into(),
                    namespace,
                ],
            )
        },
        _ => return Err(format!("Unknown k8s action: {action}")),
    };

    let _ = tx.send(format!("$ {cmd} {}", args.join(" ")));

    let mut child = Command::new(cmd)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn: {e}"))?;

    if let Some(stdout) = child.stdout.take() {
        crate::docker::spawn_output_stream(stdout, tx.clone(), None);
    }

    if let Some(stderr) = child.stderr.take() {
        crate::docker::spawn_output_stream(stderr, tx.clone(), Some("[stderr] "));
    }

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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn namespace_format_dev() {
        let ns = format!("resq-{}", "dev");
        assert_eq!(ns, "resq-dev");
    }

    #[test]
    fn namespace_format_prod() {
        let ns = format!("resq-{}", "prod");
        assert_eq!(ns, "resq-prod");
    }

    #[test]
    fn overlay_dir_construction() {
        let root = PathBuf::from("/home/user/resQ");
        let overlay = root.join("infra/k8s/overlays").join("staging");
        assert_eq!(
            overlay,
            PathBuf::from("/home/user/resQ/infra/k8s/overlays/staging")
        );
    }
}
