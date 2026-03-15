use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use markast::{RenderOptions, render_markdown_to_html_buf};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(
    name = "markast-bench",
    about = "In-process benchmark runner for markdown engines"
)]
struct Args {
    #[arg(long)]
    input: PathBuf,
}

#[derive(Debug, Deserialize)]
struct BenchInput {
    suites: Vec<SuiteInput>,
}

#[derive(Debug, Deserialize)]
struct SuiteInput {
    id: String,
    warmup_runs: usize,
    measure_runs: usize,
    docs: Vec<String>,
    options: SuiteOptions,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct SuiteOptions {
    gfm: bool,
    breaks: bool,
    pedantic: bool,
}

#[derive(Debug, Serialize)]
struct BenchOutput {
    engine: &'static str,
    suites: Vec<SuiteOutput>,
}

#[derive(Debug, Serialize)]
struct SuiteOutput {
    id: String,
    docs: usize,
    input_bytes: usize,
    output_bytes: usize,
    checksum: u64,
    warmup_runs: usize,
    measure_runs: usize,
    runs_ms: Vec<f64>,
}

fn suite_options_to_render_options(options: SuiteOptions) -> RenderOptions {
    RenderOptions {
        gfm: options.gfm,
        breaks: options.breaks,
        pedantic: options.pedantic,
    }
}

fn render_suite(docs: &[String], options: SuiteOptions) -> (usize, u64) {
    let mut output_bytes = 0usize;
    let mut hasher = DefaultHasher::new();
    let mut buf = String::with_capacity(4096);

    for doc in docs {
        render_markdown_to_html_buf(doc, suite_options_to_render_options(options), &mut buf);
        output_bytes += buf.len();
        buf.hash(&mut hasher);
    }

    (output_bytes, hasher.finish())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let raw = fs::read_to_string(&args.input)
        .with_context(|| format!("failed to read benchmark input: {}", args.input.display()))?;
    let input: BenchInput = serde_json::from_str(&raw)
        .with_context(|| format!("invalid benchmark input json: {}", args.input.display()))?;

    let mut suites = Vec::with_capacity(input.suites.len());

    for suite in input.suites {
        for _ in 0..suite.warmup_runs {
            let _ = render_suite(&suite.docs, suite.options);
        }

        let mut runs_ms = Vec::with_capacity(suite.measure_runs);
        let mut output_bytes = 0usize;
        let mut checksum = 0u64;

        for _ in 0..suite.measure_runs {
            let started = Instant::now();
            let (bytes, hash) = render_suite(&suite.docs, suite.options);
            let elapsed = started.elapsed();
            output_bytes = bytes;
            checksum = hash;
            runs_ms.push(elapsed.as_secs_f64() * 1000.0);
        }

        let input_bytes = suite.docs.iter().map(|doc| doc.len()).sum();
        suites.push(SuiteOutput {
            id: suite.id,
            docs: suite.docs.len(),
            input_bytes,
            output_bytes,
            checksum,
            warmup_runs: suite.warmup_runs,
            measure_runs: suite.measure_runs,
            runs_ms,
        });
    }

    let output = BenchOutput {
        engine: "markast",
        suites,
    };
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
