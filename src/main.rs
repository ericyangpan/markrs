use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser as ClapParser;
use markast::{RenderOptions, ThemeFile, build_html_document, render_markdown_to_html};

#[derive(ClapParser, Debug)]
#[command(name = "markast", version, about = "Render Markdown to HTML")]
struct Args {
    /// Markdown file path. If omitted, reads from stdin.
    input: Option<PathBuf>,

    /// Output full HTML document with embedded styles.
    #[arg(long)]
    document: bool,

    /// Built-in theme for --document: github | dracula | paper
    #[arg(long, default_value = "github", value_parser = ["github", "dracula", "paper"])]
    theme: String,

    /// Path to JSON theme definition file. Works with --document.
    #[arg(long)]
    theme_file: Option<PathBuf>,

    /// Path to extra CSS file appended after theme styles. Works with --document.
    #[arg(long)]
    css: Option<PathBuf>,
}

fn read_input(path: Option<PathBuf>) -> Result<String> {
    if let Some(path) = path {
        return fs::read_to_string(&path)
            .with_context(|| format!("failed to read markdown file: {}", path.display()));
    }

    let mut stdin = io::stdin();
    if stdin.is_terminal() {
        anyhow::bail!(
            "no input provided. pass a markdown file path or pipe content into markast (e.g. cat README.md | markast)"
        );
    }

    let mut buf = String::new();
    stdin
        .read_to_string(&mut buf)
        .context("failed to read markdown from stdin")?;
    Ok(buf)
}

fn read_theme_file(path: Option<PathBuf>) -> Result<Option<ThemeFile>> {
    let Some(path) = path else {
        return Ok(None);
    };

    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read theme file: {}", path.display()))?;
    let parsed: ThemeFile = serde_json::from_str(&content)
        .with_context(|| format!("invalid JSON theme file: {}", path.display()))?;
    Ok(Some(parsed))
}

fn read_extra_css(path: Option<PathBuf>) -> Result<Option<String>> {
    let Some(path) = path else {
        return Ok(None);
    };

    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read css file: {}", path.display()))?;
    Ok(Some(content))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let markdown = read_input(args.input)?;
    let fragment = render_markdown_to_html(&markdown, RenderOptions::default());

    if !args.document {
        print!("{fragment}");
        return Ok(());
    }

    let theme_file = read_theme_file(args.theme_file)?;
    let extra_css = read_extra_css(args.css)?;
    let doc = build_html_document(&fragment, &args.theme, theme_file, extra_css.as_deref());
    print!("{doc}");
    Ok(())
}
