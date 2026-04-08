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

//! Low-Quality Image Placeholder (LQIP) command.
//!
//! Generates tiny blurred image previews for progressive image loading,
//! encoding them as base64 for inline use in web applications.

use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use image::imageops::FilterType;
use std::io::Cursor;
use std::path::PathBuf;
use walkdir::WalkDir;

/// CLI arguments for the LQIP (Low-Quality Image Placeholder) command.
#[derive(clap::Args, Debug)]
pub struct LqipArgs {
    /// Directory or file to process
    #[arg(short, long)]
    pub target: String,

    /// Width of the LQIP
    #[arg(long, default_value_t = 20)]
    pub width: u32,

    /// Height of the LQIP
    #[arg(long, default_value_t = 15)]
    pub height: u32,

    /// Recursive search
    #[arg(short, long)]
    pub recursive: bool,

    /// Output format: json or text
    #[arg(long, default_value = "text")]
    pub format: String,
}

/// Run the LQIP generation command.
pub async fn run(args: LqipArgs) -> Result<()> {
    let target_path = PathBuf::from(&args.target);

    if !target_path.exists() {
        anyhow::bail!("Target path does not exist: {target_path:?}");
    }

    let mut images = Vec::new();

    if target_path.is_file() {
        images.push(target_path);
    } else {
        let walker = WalkDir::new(&target_path);
        let walker = if args.recursive {
            walker
        } else {
            walker.max_depth(1)
        };

        for entry in walker.into_iter().filter_map(std::result::Result::ok) {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if ["jpg", "jpeg", "png", "webp"].contains(&ext_str.as_str()) {
                    images.push(path.to_path_buf());
                }
            }
        }
    }

    let mut results = Vec::new();

    for path in images {
        let img = image::open(&path).context(format!("Failed to open image: {path:?}"))?;
        let resized = img.resize(args.width, args.height, FilterType::Triangle);

        // Write to buffer
        let mut buffer = Cursor::new(Vec::new());
        // Use the original format or default to PNG if checking webp compatibility is complex
        // The original script kept the format. `image` crate usage:
        // We can output as PNG or WebP. WebP is good for web.
        // Let's use the extension to decide, or just default to the original format if possible.
        // `image` crate's `write_to` requires a format.
        // Let's deduce format from path.
        let format = image::ImageFormat::from_path(&path).unwrap_or(image::ImageFormat::Png);

        resized
            .write_to(&mut buffer, format)
            .context("Failed to encode resized image")?;

        let b64 = general_purpose::STANDARD.encode(buffer.get_ref());
        let mime = match format {
            image::ImageFormat::Png => "image/png",
            image::ImageFormat::Jpeg => "image/jpeg",
            image::ImageFormat::WebP => "image/webp",
            _ => "image/png", // fallback
        };

        let data_uri = format!("data:{mime};base64,{b64}");

        if args.format == "json" {
            results.push(serde_json::json!({
                "src": path.file_stem().unwrap().to_string_lossy(),
                "path": path.to_string_lossy(),
                "lqip": data_uri
            }));
        } else {
            println!("File: {path:?}\nLQIP: {data_uri}\n");
        }
    }

    if args.format == "json" {
        println!("{}", serde_json::to_string_pretty(&results)?);
    }

    Ok(())
}
