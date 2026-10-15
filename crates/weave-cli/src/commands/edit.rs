//! `weave edit`: change code by entity, at the command line.
//!
//! ```text
//! weave edit src/lib.rs --entity parse --content-file new.rs
//! weave edit src/lib.rs --entity parse --old 'x + 1' --new 'x + 2'
//! weave edit '[{"file":"a.py","entity":"f","delete":true}, ...]'   # a batch
//! ```
//!
//! Exit 0 = written (or nothing to change), 2 = refused, nothing written.

use std::io::Read;

use weave_cli::edit::{self, Edit};

pub(crate) struct Args {
    pub file_or_batch: Option<String>,
    pub single: Edit,
    pub content_file: Option<String>,
    pub dry_run: bool,
    pub json: bool,
}

/// `weave edit --review file[=before]...`: review edits another tool made.
pub(crate) fn review(specs: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let root = weave_core::git::find_repo_root(std::path::Path::new("."))
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let files: Vec<(String, Option<String>)> = specs
        .iter()
        .map(|s| match s.split_once('=') {
            Some((f, b)) if !b.is_empty() => (f.to_string(), std::fs::read_to_string(b).ok()),
            Some((f, _)) => (f.to_string(), Some(String::new())),
            None => (s.clone(), None),
        })
        .collect();
    for line in edit::review_external(&root, &files) {
        println!("{line}");
    }
    Ok(())
}

pub(crate) fn run(mut a: Args) -> Result<(), Box<dyn std::error::Error>> {
    let edits: Vec<Edit> = match a.file_or_batch.as_deref() {
        Some(s) if s.trim_start().starts_with(['[', '{']) => batch(s)?,
        Some("-") => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            batch(&s)?
        }
        Some(file) => {
            a.single.file = file.to_string();
            if let Some(p) = &a.content_file {
                a.single.content = Some(if p == "-" {
                    let mut s = String::new();
                    std::io::stdin().read_to_string(&mut s)?;
                    s
                } else {
                    std::fs::read_to_string(p)?
                });
            }
            vec![a.single]
        }
        None => {
            return Err("name a file, or pass a JSON batch (or - to read one from stdin)".into())
        }
    };
    let root = weave_core::git::find_repo_root(std::path::Path::new("."))
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    match edit::run(&root, &edits, a.dry_run) {
        Ok(r) => {
            if r.written {
                edit::speculate(&root);
            }
            if a.json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else if r.files.is_empty() {
                println!("nothing changed");
            } else {
                for f in &r.files {
                    let mut parts = Vec::new();
                    for (verb, names) in [
                        ("added", &f.added),
                        ("modified", &f.modified),
                        ("deleted", &f.deleted),
                    ] {
                        if !names.is_empty() {
                            parts.push(format!("{verb} {}", names.join(", ")));
                        }
                    }
                    let what = if parts.is_empty() {
                        "changed".to_string()
                    } else {
                        parts.join("; ")
                    };
                    let created = if f.created { " (new file)" } else { "" };
                    let dry = if a.dry_run { " [dry run]" } else { "" };
                    println!("{}{created}: {what}{dry}", f.file);
                }
            }
            Ok(())
        }
        Err(e) => {
            if a.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({ "written": false, "error": e })
                    )?
                );
            } else {
                eprintln!("weave edit: {e}");
                for c in &e.candidates {
                    eprintln!("  {c}");
                }
                eprintln!("nothing was written");
            }
            std::process::exit(2);
        }
    }
}

fn batch(s: &str) -> Result<Vec<Edit>, Box<dyn std::error::Error>> {
    let v: serde_json::Value = serde_json::from_str(s)?;
    let v = match v {
        serde_json::Value::Object(ref o) if o.contains_key("edits") => o["edits"].clone(),
        serde_json::Value::Object(_) => serde_json::Value::Array(vec![v]),
        other => other,
    };
    Ok(serde_json::from_value(v)?)
}
