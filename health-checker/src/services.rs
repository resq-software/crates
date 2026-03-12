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

//! Service health check implementations for `ResQ` endpoints.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

/// Health status of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Health information for a single service.
#[derive(Debug, Clone)]
pub(crate) struct ServiceHealth {
    pub name: String,
    pub url: String,
    pub status: HealthStatus,
    pub latency_ms: u64,
    pub error: Option<String>,
}

/// Registry of all services to monitor.
pub(crate) struct ServiceRegistry {
    client: Client,
    services: Vec<ServiceHealth>,
}

impl ServiceRegistry {
    /// Create a new service registry.
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be initialised.
    pub(crate) fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .context("failed to create HTTP client")?;

        // Default service endpoints (can be overridden via env vars)
        let hce_url =
            std::env::var("HCE_URL").unwrap_or_else(|_| "http://localhost:5000".to_string());
        let infra_url =
            std::env::var("INFRA_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
        let pdie_url =
            std::env::var("PDIE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());
        let neo_url =
            std::env::var("NEO_RPC_URL").unwrap_or_else(|_| "http://localhost:20332".to_string());
        let ipfs_url =
            std::env::var("IPFS_URL").unwrap_or_else(|_| "http://localhost:8081".to_string());

        let services = vec![
            ServiceHealth {
                name: "coordination-hce".to_string(),
                url: format!("{hce_url}/health"),
                status: HealthStatus::Unknown,
                latency_ms: 0,
                error: None,
            },
            ServiceHealth {
                name: "infrastructure-api".to_string(),
                url: format!("{infra_url}/health"),
                status: HealthStatus::Unknown,
                latency_ms: 0,
                error: None,
            },
            ServiceHealth {
                name: "intelligence-pdie".to_string(),
                url: format!("{pdie_url}/health"),
                status: HealthStatus::Unknown,
                latency_ms: 0,
                error: None,
            },
            ServiceHealth {
                name: "neo-n3-rpc".to_string(),
                url: neo_url,
                status: HealthStatus::Unknown,
                latency_ms: 0,
                error: None,
            },
            ServiceHealth {
                name: "ipfs-gateway".to_string(),
                url: format!("{ipfs_url}/api/v0/version"),
                status: HealthStatus::Unknown,
                latency_ms: 0,
                error: None,
            },
        ];

        Ok(Self { client, services })
    }
    /// Check all services concurrently.
    pub(crate) async fn check_all(&mut self) {
        let futures: Vec<_> = self
            .services
            .iter()
            .map(|s| check_service(&self.client, s.name.clone(), s.url.clone()))
            .collect();

        let results = futures::future::join_all(futures).await;

        for (i, result) in results.into_iter().enumerate() {
            self.services[i] = result;
        }
    }

    /// Get a reference to all services.
    pub(crate) fn services(&self) -> &[ServiceHealth] {
        &self.services
    }

    /// Get summary (healthy count, total count).
    pub(crate) fn summary(&self) -> (usize, usize) {
        let healthy = self
            .services
            .iter()
            .filter(|s| s.status == HealthStatus::Healthy)
            .count();
        (healthy, self.services.len())
    }
}

/// Response from a standard health endpoint.
#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
}

/// Response from Neo N3 RPC getversion.
#[derive(Debug, Deserialize)]
struct NeoRpcResponse {
    result: Option<NeoVersion>,
}

#[derive(Debug, Deserialize)]
struct NeoVersion {
    #[serde(rename = "user_agent")]
    #[allow(dead_code)]
    user_agent: Option<String>,
}

/// Check a single service's health.
async fn check_service(client: &Client, name: String, url: String) -> ServiceHealth {
    let start = Instant::now();

    // Special handling for Neo N3 RPC (JSON-RPC)
    if name == "neo-n3-rpc" {
        return check_neo_rpc(client, name, url, start).await;
    }

    // Standard HTTP health check
    match client.get(&url).send().await {
        Ok(resp) => {
            let latency_ms = start.elapsed().as_millis() as u64;

            if resp.status().is_success() {
                // Try to parse JSON response
                match resp.json::<HealthResponse>().await {
                    Ok(health) => {
                        let status = if health.status == "ok" {
                            HealthStatus::Healthy
                        } else {
                            HealthStatus::Degraded
                        };
                        ServiceHealth {
                            name,
                            url,
                            status,
                            latency_ms,
                            error: None,
                        }
                    },
                    Err(_) => {
                        // Response was 200 but not JSON - still consider healthy
                        ServiceHealth {
                            name,
                            url,
                            status: HealthStatus::Healthy,
                            latency_ms,
                            error: None,
                        }
                    },
                }
            } else {
                ServiceHealth {
                    name,
                    url,
                    status: HealthStatus::Unhealthy,
                    latency_ms,
                    error: Some(format!("HTTP {}", resp.status())),
                }
            }
        },
        Err(e) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            let error_msg = if e.is_connect() {
                "Connection refused".to_string()
            } else if e.is_timeout() {
                "Timeout".to_string()
            } else {
                e.to_string()
            };

            ServiceHealth {
                name,
                url,
                status: HealthStatus::Unhealthy,
                latency_ms,
                error: Some(error_msg),
            }
        },
    }
}

/// Check Neo N3 RPC via JSON-RPC getversion call.
async fn check_neo_rpc(
    client: &Client,
    name: String,
    url: String,
    start: Instant,
) -> ServiceHealth {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "getversion",
        "params": [],
        "id": 1
    });

    match client.post(&url).json(&payload).send().await {
        Ok(resp) => {
            let latency_ms = start.elapsed().as_millis() as u64;

            if resp.status().is_success() {
                match resp.json::<NeoRpcResponse>().await {
                    Ok(rpc) if rpc.result.is_some() => ServiceHealth {
                        name,
                        url,
                        status: HealthStatus::Healthy,
                        latency_ms,
                        error: None,
                    },
                    _ => ServiceHealth {
                        name,
                        url,
                        status: HealthStatus::Degraded,
                        latency_ms,
                        error: Some("Invalid RPC response".to_string()),
                    },
                }
            } else {
                ServiceHealth {
                    name,
                    url,
                    status: HealthStatus::Unhealthy,
                    latency_ms,
                    error: Some(format!("HTTP {}", resp.status())),
                }
            }
        },
        Err(e) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            ServiceHealth {
                name,
                url,
                status: HealthStatus::Unhealthy,
                latency_ms,
                error: Some(if e.is_connect() {
                    "Connection refused".to_string()
                } else if e.is_timeout() {
                    "Timeout".to_string()
                } else {
                    e.to_string()
                }),
            }
        },
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_default_has_five_services() {
        let registry = ServiceRegistry::new().unwrap();
        assert_eq!(registry.services().len(), 5);
    }

    #[test]
    fn service_health_defaults_to_unknown() {
        let registry = ServiceRegistry::new().unwrap();
        for svc in registry.services() {
            assert_eq!(svc.status, HealthStatus::Unknown);
            assert_eq!(svc.latency_ms, 0);
            assert!(svc.error.is_none());
        }
    }

    #[test]
    fn summary_all_unknown_means_zero_healthy() {
        let registry = ServiceRegistry::new().unwrap();
        let (healthy, total) = registry.summary();
        assert_eq!(healthy, 0);
        assert_eq!(total, 5);
    }

    #[test]
    fn health_status_equality() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
        assert_ne!(HealthStatus::Degraded, HealthStatus::Unknown);
    }
}
