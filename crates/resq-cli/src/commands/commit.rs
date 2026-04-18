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

//! AI-powered commit message generation from staged diffs.

use anyhow::{bail, Context, Result};
use clap::Args;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode},
    execute, style,
    terminal::{self, ClearType},
};
use regex::Regex;
use std::io::{self, IsTerminal, Write};
use std::process::Command;

/// Arguments for the `resq commit` command.
#[derive(Args, Debug)]
pub struct CommitArgs {
    /// Number of candidate messages to generate
    #[arg(short = 'n', long, default_value = "3")]
    count: u8,

    /// Max diff size in estimated tokens before truncation
    #[arg(short = 'm', long, default_value = "2000")]
    max_diff: usize,

    /// Hint for commit scope (e.g., "auth", "ui")
    #[arg(short, long)]
    scope: Option<String>,

    /// Auto-pick first candidate without prompting
    #[arg(long)]
    yes: bool,

    /// Print message(s) but don't commit
    #[arg(long)]
    dry_run: bool,

    /// Override AI provider (anthropic, openai, gemini)
    #[arg(long)]
    provider: Option<String>,

    /// Override AI model
    #[arg(long)]
    model: Option<String>,

    /// API request timeout in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Show prompt, token count, and raw LLM response
    #[arg(long)]
    verbose: bool,
}

// -------------------------------------------------------------------------
// Git helpers
// -------------------------------------------------------------------------

fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("Failed to run git")?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {}: {stderr}", args.join(" "));
    }
}

fn get_staged_diff() -> Result<String> {
    let stat = git(&["diff", "--staged", "--stat"])?;
    if stat.trim().is_empty() {
        bail!(
            "No staged changes found.\n\
             Stage files first with: git add <files>\n\
             Then run: resq commit"
        );
    }
    git(&["diff", "--staged"])
}

fn check_unstaged_warning() {
    if let Ok(diff) = git(&["diff", "--stat"]) {
        if !diff.trim().is_empty() {
            eprintln!(
                "\x1b[33mWarning:\x1b[0m You have unstaged changes. \
                 Only staged changes will be included in the commit."
            );
        }
    }
}

fn get_recent_commits() -> String {
    git(&["log", "--oneline", "-10"]).unwrap_or_default()
}

// -------------------------------------------------------------------------
// Prompt building + response parsing
// -------------------------------------------------------------------------

/// Conventional Commits regex (from templates/git-hooks/commit-msg).
const CC_PATTERN: &str =
    r"^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\(.+\))?(!)?: .+$";

fn validate_conventional_commit(msg: &str) -> bool {
    Regex::new(CC_PATTERN)
        .map(|re| re.is_match(msg.lines().next().unwrap_or("")))
        .unwrap_or(false)
}

fn build_prompt(
    diff: &str,
    recent_commits: &str,
    scope: Option<&str>,
    count: u8,
) -> (String, String) {
    let scope_hint = scope
        .map(|s| format!("\nSuggested scope: {s}"))
        .unwrap_or_default();

    let system = format!(
        "You are a commit message generator for a project that uses Conventional Commits.\n\
         \n\
         Format: <type>(<scope>): <description>\n\
         \n\
         Allowed types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert\n\
         {scope_hint}\n\
         \n\
         Rules:\n\
         - Subject line must be under 72 characters\n\
         - Use imperative mood (\"add\" not \"added\")\n\
         - Be specific about what changed and why\n\
         - Match the style of recent commits shown below\n\
         \n\
         Recent commits for style reference:\n\
         {recent_commits}\n\
         \n\
         Generate exactly {count} commit message candidates. \
         Return them as a JSON array of strings. No markdown fences."
    );

    let user = format!("Staged diff:\n\n{diff}");
    (system, user)
}

fn strip_code_fences(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        rest.trim()
            .strip_suffix("```")
            .map_or_else(|| rest.trim(), str::trim)
    } else if let Some(rest) = trimmed.strip_prefix("```") {
        rest.trim()
            .strip_suffix("```")
            .map_or_else(|| rest.trim(), str::trim)
    } else {
        trimmed
    }
}

fn parse_candidates(response: &str) -> Result<Vec<String>> {
    let cleaned = strip_code_fences(response);
    let candidates: Vec<String> =
        serde_json::from_str(cleaned).context("Failed to parse LLM response as JSON array")?;
    Ok(candidates)
}

// -------------------------------------------------------------------------
// Interactive selector (crossterm-based)
// -------------------------------------------------------------------------

/// RAII guard to restore terminal raw mode on drop.
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

fn select_candidate(candidates: &[String]) -> Result<Option<usize>> {
    if !io::stdout().is_terminal() {
        bail!("Interactive selection requires a TTY. Use --yes or --dry-run in non-interactive contexts.");
    }

    let _guard = RawModeGuard::enable()?;
    let mut stdout = io::stdout();
    let mut selected: usize = 0;
    let total = candidates.len();

    // Initial render
    render_selector(&mut stdout, candidates, selected)?;

    loop {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') if selected > 0 => selected -= 1,
                KeyCode::Down | KeyCode::Char('j') if selected < total - 1 => selected += 1,
                KeyCode::Enter => {
                    clear_selector(&mut stdout, total)?;
                    return Ok(Some(selected));
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    clear_selector(&mut stdout, total)?;
                    return Ok(None);
                }
                _ => continue,
            }
            // Move up and re-render
            execute!(stdout, cursor::MoveUp((total + 2) as u16))?;
            render_selector(&mut stdout, candidates, selected)?;
        }
    }
}

fn render_selector(stdout: &mut io::Stdout, candidates: &[String], selected: usize) -> Result<()> {
    for (i, candidate) in candidates.iter().enumerate() {
        if i == selected {
            execute!(stdout, style::SetForegroundColor(style::Color::Cyan))?;
            write!(stdout, "\r  > {}. {}\n", i + 1, candidate)?;
            execute!(stdout, style::ResetColor)?;
        } else {
            write!(stdout, "\r    {}. {}\n", i + 1, candidate)?;
        }
    }
    write!(
        stdout,
        "\r\n  \x1b[2m[↑/↓/j/k] Navigate  [Enter] Select  [Esc/q] Cancel\x1b[0m"
    )?;
    stdout.flush()?;
    Ok(())
}

fn clear_selector(stdout: &mut io::Stdout, total: usize) -> Result<()> {
    execute!(
        stdout,
        cursor::MoveUp((total + 2) as u16),
        terminal::Clear(ClearType::FromCursorDown),
    )?;
    Ok(())
}

// -------------------------------------------------------------------------
// Main entry point
// -------------------------------------------------------------------------

/// Run the commit command.
pub async fn run(args: CommitArgs) -> Result<()> {
    // 1. Check staged changes
    let diff = get_staged_diff()?;
    check_unstaged_warning();

    // 2. Truncate diff to token budget
    let truncated = resq_ai::truncate_to_budget(&diff, args.max_diff);
    let recent = get_recent_commits();

    // 3. Load AI config with CLI overrides
    let mut config = resq_ai::load_config()?;
    if let Some(ref p) = args.provider {
        config.provider = match p.to_lowercase().as_str() {
            "anthropic" => resq_ai::Provider::Anthropic,
            "openai" => resq_ai::Provider::OpenAi,
            "gemini" => resq_ai::Provider::Gemini,
            other => bail!("Unknown provider: {other}. Use: anthropic, openai, gemini"),
        };
    }
    if let Some(ref m) = args.model {
        config.model = m.clone();
    }
    config.timeout_secs = args.timeout;

    // 4. Build prompt
    let (system, user_prompt) = build_prompt(truncated, &recent, args.scope.as_deref(), args.count);

    if args.verbose {
        eprintln!("--- System prompt ---\n{system}\n---");
        eprintln!(
            "Estimated tokens: {}",
            resq_ai::estimate_tokens(&user_prompt)
        );
    }

    // 5. Call LLM
    eprintln!("Generating commit messages...");
    let response = resq_ai::complete(&config, &system, &user_prompt).await?;

    if args.verbose {
        eprintln!("--- Raw response ---\n{response}\n---");
    }

    // 6. Parse + validate candidates
    let candidates = parse_candidates(&response)?;
    let valid: Vec<String> = candidates
        .into_iter()
        .filter(|c| validate_conventional_commit(c))
        .collect();

    if valid.is_empty() {
        bail!("LLM returned no valid Conventional Commit messages. Try again or write manually.");
    }

    // 7. Select
    if args.dry_run {
        for (i, c) in valid.iter().enumerate() {
            println!("{}. {c}", i + 1);
        }
        return Ok(());
    }

    let message = if args.yes {
        valid.into_iter().next().unwrap()
    } else {
        let idx = select_candidate(&valid)?;
        match idx {
            Some(i) => valid.into_iter().nth(i).unwrap(),
            None => {
                eprintln!("Cancelled.");
                return Ok(());
            }
        }
    };

    // 8. Commit
    git(&["commit", "-m", &message])?;
    eprintln!("Committed: {message}");
    Ok(())
}

// -------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_fences_json() {
        let input = "```json\n[\"feat: add thing\"]\n```";
        assert_eq!(strip_code_fences(input), "[\"feat: add thing\"]");
    }

    #[test]
    fn strip_fences_plain() {
        let input = "```\n[\"feat: add thing\"]\n```";
        assert_eq!(strip_code_fences(input), "[\"feat: add thing\"]");
    }

    #[test]
    fn strip_fences_none() {
        let input = "[\"feat: add thing\"]";
        assert_eq!(strip_code_fences(input), input);
    }

    #[test]
    fn validate_cc_valid() {
        assert!(validate_conventional_commit("feat: add new feature"));
        assert!(validate_conventional_commit(
            "fix(ui): correct button color"
        ));
        assert!(validate_conventional_commit("feat!: remove deprecated API"));
        assert!(validate_conventional_commit(
            "chore(deps): bump serde to 1.0.200"
        ));
    }

    #[test]
    fn validate_cc_invalid() {
        assert!(!validate_conventional_commit("Add new feature"));
        assert!(!validate_conventional_commit("FEAT: uppercase type"));
        assert!(!validate_conventional_commit("feat:missing space"));
        assert!(!validate_conventional_commit(""));
    }

    #[test]
    fn parse_candidates_valid_json() {
        let input = r#"["feat: add thing", "fix: repair thing"]"#;
        let result = parse_candidates(input).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "feat: add thing");
    }

    #[test]
    fn parse_candidates_with_fences() {
        let input = "```json\n[\"feat: add thing\"]\n```";
        let result = parse_candidates(input).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn build_prompt_includes_scope() {
        let (system, _user) = build_prompt("diff content", "recent commits", Some("auth"), 3);
        assert!(system.contains("auth"));
    }

    #[test]
    fn build_prompt_without_scope() {
        let (system, _user) = build_prompt("diff content", "recent commits", None, 3);
        assert!(!system.contains("Suggested scope"));
    }

    #[test]
    fn build_prompt_includes_count() {
        let (system, _user) = build_prompt("diff", "commits", None, 5);
        assert!(system.contains("exactly 5"));
    }
}
