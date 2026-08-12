//! `weave patch` — the write side, at the command line.
//!
//! ```text
//! weave patch extract <base-file> <changed-file> [--embed-base] [-o ops.json]
//! weave patch apply   <ops.json>  <target-file>  [--base <file>] [--in-place]
//! ```
//!
//! `apply` writes the resulting file to stdout (or `-o` / `--in-place`) and
//! keeps stderr for the machine-readable channel: every conflict and every op
//! that did not land is printed as a single-line `weave-warning: {json}`, so a
//! caller can stream the result and still parse the diagnostics. Exit 0 = clean,
//! 1 = conflicts (markers are in the output), 2 = refused.

use std::io::Write;

use weave_cli::patch::{self, ApplyMode, PatchError};

type R<T> = Result<T, Box<dyn std::error::Error>>;

/// Write to `out` when given, otherwise stdout.
fn emit(out: Option<&str>, content: &str) -> R<()> {
    match out {
        Some(p) => std::fs::write(p, content)?,
        None => {
            let mut stdout = std::io::stdout();
            stdout.write_all(content.as_bytes())?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn warn(value: serde_json::Value) {
    eprintln!("weave-warning: {value}");
}

pub(crate) fn extract(
    base_file: &str,
    changed_file: &str,
    path: Option<&str>,
    embed_base: bool,
    out: Option<&str>,
) -> R<()> {
    let base = std::fs::read_to_string(base_file)?;
    let changed = std::fs::read_to_string(changed_file)?;
    // The path only ever selects a parser; it defaults to the changed file's
    // own name so `weave patch extract old.py new.py` does the obvious thing.
    let path = path.unwrap_or(changed_file);

    let doc = patch::extract(path, &base, &changed, embed_base);
    if !doc.exact {
        warn(serde_json::json!({
            "class": "LOSS",
            "entity": patch::FILE_KEY,
            "source": "derived",
            "suggestion": "This change is not expressible at entity granularity, so the ops carry \
                           a whole-file fallback. Applying them to a drifted target will merge the \
                           whole file rather than the entities that changed.",
        }));
    }
    emit(out, &format!("{}\n", serde_json::to_string_pretty(&doc)?))
}

pub(crate) fn apply(
    ops_file: &str,
    target_file: &str,
    base_file: Option<&str>,
    out: Option<&str>,
    in_place: bool,
    host: &weave_core::host::Host,
) -> R<()> {
    // The one decode. Everything past this line is a typed document: a field
    // the schema does not define, an op outside the vocabulary, or a major
    // this build does not implement are all refused here, by name.
    let doc = patch::parse_ops_doc(&std::fs::read_to_string(ops_file)?)
        .map_err(|e| format!("{ops_file}: {e}"))?;

    let target = std::fs::read_to_string(target_file)?;
    let base = base_file.map(std::fs::read_to_string).transpose()?;

    let report = match patch::apply(&doc, &target, target_file, base.as_deref(), host) {
        Ok(r) => r,
        // A provably wrong ancestor is the one case worth refusing outright:
        // merging against it produces a confidently wrong answer. The two
        // digests come off the error as fields; what to do about them is this
        // boundary's line to write, not the library's.
        Err(PatchError::BaseMismatch { expected, actual }) => {
            let short = |h: &str| h[..12.min(h.len())].to_string();
            warn(serde_json::json!({
                "class": "CONFLICT",
                "entity": patch::FILE_KEY,
                "source": "conflict",
                "expected_base_sha256": expected,
                "actual_base_sha256": actual,
                "suggestion": format!(
                    "base mismatch: ops were extracted against sha256 {} but the supplied base \
                     hashes to {}. Re-extract the ops, or drop the base and apply with drift \
                     detection off.",
                    short(&expected),
                    short(&actual),
                ),
            }));
            std::process::exit(2);
        }
    };

    for u in &report.unapplied {
        warn(serde_json::json!({
            "class": "DANGLING",
            "entity": u,
            "source": "derived",
            "suggestion": "This op had no target in the file it was applied to. Nothing was \
                           dropped silently — decide whether it is stale or the target is wrong.",
        }));
    }
    for f in &report.findings {
        warn(serde_json::to_value(f)?);
    }

    let dest = if in_place { Some(target_file) } else { out };
    emit(dest, &report.content)?;

    if !report.clean {
        eprintln!(
            "weave patch apply: {} conflict(s) written as markers ({} mode)",
            report.findings.len(),
            report.mode.wire()
        );
        std::process::exit(1);
    }
    if report.mode == ApplyMode::NoBase {
        eprintln!(
            "weave patch apply: no base available — the target was treated as the ops' base, so \
             drift could not be detected. Pass --base, or extract with --embed-base."
        );
    }
    Ok(())
}
