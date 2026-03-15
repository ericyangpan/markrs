use serde::Deserialize;
use std::collections::BTreeMap;

mod markdown;

#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub gfm: bool,
    pub breaks: bool,
    pub pedantic: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            gfm: true,
            breaks: false,
            pedantic: false,
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct ThemeFile {
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    pub css: Option<String>,
}

pub fn render_markdown_to_html(input: &str, options: RenderOptions) -> String {
    markdown::render_markdown_to_html(input, options)
}

pub fn render_markdown_to_html_buf(input: &str, options: RenderOptions, buf: &mut String) {
    buf.clear();
    markdown::render_markdown_to_html_buf(input, options, buf)
}

fn preset_theme_vars(theme: &str) -> BTreeMap<String, String> {
    let mut vars = BTreeMap::new();
    match theme {
        "dracula" => {
            vars.insert("--markast-bg".to_string(), "#282a36".to_string());
            vars.insert("--markast-fg".to_string(), "#f8f8f2".to_string());
            vars.insert("--markast-muted".to_string(), "#bd93f9".to_string());
            vars.insert("--markast-border".to_string(), "#44475a".to_string());
            vars.insert("--markast-link".to_string(), "#8be9fd".to_string());
            vars.insert("--markast-code-bg".to_string(), "#1f212b".to_string());
            vars.insert("--markast-quote".to_string(), "#50fa7b".to_string());
        }
        "paper" => {
            vars.insert("--markast-bg".to_string(), "#fffdf8".to_string());
            vars.insert("--markast-fg".to_string(), "#2c241b".to_string());
            vars.insert("--markast-muted".to_string(), "#8e7f73".to_string());
            vars.insert("--markast-border".to_string(), "#e8dccf".to_string());
            vars.insert("--markast-link".to_string(), "#9f3a00".to_string());
            vars.insert("--markast-code-bg".to_string(), "#f4ede2".to_string());
            vars.insert("--markast-quote".to_string(), "#b7791f".to_string());
        }
        _ => {
            vars.insert("--markast-bg".to_string(), "#ffffff".to_string());
            vars.insert("--markast-fg".to_string(), "#1f2328".to_string());
            vars.insert("--markast-muted".to_string(), "#59636e".to_string());
            vars.insert("--markast-border".to_string(), "#d0d7de".to_string());
            vars.insert("--markast-link".to_string(), "#0969da".to_string());
            vars.insert("--markast-code-bg".to_string(), "#f6f8fa".to_string());
            vars.insert("--markast-quote".to_string(), "#6e7781".to_string());
        }
    }
    vars
}

fn base_stylesheet() -> &'static str {
    r#":root {
  color-scheme: light;
}
* {
  box-sizing: border-box;
}
body {
  margin: 0;
  background: var(--markast-bg);
  color: var(--markast-fg);
  font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Helvetica, Arial, sans-serif;
  line-height: 1.6;
}
.markast {
  max-width: 900px;
  margin: 0 auto;
  padding: 2.5rem 1.2rem 4rem;
}
.markast :is(h1, h2, h3, h4, h5, h6) {
  line-height: 1.25;
  margin: 1.35em 0 0.55em;
}
.markast h1 {
  font-size: 2rem;
  border-bottom: 1px solid var(--markast-border);
  padding-bottom: 0.35rem;
}
.markast h2 {
  font-size: 1.5rem;
  border-bottom: 1px solid var(--markast-border);
  padding-bottom: 0.25rem;
}
.markast p,
.markast ul,
.markast ol,
.markast blockquote,
.markast table,
.markast pre {
  margin: 0 0 1rem;
}
.markast a {
  color: var(--markast-link);
  text-decoration: underline;
}
.markast code {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  background: var(--markast-code-bg);
  border: 1px solid var(--markast-border);
  border-radius: 6px;
  padding: 0.1em 0.35em;
  font-size: 0.92em;
}
.markast pre {
  background: var(--markast-code-bg);
  border: 1px solid var(--markast-border);
  border-radius: 10px;
  padding: 0.9rem;
  overflow-x: auto;
}
.markast pre code {
  border: 0;
  padding: 0;
  background: transparent;
}
.markast blockquote {
  margin-left: 0;
  padding: 0.2rem 1rem;
  border-left: 4px solid var(--markast-quote);
  color: var(--markast-muted);
}
.markast table {
  border-collapse: collapse;
  width: 100%;
}
.markast th,
.markast td {
  border: 1px solid var(--markast-border);
  padding: 0.45rem 0.65rem;
  text-align: left;
}
.markast hr {
  border: 0;
  border-top: 1px solid var(--markast-border);
  margin: 1.5rem 0;
}
"#
}

fn vars_to_css(vars: &BTreeMap<String, String>) -> String {
    let mut out = String::from(":root {\n");
    for (k, v) in vars {
        out.push_str("  ");
        out.push_str(k);
        out.push_str(": ");
        out.push_str(v);
        out.push_str(";\n");
    }
    out.push_str("}\n");
    out
}

pub fn build_html_document(
    fragment: &str,
    theme: &str,
    theme_file: Option<ThemeFile>,
    extra_css: Option<&str>,
) -> String {
    let mut vars = preset_theme_vars(theme);
    let mut theme_inline_css = String::new();

    if let Some(file) = theme_file {
        for (k, v) in file.variables {
            vars.insert(k, v);
        }
        if let Some(css) = file.css {
            theme_inline_css = css;
        }
    }

    let mut styles = String::new();
    styles.push_str(&vars_to_css(&vars));
    styles.push_str(base_stylesheet());

    if !theme_inline_css.trim().is_empty() {
        styles.push('\n');
        styles.push_str(&theme_inline_css);
        styles.push('\n');
    }

    if let Some(css) = extra_css {
        if !css.trim().is_empty() {
            styles.push('\n');
            styles.push_str(css);
            styles.push('\n');
        }
    }

    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n  <meta charset=\"utf-8\">\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  <title>markast output</title>\n  <style>\n{styles}  </style>\n</head>\n<body>\n  <main class=\"markast\">\n{fragment}\n  </main>\n</body>\n</html>\n"
    )
}
