use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use pulldown_cmark::{Options as PulldownOptions, Parser as PulldownParser, html};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(
    name = "pulldown-cmark-runner",
    about = "In-process benchmark runner for pulldown-cmark"
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

fn suite_options_to_pulldown_options(options: SuiteOptions) -> PulldownOptions {
    let mut pulldown = PulldownOptions::empty();
    if options.gfm {
        pulldown.insert(PulldownOptions::ENABLE_TABLES);
        pulldown.insert(PulldownOptions::ENABLE_STRIKETHROUGH);
        pulldown.insert(PulldownOptions::ENABLE_TASKLISTS);
    }
    let _ = options.breaks;
    let _ = options.pedantic;
    pulldown
}

fn render_pulldown_to_html(doc: &str, options: SuiteOptions) -> String {
    let parser = PulldownParser::new_ext(doc, suite_options_to_pulldown_options(options));
    let mut out = String::with_capacity(doc.len().saturating_mul(2));
    html::push_html(&mut out, parser);
    out
}

fn render_suite(docs: &[String], options: SuiteOptions) -> (usize, u64) {
    let mut output_bytes = 0usize;
    let mut hasher = DefaultHasher::new();

    for doc in docs {
        let html = render_pulldown_to_html(doc, options);
        output_bytes += html.len();
        html.hash(&mut hasher);
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
        engine: "pulldown-cmark",
        suites,
    };
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
