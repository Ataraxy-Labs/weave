//! # weave-driver — the product boundary
//!
//! ## Exit-code contract (three states, stable)
//!
//! | code | meaning | what git does |
//! | ---- | ------- | ------------- |
//! | `0`  | **clean** — the merge was fully resolved; the result is written | stages the file |
//! | `1`  | **conflict** — the result is written *with conflict markers*   | leaves it conflicted |
//! | `2`  | **error** — nothing usable was produced (bad usage, unreadable input, binary file) | falls back / reports failure |
//!
//! Exit 1 still writes a usable file: conflicts are *inside* the artifact.
//! Exit 2 means the artifact must not be trusted at all. Never conflate them.
//!
//! ## Warnings channel (stderr, machine-parseable)
//!
//! "Clean" is not the same as "safe". When the merge resolves cleanly but
//! carries semantic warnings, each warning is printed to **stderr** as
//!
//! ```text
//! weave-warning: {"entity":"...","entity_type":"...","file":"...","kind":"...","related":[...]}
//! ```
//!
//! One line per warning, stable `weave-warning: ` prefix, JSON payload —
//! so "clean **with warnings**" is observable by CI and by agents without
//! parsing prose. The exit code is unaffected: warnings never fail a merge.
//!
//! ## Audit trail
//!
//! `--audit`, or the environment variable `WEAVE_AUDIT=1` (git merge-driver
//! configs can set env but cannot easily add flags), writes the per-entity
//! resolution trail as JSON. It lands next to the **real** file path (`%P`)
//! when git supplies one, falling back to the output path otherwise — so the
//! audit ends up in the working tree, not beside a git temp file.
//!
//! ## Marker refusal comments
//!
//! Enhanced conflict markers carry a `refused_by:` line whose comment prefix
//! matches the file's language (`#`, `//`, `--`, `;`). The driver does not
//! arrange that: `MarkerFormat` carries the prefix and `weave-core` renders it,
//! so the bytes this program writes are the bytes `is_clean()` was decided on.
//! This file used to re-derive "this line opens a conflict" by scanning the
//! finished artifact and rewrite it afterwards.
//!
//! ## The pull channel
//!
//! A conflicted file gets ONE extra line, at the end, in the file's own comment
//! syntax:
//!
//! ```text
//! # weave: run 'weave explain <path>' for per-hunk detail, 'weave check' to verify your resolution
//! ```
//!
//! That line is the whole findings channel. Before it, weave's answer to "the
//! agent needs the diagnosis" was to *materialize* a findings document beside
//! the file and hope it got opened — a document that costs bytes on every
//! merge whether or not it changes what the agent does. So the document is
//! off by default (`WEAVE_FINDINGS=1` writes it back) and the artifact now
//! teaches the two commands that answer the questions instead. Push became
//! pull.

use std::env;
use std::fs;
use std::path::Path;
use std::process;

use weave_core::host::{git_line_merge, Host};
use weave_core::merge::is_binary;
use weave_core::stats::WeaveLifetimeStats;
use weave_core::validate::{SemanticWarning, WarningKind};
use weave_core::{entity_merge_fmt, MarkerFormat};

const HELP: &str = "\
weave-driver — entity-level semantic merge driver for git and jj

USAGE:
    weave-driver <base> <ours> <theirs> [marker-size] [file-path]
    weave-driver <base> <ours> <theirs> -o <output> [-l <marker-length>] [-p <path>]

    git invokes this as:  weave-driver %O %A %B %L %P
    jj  invokes this as:  weave-driver $base $left $right -o $out -l $len -p $path

FLAGS:
    -o, --output <path>          write the merged result here (default: <ours>)
    -l, --marker-length <n>      use standard git-compatible conflict markers
    -p, --path <path>            the real repository path of the file
        --audit                  write a per-entity resolution audit as JSON
    -h, --help                   print this help
    -V, --version                print version

ENVIRONMENT:
    WEAVE_AUDIT=1                same as --audit (for git merge-driver configs,
                                 which can set env but not add flags)
    WEAVE_FINDINGS=1             write <path>.weave-findings.json beside a
                                 CONFLICTED file. Off by default: it costs
                                 bytes on every merge whether or not it's
                                 read. The pull route is `weave explain
                                 <file>`.
    WEAVE_EVENT=1                write one JSON line per merge to stderr,
                                 prefixed `weave-event: ` — outcome, entity
                                 summary, counts, sizes and timings in one
                                 record. Off by default; see
                                 schema/weave-event.schema.json.
    WEAVE_MAX_DUPLICATES=<n>     how often one name may repeat in a file before
                                 identity stops meaning anything and the merge
                                 takes the line-level route (default 10). Read
                                 once, here, and handed to the merge.
    WEAVE_VERBOSE=1              print merge stats to stderr on every merge
    WEAVE_STATS=1                accumulate lifetime counters in
                                 ~/.weave/stats.json (read by `weave stats`).
                                 Off by default: the merge path does no
                                 filesystem work beyond the files it was
                                 given.

EXIT CODES (three states, stable contract):
    exit 0    clean     — merge fully resolved; result written
    exit 1    conflict  — result written WITH conflict markers; needs a human
    exit 2    error     — no usable result (bad usage, unreadable input, binary)

    exit 1 still writes a usable file; exit 2 means the output must not be
    trusted. Never conflate the two.

STDERR CHANNELS (machine-parseable, stable prefixes):
    weave-warning: <json>        one line per semantic warning. A merge can be
                                 CLEAN (exit 0) and still emit these — that is
                                 the \"clean with warnings\" state. Warnings
                                 never change the exit code.
    weave-event: <json>          ONE line per merge, only with WEAVE_EVENT=1.
                                 Written for every outcome, including exit 2.
";

/// What the driver produced, in the vocabulary of the exit-code contract.
///
/// Exit 0 and exit 1 are both *results* — the file was written either way —
/// and the difference between them is a property of the merge, not a failure.
/// Keeping them in the success half of a `Result` says that, and leaves
/// [`Refusal`] to mean only the third state.
enum Verdict {
    /// Fully resolved; the result is written. Exit 0.
    Clean,
    /// Written with conflict markers; needs a human. Exit 1.
    Conflicted,
}

/// Why nothing usable was produced — the exit-2 half of the contract.
///
/// All five exit 2, and they used to be five `eprintln!` + `process::exit(2)`
/// pairs scattered through `main`, so "you invoked me wrong" and "your disk is
/// full" were the same event to anything reading the driver. Naming them keeps
/// the contract in one place: `main` is the only function that maps a state to
/// a code, and each variant carries the operands its message needs.
#[derive(Debug, thiserror::Error)]
enum Refusal {
    #[error("{flag} requires a path argument")]
    FlagNeedsValue { flag: &'static str },

    #[error("expected at least <base> <ours> <theirs>, got {got}")]
    NotEnoughArguments { got: usize },

    #[error("failed to read {label} file '{path}': {source}")]
    UnreadableInput {
        label: &'static str,
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("binary file detected, skipping entity merge for '{path}'")]
    BinaryInput { path: String },

    #[error("failed to write result to '{path}': {source}")]
    UnwritableOutput {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

impl Refusal {
    /// A stable name for this refusal, for anything reading the event line.
    /// The `Display` text is for a person; this is for a query.
    fn kind(&self) -> &'static str {
        match self {
            Refusal::FlagNeedsValue { .. } => "flag_needs_value",
            Refusal::NotEnoughArguments { .. } => "not_enough_arguments",
            Refusal::UnreadableInput { .. } => "unreadable_input",
            Refusal::BinaryInput { .. } => "binary_input",
            Refusal::UnwritableOutput { .. } => "unwritable_output",
        }
    }
}

fn main() {
    env_logger::init();
    let started = std::time::Instant::now();

    match run(started) {
        Ok(Verdict::Clean) => process::exit(0),
        Ok(Verdict::Conflicted) => process::exit(1),
        Err(refusal) => {
            // The event covers the failures too, or the only merges anyone can
            // query are the ones that worked.
            if env_flag("WEAVE_EVENT") {
                emit_event(serde_json::json!({
                    "schema": EVENT_SCHEMA,
                    "schema_version": EVENT_SCHEMA_VERSION,
                    "weave_version": env!("CARGO_PKG_VERSION"),
                    "outcome": "error",
                    "exit_code": 2,
                    "refusal": refusal.kind(),
                    "message": refusal.to_string(),
                    "ms_total": elapsed_ms(started),
                }));
            }
            eprintln!("weave: {refusal}");
            if let Refusal::NotEnoughArguments { .. } = refusal {
                eprintln!("Usage: weave-driver <base> <ours> <theirs> [marker-size] [file-path]");
                eprintln!("       weave-driver <base> <ours> <theirs> -o <output> [-l <marker-length>] [-p <path>]");
                eprintln!("  Invoked by git as a merge driver, or by jj as a merge tool.");
                eprintln!("  Run `weave-driver --help` for the full exit-code contract.");
            }
            process::exit(2);
        }
    }
}

fn run(started: std::time::Instant) -> Result<Verdict, Refusal> {
    let raw_args: Vec<String> = env::args().collect();

    // Parse optional flags before positional args
    // Supported flags: -o <path> / --output <path>, --audit
    let mut output_override: Option<String> = None;
    // WEAVE_AUDIT=1 is equivalent to --audit: git merge-driver configuration
    // can export environment variables but cannot easily append flags.
    let mut audit_enabled = env_flag("WEAVE_AUDIT");
    let mut marker_length: Option<usize> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut i = 1;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--version" | "-V" => {
                println!("weave-driver {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "--help" | "-h" => {
                print!("{}", HELP);
                process::exit(0);
            }
            "-o" | "--output" => {
                if i + 1 < raw_args.len() {
                    output_override = Some(raw_args[i + 1].clone());
                    i += 2;
                } else {
                    return Err(Refusal::FlagNeedsValue {
                        flag: "-o/--output",
                    });
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
        return Err(Refusal::NotEnoughArguments {
            got: positional.len(),
        });
    }

    let base_path = &positional[0];
    let ours_path = &positional[1];
    let theirs_path = &positional[2];
    // Git calls: %O %A %B %L %P (positional[3] = marker size, positional[4] = file path)
    // jj calls: base left right -o output -l marker_length -p path (flags already parsed above)
    // Note: positional[3] from git (%L) does NOT trigger standard markers; only the -l flag does.
    let file_path = positional
        .get(4)
        .or_else(|| positional.get(3))
        .cloned()
        .unwrap_or_else(|| ours_path.clone());

    // Read input files
    let read_at = std::time::Instant::now();
    let base = read_input(base_path, "base")?;
    let ours = read_input(ours_path, "ours")?;
    let theirs = read_input(theirs_path, "theirs")?;
    let ms_read = elapsed_ms(read_at);

    // Detect binary content (null bytes in first 8KB)
    if is_binary(&base) || is_binary(&ours) || is_binary(&theirs) {
        return Err(Refusal::BinaryInput {
            path: file_path.clone(),
        });
    }

    // Run entity merge
    // When -l is passed (by jj or other tools), use standard git-compatible markers.
    // Otherwise (git merge driver), use enhanced markers with entity metadata.
    let fmt = marker_length
        .map(MarkerFormat::standard)
        .unwrap_or_default()
        .for_file(&file_path);
    // The one place this program says what a merge may touch. `git merge-file`
    // is a real second opinion the driver has always paid for, and
    // WEAVE_MAX_DUPLICATES is read here — at the boundary that owns the
    // environment — rather than in the middle of the merge that uses it.
    let host = Host {
        max_duplicates: env_usize("WEAVE_MAX_DUPLICATES").unwrap_or(Host::default().max_duplicates),
        line_merge: Some(git_line_merge),
    };
    let merged_at = std::time::Instant::now();
    let result = entity_merge_fmt(&base, &ours, &theirs, &file_path, &fmt, &host);
    let ms_merge = elapsed_ms(merged_at);

    // The pull channel's one line. A conflicted artifact is the only place an
    // agent is guaranteed to look, so it is the only place worth teaching from;
    // a clean merge is not told anything, which is why `is_clean()` gates this
    // and why the byte-for-byte contract on clean output is untouched.
    let content = if result.is_clean() {
        result.content.clone()
    } else {
        teach(&result.content, &file_path, &fmt.comment_prefix)
    };
    let content = &content;
    // Write result: to -o path if specified (jj), else to ours path (git convention: %A)
    let write_path = output_override.as_deref().unwrap_or(ours_path);
    if let Err(source) = fs::write(write_path, content) {
        return Err(Refusal::UnwritableOutput {
            path: write_path.to_string(),
            source,
        });
    }

    // Warnings channel: a merge can be CLEAN and still be risky. Emit one
    // machine-parseable line per warning to stderr, with a stable prefix, so
    // "clean with warnings" is an observable product state. Never affects the
    // exit code — warnings are always advisory.
    for w in &result.warnings {
        println_warning(w);
    }

    // Write audit file if requested (--audit or WEAVE_AUDIT=1)
    if audit_enabled && !result.audit.is_empty() {
        let audit_json = serde_json::json!({
            "file": file_path,
            "confidence": result.stats.confidence(),
            "stats": result.stats,
            "entities": result.audit,
        });
        // Land the audit next to the REAL file (git's %P) when there is one:
        // git passes %A as a temp path, so the default would bury the audit in
        // a directory the user never looks at and git deletes.
        let audit_path = audit_output_path(&file_path, write_path);
        if let Err(e) = fs::write(
            &audit_path,
            serde_json::to_string_pretty(&audit_json).unwrap_or_default(),
        ) {
            eprintln!("weave: failed to write audit to '{}': {}", audit_path, e);
        }
    }

    // The findings document: OPT-IN, because pushing it did not work. Making
    // it shorter and more precise did not change that: a channel that adds a
    // step without removing one is overhead however good its contents are,
    // so the default is now silence and the artifact carries the address of
    // the answer instead.
    //
    // `WEAVE_FINDINGS=1` restores it, unchanged, for anyone whose pipeline
    // reads it — the same shape `weave explain --json` prints, so there is one
    // producer and not two.
    if env_flag("WEAVE_FINDINGS") && !result.is_clean() {
        let doc = weave_core::explain::explain(&base, &ours, &theirs, &file_path, &host);
        let path = format!(
            "{}.weave-findings.json",
            audit_base_path(&file_path, write_path)
        );
        if let Err(e) = fs::write(
            &path,
            serde_json::to_string_pretty(&doc).unwrap_or_default(),
        ) {
            eprintln!("weave: failed to write findings to '{}': {}", path, e);
        }
    }

    // Record lifetime stats — opt-in, because nobody asked for it.
    //
    // This used to run on every merge: read `~/.weave/stats.json`, parse it,
    // re-serialise it, write it back, `create_dir_all` included. Measured at
    // 0.05ms per merge — about 1% of a small-file run, so speed is the weaker
    // half of the argument and it would be dishonest to lead with it. The
    // stronger half: it is a *write* on a path whose whole contract is "read
    // three files, produce one", and it is a read-modify-write race — the N
    // merge drivers git forks for one `git merge` all clobber the same file,
    // so the counter was already wrong.
    //
    // `WEAVE_STATS=1` turns it back on for anyone who wants the counter. The
    // default path now touches no file except the merge's own inputs and
    // output.
    let auto_resolved = result.stats.auto_resolved();
    if env_flag("WEAVE_STATS") {
        // The path is this program's to decide: weave-core is handed one.
        if let Some(path) = weave_core::stats::default_path(home_dir().as_deref()) {
            WeaveLifetimeStats::load(&path)
                .record_merge(&result.stats)
                .save(&path);
        }
    }
    if auto_resolved > 0 {
        eprintln!(
            "weave: {} entities auto-resolved ({} confidence)",
            auto_resolved,
            result.stats.confidence()
        );
    }

    // Print stats to stderr only when verbose or conflicted
    let verbose = env::var("WEAVE_VERBOSE").is_ok();
    if verbose || !result.is_clean() {
        eprintln!("weave [{}]: {}", file_path, result.stats);
    }

    // Optionally record merge in CRDT state
    #[cfg(feature = "crdt")]
    record_merge_in_crdt(&file_path, &result.content);

    // One line per merge, off by default. Everything on it was already
    // computed — the summary the audit writes, the counts stderr reports, the
    // verdict the exit code carries — so the cost of WEAVE_EVENT=0 is one
    // environment read, and the cost of WEAVE_EVENT=1 is one line. It is a
    // single record per merge rather than a stream of steps, because the
    // question a reader has ("which merges conflicted, on what, how long did
    // they take, and which build was it") is one query over one row.
    if env_flag("WEAVE_EVENT") {
        emit_event(serde_json::json!({
            "schema": EVENT_SCHEMA,
            "schema_version": EVENT_SCHEMA_VERSION,
            "weave_version": env!("CARGO_PKG_VERSION"),
            "file": file_path,
            "outcome": if result.is_clean() { "clean" } else { "conflict" },
            "exit_code": if result.is_clean() { 0 } else { 1 },
            "confidence": result.stats.confidence(),
            "entities": result.stats,
            "conflicts": result.conflicts.len(),
            "warnings": result.warnings.len(),
            "findings": result.conflicts.len() + result.warnings.len(),
            "audit_rows": result.audit.len(),
            "bytes_base": base.len(),
            "bytes_ours": ours.len(),
            "bytes_theirs": theirs.len(),
            "bytes_out": content.len(),
            "markers": if marker_length.is_some() { "standard" } else { "enhanced" },
            "line_merge_granted": host.line_merge.is_some(),
            "max_duplicates": host.max_duplicates,
            "ms_read": ms_read,
            "ms_merge": ms_merge,
            "ms_total": elapsed_ms(started),
        }));
    }

    if result.is_clean() {
        Ok(Verdict::Clean)
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
        Ok(Verdict::Conflicted)
    }
}

/// The pull channel: one comment line at the end of a conflicted artifact,
/// naming the two commands that answer the two questions a reader has.
///
/// **One line, and only on conflicted output.** A per-marker `hint:` comment
/// used to sit inside every box and got stripped as litter — a second
/// annotation per box is a tax on every box. This is one line per *file*, it
/// sits at the
/// end where it cannot interleave with either side's claim, and it survives
/// exactly as long as the markers do: whoever removes the conflict removes it.
///
/// It states commands, not conclusions. That is the whole shape of the change —
/// weave stops pushing a document nobody asked for and tells the reader how to
/// ask.
fn teach(content: &str, file_path: &str, comment_prefix: &str) -> String {
    let mut out = String::with_capacity(content.len() + 128);
    out.push_str(content);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&weave_core::conflict::teach_line(comment_prefix, file_path));
    out.push('\n');
    out
}

/// Read a UTF-8 file to completion, or say which of the three it was.
///
/// The three merge inputs (base/ours/theirs) share this failure mode
/// exactly: an unreadable input is not a conflict, it is an error that must
/// not produce a trusted result (see the exit-code contract above). The label
/// travels on the error rather than into a message, so the one function that
/// owns the exit codes is also the only one that writes to stderr.
fn read_input(path: &str, label: &'static str) -> Result<String, Refusal> {
    fs::read_to_string(path).map_err(|source| Refusal::UnreadableInput {
        label,
        path: path.to_string(),
        source,
    })
}

/// The `schema` and `schema_version` every event line carries.
///
/// A consumer keys on the pair: the name says which document this is, the
/// version says which fields it may rely on. Additive fields are a minor bump;
/// removing one or changing what it means is a major.
const EVENT_SCHEMA: &str = "weave-event";
const EVENT_SCHEMA_VERSION: &str = "1.0.0";

/// Write one event line on the stable stderr channel.
fn emit_event(payload: serde_json::Value) {
    eprintln!("weave-event: {payload}");
}

/// Milliseconds since `at`, to three decimals — enough to tell a 0.4ms merge
/// from a 4ms one, which is the range that matters here.
fn elapsed_ms(at: std::time::Instant) -> f64 {
    (at.elapsed().as_secs_f64() * 1000.0 * 1000.0).round() / 1000.0
}

/// The user's home directory, by the two names the platforms use for it.
fn home_dir() -> Option<String> {
    env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok()
}

/// A whole-number environment setting, or `None` when unset or unreadable.
fn env_usize(name: &str) -> Option<usize> {
    env::var(name).ok()?.trim().parse().ok()
}

/// An environment flag is set when it exists and is not "0"/"false"/"".
fn env_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|v| {
        let v = v.trim().to_ascii_lowercase();
        !(v.is_empty() || v == "0" || v == "false" || v == "no")
    })
}

/// Emit one semantic warning on the stable, machine-parseable stderr channel.
///
/// `SemanticWarning` is not `Serialize` in weave-core, so the JSON shape is
/// pinned here — at the boundary that publishes it. Keys are stable API:
/// `entity`, `entity_type`, `file`, `kind`, `related`.
fn println_warning(w: &SemanticWarning) {
    let kind = match w.kind {
        WarningKind::DependencyAlsoModified => "dependency_also_modified",
        WarningKind::DependentAlsoModified => "dependent_also_modified",
        WarningKind::ParseFailedAfterMerge => "parse_failed_after_merge",
        WarningKind::CompositionLicensed { .. } => "composition_licensed",
        WarningKind::ConflictFrameDuplicate { .. } => "conflict_frame_duplicate",
    };
    // Emit both read and write sets on the warning line, so a reader can inspect
    // the evidence a resolution rests on rather than only being told it was
    // computed.
    let evidence = match &w.kind {
        WarningKind::CompositionLicensed {
            ours_binds,
            theirs_binds,
        } => Some(serde_json::json!({
            "ours_binds": ours_binds,
            "theirs_binds": theirs_binds,
        })),
        // The frame check's evidence is the count itself: how often the line is
        // stated outside every box, and how often any union of the two sides
        // could have stated it. A reader who wants to check the claim needs both
        // numbers, not the assurance that one was computed.
        WarningKind::ConflictFrameDuplicate {
            line,
            found,
            allowed,
        } => Some(serde_json::json!({
            "line": line,
            "found": found,
            "allowed": allowed,
        })),
        _ => None,
    };
    let related: Vec<serde_json::Value> = w
        .related
        .iter()
        .map(|r| {
            serde_json::json!({
                "entity": r.name,
                "entity_type": r.entity_type,
                "file": r.file_path,
            })
        })
        .collect();
    let mut payload = serde_json::json!({
        "entity": w.entity_name,
        "entity_type": w.entity_type,
        "file": w.file_path,
        "kind": kind,
        "message": w.to_string(),
        "related": related,
    });
    if let (Some(map), Some(e)) = (payload.as_object_mut(), evidence) {
        map.insert("evidence".to_string(), e);
    }
    eprintln!("weave-warning: {}", payload);
}

/// Where the audit JSON should land.
///
/// Prefer the real repository path (git's `%P`) — that is the file the user
/// is actually merging. Fall back to the output path when `%P` is absent or
/// points into a directory that does not exist (so we never fail a merge over
/// an audit file).
fn audit_output_path(file_path: &str, write_path: &str) -> String {
    format!(
        "{}.weave-audit.json",
        audit_base_path(file_path, write_path)
    )
}

/// The path a sidecar document should hang off: the real repository path when
/// git gave one and its directory exists, else the output path.
///
/// Two sidecars now share this (`--audit` and `WEAVE_FINDINGS=1`), and they
/// must land in the same place for the same reason — git passes `%A` as a temp
/// path it will delete, so the default would bury the document in a directory
/// the user never looks at.
fn audit_base_path(file_path: &str, write_path: &str) -> String {
    let real = Path::new(file_path);
    let parent_exists = match real.parent() {
        // Bare filename: relative to the cwd, which git sets to the repo root.
        Some(p) if p.as_os_str().is_empty() => true,
        Some(p) => p.is_dir(),
        None => false,
    };
    if !file_path.is_empty() && parent_exists {
        file_path.to_string()
    } else {
        write_path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_flag_reads_the_usual_falsy_spellings() {
        assert!(!env_flag("WEAVE_DEFINITELY_UNSET_XYZ"));
        env::set_var("WEAVE_TEST_FLAG", "1");
        assert!(env_flag("WEAVE_TEST_FLAG"));
        env::set_var("WEAVE_TEST_FLAG", "0");
        assert!(!env_flag("WEAVE_TEST_FLAG"));
        env::set_var("WEAVE_TEST_FLAG", "false");
        assert!(!env_flag("WEAVE_TEST_FLAG"));
        env::remove_var("WEAVE_TEST_FLAG");
    }

    #[test]
    fn audit_lands_next_to_the_real_path_when_it_is_real() {
        let dir = env::temp_dir();
        let real = dir.join("weave_audit_probe.py");
        let real = real.to_string_lossy().to_string();
        assert_eq!(
            audit_output_path(&real, "/tmp/git-temp-ABC"),
            format!("{real}.weave-audit.json")
        );
        // Non-existent directory → fall back to the output path.
        assert_eq!(
            audit_output_path("no/such/dir/app.py", "/tmp/git-temp-ABC"),
            "/tmp/git-temp-ABC.weave-audit.json"
        );
    }
}

/// Record merge results in CRDT state if `.weave/state.automerge` exists.
/// Fails silently — this is purely advisory and must never break the merge.
#[cfg(feature = "crdt")]
fn record_merge_in_crdt(file_path: &str, _merged_content: &str) {
    let _ = (|| -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = weave_core::git::find_repo_root(std::path::Path::new("."))?;
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
