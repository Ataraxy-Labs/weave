//! `weave land --check sem`: the merged tree is verified by `sem check`
//! before anything is published.
//!
//! `sem check --base <tip> --json` runs in the directory holding the merged
//! tree (the agent's repository with `--onto`, the lander's worktree with
//! `--queue`). It returns the verdict of the project's own compiler, type
//! checker, linter and tests on the whole project, rechecking incrementally
//! only where that is provably the same verdict. Exit 0 lands; exit 1 (fail),
//! exit 2 (could not decide), any other exit, unreadable output, or no `sem`
//! at all refuses: the check is never skipped once asked for.
//!
//! Turned on by `--check sem`, or by default with `.weave/config`
//! (git-config syntax) in the repository root:
//!
//! ```text
//! [land]
//!     check = sem
//!     checkers = ts,lint,tests   # optional: passed as --checkers
//! ```
//!
//! `--check none` turns a configured default off for one run.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};

use crate::gitscan;

type R<T> = Result<T, Box<dyn std::error::Error>>;

/// What `--check sem` runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// `--checkers` for sem check (None: its own default, every checker it detects).
    pub checkers: Option<String>,
}

/// `--check` (`sem` | `none`) over the repository default in `.weave/config`.
pub fn resolve(flag: Option<&str>, checkers: Option<&str>, repo: &Path) -> R<Option<Check>> {
    let (configured, cfg_checkers) = config_default(repo);
    let want = match flag {
        Some("sem") => true,
        Some("none") => false,
        Some(other) => return Err(format!("--check wants `sem` or `none`, not `{other}`").into()),
        None => match configured.as_deref() {
            Some("sem") => true,
            None | Some("none") | Some("") => false,
            Some(other) => {
                return Err(format!(".weave/config: land.check wants `sem` or `none`, not `{other}`").into())
            }
        },
    };
    Ok(want.then(|| Check {
        checkers: checkers.map(str::to_string).or(cfg_checkers),
    }))
}

/// `land.check` and `land.checkers` from `<repo>/.weave/config`.
pub fn config_default(repo: &Path) -> (Option<String>, Option<String>) {
    let top = gitscan::git(repo, &["rev-parse", "--show-toplevel"])
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|_| repo.to_path_buf());
    let file = top.join(".weave/config");
    if !file.exists() {
        return (None, None);
    }
    let get = |key: &str| {
        Command::new("git")
            .arg("config")
            .arg("--file")
            .arg(&file)
            .arg("--get")
            .arg(key)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    (get("land.check"), get("land.checkers"))
}

/// The `sem` binary: `$WEAVE_SEM`, else `sem` on PATH.
fn sem_binary() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("WEAVE_SEM").filter(|p| !p.is_empty()) {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join("sem"))
        .find(|p| p.is_file())
}

/// How a check ended.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Exit 0: the sem check document (verdict, checkers, certificate).
    Pass(Value),
    /// Not landable: short machine reason, the report for the agent, and the
    /// sem check document when there is one.
    Refused {
        reason: &'static str,
        report: String,
        doc: Option<Value>,
    },
}

/// Run `sem check --base <tip> --json` in `dir`, whose HEAD has tree `tree`
/// (the tree to be published). A pass counts only if sem checked exactly that
/// tree: untracked files in `dir` would make its verdict one about another tree.
pub fn run(dir: &Path, tip: &str, tree: &str, check: &Check, limit: Duration, env: &[(&str, &str)]) -> R<Outcome> {
    let Some(sem) = sem_binary() else {
        return Ok(Outcome::Refused {
            reason: "sem not installed",
            report: "land: --check sem was asked for (or .weave/config sets land.check = sem), but \
                     `sem` is not installed: it is not on PATH and WEAVE_SEM does not name it. \
                     The merged tree was not verified, so nothing was published. Install sem, \
                     or pass --check none to land without it."
                .to_string(),
            doc: None,
        });
    };
    let mut cmd = Command::new(&sem);
    cmd.args(["check", "--base", tip, "--json"]).current_dir(dir);
    if let Some(c) = &check.checkers {
        cmd.args(["--checkers", c]);
    }
    cmd.arg("--timeout").arg(limit.as_secs().max(1).to_string());
    for (k, v) in env {
        cmd.env(k, v);
    }
    // sem's own per-tool limit is `limit`; give the whole run a little more
    let run = gitscan::run_bounded(cmd, "sem check", None, limit + Duration::from_secs(60));
    let (status, stdout, stderr) = match run {
        Ok(r) => r,
        Err(e) => {
            return Ok(Outcome::Refused {
                reason: "sem check could not decide",
                report: format!("land: sem check did not finish: {e}"),
                doc: None,
            })
        }
    };
    let doc: Option<Value> = serde_json::from_slice(&stdout).ok();
    let code = status.code();
    let Some(doc) = doc else {
        let tail = |b: &[u8]| {
            let t = String::from_utf8_lossy(b);
            let lines: Vec<&str> = t.lines().collect();
            lines[lines.len().saturating_sub(30)..].join("\n")
        };
        return Ok(Outcome::Refused {
            reason: "sem check could not decide",
            report: format!(
                "land: sem check exited {} without a readable result:\n{}\n{}",
                code.map_or("on a signal".into(), |c| c.to_string()),
                tail(&stdout),
                tail(&stderr)
            ),
            doc: None,
        });
    };
    match code {
        Some(0) if doc["verdict"] == "pass" && doc["head"]["tree"].as_str() != Some(tree) => {
            let untracked = gitscan::git(dir, &["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();
            Ok(Outcome::Refused {
                reason: "sem check checked another tree",
                report: format!(
                    "land: sem check passed, but on a tree that is not the one to be published \
                     (checked {}, publishing {tree}): untracked files in the working tree are \
                     part of what the tools saw:\n{}\nRemove them, ignore them (.gitignore), or \
                     commit them, then run land again.",
                    doc["head"]["tree"].as_str().unwrap_or("?"),
                    untracked.lines().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n")
                ),
                doc: Some(doc),
            })
        }
        Some(0) if doc["verdict"] == "pass" => Ok(Outcome::Pass(doc)),
        Some(1) => Ok(Outcome::Refused {
            reason: "sem check failed",
            report: render(&doc, "FAILED"),
            doc: Some(doc),
        }),
        _ => Ok(Outcome::Refused {
            reason: "sem check could not decide",
            report: render(&doc, "COULD NOT DECIDE"),
            doc: Some(doc),
        }),
    }
}

/// The sem check result in words for the agent: every checker's verdict,
/// mode and reasons, then its diagnostics (the first 100 per checker).
pub fn render(doc: &Value, headline: &str) -> String {
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let mut out = vec![format!("land: sem check {headline} on the merged tree:")];
    if let Some(e) = doc.get("error").and_then(Value::as_str) {
        out.push(format!("  {e}"));
    }
    for n in doc["notes"].as_array().into_iter().flatten() {
        out.push(format!("  note: {}", s(n)));
    }
    for c in doc["checkers"].as_array().into_iter().flatten() {
        out.push(format!(
            "  {} {} ({}, {}, {} rechecked)",
            s(&c["name"]),
            s(&c["verdict"]).to_uppercase(),
            s(&c["tool"]),
            s(&c["mode"]),
            c["filesRecheckedCount"]
        ));
        for r in c["reasons"].as_array().into_iter().flatten() {
            out.push(format!("    because: {}", s(r)));
        }
        let diags = c["diagnostics"].as_array().cloned().unwrap_or_default();
        for d in diags.iter().take(100) {
            for (i, line) in s(d).lines().enumerate() {
                out.push(format!("{}{line}", if i == 0 { "    " } else { "      " }));
            }
        }
        if diags.len() > 100 {
            out.push(format!("    ... {} more", diags.len() - 100));
        }
    }
    out.join("\n")
}

/// One line per checker for a passing check.
pub fn summary(doc: &Value) -> String {
    let parts: Vec<String> = doc["checkers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            format!(
                "{} {} {} ({} rechecked)",
                c["name"].as_str().unwrap_or("?"),
                c["verdict"].as_str().unwrap_or("?"),
                c["mode"].as_str().unwrap_or("?"),
                c["filesRecheckedCount"]
            )
        })
        .collect();
    parts.join("; ")
}

/// The document weave records for a check: what was checked and sem check's
/// own certificate (input trees, tool versions, mode per checker).
pub fn certificate(tip: &str, head: &str, tree: &str, outcome: &Outcome) -> Value {
    let (verdict, reason, doc) = match outcome {
        Outcome::Pass(d) => ("pass", None, Some(d)),
        Outcome::Refused { reason, doc, .. } => ("refused", Some(*reason), doc.as_ref()),
    };
    json!({
        "kind": "sem-check",
        "onto": tip,
        "head": head,
        "tree": tree,
        "verdict": verdict,
        "reason": reason,
        "sem": doc.map(|d| json!({
            "verdict": d["verdict"],
            "exitCode": d["exitCode"],
            "checkers": d["checkers"].as_array().map(|a| a.iter().map(|c| json!({
                "name": c["name"], "tool": c["tool"], "toolVersion": c["toolVersion"],
                "verdict": c["verdict"], "mode": c["mode"], "reasons": c["reasons"],
                "filesRechecked": c["filesRechecked"], "errors": c["errors"],
                "diagnostics": c["diagnostics"],
            })).collect::<Vec<_>>()),
            "certificate": d["certificate"],
        })),
    })
}
