mod compat_support;
mod test_support;

use std::collections::{BTreeSet, HashMap};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use compat_support::{
    build_pattern_matcher, collect_all_compat_cases, load_xfail_config, normalize_html, write_xfail,
};
use markast::render_markdown_to_html;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct RuntimeCompatCase<'a> {
    id: &'a str,
    markdown: &'a str,
    options: RuntimeCompatOptions,
}

#[derive(Debug, Serialize)]
struct RuntimeCompatOptions {
    gfm: bool,
    breaks: bool,
    pedantic: bool,
}

#[derive(Debug, Deserialize)]
struct RuntimeCompatResponse {
    #[serde(rename = "markedVersion")]
    marked_version: String,
    cases: Vec<RuntimeCompatRenderedCase>,
}

#[derive(Debug, Deserialize)]
struct RuntimeCompatRenderedCase {
    id: String,
    html: String,
}

fn render_marked_runtime(
    repo_root: &Path,
    cases: &[(String, String, RuntimeCompatOptions)],
) -> RuntimeCompatResponse {
    let payload: Vec<RuntimeCompatCase<'_>> = cases
        .iter()
        .map(|(id, markdown, options)| RuntimeCompatCase {
            id,
            markdown,
            options: RuntimeCompatOptions {
                gfm: options.gfm,
                breaks: options.breaks,
                pedantic: options.pedantic,
            },
        })
        .collect();

    let input = serde_json::to_vec(&payload).expect("failed to serialize runtime compat cases");
    let script = repo_root.join("scripts/render-marked-runtime.mjs");
    let mut child = Command::new("node")
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", script.display()));

    {
        let mut stdin = child.stdin.take().expect("runtime compat stdin missing");
        stdin
            .write_all(&input)
            .expect("failed writing runtime compat input");
    }

    let output = child
        .wait_with_output()
        .expect("failed waiting for runtime compat process");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "runtime compat oracle failed (status={}): {}",
            output.status,
            stderr.trim()
        );
    }

    serde_json::from_slice(&output.stdout).expect("failed to parse runtime compat oracle output")
}

#[test]
fn marked_runtime_compatibility_suite() {
    let repo_root = test_support::repo_root();
    let xfail_path = repo_root.join("tests/compat/runtime_xfail.yaml");
    let ignore_xfail = std::env::var("MARKAST_IGNORE_RUNTIME_XFAIL")
        .ok()
        .as_deref()
        == Some("1");
    let print_diffs = std::env::var("MARKAST_PRINT_RUNTIME_DIFFS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    let cases = collect_all_compat_cases(&repo_root);
    assert!(
        !cases.is_empty(),
        "no marked runtime compatibility cases found"
    );

    let runtime_cases: Vec<(String, String, RuntimeCompatOptions)> = cases
        .iter()
        .map(|case| {
            (
                case.id.clone(),
                case.markdown.clone(),
                RuntimeCompatOptions {
                    gfm: case.options.gfm,
                    breaks: case.options.breaks,
                    pedantic: case.options.pedantic,
                },
            )
        })
        .collect();
    let runtime = render_marked_runtime(&repo_root, &runtime_cases);
    let runtime_map: HashMap<&str, &str> = runtime
        .cases
        .iter()
        .map(|case| (case.id.as_str(), case.html.as_str()))
        .collect();

    let xfail = load_xfail_config(&xfail_path);
    let pattern_matcher = build_pattern_matcher(&xfail.patterns);
    let exact: HashMap<&str, &str> = xfail
        .cases
        .iter()
        .map(|c| (c.id.as_str(), c.reason.as_str()))
        .collect();

    let mut failures = Vec::new();
    let mut xfailed = Vec::new();
    let mut recovered = Vec::new();
    let mut mismatch_samples: Vec<(String, String, String, bool)> = Vec::new();

    for case in &cases {
        let actual = render_markdown_to_html(&case.markdown, case.options);
        let normalized_actual = normalize_html(&actual);
        let runtime_html = runtime_map
            .get(case.id.as_str())
            .unwrap_or_else(|| panic!("runtime output missing case {}", case.id));
        let normalized_expected = normalize_html(runtime_html);
        let ok = normalized_actual == normalized_expected;

        let exact_xfail = exact.get(case.id.as_str()).copied();
        let pattern_xfail = pattern_matcher.is_match(case.id.as_str());
        let is_xfail = !ignore_xfail && (exact_xfail.is_some() || pattern_xfail);

        if ok {
            if exact_xfail.is_some() || pattern_xfail {
                recovered.push(case.id.clone());
            }
            continue;
        }

        if print_diffs > 0 && mismatch_samples.len() < print_diffs {
            mismatch_samples.push((
                case.id.clone(),
                normalized_expected,
                normalized_actual,
                is_xfail,
            ));
        }

        if is_xfail {
            xfailed.push(case.id.clone());
        } else {
            failures.push(case.id.clone());
        }
    }

    if std::env::var("MARKAST_WRITE_RUNTIME_XFAIL").ok().as_deref() == Some("1") {
        let mut baseline = failures.clone();
        baseline.extend(xfailed.clone());
        baseline.sort();
        baseline.dedup();
        let reason = format!("runtime mismatch vs marked@{}", runtime.marked_version);
        write_xfail(
            &xfail_path,
            &baseline,
            "current marked runtime mismatches",
            "MARKAST_WRITE_RUNTIME_XFAIL=1 cargo test --test compat_runtime -- --nocapture",
            &reason,
        );
        eprintln!(
            "wrote {} runtime baseline xfail entries to {}",
            baseline.len(),
            xfail_path.display()
        );
        return;
    }

    if !mismatch_samples.is_empty() {
        eprintln!(
            "runtime compat mismatch samples against marked@{}:",
            runtime.marked_version
        );
        for (id, expected, actual, is_xfail) in &mismatch_samples {
            let state = if *is_xfail { "xfail" } else { "fail" };
            eprintln!("--- [{state}] {id}");
            eprintln!("expected: {expected}");
            eprintln!("actual  : {actual}");
        }
    }

    let case_ids: BTreeSet<&str> = cases.iter().map(|c| c.id.as_str()).collect();
    let stale_xfail: Vec<String> = xfail
        .cases
        .iter()
        .filter(|x| !case_ids.contains(x.id.as_str()))
        .map(|x| x.id.clone())
        .collect();

    let mut report = String::new();

    if !failures.is_empty() {
        report.push_str("\nnew runtime compat failures (not in xfail):\n");
        for id in failures.iter().take(40) {
            report.push_str("  - ");
            report.push_str(id);
            report.push('\n');
        }
        if failures.len() > 40 {
            report.push_str("  ...\n");
        }
    }

    if !recovered.is_empty() {
        report.push_str("\nruntime xfail recovered (should be removed):\n");
        for id in recovered.iter().take(40) {
            report.push_str("  - ");
            report.push_str(id);
            report.push('\n');
        }
        if recovered.len() > 40 {
            report.push_str("  ...\n");
        }
    }

    if !stale_xfail.is_empty() {
        report.push_str("\nstale runtime xfail ids (fixture missing):\n");
        for id in stale_xfail.iter().take(40) {
            report.push_str("  - ");
            report.push_str(id);
            report.push('\n');
        }
        if stale_xfail.len() > 40 {
            report.push_str("  ...\n");
        }
    }

    if !report.is_empty() {
        panic!(
            "marked runtime compatibility check failed.\n{}\nsummary: marked_version={}, total_cases={}, xfailed={}, new_failures={}, recovered={}, stale_xfail={}\n\nIf runtime baseline changed intentionally, refresh with:\nMARKAST_WRITE_RUNTIME_XFAIL=1 cargo test --test compat_runtime -- --nocapture",
            report,
            runtime.marked_version,
            cases.len(),
            xfailed.len(),
            failures.len(),
            recovered.len(),
            stale_xfail.len(),
        );
    }
}
