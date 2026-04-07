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

use crate::CacheOptions;
use anyhow::{Context, Result};
use bin_explorer::analysis::{AnalyzeOptions, BinaryReport};
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    report: BinaryReport,
}

pub(crate) struct CacheLookup {
    pub(crate) report: BinaryReport,
    pub(crate) cache_hit: bool,
}

pub(crate) fn load_or_analyze<F>(
    path: &Path,
    analysis_options: &AnalyzeOptions,
    cache_options: &CacheOptions,
    analyzer: F,
) -> Result<CacheLookup>
where
    F: FnOnce(&Path, &AnalyzeOptions) -> Result<BinaryReport>,
{
    if !cache_options.enabled {
        return analyzer(path, analysis_options).map(|report| CacheLookup {
            report,
            cache_hit: false,
        });
    }

    let key = cache_key(path, analysis_options)
        .with_context(|| format!("failed to compute cache key for {}", path.display()))?;
    let cache_file = cache_options.dir.join(format!("{key}.json"));

    if !cache_options.rebuild && cache_file.exists() {
        let raw = std::fs::read_to_string(&cache_file)
            .with_context(|| format!("failed to read cache file {}", cache_file.display()))?;
        if let Ok(entry) = serde_json::from_str::<CacheEntry>(&raw) {
            return Ok(CacheLookup {
                report: entry.report,
                cache_hit: true,
            });
        }
    }

    let report = analyzer(path, analysis_options)?;

    std::fs::create_dir_all(&cache_options.dir)
        .with_context(|| format!("failed to create cache dir {}", cache_options.dir.display()))?;
    let payload = serde_json::to_string(&CacheEntry {
        report: report.clone(),
    })?;
    std::fs::write(&cache_file, payload)
        .with_context(|| format!("failed to write cache file {}", cache_file.display()))?;

    Ok(CacheLookup {
        report,
        cache_hit: false,
    })
}

fn cache_key(path: &Path, options: &AnalyzeOptions) -> Result<String> {
    let canonical = path
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path.as_os_str()));

    let metadata = std::fs::metadata(path)
        .with_context(|| format!("failed to stat {} for cache key", path.display()))?;

    let mut fingerprint = format!(
        "v5|path={}|len={}|disasm={}|max_fn={}|max_sym={}|max_insn={}",
        canonical.display(),
        metadata.len(),
        options.include_disassembly,
        options.max_functions,
        options.max_symbols,
        options.max_instructions_per_function,
    );

    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
            fingerprint.push_str(&format!(
                "|mtime={}.{}",
                duration.as_secs(),
                duration.subsec_nanos()
            ));
        }
    } else {
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read {} for cache key hash", path.display()))?;
        let mut content_hasher = Hasher::new();
        content_hasher.update(&bytes);
        fingerprint.push_str(&format!("|content_crc={:08x}", content_hasher.finalize()));
    }

    let mut key_hasher = Hasher::new();
    key_hasher.update(fingerprint.as_bytes());
    Ok(format!("{:08x}", key_hasher.finalize()))
}
