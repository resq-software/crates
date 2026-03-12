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

//! Binary metadata and disassembly analysis.

use anyhow::{anyhow, Context, Result};
use capstone::prelude::*;
use object::{Object, ObjectSection, ObjectSymbol, SectionKind, SymbolKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Analysis options for a single binary input.
#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    /// Include machine-level disassembly when available.
    pub include_disassembly: bool,
    /// Maximum number of functions to disassemble.
    pub max_functions: usize,
    /// Maximum number of symbols to include in report.
    pub max_symbols: usize,
    /// Maximum instructions per function.
    pub max_instructions_per_function: usize,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            include_disassembly: true,
            max_functions: 40,
            max_symbols: 1000,
            max_instructions_per_function: 200,
        }
    }
}

/// Structured report generated for a binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryReport {
    /// Input path.
    pub path: PathBuf,
    /// File format (elf, pe, macho, wasm, ...).
    pub format: String,
    /// CPU architecture.
    pub architecture: String,
    /// Endianness.
    pub endianness: String,
    /// Entrypoint virtual address.
    pub entry: u64,
    /// Binary size in bytes.
    pub size_bytes: u64,
    /// Section inventory.
    pub sections: Vec<SectionInfo>,
    /// Symbol inventory.
    pub symbols: Vec<SymbolInfo>,
    /// Per-function disassembly.
    pub functions: Vec<FunctionReport>,
    /// Backend used for disassembly generation.
    #[serde(default)]
    pub disassembly_backend: Option<String>,
    /// Attempt log for disassembly backends (success/fallback diagnostics).
    #[serde(default)]
    pub disassembly_attempts: Vec<String>,
    /// Coverage summary for per-function disassembly backend assignment.
    #[serde(default)]
    pub disassembly_coverage: Option<DisassemblyCoverage>,
    /// Per-function backend assignment and instruction counts.
    #[serde(default)]
    pub function_backend_coverage: Vec<FunctionBackendCoverage>,
    /// Non-fatal diagnostics from tooling.
    pub warnings: Vec<String>,
}

/// Summary counters for disassembly backend coverage.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DisassemblyCoverage {
    /// Number of functions considered for disassembly.
    pub total_functions: usize,
    /// Number of functions that have at least one decoded instruction.
    pub functions_with_instructions: usize,
    /// Number of functions populated from Capstone.
    pub capstone_functions: usize,
    /// Number of functions populated from objdump fallback.
    pub objdump_functions: usize,
    /// Number of functions with zero instructions after all backends.
    pub missing_functions: usize,
}

/// Backend assignment for a specific function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionBackendCoverage {
    /// Function name.
    pub name: String,
    /// Backend used (`capstone`, `objdump`, or `none`).
    pub backend: String,
    /// Number of instructions captured for this function.
    pub instruction_count: usize,
}

/// Section metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionInfo {
    /// Section name.
    pub name: String,
    /// Load address.
    pub address: u64,
    /// Raw size.
    pub size: u64,
    /// Canonical kind.
    pub kind: String,
}

/// Symbol metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// Symbol name.
    pub name: String,
    /// Address.
    pub address: u64,
    /// Size in bytes.
    pub size: u64,
    /// Symbol kind.
    pub kind: String,
    /// Whether symbol is externally visible.
    pub is_global: bool,
}

/// Function-level analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionReport {
    /// Function symbol name.
    pub name: String,
    /// Start address.
    pub address: u64,
    /// Declared symbol size.
    pub size: u64,
    /// Disassembled instructions.
    pub instructions: Vec<Instruction>,
}

/// Machine instruction row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instruction {
    /// Instruction address.
    pub address: u64,
    /// Disassembled opcode + operands.
    pub text: String,
}

/// Analyzer for parsing binary metadata and optional disassembly.
#[derive(Default)]
pub struct BinaryAnalyzer;

impl BinaryAnalyzer {
    /// Analyze a binary file and return a normalized report.
    pub fn analyze_path(path: &Path, options: &AnalyzeOptions) -> Result<BinaryReport> {
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        let object = object::File::parse(bytes.as_slice())
            .with_context(|| format!("failed to parse object file {}", path.display()))?;

        let sections = object
            .sections()
            .map(|section| SectionInfo {
                name: section.name().unwrap_or("<unknown>").to_string(),
                address: section.address(),
                size: section.size(),
                kind: format!("{:?}", section.kind()),
            })
            .collect::<Vec<_>>();

        let mut symbols = object
            .symbols()
            .filter(|symbol| symbol.kind() != SymbolKind::Unknown)
            .map(|symbol| SymbolInfo {
                name: symbol.name().unwrap_or("<unnamed>").to_string(),
                address: symbol.address(),
                size: symbol.size(),
                kind: format!("{:?}", symbol.kind()),
                is_global: symbol.is_global(),
            })
            .collect::<Vec<_>>();

        symbols.sort_by_key(|s| s.address);
        symbols.truncate(options.max_symbols);

        let mut functions = collect_functions(&object);
        functions.sort_by_key(|f| f.address);
        functions.truncate(options.max_functions);

        let mut warnings = Vec::new();
        let mut instructions_by_name = HashMap::new();
        let mut backend_by_name: HashMap<String, String> = HashMap::new();
        let mut disassembly_backend = None;
        let mut disassembly_attempts = Vec::new();

        if options.include_disassembly && !functions.is_empty() {
            let capstone_result =
                CapstoneDisassembler::new(&object, bytes.as_slice()).and_then(|d| {
                    d.disassemble(path, &functions, options.max_instructions_per_function)
                });

            match capstone_result {
                Ok(disassembly) => {
                    disassembly_attempts.push("capstone: ok".to_string());
                    disassembly_backend = Some("capstone".to_string());
                    for (name, instructions) in disassembly {
                        if !instructions.is_empty() {
                            backend_by_name.insert(name.clone(), "capstone".to_string());
                            instructions_by_name.insert(name, instructions);
                        }
                    }

                    let missing = functions
                        .iter()
                        .filter(|f| {
                            instructions_by_name
                                .get(&f.name)
                                .is_none_or(std::vec::Vec::is_empty)
                        })
                        .cloned()
                        .collect::<Vec<_>>();

                    if !missing.is_empty() {
                        match ObjdumpDisassembler::new().and_then(|d| {
                            d.disassemble(path, &missing, options.max_instructions_per_function)
                        }) {
                            Ok(fallback) => {
                                disassembly_attempts
                                    .push("objdump: ok (filled missing)".to_string());
                                if !fallback.is_empty() {
                                    disassembly_backend = Some("capstone+objdump".to_string());
                                }
                                for (name, instructions) in fallback {
                                    if instructions.is_empty() {
                                        continue;
                                    }
                                    instructions_by_name
                                        .entry(name.clone())
                                        .or_insert(instructions);
                                    backend_by_name.entry(name).or_insert("objdump".to_string());
                                }
                            },
                            Err(obj_err) => {
                                disassembly_attempts.push(format!(
                                    "objdump: unavailable while filling missing ({obj_err})"
                                ));
                            },
                        }
                    }
                },
                Err(err) => {
                    disassembly_attempts.push(format!("capstone: unavailable ({err})"));
                    match ObjdumpDisassembler::new().and_then(|d| {
                        d.disassemble(path, &functions, options.max_instructions_per_function)
                    }) {
                        Ok(disassembly) => {
                            disassembly_attempts.push("objdump: ok (fallback)".to_string());
                            disassembly_backend = Some("objdump".to_string());
                            for (name, instructions) in disassembly {
                                if instructions.is_empty() {
                                    continue;
                                }
                                backend_by_name.insert(name.clone(), "objdump".to_string());
                                instructions_by_name.insert(name, instructions);
                            }
                        },
                        Err(obj_err) => {
                            disassembly_attempts.push(format!("objdump: unavailable ({obj_err})"));
                            warnings.push(format!(
                                "disassembly unavailable: capstone failed ({err}); objdump failed ({obj_err})"
                            ));
                        },
                    }
                },
            }
        }

        let functions = functions
            .into_iter()
            .map(|f| FunctionReport {
                name: f.name.clone(),
                address: f.address,
                size: f.size,
                instructions: instructions_by_name.remove(&f.name).unwrap_or_default(),
            })
            .collect::<Vec<_>>();

        let mut coverage = DisassemblyCoverage {
            total_functions: functions.len(),
            ..Default::default()
        };
        let mut function_backend_coverage = Vec::with_capacity(functions.len());
        for function in &functions {
            let instruction_count = function.instructions.len();
            let backend = if instruction_count == 0 {
                "none".to_string()
            } else {
                backend_by_name
                    .get(&function.name)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string())
            };

            if instruction_count > 0 {
                coverage.functions_with_instructions += 1;
                match backend.as_str() {
                    "capstone" => coverage.capstone_functions += 1,
                    "objdump" => coverage.objdump_functions += 1,
                    _ => {},
                }
            } else {
                coverage.missing_functions += 1;
            }

            function_backend_coverage.push(FunctionBackendCoverage {
                name: function.name.clone(),
                backend,
                instruction_count,
            });
        }

        Ok(BinaryReport {
            path: path.to_path_buf(),
            format: format!("{:?}", object.format()),
            architecture: format!("{:?}", object.architecture()),
            endianness: format!("{:?}", object.endianness()),
            entry: object.entry(),
            size_bytes: bytes.len() as u64,
            sections,
            symbols,
            functions,
            disassembly_backend,
            disassembly_attempts,
            disassembly_coverage: Some(coverage),
            function_backend_coverage,
            warnings,
        })
    }
}

#[derive(Debug, Clone)]
struct FunctionSymbol {
    name: String,
    address: u64,
    size: u64,
}

fn collect_functions(object: &object::File<'_>) -> Vec<FunctionSymbol> {
    let text_sections = object
        .sections()
        .filter(|s| s.kind() == SectionKind::Text)
        .map(|s| s.index())
        .collect::<HashSet<_>>();

    object
        .symbols()
        .filter(|symbol| symbol.kind() == SymbolKind::Text)
        .filter(|symbol| !symbol.is_undefined())
        .filter(|symbol| {
            symbol
                .section_index()
                .is_some_and(|section_index| text_sections.contains(&section_index))
        })
        .filter_map(|symbol| {
            let name = symbol.name().ok()?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            Some(FunctionSymbol {
                name,
                address: symbol.address(),
                size: symbol.size(),
            })
        })
        .collect()
}

trait Disassembler {
    fn disassemble(
        &self,
        path: &Path,
        functions: &[FunctionSymbol],
        max_instructions_per_function: usize,
    ) -> Result<HashMap<String, Vec<Instruction>>>;
}

#[derive(Debug, Clone)]
struct CodeRegion {
    start: u64,
    data: Vec<u8>,
}

#[derive(Debug)]
struct CapstoneDisassembler {
    cs: Capstone,
    regions: Vec<CodeRegion>,
}

impl CapstoneDisassembler {
    fn new(object: &object::File<'_>, _bytes: &[u8]) -> Result<Self> {
        let cs = match object.architecture() {
            object::Architecture::X86_64 => Capstone::new()
                .x86()
                .mode(capstone::arch::x86::ArchMode::Mode64)
                .build()
                .context("capstone init failed for x86_64")?,
            object::Architecture::Aarch64 => Capstone::new()
                .arm64()
                .mode(capstone::arch::arm64::ArchMode::Arm)
                .build()
                .context("capstone init failed for aarch64")?,
            other => {
                return Err(anyhow!("capstone unsupported architecture: {other:?}"));
            },
        };

        let regions = object
            .sections()
            .filter(|section| section.kind() == SectionKind::Text)
            .filter_map(|section| {
                let start = section.address();
                let data = section.data().ok()?;
                if data.is_empty() {
                    return None;
                }
                let owned = data.to_vec();
                if owned.is_empty() {
                    None
                } else {
                    Some(CodeRegion { start, data: owned })
                }
            })
            .collect::<Vec<_>>();

        if regions.is_empty() {
            return Err(anyhow!("no text sections available for capstone"));
        }
        Ok(Self { cs, regions })
    }

    fn slice_for_address(&self, address: u64, size_hint: usize) -> Option<&[u8]> {
        self.regions.iter().find_map(|region| {
            let offset = address.checked_sub(region.start)? as usize;
            if offset >= region.data.len() {
                return None;
            }
            let end = if size_hint > 0 {
                (offset + size_hint).min(region.data.len())
            } else {
                region.data.len()
            };
            Some(&region.data[offset..end])
        })
    }
}

impl Disassembler for CapstoneDisassembler {
    fn disassemble(
        &self,
        _path: &Path,
        functions: &[FunctionSymbol],
        max_instructions_per_function: usize,
    ) -> Result<HashMap<String, Vec<Instruction>>> {
        if functions.is_empty() {
            return Ok(HashMap::new());
        }

        let mut sorted = functions.to_vec();
        sorted.sort_by_key(|f| f.address);

        let mut out = HashMap::new();
        for (idx, function) in sorted.iter().enumerate() {
            let next_addr = sorted
                .iter()
                .skip(idx + 1)
                .find(|f| f.address > function.address)
                .map(|f| f.address);

            let size_hint = if function.size > 0 {
                function.size as usize
            } else {
                next_addr
                    .and_then(|next| next.checked_sub(function.address))
                    .unwrap_or(64) as usize
            };

            let Some(code) = self.slice_for_address(function.address, size_hint.max(1)) else {
                continue;
            };
            let Ok(insns) = self.cs.disasm_all(code, function.address) else {
                continue;
            };

            let instructions = insns
                .iter()
                .take(max_instructions_per_function)
                .map(|insn| {
                    let mut text = String::new();
                    if let Some(mnemonic) = insn.mnemonic() {
                        text.push_str(mnemonic);
                    }
                    if let Some(op_str) = insn.op_str() {
                        if !text.is_empty() && !op_str.is_empty() {
                            text.push(' ');
                        }
                        text.push_str(op_str);
                    }
                    Instruction {
                        address: insn.address(),
                        text,
                    }
                })
                .collect::<Vec<_>>();

            if !instructions.is_empty() {
                out.insert(function.name.clone(), instructions);
            }
        }

        if out.is_empty() {
            return Err(anyhow!(
                "capstone produced no disassembly for target functions"
            ));
        }

        Ok(out)
    }
}

#[derive(Debug, Clone)]
struct ObjdumpDisassembler {
    tool: String,
}

impl ObjdumpDisassembler {
    fn new() -> Result<Self> {
        for tool in ["llvm-objdump", "objdump"] {
            if Command::new(tool).arg("--version").output().is_ok() {
                return Ok(Self {
                    tool: tool.to_string(),
                });
            }
        }
        Err(anyhow!("neither llvm-objdump nor objdump is available"))
    }

    fn parse_output(stdout: &str, max_per_function: usize) -> Vec<DisasmBlock> {
        let mut current: Option<DisasmBlock> = None;
        let mut blocks = Vec::new();

        for raw_line in stdout.lines() {
            let line = raw_line.trim_end();

            if let Some((address, name)) = parse_block_header(line) {
                if let Some(prev) = current.take() {
                    blocks.push(prev);
                }
                current = Some(DisasmBlock {
                    name,
                    address,
                    instructions: Vec::new(),
                });
                continue;
            }

            let Some(block) = current.as_mut() else {
                continue;
            };

            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                continue;
            }

            let Some((addr_part, inst_part)) = trimmed.split_once(':') else {
                continue;
            };
            let Ok(address) = u64::from_str_radix(addr_part.trim(), 16) else {
                continue;
            };

            let text = inst_part.trim();
            if text.is_empty() || block.instructions.len() >= max_per_function {
                continue;
            }

            block.instructions.push(Instruction {
                address,
                text: text.to_string(),
            });
        }

        if let Some(last) = current {
            blocks.push(last);
        }

        blocks
    }
}

impl Disassembler for ObjdumpDisassembler {
    fn disassemble(
        &self,
        path: &Path,
        functions: &[FunctionSymbol],
        max_instructions_per_function: usize,
    ) -> Result<HashMap<String, Vec<Instruction>>> {
        if functions.is_empty() {
            return Ok(HashMap::new());
        }

        let output = Command::new(&self.tool)
            .arg("-d")
            .arg("--no-show-raw-insn")
            .arg(path)
            .output()
            .with_context(|| format!("failed to execute {}", self.tool))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "{} failed with status {}: {}",
                self.tool,
                output.status,
                stderr.trim()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let blocks = Self::parse_output(&stdout, max_instructions_per_function);
        Ok(map_blocks_to_functions(functions, &blocks))
    }
}

#[derive(Debug, Clone)]
struct DisasmBlock {
    name: String,
    address: u64,
    instructions: Vec<Instruction>,
}

fn parse_block_header(line: &str) -> Option<(u64, String)> {
    let trimmed = line.trim();
    let (left, right) = trimmed.split_once('<')?;
    let left = left.trim();
    if left.is_empty() {
        return None;
    }
    let address = u64::from_str_radix(left, 16).ok()?;

    let right = right.strip_suffix(':')?;
    let name = right.strip_suffix('>')?.trim();
    if name.is_empty() {
        return None;
    }
    Some((address, name.to_string()))
}

fn normalize_symbol_name(name: &str) -> String {
    let mut value = name.trim().to_string();

    if let Some((base, _)) = value.split_once("@@") {
        value = base.to_string();
    } else if let Some((base, _)) = value.split_once('@') {
        value = base.to_string();
    }

    if let Some(pos) = value.rfind("::h") {
        let hash = &value[pos + 3..];
        if hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
            value.truncate(pos);
        }
    }

    value
}

fn map_blocks_to_functions(
    functions: &[FunctionSymbol],
    blocks: &[DisasmBlock],
) -> HashMap<String, Vec<Instruction>> {
    let mut out = HashMap::new();
    if blocks.is_empty() || functions.is_empty() {
        return out;
    }

    let mut sorted_functions = functions.to_vec();
    sorted_functions.sort_by_key(|f| f.address);

    let mut next_addr_by_name = HashMap::new();
    for (idx, function) in sorted_functions.iter().enumerate() {
        let next = sorted_functions
            .iter()
            .skip(idx + 1)
            .find(|f| f.address > function.address)
            .map(|f| f.address);
        next_addr_by_name.insert(function.name.clone(), next);
    }

    let mut blocks_by_normalized_name: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, block) in blocks.iter().enumerate() {
        blocks_by_normalized_name
            .entry(normalize_symbol_name(&block.name))
            .or_default()
            .push(idx);
    }

    let mut used_block_indices = HashSet::new();

    for function in functions {
        let mut selected = blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| block.address == function.address)
            .max_by_key(|(_, block)| block.instructions.len())
            .map(|(idx, _)| idx);

        if selected.is_none() {
            let end = if function.size > 0 {
                function.address.saturating_add(function.size)
            } else {
                next_addr_by_name
                    .get(&function.name)
                    .and_then(|v| *v)
                    .unwrap_or(function.address.saturating_add(1))
            };

            selected = blocks
                .iter()
                .enumerate()
                .filter(|(_, block)| block.address >= function.address && block.address < end)
                .min_by_key(|(_, block)| block.address.saturating_sub(function.address))
                .map(|(idx, _)| idx);
        }

        if selected.is_none() {
            let normalized = normalize_symbol_name(&function.name);
            if let Some(candidates) = blocks_by_normalized_name.get(&normalized) {
                selected = candidates
                    .iter()
                    .copied()
                    .find(|idx| !used_block_indices.contains(idx))
                    .or_else(|| candidates.first().copied());
            }
        }

        if selected.is_none() {
            selected = blocks
                .iter()
                .enumerate()
                .min_by_key(|(_, block)| block.address.abs_diff(function.address))
                .map(|(idx, _)| idx);
        }

        if let Some(idx) = selected {
            used_block_indices.insert(idx);
            out.insert(function.name.clone(), blocks[idx].instructions.clone());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzes_current_executable_metadata() {
        let exe = std::env::current_exe().expect("current exe");
        let options = AnalyzeOptions {
            include_disassembly: false,
            ..Default::default()
        };

        let report = BinaryAnalyzer::analyze_path(&exe, &options).expect("analysis should succeed");

        assert!(report.size_bytes > 0);
        assert!(!report.sections.is_empty());
        assert!(!report.format.is_empty());
    }

    #[test]
    fn parses_objdump_style_function_blocks() {
        let disasm = r"
0000000000001139 <main>:
    1139:    push   %rbp
    113a:    mov    %rsp,%rbp

0000000000001140 <helper>:
    1140:    ret
";

        let blocks = ObjdumpDisassembler::parse_output(disasm, 10);
        let main = blocks
            .iter()
            .find(|b| b.name == "main")
            .expect("main exists");

        assert_eq!(main.instructions.len(), 2);
        assert_eq!(main.instructions[0].address, 0x1139);
        assert_eq!(main.instructions[0].text, "push   %rbp");
        assert!(blocks.iter().any(|b| b.name == "helper"));
    }

    #[test]
    fn maps_blocks_to_functions_by_address_range_and_name_normalization() {
        let disasm = r"
0000000000001200 <helper@@GLIBC_2.2.5>:
    1200:    ret
0000000000001300 <main+0x0>:
    1300:    push   %rbp
    1301:    mov    %rsp,%rbp
";
        let blocks = ObjdumpDisassembler::parse_output(disasm, 10);
        let functions = vec![
            FunctionSymbol {
                name: "main".to_string(),
                address: 0x1300,
                size: 32,
            },
            FunctionSymbol {
                name: "helper".to_string(),
                address: 0x1200,
                size: 16,
            },
        ];

        let mapped = map_blocks_to_functions(&functions, &blocks);
        let main = mapped.get("main").expect("main mapped");
        assert_eq!(main.len(), 2);
        assert_eq!(main[0].address, 0x1300);

        let helper = mapped.get("helper").expect("helper mapped");
        assert_eq!(helper.len(), 1);
        assert_eq!(helper[0].address, 0x1200);
    }
}
