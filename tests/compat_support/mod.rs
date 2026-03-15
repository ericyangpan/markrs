use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use globset::{Glob, GlobSet, GlobSetBuilder};
use markast::RenderOptions;
use regex::Regex;
use serde::Deserialize;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub(crate) struct CompatCase {
    pub(crate) id: String,
    pub(crate) markdown: String,
    #[allow(dead_code)]
    pub(crate) expected_html: String,
    pub(crate) options: RenderOptions,
}

#[derive(Debug, Deserialize)]
struct JsonSpecCase {
    markdown: String,
    html: String,
    #[allow(dead_code)]
    example: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct XfailConfig {
    #[serde(default)]
    pub(crate) cases: Vec<XfailCase>,
    #[serde(default)]
    pub(crate) patterns: Vec<XfailPattern>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct XfailCase {
    pub(crate) id: String,
    pub(crate) reason: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct XfailPattern {
    pub(crate) pattern: String,
    #[allow(dead_code)]
    pub(crate) reason: String,
}

static WS_BETWEEN_TAGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r">\s+<").expect("invalid regex"));
static XHTML_VOID_SLASH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<(br|hr|img|input)([^>]*?)\s*/?\s*>").expect("invalid regex"));
static BR_FOLLOWED_BY_NEWLINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<br>\s*\n+").expect("invalid regex"));
static TEXT_BEFORE_BLOCKQUOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([^\s>])\s+<blockquote>").expect("invalid regex"));
static INPUT_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<input([^>]*)>").expect("invalid regex"));

pub(crate) fn normalize_html(input: &str) -> String {
    let compact = WS_BETWEEN_TAGS.replace_all(input, "><");
    let normalized_void = XHTML_VOID_SLASH.replace_all(&compact, "<$1$2>");
    let normalized_break = BR_FOLLOWED_BY_NEWLINE.replace_all(&normalized_void, "<br>");
    let normalized_blockquote =
        TEXT_BEFORE_BLOCKQUOTE.replace_all(&normalized_break, "$1<blockquote>");
    let normalized_input = normalize_input_attrs(&normalized_blockquote);
    normalized_input
        .replace("\r\n", "\n")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#x22;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&gt;", ">")
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub(crate) fn should_use_gfm(case_id: &str) -> bool {
    if case_id.contains("/commonmark/") || case_id.contains("/commonmark.") {
        return false;
    }
    if case_id.contains("_nogfm") {
        return false;
    }
    true
}

pub(crate) fn strip_marked_front_matter(
    markdown: &str,
    mut options: RenderOptions,
) -> (String, RenderOptions) {
    let Some(rest) = markdown.strip_prefix("---\n") else {
        return (markdown.to_string(), options);
    };
    let Some(end) = rest.find("\n---\n") else {
        return (markdown.to_string(), options);
    };

    let header = &rest[..end];
    for line in header.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim();
        if key == "gfm" {
            options.gfm = val == "true";
        }
        if key == "breaks" {
            options.breaks = val == "true";
        }
        if key == "pedantic" {
            options.pedantic = val == "true";
        }
    }

    let body = &rest[end + "\n---\n".len()..];
    (body.to_string(), options)
}

pub(crate) fn collect_all_compat_cases(repo_root: &Path) -> Vec<CompatCase> {
    let specs_root = repo_root.join("third_party/marked/test/specs");
    assert!(
        specs_root.exists(),
        "missing third_party marked specs: {}",
        specs_root.display()
    );

    let mut cases = collect_md_html_cases(&specs_root.join("new"));
    cases.extend(collect_md_html_cases(&specs_root.join("original")));

    for json_file in [
        specs_root.join("commonmark/commonmark.0.31.2.json"),
        specs_root.join("gfm/commonmark.0.31.2.json"),
        specs_root.join("gfm/gfm.0.29.json"),
    ] {
        if json_file.exists() {
            cases.extend(collect_json_cases(
                &json_file,
                &repo_root.join("third_party/marked"),
            ));
        }
    }

    cases.sort_by(|a, b| a.id.cmp(&b.id));
    cases
}

pub(crate) fn load_xfail_config(path: &Path) -> XfailConfig {
    if !path.exists() {
        return XfailConfig::default();
    }

    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed reading {}: {e}", path.display()));
    serde_yaml::from_str(&content)
        .unwrap_or_else(|e| panic!("invalid yaml {}: {e}", path.display()))
}

pub(crate) fn build_pattern_matcher(patterns: &[XfailPattern]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(&pattern.pattern)
            .unwrap_or_else(|e| panic!("invalid xfail pattern '{}': {e}", pattern.pattern));
        builder.add(glob);
    }
    builder.build().expect("failed building xfail glob set")
}

pub(crate) fn write_xfail(
    path: &Path,
    failing_ids: &[String],
    headline: &str,
    update_command: &str,
    reason: &str,
) {
    let mut out = String::new();
    out.push_str("# Auto-generated baseline for ");
    out.push_str(headline);
    out.push_str(".\n");
    out.push_str("# Update with: ");
    out.push_str(update_command);
    out.push('\n');
    out.push_str("cases:\n");

    for id in failing_ids {
        out.push_str("  - id: \"");
        out.push_str(id);
        out.push_str("\"\n");
        out.push_str("    reason: \"");
        out.push_str(reason);
        out.push_str("\"\n");
    }

    out.push_str("patterns: []\n");
    fs::write(path, out).unwrap_or_else(|e| panic!("failed writing {}: {e}", path.display()));
}

fn collect_md_html_cases(specs_root: &Path) -> Vec<CompatCase> {
    let mut cases = Vec::new();

    for entry in WalkDir::new(specs_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        let html_path = path.with_extension("html");
        if !html_path.exists() {
            continue;
        }

        let rel = path
            .strip_prefix(specs_root.parent().unwrap_or(specs_root))
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let markdown_raw = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed reading {}: {e}", path.display()));
        let expected_html = fs::read_to_string(&html_path)
            .unwrap_or_else(|e| panic!("failed reading {}: {e}", html_path.display()));
        let default_options = RenderOptions {
            gfm: should_use_gfm(&rel),
            breaks: false,
            pedantic: false,
        };
        let (markdown, options) = strip_marked_front_matter(&markdown_raw, default_options);

        cases.push(CompatCase {
            id: rel,
            markdown,
            expected_html,
            options,
        });
    }

    cases
}

fn collect_json_cases(json_path: &Path, root_for_id: &Path) -> Vec<CompatCase> {
    let rel_base = json_path
        .strip_prefix(root_for_id)
        .unwrap_or(json_path)
        .to_string_lossy()
        .replace('\\', "/");

    let content = fs::read_to_string(json_path)
        .unwrap_or_else(|e| panic!("failed reading {}: {e}", json_path.display()));
    let list: Vec<JsonSpecCase> = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("invalid JSON {}: {e}", json_path.display()));

    let gfm = should_use_gfm(&rel_base);

    list.into_iter()
        .enumerate()
        .map(|(idx, item)| CompatCase {
            id: format!("{rel_base}#example-{}", idx + 1),
            markdown: item.markdown,
            expected_html: item.html,
            options: RenderOptions {
                gfm,
                breaks: false,
                pedantic: false,
            },
        })
        .collect()
}

fn normalize_input_attrs(input: &str) -> String {
    INPUT_TAG
        .replace_all(input, |caps: &regex::Captures<'_>| {
            let attrs = parse_attrs(caps.get(1).map(|m| m.as_str()).unwrap_or_default());
            if attrs.is_empty() {
                return "<input>".to_string();
            }
            let mut sorted = attrs;
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = String::from("<input");
            for (name, value) in sorted {
                out.push(' ');
                out.push_str(&name);
                if let Some(v) = value {
                    out.push_str("=\"");
                    out.push_str(&v);
                    out.push('"');
                }
            }
            out.push('>');
            out
        })
        .to_string()
}

fn parse_attrs(raw: &str) -> Vec<(String, Option<String>)> {
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();

    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        let name_start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '=' {
            i += 1;
        }
        if name_start == i {
            i += 1;
            continue;
        }
        let name: String = chars[name_start..i].iter().collect();

        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }

        let mut value = None;
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                let quote = chars[i];
                i += 1;
                let val_start = i;
                while i < chars.len() && chars[i] != quote {
                    i += 1;
                }
                value = Some(chars[val_start..i].iter().collect());
                if i < chars.len() && chars[i] == quote {
                    i += 1;
                }
            } else {
                let val_start = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                value = Some(chars[val_start..i].iter().collect());
            }
        }

        out.push((name, value));
    }

    out
}
