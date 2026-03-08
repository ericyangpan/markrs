mod compat_support;

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use compat_support::{
    build_pattern_matcher, collect_all_compat_cases, load_xfail_config, normalize_html,
    should_use_gfm, write_xfail,
};
use markrs::render_markdown_to_html;

#[test]
fn compat_commonmark_mirror_cases_do_not_force_gfm() {
    assert!(!should_use_gfm(
        "test/specs/commonmark/commonmark.0.31.2.json#example-1"
    ));
    assert!(!should_use_gfm(
        "test/specs/gfm/commonmark.0.31.2.json#example-611"
    ));
    assert!(should_use_gfm("test/specs/gfm/gfm.0.29.json#example-13"));
}

#[test]
fn marked_snapshot_compatibility_suite() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let xfail_path = repo_root.join("tests/compat/xfail.yaml");
    let ignore_xfail = std::env::var("MARKRS_IGNORE_XFAIL").ok().as_deref() == Some("1");
    let print_diffs = std::env::var("MARKRS_PRINT_DIFFS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    let cases = collect_all_compat_cases(&repo_root);
    assert!(!cases.is_empty(), "no marked snapshot compatibility cases found");

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
        let normalized_expected = normalize_html(&case.expected_html);
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

    if std::env::var("MARKRS_WRITE_XFAIL").ok().as_deref() == Some("1") {
        let mut baseline = failures.clone();
        baseline.extend(xfailed.clone());
        baseline.sort();
        baseline.dedup();
        write_xfail(
            &xfail_path,
            &baseline,
            "vendored marked snapshot mismatches",
            "MARKRS_WRITE_XFAIL=1 cargo test --test compat_snapshot -- --nocapture",
            "snapshot mismatch vs vendored marked fixtures",
        );
        eprintln!(
            "wrote {} baseline xfail entries to {}",
            baseline.len(),
            xfail_path.display()
        );
        return;
    }

    if !mismatch_samples.is_empty() {
        eprintln!("snapshot compat mismatch samples:");
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
        report.push_str("\nnew snapshot compat failures (not in xfail):\n");
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
        report.push_str("\nxfail recovered (should be removed):\n");
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
        report.push_str("\nstale xfail ids (fixture missing):\n");
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
            "marked snapshot compatibility check failed.\n{}\nsummary: total_cases={}, xfailed={}, new_failures={}, recovered={}, stale_xfail={}\n\nIf snapshot baseline changed intentionally, refresh with:\nMARKRS_WRITE_XFAIL=1 cargo test --test compat_snapshot -- --nocapture",
            report,
            cases.len(),
            xfailed.len(),
            failures.len(),
            recovered.len(),
            stale_xfail.len(),
        );
    }
}
