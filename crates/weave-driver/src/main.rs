use std::fs;
use std::process;

use weave_core::stats::WeaveLifetimeStats;
use weave_core::{entity_merge_fmt, MarkerFormat};

fn main() {
    env_logger::init();

    let raw_args: Vec<String> = std::env::args().collect();

    // Parse optional flags before positional args
    // Supported flags: -o <path> / --output <path>, --audit
    let mut output_override: Option<String> = None;
    let mut audit_enabled = false;
    let mut marker_length: Option<usize> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut i = 1;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--version" | "-V" => {
                println!("weave-driver {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "-o" | "--output" => {
                if i + 1 < raw_args.len() {
                    output_override = Some(raw_args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("weave: -o/--output requires a path argument");
                    process::exit(2);
                }
            }
            "--audit" => {
                audit_enabled = true;
                i += 1;
            }
            "-l" | "--marker-length" => {
                if i + 1 < raw_args.len() {
                    marker_length = raw_args[i + 1].parse::<usize>().ok();
                }
                i += 2;
            }
            "-p" | "--path" => {
                if i + 1 < raw_args.len() {
                    // Will be picked up from positional or used directly
                    positional.push(raw_args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                positional.push(raw_args[i].clone());
                i += 1;
            }
        }
    }

    // Git calls: weave-driver %O %A %B %L %P
    // jj calls:  weave-driver $base $left $right -o $output -l $marker_length -p $path
    // %O = ancestor (base), %A = current (ours), %B = other (theirs)
    // %L = conflict marker size, %P = file path
    if positional.len() < 3 {
        eprintln!("Usage: weave-driver <base> <ours> <theirs> [marker-size] [file-path]");
        eprintln!("       weave-driver <base> <ours> <theirs> -o <output> [-l <marker-length>] [-p <path>]");
        eprintln!("  Invoked by git as a merge driver, or by jj as a merge tool.");
        process::exit(2);
    }

    let base_path = &positional[0];
    let ours_path = &positional[1];
    let theirs_path = &positional[2];
    // Git calls: %O %A %B %L %P (positional[3] = marker size, positional[4] = file path)
    // jj calls: base left right -o output -l marker_length -p path (flags already parsed above)
    // Note: positional[3] from git (%L) does NOT trigger standard markers; only the -l flag does.
    let file_path = if positional.len() > 4 {
        positional[4].clone()
    } else if positional.len() > 3 {
        positional[3].clone()
    } else {
        ours_path.clone()
    };

    // Read input files
    let base = match fs::read_to_string(base_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("weave: failed to read base file '{}': {}", base_path, e);
            process::exit(2);
        }
    };
    let ours = match fs::read_to_string(ours_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("weave: failed to read ours file '{}': {}", ours_path, e);
            process::exit(2);
        }
    };
    let theirs = match fs::read_to_string(theirs_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("weave: failed to read theirs file '{}': {}", theirs_path, e);
            process::exit(2);
        }
    };

    // Detect binary content (null bytes in first 8KB)
    if is_binary(&base) || is_binary(&ours) || is_binary(&theirs) {
        eprintln!(
            "weave: binary file detected, skipping entity merge for '{}'",
            file_path
        );
        process::exit(2);
    }

    // Run entity merge
    // When -l is passed (by jj or other tools), use standard git-compatible markers.
    // Otherwise (git merge driver), use enhanced markers with entity metadata.
    let fmt = match marker_length {
        Some(len) => MarkerFormat::standard(len),
        None => MarkerFormat::default(),
    };
    let result = entity_merge_fmt(&base, &ours, &theirs, &file_path, &fmt);

    // Write result: to -o path if specified (jj), else to ours path (git convention: %A)
    let write_path = output_override.as_deref().unwrap_or(ours_path);
    if let Err(e) = fs::write(write_path, &result.content) {
        eprintln!("weave: failed to write result to '{}': {}", write_path, e);
        process::exit(2);
    }

    // Write audit file if requested
    if audit_enabled && !result.audit.is_empty() {
        let audit_json = serde_json::json!({
            "file": file_path,
            "confidence": result.stats.confidence(),
            "stats": result.stats,
            "entities": result.audit,
        });
        let audit_path = format!("{}.weave-audit.json", write_path);
        if let Err(e) = fs::write(
            &audit_path,
            serde_json::to_string_pretty(&audit_json).unwrap_or_default(),
        ) {
            eprintln!("weave: failed to write audit to '{}': {}", audit_path, e);
        }
    }

    // Record lifetime stats (best-effort, never fails the merge)
    let auto_resolved = result.stats.entities_ours_only
        + result.stats.entities_theirs_only
        + result.stats.entities_both_changed_merged;
    let _ = WeaveLifetimeStats::load().record_merge(&result.stats).save();
    if auto_resolved > 0 {
        eprintln!(
            "weave: {} entities auto-resolved ({} confidence)",
            auto_resolved,
            result.stats.confidence()
        );
    }

    // Print stats to stderr only when verbose or conflicted
    let verbose = std::env::var("WEAVE_VERBOSE").is_ok();
    if verbose || !result.is_clean() {
        eprintln!("weave [{}]: {}", file_path, result.stats);
    }

    // Optionally record merge in CRDT state
    #[cfg(feature = "crdt")]
    record_merge_in_crdt(&file_path, &result.content);

    if result.is_clean() {
        process::exit(0);
    } else {
        eprintln!(
            "weave: {} conflict(s) in '{}'",
            result.conflicts.len(),
            file_path
        );
        for conflict in &result.conflicts {
            eprintln!(
                "  - {} `{}`: {}",
                conflict.entity_type, conflict.entity_name, conflict.kind
            );
        }
        process::exit(1);
    }
}

fn is_binary(content: &str) -> bool {
    content.as_bytes().iter().take(8192).any(|&b| b == 0)
}

/// Record merge results in CRDT state if `.weave/state.automerge` exists.
/// Fails silently — this is purely advisory and must never break the merge.
#[cfg(feature = "crdt")]
fn record_merge_in_crdt(file_path: &str, _merged_content: &str) {
    let _ = (|| -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = weave_core::git::find_repo_root()?;
        let state_path = repo_root.join(".weave").join("state.automerge");
        if !state_path.exists() {
            return Ok(());
        }
        let mut state = weave_crdt::EntityStateDoc::open(&state_path)?;
        let registry = sem_core::parser::plugins::create_default_registry();
        weave_crdt::sync_from_files(&mut state, &repo_root, &[file_path.to_string()], &registry)?;
        state.save()?;
        Ok(())
    })();
}
