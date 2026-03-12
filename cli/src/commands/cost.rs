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

//! Cloud cost estimation command.
//!
//! Analyzes infrastructure configuration to estimate cloud costs across
//! different providers and usage patterns.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Semaphore;

/// CLI arguments for the project cost estimation command.
#[derive(clap::Args, Debug)]
pub struct CostArgs {
    /// Root directory containing project manifest
    #[arg(long, default_value = ".")]
    pub root: PathBuf,

    /// Output directory
    #[arg(long, default_value = "scripts/out")]
    pub output: PathBuf,

    /// Force a specific project type (node, rust, python)
    #[arg(long)]
    pub project_type: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum ProjectType {
    Node,
    Rust,
    Python,
}

impl ProjectType {
    fn detect(root: &Path) -> Result<Self> {
        if root.join("Cargo.toml").exists() {
            return Ok(ProjectType::Rust);
        }
        if root.join("package.json").exists() {
            return Ok(ProjectType::Node);
        }
        if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
            return Ok(ProjectType::Python);
        }
        anyhow::bail!("No recognized project manifest found (Cargo.toml, package.json, pyproject.toml, or requirements.txt)")
    }

    fn from_string(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "node" | "nodejs" | "npm" => Ok(ProjectType::Node),
            "rust" | "cargo" => Ok(ProjectType::Rust),
            "python" | "py" | "pip" => Ok(ProjectType::Python),
            _ => anyhow::bail!("Unknown project type: {s}"),
        }
    }
}

// Node.js structures
#[derive(Deserialize)]
struct PackageJson {
    dependencies: Option<HashMap<String, String>>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: Option<HashMap<String, String>>,
}

// Rust structures
#[derive(Deserialize)]
struct CargoToml {
    dependencies: Option<HashMap<String, toml::Value>>,
    #[serde(rename = "dev-dependencies")]
    dev_dependencies: Option<HashMap<String, toml::Value>>,
}

#[derive(Deserialize)]
struct CratesIoResponse {
    #[serde(rename = "crate")]
    crate_info: CrateInfo,
}

#[derive(Deserialize)]
struct CrateInfo {
    newest_version: String,
}

// Python structures
#[derive(Deserialize)]
struct PyProjectToml {
    project: Option<PyProject>,
    tool: Option<ToolSection>,
}

#[derive(Deserialize)]
struct PyProject {
    dependencies: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ToolSection {
    poetry: Option<PoetrySection>,
}

#[derive(Deserialize)]
struct PoetrySection {
    dependencies: Option<HashMap<String, toml::Value>>,
    #[serde(rename = "dev-dependencies")]
    dev_dependencies: Option<HashMap<String, toml::Value>>,
}

#[derive(Serialize, Clone)]
struct PackageSize {
    name: String,
    size: u64,
}

// Node.js implementation
async fn get_npm_package_size(name: String, semaphore: Arc<Semaphore>) -> Option<PackageSize> {
    let _permit = semaphore.acquire().await.ok()?;

    let output = Command::new("npm")
        .args(["view", &name, "dist.unpackedSize", "--json"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let size: u64 = serde_json::from_str(&text).ok()?;

    Some(PackageSize { name, size })
}

fn parse_node_deps(root: &Path) -> Result<Vec<String>> {
    let pkg_path = root.join("package.json");
    let pkg_content = fs::read_to_string(&pkg_path).context("Failed to read package.json")?;
    let pkg: PackageJson =
        serde_json::from_str(&pkg_content).context("Failed to parse package.json")?;

    let mut all_deps = Vec::new();
    if let Some(deps) = pkg.dependencies {
        all_deps.extend(deps.into_keys());
    }
    if let Some(dev_deps) = pkg.dev_dependencies {
        all_deps.extend(dev_deps.into_keys());
    }

    Ok(all_deps)
}

// Rust implementation
async fn get_crate_size(name: String, semaphore: Arc<Semaphore>) -> Option<PackageSize> {
    let _permit = semaphore.acquire().await.ok()?;

    // First, get the latest version
    let client = reqwest::Client::new();
    let crate_url = format!("https://crates.io/api/v1/crates/{name}");

    let response = client
        .get(&crate_url)
        .header("User-Agent", "dependency-cost-analyzer")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let crate_info: CratesIoResponse = response.json().await.ok()?;
    let version = crate_info.crate_info.newest_version;

    // Now get the version-specific info
    let version_url = format!("https://crates.io/api/v1/crates/{name}/{version}");
    let version_response = client
        .get(&version_url)
        .header("User-Agent", "dependency-cost-analyzer")
        .send()
        .await
        .ok()?;

    if !version_response.status().is_success() {
        return None;
    }

    let version_info: serde_json::Value = version_response.json().await.ok()?;

    // crates.io provides crate_size which is the .crate file size
    let size = version_info["version"]["crate_size"].as_u64()?;

    Some(PackageSize { name, size })
}

fn parse_rust_deps(root: &Path) -> Result<Vec<String>> {
    let cargo_path = root.join("Cargo.toml");
    let cargo_content = fs::read_to_string(&cargo_path).context("Failed to read Cargo.toml")?;
    let cargo: CargoToml = toml::from_str(&cargo_content).context("Failed to parse Cargo.toml")?;

    let mut all_deps = Vec::new();

    if let Some(deps) = cargo.dependencies {
        all_deps.extend(deps.into_keys());
    }
    if let Some(dev_deps) = cargo.dev_dependencies {
        all_deps.extend(dev_deps.into_keys());
    }

    Ok(all_deps)
}

// Python implementation
async fn get_pypi_package_size(name: String, semaphore: Arc<Semaphore>) -> Option<PackageSize> {
    let _permit = semaphore.acquire().await.ok()?;

    let client = reqwest::Client::new();
    let url = format!("https://pypi.org/pypi/{name}/json");

    let response = client
        .get(&url)
        .header("User-Agent", "dependency-cost-analyzer")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let data: serde_json::Value = response.json().await.ok()?;

    // PyPI provides file sizes per distribution
    // We'll take the wheel file size if available, otherwise source distribution
    let urls = data["urls"].as_array()?;

    let size = urls
        .iter()
        .filter(|url| {
            url["packagetype"].as_str() == Some("bdist_wheel")
                || url["packagetype"].as_str() == Some("sdist")
        })
        .find_map(|url| url["size"].as_u64())?;

    Some(PackageSize { name, size })
}

fn parse_python_deps(root: &Path) -> Result<Vec<String>> {
    let mut all_deps = Vec::new();

    // Try pyproject.toml first
    let pyproject_path = root.join("pyproject.toml");
    if pyproject_path.exists() {
        let content =
            fs::read_to_string(&pyproject_path).context("Failed to read pyproject.toml")?;
        let pyproject: PyProjectToml =
            toml::from_str(&content).context("Failed to parse pyproject.toml")?;

        // Standard PEP 621 format
        if let Some(project) = pyproject.project {
            if let Some(deps) = project.dependencies {
                for dep in deps {
                    // Parse "package>=1.0.0" -> "package"
                    if let Some(name) = dep
                        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                        .next()
                    {
                        all_deps.push(name.to_string());
                    }
                }
            }
        }

        // Poetry format
        if let Some(tool) = pyproject.tool {
            if let Some(poetry) = tool.poetry {
                if let Some(deps) = poetry.dependencies {
                    all_deps.extend(deps.into_keys().filter(|k| k != "python"));
                }
                if let Some(dev_deps) = poetry.dev_dependencies {
                    all_deps.extend(dev_deps.into_keys());
                }
            }
        }
    }

    // Fallback to requirements.txt
    let req_path = root.join("requirements.txt");
    if req_path.exists() && all_deps.is_empty() {
        let content = fs::read_to_string(&req_path).context("Failed to read requirements.txt")?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Parse "package>=1.0.0" -> "package"
            if let Some(name) = line
                .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                .next()
            {
                all_deps.push(name.to_string());
            }
        }
    }

    if all_deps.is_empty() {
        anyhow::bail!("No dependencies found in pyproject.toml or requirements.txt");
    }

    Ok(all_deps)
}

/// Run the project cost estimation.
pub async fn run(args: CostArgs) -> Result<()> {
    let root = if args.root == std::path::Path::new(".") {
        crate::utils::find_project_root()
    } else {
        args.root
    };

    // Detect or use forced project type
    let project_type = if let Some(ref type_str) = args.project_type {
        ProjectType::from_string(type_str)?
    } else {
        ProjectType::detect(&root)?
    };

    println!("📦 Detected project type: {project_type:?}");

    // Parse dependencies based on project type
    let all_deps = match project_type {
        ProjectType::Node => parse_node_deps(&root)?,
        ProjectType::Rust => parse_rust_deps(&root)?,
        ProjectType::Python => parse_python_deps(&root)?,
    };

    println!("Fetching sizes for {} packages...", all_deps.len());

    let semaphore = Arc::new(Semaphore::new(10)); // BATCH_SIZE = 10
    let mut tasks = Vec::new();

    for dep in all_deps {
        let sem = semaphore.clone();
        let task = match project_type {
            ProjectType::Node => tokio::spawn(async move { get_npm_package_size(dep, sem).await }),
            ProjectType::Rust => tokio::spawn(async move { get_crate_size(dep, sem).await }),
            ProjectType::Python => {
                tokio::spawn(async move { get_pypi_package_size(dep, sem).await })
            }
        };
        tasks.push(task);
    }

    let mut package_sizes = Vec::new();
    for task in tasks {
        if let Ok(Some(size)) = task.await {
            package_sizes.push(size);
        }
    }

    package_sizes.sort_by(|a, b| b.size.cmp(&a.size));

    let low_threshold = 1_000_000; // 1 MB
    let medium_threshold = 10_000_000; // 10 MB

    let high: Vec<_> = package_sizes
        .iter()
        .filter(|p| p.size > medium_threshold)
        .cloned()
        .collect();
    let medium: Vec<_> = package_sizes
        .iter()
        .filter(|p| p.size > low_threshold && p.size <= medium_threshold)
        .cloned()
        .collect();
    let low: Vec<_> = package_sizes
        .iter()
        .filter(|p| p.size <= low_threshold)
        .cloned()
        .collect();

    let output_dir = root.join(args.output);
    fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

    fs::write(
        output_dir.join("high.json"),
        serde_json::to_string_pretty(&high.iter().map(|p| &p.name).collect::<Vec<_>>())?,
    )?;
    fs::write(
        output_dir.join("medium.json"),
        serde_json::to_string_pretty(&medium.iter().map(|p| &p.name).collect::<Vec<_>>())?,
    )?;
    fs::write(
        output_dir.join("low.json"),
        serde_json::to_string_pretty(&low.iter().map(|p| &p.name).collect::<Vec<_>>())?,
    )?;

    let total_size: u64 = package_sizes.iter().map(|p| p.size).sum();
    let total_mb = total_size as f64 / 1_000_000.0;

    println!("\n📦 Package Size Summary:");
    println!("   🔴 High (> 10 MB): {} packages", high.len());
    println!("   🟡 Medium (1-10 MB): {} packages", medium.len());
    println!("   🟢 Low (< 1 MB): {} packages", low.len());
    println!("\n📊 Total size: {total_mb:.2} MB");
    println!("\n📄 Results saved to {output_dir:?}/");

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ProjectType::from_string ─────────────────────────────────────────────

    #[test]
    fn from_string_node_variants() {
        assert!(matches!(
            ProjectType::from_string("node").unwrap(),
            ProjectType::Node
        ));
        assert!(matches!(
            ProjectType::from_string("nodejs").unwrap(),
            ProjectType::Node
        ));
        assert!(matches!(
            ProjectType::from_string("npm").unwrap(),
            ProjectType::Node
        ));
    }

    #[test]
    fn from_string_rust_variants() {
        assert!(matches!(
            ProjectType::from_string("rust").unwrap(),
            ProjectType::Rust
        ));
        assert!(matches!(
            ProjectType::from_string("cargo").unwrap(),
            ProjectType::Rust
        ));
    }

    #[test]
    fn from_string_python_variants() {
        assert!(matches!(
            ProjectType::from_string("python").unwrap(),
            ProjectType::Python
        ));
        assert!(matches!(
            ProjectType::from_string("py").unwrap(),
            ProjectType::Python
        ));
        assert!(matches!(
            ProjectType::from_string("pip").unwrap(),
            ProjectType::Python
        ));
    }

    #[test]
    fn from_string_case_insensitive() {
        assert!(matches!(
            ProjectType::from_string("NODE").unwrap(),
            ProjectType::Node
        ));
        assert!(matches!(
            ProjectType::from_string("Rust").unwrap(),
            ProjectType::Rust
        ));
    }

    #[test]
    fn from_string_unknown_errors() {
        assert!(ProjectType::from_string("java").is_err());
    }

    // ── ProjectType::detect ──────────────────────────────────────────────────

    #[test]
    fn detect_cargo_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();
        assert!(matches!(
            ProjectType::detect(dir.path()).unwrap(),
            ProjectType::Rust
        ));
    }

    #[test]
    fn detect_node_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert!(matches!(
            ProjectType::detect(dir.path()).unwrap(),
            ProjectType::Node
        ));
    }

    #[test]
    fn detect_python_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("requirements.txt"), "flask>=2.0\n").unwrap();
        assert!(matches!(
            ProjectType::detect(dir.path()).unwrap(),
            ProjectType::Python
        ));
    }

    #[test]
    fn detect_no_manifest_errors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ProjectType::detect(dir.path()).is_err());
    }

    // ── parse_node_deps ──────────────────────────────────────────────────────

    #[test]
    fn parse_node_deps_extracts_all() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies":{"react":"^18.0","next":"^14.0"},"devDependencies":{"jest":"^29.0"}}"#,
        )
        .unwrap();
        let mut deps = parse_node_deps(dir.path()).unwrap();
        deps.sort();
        assert_eq!(deps, vec!["jest", "next", "react"]);
    }

    // ── parse_rust_deps ──────────────────────────────────────────────────────

    #[test]
    fn parse_rust_deps_extracts_all() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"[dependencies]
serde = "1"
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
tempfile = "3"
"#,
        )
        .unwrap();
        let mut deps = parse_rust_deps(dir.path()).unwrap();
        deps.sort();
        assert_eq!(deps, vec!["serde", "tempfile", "tokio"]);
    }

    // ── parse_python_deps ────────────────────────────────────────────────────

    #[test]
    fn parse_python_deps_from_requirements_txt() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("requirements.txt"),
            "# comment\nflask>=2.0\nrequests\nnumpy==1.24\n",
        )
        .unwrap();
        let deps = parse_python_deps(dir.path()).unwrap();
        assert!(deps.contains(&"flask".to_string()));
        assert!(deps.contains(&"requests".to_string()));
        assert!(deps.contains(&"numpy".to_string()));
    }

    #[test]
    fn parse_python_deps_empty_errors() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("requirements.txt"), "# only comments\n").unwrap();
        assert!(parse_python_deps(dir.path()).is_err());
    }
}
