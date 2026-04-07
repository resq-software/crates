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

use anyhow::{anyhow, Context};
use base64::Engine;
use clap::Args;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

/// Arguments for the `docs` command.
#[derive(Args)]
pub struct DocsArgs {
    /// Only export the specifications locally without publishing
    #[arg(short, long)]
    pub export_only: bool,

    /// Publish the specifications to the documentation repository
    #[arg(short, long)]
    pub publish: bool,

    /// Dry run: show what would be done without executing
    #[arg(long)]
    pub dry_run: bool,
}

/// Run the documentation export and publication process.
///
/// # Errors
/// Returns an error if any of the export or publication steps fail,
/// or if there are issues accessing the file system or GitHub API.
pub fn run(args: DocsArgs) -> anyhow::Result<()> {
    let root_dir = crate::utils::find_project_root();

    // 1. Export Infrastructure API Spec
    header("Exporting Infrastructure API spec");
    export_infrastructure(&root_dir, &args)?;

    // 2. Export Coordination HCE Spec
    header("Exporting Coordination HCE spec");
    export_coordination(&root_dir, &args)?;

    if args.publish && !args.export_only {
        header("Publishing specifications to GitHub");
        publish_spec(
            &root_dir,
            "infrastructure.json",
            "specs/infrastructure.json",
            &args,
        )?;
        publish_spec(
            &root_dir,
            "coordination.json",
            "specs/coordination.json",
            &args,
        )?;
    }

    Ok(())
}

fn export_infrastructure(root: &Path, args: &DocsArgs) -> anyhow::Result<()> {
    let output_path = root.join("../docs/specs/infrastructure.json");

    if args.dry_run {
        println!(
            "[Dry Run] Would run: cargo run -p resq-api --bin export_openapi {}",
            output_path.display()
        );
        return Ok(());
    }

    let output = Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "resq-api",
            "--bin",
            "export_openapi",
            "--",
            output_path
                .to_str()
                .context("Output path contains invalid UTF-8")?,
        ])
        .current_dir(root.join("services/infrastructure-api"))
        .output()
        .context("Failed to execute infrastructure export")?;

    if !output.status.success() {
        return Err(anyhow!(
            "Infrastructure export failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    println!(
        "✓ Infrastructure API spec exported to {}",
        output_path.display()
    );

    Ok(())
}

fn export_coordination(root: &Path, args: &DocsArgs) -> anyhow::Result<()> {
    let output_path = root.join("../docs/specs/coordination.json");

    if args.dry_run {
        println!(
            "[Dry Run] Would run: bun run export-openapi.ts {}",
            output_path.display()
        );
        return Ok(());
    }

    let output = Command::new("bun")
        .args([
            "run",
            "export-openapi.ts",
            output_path
                .to_str()
                .context("Output path contains invalid UTF-8")?,
        ])
        .current_dir(root.join("services/coordination-hce"))
        .output()
        .context("Failed to execute coordination export")?;

    if !output.status.success() {
        return Err(anyhow!(
            "Coordination export failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    println!(
        "✓ Coordination HCE spec exported to {}",
        output_path.display()
    );

    Ok(())
}

fn publish_spec(
    root: &Path,
    local_filename: &str,
    remote_path: &str,
    args: &DocsArgs,
) -> anyhow::Result<()> {
    let repo = "resq-software/docs";
    let local_path = root.join("../docs/specs").join(local_filename);

    if args.dry_run {
        println!(
            "[Dry Run] Would publish {} to {} as {}",
            local_path.display(),
            repo,
            remote_path
        );
        return Ok(());
    }

    // Read local file
    let content = std::fs::read_to_string(&local_path)
        .with_context(|| format!("Failed to read local spec: {}", local_path.display()))?;

    let base64_content = base64::engine::general_purpose::STANDARD.encode(content);

    // Get current SHA
    let repo_url = format!("repos/{repo}/contents/{remote_path}");
    let output = Command::new("gh")
        .args(["api", &repo_url])
        .output()
        .context("Failed to get existing file metadata from GitHub")?;

    let sha = if output.status.success() {
        let json: Value = serde_json::from_slice(&output.stdout)?;
        json["sha"].as_str().map(ToString::to_string)
    } else {
        None
    };

    // Prepare update command
    let message_arg = format!("message=docs: update {local_filename} [skip ci]");
    let content_arg = format!("content={base64_content}");

    let mut gh_args = vec![
        "api",
        "--method",
        "PUT",
        &repo_url,
        "-f",
        &message_arg,
        "-f",
        &content_arg,
    ];

    let sha_arg; // Must declare outside to live long enough
    if let Some(sha_val) = sha {
        gh_args.push("-f");
        sha_arg = format!("sha={sha_val}");
        gh_args.push(&sha_arg);
    }

    let put_output = Command::new("gh")
        .args(&gh_args)
        .output()
        .context("Failed to publish content to GitHub")?;

    if !put_output.status.success() {
        return Err(anyhow!(
            "Failed to publish {local_filename} to GitHub:\n{}",
            String::from_utf8_lossy(&put_output.stderr)
        ));
    }

    println!("✓ Successfully published {local_filename} to {repo}");
    Ok(())
}

fn header(title: &str) {
    let bar = "━".repeat(74usize.saturating_sub(title.len() + 1));
    println!("\n━━━ {title} {bar}");
}
