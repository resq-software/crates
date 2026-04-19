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

//! Shell-completion emitter.
//!
//! Generates completion scripts for bash, zsh, fish, elvish, and powershell
//! by introspecting the `clap`-derived top-level `Cli` parser. Users wire the
//! output into their shell's completion loader, e.g.:
//!
//! ```bash
//! resq completions bash > /usr/local/share/bash-completion/completions/resq
//! ```

use clap::{Args, Command};
use clap_complete::{generate, Shell};

/// Arguments for `resq completions <shell>`.
#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Target shell.
    pub shell: Shell,
}

/// Emit a completion script for the given shell to `stdout`.
///
/// Takes the already-built root `Command` so the caller (`main`) owns the parser
/// definition and this module stays decoupled from the `Cli` struct layout.
pub fn run(args: CompletionsArgs, mut cmd: Command) -> anyhow::Result<()> {
    let bin = cmd.get_name().to_string();
    generate(args.shell, &mut cmd, bin, &mut std::io::stdout());
    Ok(())
}
