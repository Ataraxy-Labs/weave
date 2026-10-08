//! Verifying the WORKING TREE — the check an agent actually wants.
//!
//! `weave check` used to compare `HEAD` against `MERGE_HEAD`: two *commits*,
//! not the working-tree resolution someone had just written. That misses the
//! dominant real defect in these merges, which is **not** a mis-resolved
//! marker — it is silent breakage in the *unconflicted* region that no marker
//! points at, findable only by reading the whole file by hand. That reading
//! is the cost this module exists to remove.
//!
//! So the subject is the file on disk, and the questions are the four an agent
//! asks after editing:
//!
//! 1. **markers** — did I finish? (including weave's own teach line)
//! 2. **loss** — did I drop a line BOTH sides kept? Unanimity is the one thing
//!    a resolution may never overrule: neither developer asked for it to go.
//! 3. **duplicates** — did the merge state something more times than either
//!    side did? Same ruler as [`weave_core::frame`], applied to the whole file
//!    instead of the frame.
//! 4. **dangling** — is anything still called that nothing defines any more,
//!    or used under a name this file no longer imports? Binding evidence,
//!    repo-wide, the same functions the per-file pass uses.
//!
//! Every verdict is stated as a **sentence**, never as an empty array. A
//! reader who gets back a bare `[]` reasonably takes it for approval; a
//! channel whose "nothing found" and "nothing looked at" render identically
//! has a failure mode where silence is indistinguishable from a clean bill of
//! health.
//!
//! ## Why the bounds below are the ones they are
//!
//! A verifier that replaces a manual pass is only worth its calls if its
//! findings are evidence. Every rule here is therefore stated as a *necessary*
//! condition on a correct resolution — one the developer's own committed
//! resolution satisfies — and the three ways that can be got wrong are all
//! ways this file once got them wrong:
//!
//! * **The unanimity floor is base-relative and additive.** `min(ours, theirs)`
//!   is what a merge must keep only if the two sides always delete the *same*
//!   copies of a repeated line, which on real code they never do. The bound is
//!   `min(n, ours) + min(n, theirs) − n` over base's `n` copies, floored at
//!   zero — the predicate [`weave_core::container`] enforces inside the merge.
//!   Two owners of one rule is one owner too many, so this states it the same
//!   way rather than a second way.
//! * **The duplication ceiling is additive too.** Two sides that each add a
//!   copy of the same line license a resolution that has one *more* copy than
//!   either side does; `max(base, ours, theirs)` calls every such union a
//!   duplicate. The ceiling is the one [`weave_core::frame::frame_duplicates`]
//!   already uses.
//! * **Multiplicity is only evidence for a line that has an identity.**
//!   `frame` says this for structural punctuation. It is equally true of a
//!   continuation fragment — `anyInt(),`, `await fire_event(` — which is not a
//!   statement but a slice of one, and repeats wherever the same call shape is
//!   written. A fragment counts only when it is part of a *run* of over-budget
//!   lines, which is what a doubled edit actually looks like.
//!
//! and one that is about names rather than lines:
//!
//! * **A declaration and its impl blocks are ONE nameable thing.** Rust's
//!   `struct X` plus `impl X` is not `X` defined twice; neither is a namespace
//!   reopened, an interface merged, two `test('…')` calls sharing a title, or
//!   two overloads sharing a name. Identity is (kind, name, signature) with the
//!   kinds that *attach* to a declaration excluded — not the bare name.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use sem_core::model::change::ChangeType;
use sem_core::model::entity::SemanticEntity;
use sem_core::model::identity::match_entities;

use crate::parsers::{defined_names_of, entities_of, is_code, is_supported, oversize};
use crate::repo_scope::Tree;
use weave_core::binding::{
    has_binding, has_call_reference, has_declaration, has_value_reference, identifiers,
    import_bindings, may_mention,
};
// The line and declaration rulers are the merge's own ([`weave_core::verify`]):
// the driver refuses to call a merge clean on exactly the evidence this check
// reports, so the two cannot drift apart. The one difference — a person may
// write a line no side wrote — is `duplication_ceiling`'s parameter.
use weave_core::verify::{
    carries_identity, definition_key, duplication_ceiling, line_counts as counts, significant,
    unanimity_floor, DefinitionKey,
};

/// One finding about one file, already in the words the reader gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// `MARKERS` | `LOSS` | `DUP` | `PARSE` | `DANGLING` | `MODDEL` |
    /// `MISSING` | `UNREAD`.
    pub class: &'static str,
    pub detail: String,
    /// A repair that follows mechanically from the finding, when one does.
    /// `None` is honest silence, not an empty string.
    pub suggestion: Option<String>,
}

/// One advisory about one file — a fact weave surfaces without deciding it. It
/// is NOT a finding: it never makes a verdict `FOUND`, never counts in the
/// tally, and never changes the exit code. It rides beside the verdict as
/// something a reviewer may want to glance at, not something they must fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advisory {
    /// `COOCCUPANCY` (siblings both sides changed, merged clean) | `MODDEL`
    /// (a modify/delete resolved in a way only the authors' intent can judge).
    pub class: &'static str,
    /// The container the co-change happened in; for `MODDEL`, the file.
    pub entity: String,
    pub entity_type: String,
    /// The one-line human reading, e.g. `both sides changed siblings (ours
    /// added `bar`; theirs changed `baz`)`.
    pub detail: String,
}

/// One file's verdict. An empty `findings` is the OK verdict and prints as a
/// sentence. `advisories` are separate: a file may be OK and still carry them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub file: String,
    pub findings: Vec<Finding>,
    pub advisories: Vec<Advisory>,
    /// Why an OK file was not read line by line — it is not in the working
    /// tree as the merge asked, or it is not source. `None` for a file that
    /// was checked.
    pub note: Option<String>,
}

impl Verdict {
    /// An OK verdict for a file there was nothing to verify in, saying why.
    /// Every file the merge touched gets a line; silence about one would read
    /// the same as approval of it.
    pub fn noted(file: &str, note: &str) -> Verdict {
        Verdict {
            file: file.to_string(),
            findings: Vec::new(),
            advisories: Vec::new(),
            note: Some(note.to_string()),
        }
    }

    /// The verdict for a file whose merge stage could not be read: NOT
    /// checked, and said so. Leaving it out of the report would make it
    /// indistinguishable from a file nobody asked about.
    pub fn unread(file: &str, why: &str) -> Verdict {
        Verdict {
            file: file.to_string(),
            findings: vec![Finding {
                class: "UNREAD",
                detail: format!("NOT CHECKED — a merge stage could not be read ({why})"),
                suggestion: Some(
                    "fetch the missing objects (in a partial clone: `git fetch` from the \
                     promisor remote) and run `weave check` again"
                        .to_string(),
                ),
            }],
            advisories: Vec::new(),
            note: None,
        }
    }

    /// OK is about FINDINGS only. Advisories never make a file not-OK — that is
    /// the whole point of the register.
    pub fn ok(&self) -> bool {
        self.findings.is_empty()
    }

    /// The one line this file gets.
    pub fn line(&self) -> String {
        if let (true, Some(note)) = (self.ok(), &self.note) {
            return format!("OK: {} — {note}", self.file);
        }
        if self.ok() {
            return format!(
                "OK: {} — markers cleared, no unanimous-line loss, no duplicated \
                 definitions or lines, no dangling references",
                self.file
            );
        }
        let details: Vec<&str> = self.findings.iter().map(|f| f.detail.as_str()).collect();
        format!("FOUND: {} — {}", self.file, details.join("; "))
    }
}

/// Everything one run of the working-tree check looked at and concluded.
///
/// The *scope* travels with the verdicts on purpose: "no findings" and "no
/// files" are different answers and a reader must never have to infer which
/// one they got.
#[derive(Debug, Clone)]
pub struct Report {
    /// What was compared, in words — `HEAD × MERGE_HEAD`, or why nothing was.
    pub scope: String,
    pub verdicts: Vec<Verdict>,
}

/// What `weave check` says when there is no merge to verify against.
pub const NOTHING_TO_CHECK: &str = "no merge, rebase, cherry-pick or revert in progress and HEAD \
     is not a merge commit, so there is no three-way context to verify a resolution against. \
     NOTHING WAS CHECKED — this is not a clean bill of health.";

/// Verify the resolution on disk in `dir` against the merge in progress (or
/// HEAD's merge commit). `None`: no three-way context to verify against.
pub fn check_in_progress(
    dir: &std::path::Path,
) -> Result<Option<Report>, Box<dyn std::error::Error>> {
    let Some(scope) = crate::gitscan::merge_scope(dir)? else {
        return Ok(None);
    };
    let mut verdicts = check(
        &scope.base,
        &scope.ours,
        &scope.theirs,
        &scope.work,
        &scope.subjects,
    );
    verdicts.extend(
        scope
            .unreadable
            .iter()
            .map(|(file, why)| Verdict::unread(file, why)),
    );
    verdicts.extend(scope.irregular.iter().map(|file| {
        Verdict::noted(
            file,
            "a symlink or submodule in a merge stage — not source, so git's guarantees stand",
        )
    }));
    verdicts.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(Some(Report {
        scope: match oversize_note(&scope.work) {
            Some(note) => format!("{}; {note}", scope.scope),
            None => scope.scope,
        },
        verdicts,
    }))
}

impl Report {
    /// The report as `weave check --json` prints it.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "scope": self.scope,
            "files": self.verdicts.iter().map(|v| serde_json::json!({
                "file": v.file,
                "ok": v.ok(),
                "verdict": v.line(),
                "findings": v.findings.iter().map(|f| serde_json::json!({
                    "class": f.class,
                    "detail": f.detail,
                    "suggestion": f.suggestion,
                })).collect::<Vec<_>>(),
                // Advisories are non-blocking: they ride beside the verdict and
                // never move `ok` or the exit code.
                "advisories": v.advisories.iter().map(|a| serde_json::json!({
                    "class": a.class,
                    "entity": a.entity,
                    "entity_type": a.entity_type,
                    "detail": a.detail,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }

    /// (files verified clean, files with findings, findings). One pass, one
    /// answer: the summary sentence and the exit code are two readings of the
    /// same tally, and computing them separately is how they drift.
    pub fn tally(&self) -> (usize, usize, usize) {
        let mut clean = 0;
        let mut dirty = 0;
        let mut findings = 0;
        for v in &self.verdicts {
            if v.ok() {
                clean += 1;
            } else {
                dirty += 1;
            }
            findings += v.findings.len();
        }
        (clean, dirty, findings)
    }

    /// The whole report as the text a reader gets. Never empty, ever.
    pub fn render(&self) -> String {
        let mut out = format!("weave check: {}\n", self.scope);
        if self.verdicts.is_empty() {
            out.push_str(
                "weave check: NOTHING WAS VERIFIED. This is not a clean bill of health — \
                 weave had no file to look at.\n",
            );
            return out;
        }
        for v in &self.verdicts {
            out.push_str(&v.line());
            out.push('\n');
            for f in &v.findings {
                if let Some(s) = &f.suggestion {
                    out.push_str(&format!("    suggestion ({}): {}\n", f.class, s));
                }
            }
            // Advisories print UNDER the verdict, tagged `review`, whether or not
            // the file is OK. They are not problems — they carry no exit code and
            // no "FOUND" — so they read as a glance, not a task.
            for a in &v.advisories {
                out.push_str(&match a.class {
                    "COOCCUPANCY" => format!(
                        "    review (COOCCUPANCY): cleanly merged {} `{}` — {}; confirm they are meant to coexist\n",
                        a.entity_type, a.entity, a.detail
                    ),
                    class => format!("    review ({class}): {}\n", a.detail),
                });
            }
        }
        let (good, bad, findings) = self.tally();
        if bad == 0 {
            out.push_str(&format!(
                "weave check: {good} file(s) verified against the three merge stages — \
                 every line both sides kept is present, nothing is stated more often than \
                 either side stated it, every reference still resolves. Re-reading these \
                 files end to end will not find anything this did not.\n"
            ));
        } else {
            out.push_str(&format!(
                "weave check: {good} file(s) clean, {bad} file(s) with findings \
                 ({findings} total). Fix the files above; the clean ones need no further \
                 reading.\n"
            ));
        }
        out
    }
}

/// The four checks, over one file.
///
/// `work` is the bytes on disk. `base` / `ours` / `theirs` are the three merge
/// stages — the only reference frame in which "loss" and "duplicate" mean
/// anything, because both are statements about what the two developers agreed.
fn verify_file(
    file: &str,
    base: Option<&str>,
    ours: Option<&str>,
    theirs: Option<&str>,
    work: &str,
) -> Vec<Finding> {
    let mut out = Vec::new();

    // ---- 1. Markers -------------------------------------------------------
    let marker_lines = weave_core::frame::marker_line_count(work);
    if marker_lines > 0 {
        out.push(Finding {
            class: "MARKERS",
            detail: format!("{marker_lines} conflict marker line(s) still present"),
            suggestion: Some(
                "the file is not resolved — every opening, separator and closing marker \
                 line must go, along with the refused_by: comment inside each box"
                    .to_string(),
            ),
        });
    }
    if work.lines().any(weave_core::conflict::is_teach_line) {
        out.push(Finding {
            class: "MARKERS",
            detail: "weave's teach line is still in the file".to_string(),
            suggestion: Some(
                "delete the trailing `weave: run 'weave explain …'` comment; it is marker \
                 furniture, not source"
                    .to_string(),
            ),
        });
    }

    // ---- 2. Unanimous loss ------------------------------------------------
    //
    // A line both sides kept is a line neither developer asked to remove. That
    // is the one class where "the resolver chose differently" is not a defence.
    //
    // The bound is base-relative and additive, because deletions from the two
    // sides are: of base's `n` copies, ours kept `min(n, ours)` and theirs kept
    // `min(n, theirs)`, and nothing says those are the same copies, so the
    // survivors are the sum minus `n`. `min(ours, theirs)` — what this used to
    // ask — is a condition the *developer's own resolution* fails wherever the
    // two sides deleted different copies of a repeated line.
    if let (Some(b), Some(o), Some(t)) = (base, ours, theirs) {
        let (cb, co, ct, cw) = (counts(b), counts(o), counts(t), counts(work));
        let mut lost: Vec<(&str, usize)> = cb
            .iter()
            .filter_map(|(line, n)| {
                let required = unanimity_floor(
                    *n,
                    co.get(line).copied().unwrap_or(0),
                    ct.get(line).copied().unwrap_or(0),
                );
                let found = cw.get(line).copied().unwrap_or(0);
                // `then`, not `then_some`: the argument of `then_some` is
                // evaluated whether or not the condition holds, and
                // `required - found` underflows on every line that is NOT lost
                // — which is almost all of them.
                (found < required).then(|| (*line, required - found))
            })
            .collect();
        lost.sort();
        if !lost.is_empty() {
            let total: usize = lost.iter().map(|(_, n)| n).sum();
            out.push(Finding {
                class: "LOSS",
                detail: format!(
                    "{total} line(s) BOTH sides kept are missing, e.g. `{}`",
                    clip(lost[0].0)
                ),
                suggestion: Some(format!(
                    "restore the {} distinct line(s) neither side deleted; nothing in this \
                     merge licenses dropping them",
                    lost.len()
                )),
            });
        }
    }

    // ---- 3. Duplication ---------------------------------------------------
    //
    // Same ruler as the frame check: a line may appear as often as the side
    // that says it most says it, and no oftener. This is checked over the
    // whole file and not only over the frame because git folds one side's
    // rename into "clean" context and the declaration lands twice, with no
    // marker anywhere near it.
    {
        let cw = counts(work);
        let (cb, co, ct) = (
            counts(base.unwrap_or("")),
            counts(ours.unwrap_or("")),
            counts(theirs.unwrap_or("")),
        );
        // The ceiling `frame_duplicates` already uses. Additions from the two
        // sides are additive exactly as deletions are, so a resolution that
        // keeps both sides' new copy of a line is inside its budget — and at
        // least one copy is always allowed, because a line the resolver wrote
        // itself is allowed to exist. Twice is the question.
        let allowance = |line: &str| {
            duplication_ceiling(
                cb.get(line).copied().unwrap_or(0),
                co.get(line).copied().unwrap_or(0),
                ct.get(line).copied().unwrap_or(0),
                true,
            )
        };
        let over: BTreeSet<&str> = cw
            .iter()
            .filter(|(line, found)| **found > allowance(line))
            .map(|(line, _)| *line)
            .collect();
        // A fragment is only evidence in company: a doubled edit lands as a
        // RUN of over-budget lines, whereas a call argument that repeats on its
        // own is how the same call shape gets written twice. Adjacency is asked
        // of the working tree, because that is where the run would be.
        let mut in_a_run: BTreeSet<&str> = BTreeSet::new();
        {
            let lines: Vec<&str> = work.lines().map(str::trim).collect();
            for pair in lines.windows(2) {
                if over.contains(pair[0]) && over.contains(pair[1]) {
                    in_a_run.insert(pair[0]);
                    in_a_run.insert(pair[1]);
                }
            }
        }
        let mut dup: Vec<(&str, usize, usize)> = over
            .iter()
            .filter(|line| carries_identity(line) || in_a_run.contains(*line))
            .map(|line| (*line, cw[line], allowance(line)))
            .collect();
        dup.sort();
        if !dup.is_empty() {
            out.push(Finding {
                class: "DUP",
                detail: format!(
                    "{} line(s) appear more often than any version states them, e.g. `{}` \
                     ({}x, at most {}x in base/ours/theirs)",
                    dup.len(),
                    clip(dup[0].0),
                    dup[0].1,
                    dup[0].2
                ),
                suggestion: Some(
                    "delete the extra copies — a line stated twice usually means one side's \
                     edit landed both inside and outside a marker"
                        .to_string(),
                ),
            });
        }
    }

    // A data file must still load, and state no key at one table path more
    // often than either side does — the merge's own rule
    // ([`weave_core::verify::structured_data`]).
    if let Some(u) = weave_core::verify::structured_data(ours, theirs, work, file) {
        let loads = weave_core::datafile::keys(file, work).is_some_and(|r| r.is_ok());
        out.push(Finding {
            class: if loads { "DUP" } else { "PARSE" },
            detail: u.detail,
            suggestion: Some(if loads {
                "keep one entry for the key, with the settings both sides need".to_string()
            } else {
                "make the file load again; a side's version of it does".to_string()
            }),
        });
    }

    // A key stated twice in one switch / match / literal where neither side
    // states it twice: two branches for one `case`, two values for one map
    // key — the merge's own rule ([`weave_core::verify::duplicate_keys`]).
    if let (Some(o), Some(t)) = (ours, theirs) {
        if let Some(u) =
            weave_core::verify::duplicate_keys(file, o, t, work, &crate::parsers::REGISTRY)
        {
            out.push(Finding {
                class: "DUP",
                detail: u.detail,
                suggestion: Some(
                    "keep one element for the key, combining what both sides need; two \
                     elements for one key do not compile in some languages and silently \
                     drop one side's in others"
                        .to_string(),
                ),
            });
        }
    }

    // Duplicate DEFINITIONS are worth their own finding: two `def f` in one
    // file is a language-level bug, not a stylistic repeat, and the second one
    // silently wins. Which is why the *identity* has to be the language's and
    // not the name string's — see [`definition_key`].
    let mut seen: BTreeMap<DefinitionKey, (String, usize)> = BTreeMap::new();
    for e in entities_of(file, work) {
        let Some(key) = definition_key(file, &e) else {
            continue;
        };
        let slot = seen.entry(key).or_insert_with(|| (e.name.clone(), 0));
        slot.1 += 1;
    }
    let dup_defs: Vec<&(String, usize)> = seen.values().filter(|(_, n)| *n > 1).collect();
    if !dup_defs.is_empty() {
        out.push(Finding {
            class: "DUP",
            detail: format!(
                "{} name(s) are defined more than once: {}",
                dup_defs.len(),
                dup_defs
                    .iter()
                    .take(4)
                    .map(|(n, c)| format!("`{n}` ({c}x)"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            suggestion: Some(
                "keep one definition of each name; the later one shadows the earlier and \
                 the earlier one's edits are silently dead"
                    .to_string(),
            ),
        });
    }

    out
}
fn clip(l: &str) -> String {
    let t = l.trim();
    if t.chars().count() > 72 {
        t.chars().take(71).chain(['…']).collect()
    } else {
        t.to_string()
    }
}

/// Verify a whole working tree against the three merge stages.
///
/// `work` holds the bytes on disk for every supported file — the dangling pass
/// needs the *repo*, not the file, because a definition deleted in `a.py` with
/// its caller in `b.py` is invisible to any per-file rule.
///
/// Every repo-wide question is asked ONCE and indexed, never per subject: each
/// file is parsed at most once per version, and a vanished name is looked up in
/// a file's identifier set before any line of it is scanned. A merge with two
/// thousand changed files is otherwise "every name × every file × every line",
/// which is a check that does not finish.
pub fn check(
    base: &Tree,
    ours: &Tree,
    theirs: &Tree,
    work: &Tree,
    subjects: &[String],
) -> Vec<Verdict> {
    // Names still defined anywhere on disk. Repo-wide on purpose: a name a
    // subject still calls is bound — not dangling — the moment ANY file, touched
    // or not, still defines it, so the suppression set cannot be scoped to the
    // subjects the way the stage trees are. A file too large to parse is asked
    // lexically instead, below. In code, every name a file defines counts, at
    // any depth: a function that moved into a class (or that the parser sees
    // at a different depth than in base) is still defined, and so is a member
    // defined out of its class under a qualified name (`Sheet::add_row`). A
    // data file's nested keys are not definitions a call can reach, so only
    // its top-level names count, as before.
    let defined_now: BTreeSet<String> = work
        .iter()
        .filter(|(p, _)| is_supported(p))
        .flat_map(|(p, c)| -> Vec<String> {
            if !is_code(p) {
                return entities_of(p, c).into_iter().map(|e| e.name).collect();
            }
            defined_names_of(p, c)
                .into_iter()
                .flat_map(|name| {
                    let member = name
                        .rsplit_once("::")
                        .map(|(_, member)| member.to_string())
                        .filter(|m| !m.is_empty());
                    std::iter::once(name).chain(member)
                })
                .collect()
        })
        .collect();

    // The names a merge stage defined that nothing on disk defines any more.
    // This depends on the stages and the working tree, NOT on which subject we
    // are looking at, so it is computed ONCE. The stage trees are already
    // scoped to the subjects (see [`crate::gitscan::merge_scope`]); an
    // untouched file's stage equals its working-tree copy, so it contributes
    // only names that are in `defined_now` and can never be `gone`.
    // Code files only: a Markdown heading or a YAML key is an entity weave
    // merges, not a name a program calls. Base's parse is kept: it is where a
    // vanished name's successor is looked for.
    let mut gone: BTreeSet<String> = BTreeSet::new();
    let mut base_entities: BTreeMap<&str, Vec<SemanticEntity>> = BTreeMap::new();
    for (i, stage) in [base, ours, theirs].into_iter().enumerate() {
        for (p, c) in stage.iter().filter(|(p, _)| is_code(p)) {
            let entities = entities_of(p, c);
            for e in &entities {
                if !defined_now.contains(&e.name) {
                    gone.insert(e.name.clone());
                }
            }
            if i == 0 && !entities.is_empty() {
                base_entities.insert(p.as_str(), entities);
            }
        }
    }
    // An oversize file on disk still defines what it declares.
    for (_, c) in work.iter().filter(|(p, c)| is_code(p) && oversize(c)) {
        let ids = identifiers(c);
        gone.retain(|n| !(may_mention(&ids, c, n) && has_declaration(c, n)));
    }

    let renames = renames(base, ours, theirs, subjects);
    let relocated = relocations(base, ours, theirs, work, subjects);
    let mut successors = Successors {
        base: &base_entities,
        work,
        memo: BTreeMap::new(),
    };

    let moddel = ModDelContext {
        base,
        ours,
        theirs,
        work,
        subjects,
        defined_now: &defined_now,
    };
    let mut verdicts = Vec::new();
    for file in subjects {
        let Some(w) = work.get(file) else {
            if let Some(to) = relocated
                .iter()
                .find_map(|(to, (from, _))| (from == file).then_some(to))
            {
                verdicts.push(Verdict::noted(
                    file,
                    &format!(
                        "not in the working tree: git moved it to `{to}`, where the directory \
                         it was added to was renamed"
                    ),
                ));
                continue;
            }
            if let Some(md) = ModDel::of(file, base, ours, theirs) {
                verdicts.push(moddel.deleted(file, &md));
                continue;
            }
            verdicts.push(
                match missing_file_finding(file, base, ours, theirs, work, &renames) {
                    Some(finding) => Verdict {
                        file: file.clone(),
                        findings: vec![finding],
                        advisories: Vec::new(),
                        note: None,
                    },
                    None => Verdict::noted(
                        file,
                        "not in the working tree, as the merge asked (one side deleted or \
                     renamed it and the other left it alone)",
                    ),
                },
            );
            continue;
        };
        let (b, o, t) = match relocated.get(file) {
            Some((from, _)) => (
                None,
                ours.get(from).map(String::as_str),
                theirs.get(from).map(String::as_str),
            ),
            None => stages(file, base, ours, theirs, &renames),
        };
        let mut findings = verify_file(file, b, o, t, w);
        if is_code(file) {
            findings.extend(dangling(w, &gone, &mut successors));
            findings.extend(dangling_imports(file, w, [b, o, t]));
        }
        let mut advisories = advisories_for(file, b, o, t);
        if let Some(md) = ModDel::of(file, base, ours, theirs) {
            let (f, a) = moddel.kept(file, w, &md);
            findings.extend(f);
            advisories.extend(a);
        }
        verdicts.push(Verdict {
            file: file.clone(),
            findings,
            advisories,
            note: None,
        });
    }
    verdicts
}

/// The sentence a scope owes its reader about files too large to parse, if
/// any: they were checked line by line, not for structure.
pub fn oversize_note(work: &Tree) -> Option<String> {
    let n = work
        .iter()
        .filter(|(p, c)| is_supported(p) && oversize(c))
        .count();
    (n > 0).then(|| {
        format!(
            "{n} file(s) over {} bytes were read for names lexically, not parsed for structure",
            weave_core::merge::STRUCTURE_LIMIT_BYTES
        )
    })
}

/// Renames in the merge: `new path -> (old path, the side that renamed)`.
///
/// A subject that base does not have, that exactly one side has, is a rename
/// when base has a path that side no longer has and whose content that side's
/// new file mostly still states. Git's merge carries the other side's edits to
/// the new path, so the three stages of the new path are base's and the other
/// side's copies at the OLD path, and the renaming side's copy at the new one.
/// Read without the rename, the new path has no base and no other side, and
/// every line the other side added looks like a duplicate.
///
/// Scored through an index of base's lines, so an added file is compared only
/// with the base files it shares a line with — not with every base file.
fn renames(
    base: &Tree,
    ours: &Tree,
    theirs: &Tree,
    subjects: &[String],
) -> BTreeMap<String, (String, Side)> {
    let mut out = BTreeMap::new();
    let added: Vec<(&String, Side)> = subjects
        .iter()
        .filter(|f| !base.contains_key(*f))
        .filter_map(|f| match (ours.get(f), theirs.get(f)) {
            (Some(_), None) => Some((f, Side::Ours)),
            (None, Some(_)) => Some((f, Side::Theirs)),
            _ => None,
        })
        .collect();
    if added.is_empty() {
        return out;
    }
    // line -> [(base path, copies of it there)], and each base file's count of
    // significant lines.
    let mut index: HashMap<&str, Vec<(&str, usize)>> = HashMap::new();
    let mut sizes: HashMap<&str, usize> = HashMap::new();
    for (p, old) in base {
        let mut copies: HashMap<&str, usize> = HashMap::new();
        for l in old.lines().map(str::trim).filter(|l| significant(l)) {
            *copies.entry(l).or_insert(0) += 1;
        }
        sizes.insert(p.as_str(), copies.values().sum());
        for (l, n) in copies {
            index.entry(l).or_default().push((p.as_str(), n));
        }
    }
    for (file, side) in added {
        let renamer = side.of(ours, theirs);
        let lines_b: BTreeSet<&str> = renamer[file]
            .lines()
            .map(str::trim)
            .filter(|l| significant(l))
            .collect();
        let mut kept: HashMap<&str, usize> = HashMap::new();
        for l in &lines_b {
            for (p, n) in index.get(l).into_iter().flatten() {
                *kept.entry(p).or_insert(0) += n;
            }
        }
        let best = kept
            .into_iter()
            .filter(|(p, _)| *p != file.as_str() && !renamer.contains_key(*p))
            .map(|(p, k)| (k as f64 / sizes[p].max(lines_b.len()) as f64, p))
            .filter(|(score, _)| *score >= 0.5)
            .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
        if let Some((_, old_path)) = best {
            out.insert(file.clone(), (old_path.to_string(), side));
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Ours,
    Theirs,
}

impl Side {
    fn of<'a>(self, ours: &'a Tree, theirs: &'a Tree) -> &'a Tree {
        match self {
            Side::Ours => ours,
            Side::Theirs => theirs,
        }
    }
}

/// Files git relocated: `placed path -> (the path a side added it at, side)`.
///
/// When one side renames a directory and the other adds a file inside it, git
/// writes the addition at the renamed location and reports a conflict
/// (`file location`). No merge stage has the placed path; the side's stage is
/// at the path it was added at, which is no longer on disk. Read without this,
/// the placed file is checked against three absent stages and every line it
/// states twice looks like a duplicate — while the added path looks like a
/// file the merge lost. The pair is recognised by what git keeps: the same file
/// name, and the side's content on disk.
fn relocations(
    base: &Tree,
    ours: &Tree,
    theirs: &Tree,
    work: &Tree,
    subjects: &[String],
) -> BTreeMap<String, (String, Side)> {
    let name = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
    let mut added: HashMap<String, Vec<(&String, Side)>> = HashMap::new();
    for q in subjects {
        if base.contains_key(q) || work.contains_key(q) {
            continue;
        }
        let side = match (ours.get(q), theirs.get(q)) {
            (Some(_), None) => Side::Ours,
            (None, Some(_)) => Side::Theirs,
            _ => continue,
        };
        added.entry(name(q)).or_default().push((q, side));
    }
    let mut out = BTreeMap::new();
    for p in subjects {
        let Some(w) = work.get(p) else { continue };
        if base.contains_key(p) || ours.contains_key(p) || theirs.contains_key(p) {
            continue;
        }
        let same: Vec<&(&String, Side)> = added
            .get(&name(p))
            .into_iter()
            .flatten()
            .filter(|(q, side)| side.of(ours, theirs).get(*q) == Some(w))
            .collect();
        if let [(q, side)] = same.as_slice() {
            out.insert(p.clone(), ((*q).clone(), *side));
        }
    }
    out
}

/// The three merge stages of one subject, following a rename.
fn stages<'a>(
    file: &str,
    base: &'a Tree,
    ours: &'a Tree,
    theirs: &'a Tree,
    renames: &BTreeMap<String, (String, Side)>,
) -> (Option<&'a str>, Option<&'a str>, Option<&'a str>) {
    let at = |tree: &'a Tree, path: &str| tree.get(path).map(String::as_str);
    match renames.get(file) {
        Some((old, Side::Theirs)) => (at(base, old), at(ours, old), at(theirs, file)),
        Some((old, Side::Ours)) => (at(base, old), at(ours, file), at(theirs, old)),
        None => (at(base, file), at(ours, file), at(theirs, file)),
    }
}

/// What to say about a subject the working tree no longer has, if anything.
///
/// A deletion one side made and the other side did not touch is what the
/// merge was asked to do, and so is a path the merge renamed away. Only a
/// deletion that throws away someone's edit, or a file that both sides kept,
/// is a finding.
fn missing_file_finding(
    file: &str,
    base: &Tree,
    ours: &Tree,
    theirs: &Tree,
    work: &Tree,
    renames: &BTreeMap<String, (String, Side)>,
) -> Option<Finding> {
    let renamed_away = renames
        .iter()
        .any(|(new, (old, _))| old == file && work.contains_key(new));
    if renamed_away {
        return None;
    }
    let (b, o, t) = (base.get(file), ours.get(file), theirs.get(file));
    let detail = match (b, o, t) {
        (Some(_), None, None) => return None,
        (Some(b), None, Some(t)) | (Some(b), Some(t), None) if b == t => return None,
        // A modify/delete is judged by `ModDelContext`, never here.
        (Some(_), None, Some(_)) | (Some(_), Some(_), None) => return None,
        (_, Some(_), Some(_)) => {
            "the file is not in the working tree, but both sides kept it (deleted during the \
             merge)"
        }
        (None, _, _) => "the file is not in the working tree, but a side added it",
    };
    Some(Finding {
        class: "MISSING",
        detail: detail.to_string(),
        suggestion: None,
    })
}

/// A modify/delete: base had the file, one side deleted it, and the other
/// side changed it.
struct ModDel<'a> {
    base: &'a str,
    /// The modifying side's version.
    modified: &'a str,
    /// The side that deleted it.
    deleter: Side,
}

impl<'a> ModDel<'a> {
    fn of(file: &str, base: &'a Tree, ours: &'a Tree, theirs: &'a Tree) -> Option<Self> {
        let b = base.get(file)?;
        let (modified, deleter) = match (ours.get(file), theirs.get(file)) {
            (None, Some(t)) => (t, Side::Ours),
            (Some(o), None) => (o, Side::Theirs),
            _ => return None,
        };
        (modified != b).then_some(ModDel {
            base: b,
            modified,
            deleter,
        })
    }

    fn sides(&self) -> (&'static str, &'static str) {
        match self.deleter {
            Side::Ours => ("ours", "theirs"),
            Side::Theirs => ("theirs", "ours"),
        }
    }
}

/// Where the modifier's edit stands relative to the deleted file's successor.
enum Port {
    /// No file the deleter wrote is recognisably the deleted one.
    NoSuccessor,
    /// The successor already carries the edit.
    Present(String),
    /// The edit re-applies to the successor as a clean three-way merge; the
    /// successor as it would be with it.
    Portable(String, String),
    /// The edit does not re-apply cleanly.
    Conflicts(String),
}

/// The modify/delete rule, derived from what the two resolutions can lose.
///
/// Deleting a file one side modified loses the modification, unless it did
/// not say anything (it only re-laid-out the file) or it already lives on in
/// the file the deleter moved the content to. Keeping it loses the deletion,
/// which matters only if the deletion was a MOVE — the content now exists
/// twice — or if the kept file leans on names the deleter removed. Each of
/// those is checked; anything else is a choice between two intents, which
/// weave reports as an advisory and does not decide.
///
/// * DELETE is a finding only if (a) a surviving file still calls a name
///   defined only in the deleted file, or (b) the modifier's edit is neither
///   layout-only nor present in a successor file.
/// * KEEP is a finding only if (a) the successor duplicates a definition the
///   kept file still makes; (b) — the kept file calling a name the deleter
///   removed elsewhere — is the dangling pass's `DANGLING` finding, which
///   runs on every file on disk.
///
/// The successor is found, not assumed: a file the deleter added or changed,
/// sharing the most significant lines with the deleted file's base, the only
/// one at the first similarity tier (½, ⅓, ⅕) that has any candidate at all.
/// "Present" and "portable" are decided by the merge itself — the modifier's
/// edit, base → modified, merged onto the successor as found on disk.
///
/// This is where a move plus an edit (D5) is settled, and the only place it can
/// be: git hands the merge driver one path at a time, and a modify/delete is
/// never given to the driver at all, so no driver sees both the deleted file
/// and the file it moved to. `weave check` sees both trees. It OFFERS the port
/// — a `git apply` patch in the finding — and does not write it: the check
/// reads the working tree and never changes it.
struct ModDelContext<'a> {
    base: &'a Tree,
    ours: &'a Tree,
    theirs: &'a Tree,
    work: &'a Tree,
    subjects: &'a [String],
    defined_now: &'a BTreeSet<String>,
}

impl ModDelContext<'_> {
    fn deleted(&self, file: &str, md: &ModDel) -> Verdict {
        let (deleter, modifier) = md.sides();
        let mut findings = Vec::new();
        let mut advisories = Vec::new();
        let advise = |detail: String| Advisory {
            class: "MODDEL",
            entity: file.to_string(),
            entity_type: "file".to_string(),
            detail,
        };

        // (a) a name only the deleted file defined, still called on disk.
        if is_code(file) {
            let mut names: BTreeSet<String> = BTreeSet::new();
            for text in [md.base, md.modified] {
                names.extend(
                    entities_of(file, text)
                        .into_iter()
                        .map(|e| e.name)
                        .filter(|n| n.len() >= 3 && !self.defined_now.contains(n)),
                );
            }
            if let Some((name, caller)) = self.called_somewhere(&names) {
                findings.push(Finding {
                    class: "MODDEL",
                    detail: format!(
                        "{deleter} deleted this file, but `{caller}` still calls `{name}`, which \
                         only this file defined (modify/delete)"
                    ),
                    suggestion: Some(format!(
                        "keep the version {modifier} modified, or remove the calls to `{name}`"
                    )),
                });
            }
        }

        // (b) the modifier's edit: layout, carried, portable, or lost.
        if weave_core::layout::layout_equal(md.base, md.modified, file) {
            advisories.push(advise(format!(
                "deleted as {deleter} asked; the change {modifier} made to it was layout only \
                 (whitespace), so nothing it said is lost"
            )));
        } else {
            match self.port(file, md) {
                Port::Present(to) => advisories.push(advise(format!(
                    "deleted as {deleter} asked; {deleter} moved its content to `{to}`, which \
                     already carries the edit {modifier} made"
                ))),
                Port::Portable(to, ported) => findings.push(Finding {
                    class: "MODDEL",
                    detail: format!(
                        "{deleter} moved this file to `{to}` and {modifier} edited it here; the \
                         edit is not in `{to}` (modify/delete)"
                    ),
                    suggestion: Some(format!(
                        "the edit re-applies to `{to}` cleanly — apply this patch (`git apply`):\n{}",
                        patch(&to, &self.work[&to], &ported)
                    )),
                }),
                Port::Conflicts(to) => findings.push(Finding {
                    class: "MODDEL",
                    detail: format!(
                        "{deleter} moved this file to `{to}` and {modifier} edited it here; the \
                         edit is not in `{to}` and does not re-apply there cleanly (modify/delete)"
                    ),
                    suggestion: Some(format!(
                        "carry the edit {modifier} made here into `{to}` by hand, or restore \
                         this file"
                    )),
                }),
                Port::NoSuccessor => findings.push(Finding {
                    class: "MODDEL",
                    detail: format!(
                        "the file is not in the working tree: {deleter} deleted it but \
                         {modifier} modified it, and the edit exists nowhere else \
                         (modify/delete)"
                    ),
                    suggestion: Some(format!(
                        "restore the version {modifier} modified if its edit should survive"
                    )),
                }),
            }
        }
        Verdict {
            file: file.to_string(),
            findings,
            advisories,
            note: None,
        }
    }

    fn kept(&self, file: &str, w: &str, md: &ModDel) -> (Vec<Finding>, Vec<Advisory>) {
        let (deleter, _) = md.sides();
        if let Some(to) = self.successor(file, md) {
            let here: BTreeSet<String> = entities_of(file, w)
                .into_iter()
                .filter(|e| definition_key(file, e).is_some())
                .map(|e| e.name)
                .collect();
            let there: BTreeSet<String> = entities_of(&to, &self.work[&to])
                .into_iter()
                .filter(|e| definition_key(&to, e).is_some())
                .map(|e| e.name)
                .collect();
            let twice: Vec<String> = here.intersection(&there).take(4).cloned().collect();
            if !twice.is_empty() {
                let listed = twice
                    .iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                return (
                    vec![Finding {
                        class: "MODDEL",
                        detail: format!(
                            "{deleter} moved this file to `{to}`, and keeping it here defines \
                             {listed} in both (modify/delete)"
                        ),
                        suggestion: Some(format!(
                            "delete this file and carry the edit into `{to}` instead"
                        )),
                    }],
                    Vec::new(),
                );
            }
        }
        (
            Vec::new(),
            vec![Advisory {
                class: "MODDEL",
                entity: file.to_string(),
                entity_type: "file".to_string(),
                detail: format!(
                    "kept although {deleter} deleted it — whether the file should exist is the \
                     authors' call; nothing it defines is duplicated elsewhere"
                ),
            }],
        )
    }

    /// The first vanished name some file on disk still calls, and that file.
    fn called_somewhere(&self, names: &BTreeSet<String>) -> Option<(String, String)> {
        if names.is_empty() {
            return None;
        }
        for (path, c) in self.work.iter().filter(|(p, _)| is_code(p)) {
            let ids = identifiers(c);
            for n in names {
                if may_mention(&ids, c, n) && !has_declaration(c, n) && has_call_reference(c, n) {
                    return Some((n.clone(), path.clone()));
                }
            }
        }
        None
    }

    /// The file the deleter moved this one's content to, if exactly one
    /// candidate leads. See the type's docs.
    fn successor(&self, file: &str, md: &ModDel) -> Option<String> {
        let deleter_tree = md.deleter.of(self.ours, self.theirs);
        let mut want: HashMap<&str, usize> = HashMap::new();
        for l in md.base.lines().map(str::trim).filter(|l| significant(l)) {
            *want.entry(l).or_insert(0) += 1;
        }
        let size = want.values().sum::<usize>();
        if size == 0 {
            return None;
        }
        let scored: Vec<(f64, &String)> = self
            .subjects
            .iter()
            .filter(|p| p.as_str() != file && self.work.contains_key(*p))
            .filter(|p| match (deleter_tree.get(*p), self.base.get(*p)) {
                (Some(d), Some(b)) => d != b,
                (Some(_), None) => true,
                (None, _) => false,
            })
            .map(|p| {
                let mut have: HashMap<&str, usize> = HashMap::new();
                for l in self.work[p]
                    .lines()
                    .map(str::trim)
                    .filter(|l| significant(l))
                {
                    *have.entry(l).or_insert(0) += 1;
                }
                let shared: usize = want
                    .iter()
                    .map(|(l, n)| (*n).min(have.get(l).copied().unwrap_or(0)))
                    .sum();
                let theirs = have.values().sum::<usize>();
                (shared as f64 / size.max(theirs) as f64, p)
            })
            .collect();
        for tier in [0.5, 1.0 / 3.0, 0.2] {
            let at: Vec<&String> = scored
                .iter()
                .filter(|(s, _)| *s >= tier)
                .map(|(_, p)| *p)
                .collect();
            match at.len() {
                0 => continue,
                1 => return Some(at[0].clone()),
                _ => return None,
            }
        }
        None
    }

    fn port(&self, file: &str, md: &ModDel) -> Port {
        let Some(to) = self.successor(file, md) else {
            return Port::NoSuccessor;
        };
        let now = &self.work[&to];
        let r = weave_core::entity_merge(md.base, now, md.modified, &to);
        if !r.is_clean() {
            return Port::Conflicts(to);
        }
        if weave_core::layout::layout_equal(&r.content, now, &to) {
            Port::Present(to)
        } else {
            Port::Portable(to, r.content)
        }
    }
}

/// `old` → `new` at `path`, as a patch `git apply` takes.
fn patch(path: &str, old: &str, new: &str) -> String {
    let body = diffy::create_patch(old, new).to_string();
    let hunks = body.find("@@").map(|i| &body[i..]).unwrap_or("");
    format!("--- a/{path}\n+++ b/{path}\n{hunks}")
}

/// Names a stage of THIS file bound — imported, or declared at file scope —
/// that the working tree still uses and no longer binds.
///
/// The repo-wide pass above cannot see these: the definition the name points
/// at still exists in its own file, and the break is that this file stopped
/// importing it. A merge produces it when each side deletes a different import
/// and one side's surviving code uses the name the other side's deletion
/// unbound.
///
/// With all three stages, this is the merge's own rule
/// ([`weave_core::verify::dangling_use`]), so the driver and `weave check`
/// cannot disagree about it. Without a base (an add/add), only imports are
/// read.
fn dangling_imports(file: &str, w: &str, stages: [Option<&str>; 3]) -> Vec<Finding> {
    if let [Some(b), Some(o), Some(t)] = stages {
        let parsed = [b, o, t, w].map(|text| entities_of(file, text));
        return weave_core::verify::dangling_use(
            file,
            [b, o, t, w],
            [&parsed[0], &parsed[1], &parsed[2], &parsed[3]],
        )
        .map(|n| {
            let import = [b, o, t]
                .iter()
                .flat_map(|s| s.lines())
                .find(|l| weave_core::verify::import_names(file, l).contains(&n));
            Finding {
                class: "DANGLING",
                detail: format!(
                    "`{n}` is still used here but no longer declared or imported; a merge \
                         stage bound it"
                ),
                suggestion: Some(match import {
                    Some(line) => format!(
                        "restore the import a merge stage had for `{n}`: `{}`",
                        line.trim()
                    ),
                    None => format!(
                        "restore the declaration of `{n}` a merge stage had, or remove its \
                             uses"
                    ),
                }),
            }
        })
        .into_iter()
        .collect();
    }
    let mut hits: BTreeMap<String, String> = BTreeMap::new();
    for stage in stages.into_iter().flatten() {
        for line in stage.lines() {
            for name in import_bindings(line) {
                if hits.contains_key(&name)
                    || has_binding(w, &name)
                    || !has_value_reference(w, &name)
                {
                    continue;
                }
                hits.insert(name, line.trim().to_string());
            }
        }
    }
    if hits.is_empty() {
        return Vec::new();
    }
    let listed = hits
        .keys()
        .take(4)
        .map(|n| format!("`{n}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let (first, line) = hits.iter().next().expect("non-empty");
    vec![Finding {
        class: "DANGLING",
        detail: format!(
            "{} name(s) are still used here but no longer imported: {listed}",
            hits.len()
        ),
        suggestion: Some(format!(
            "restore the import a merge stage had for `{first}`: `{line}`"
        )),
    }]
}

/// The co-occupancy advisories weave's own merge of the three stages would
/// raise: containers both sides changed different siblings of, which merged
/// clean. Read off the three stage texts, not off the bytes on disk — the fact
/// is about what the two authors did, and holds however the resolution was
/// written. Empty when any stage lacks the file (an add or a delete is not a
/// co-change) or nothing on disk parses it.
fn advisories_for(
    file: &str,
    base: Option<&str>,
    ours: Option<&str>,
    theirs: Option<&str>,
) -> Vec<Advisory> {
    let (Some(b), Some(o), Some(t)) = (base, ours, theirs) else {
        return Vec::new();
    };
    weave_core::entity_merge(b, o, t, file)
        .warnings
        .iter()
        .filter_map(|w| match &w.kind {
            weave_core::validate::WarningKind::SiblingCoChange {
                ours_added,
                ours_changed,
                theirs_added,
                theirs_changed,
            } => Some(Advisory {
                class: "COOCCUPANCY",
                entity: w.entity_name.clone(),
                entity_type: w.entity_type.clone(),
                detail: format!(
                    "both sides changed siblings ({}; {})",
                    weave_core::validate::co_change_side_phrase("ours", ours_added, ours_changed),
                    weave_core::validate::co_change_side_phrase(
                        "theirs",
                        theirs_added,
                        theirs_changed
                    ),
                ),
            }),
            _ => None,
        })
        .collect()
}

/// The subset of the repo-wide vanished-name set (`gone`) that THIS file still
/// calls without defining — the file's dangling references.
///
/// `gone` is computed once by the caller and shared across every subject,
/// because it is a fact about the merge, not about the file being verified. Only
/// the per-file half lives here: which of those vanished names this file's bytes
/// actually call — asked of the file's identifier set first, so a name the file
/// never mentions costs a hash lookup, not a scan.
///
/// The rename repair is the derivable half: when the vanished name has a
/// same-file successor that IS defined now and did not exist in base, the fix
/// is a rename and weave can say which one.
fn dangling(w: &str, gone: &BTreeSet<String>, successors: &mut Successors) -> Vec<Finding> {
    let ids = identifiers(w);
    let mut hits: Vec<(String, Option<(String, String)>)> = Vec::new();
    for name in gone {
        if name.len() < 3
            || !may_mention(&ids, w, name)
            || has_declaration(w, name)
            || !has_call_reference(w, name)
        {
            continue;
        }
        hits.push((name.clone(), successors.of(name)));
    }
    if hits.is_empty() {
        return Vec::new();
    }
    let listed = hits
        .iter()
        .take(4)
        .map(|(n, _)| format!("`{n}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let suggestion = hits.iter().find_map(|(n, s)| {
        s.as_ref().map(|(to, where_)| {
            format!(
                "`{n}` left `{where_}` and `{to}` arrived in its place — if that is the \
                 rename, rewrite every call of `{n}` here to `{to}`"
            )
        })
    });
    vec![Finding {
        class: "DANGLING",
        detail: format!(
            "{} name(s) are still referenced here but defined nowhere in the working tree: {}",
            hits.len(),
            listed
        ),
        suggestion: suggestion
            .or_else(|| Some("restore the definition, or remove the call sites".to_string())),
    }]
}

/// The name a vanished definition was RENAMED to, and the file that says so —
/// answered once per name, however many subjects still call it.
///
/// This is the derivable half of a `DANGLING` finding, and it is derived by
/// the one matcher both weave passes already use — `sem-core`'s
/// `match_entities`, so the answer here and the answer the merge gave cannot
/// disagree. It is looked up in the file where the definition *lived*, not in
/// the file where the call survives: a new name next to a broken call is a
/// coincidence, a matched pair in the defining file is evidence.
struct Successors<'a> {
    /// Base's parse of every code subject, made once by [`check`].
    base: &'a BTreeMap<&'a str, Vec<SemanticEntity>>,
    work: &'a Tree,
    memo: BTreeMap<String, Option<(String, String)>>,
}

impl Successors<'_> {
    fn of(&mut self, name: &str) -> Option<(String, String)> {
        if let Some(known) = self.memo.get(name) {
            return known.clone();
        }
        let found = self.find(name);
        self.memo.insert(name.to_string(), found.clone());
        found
    }

    fn find(&self, name: &str) -> Option<(String, String)> {
        for (path, old) in self.base {
            if !old.iter().any(|e| e.name == name) {
                continue;
            }
            let Some(now) = self.work.get(*path) else {
                continue;
            };
            let new = entities_of(path, now);
            for c in match_entities(old, &new, path, None, None, None).changes {
                if c.change_type == ChangeType::Renamed
                    && c.old_entity_name.as_deref() == Some(name)
                    && c.entity_name != name
                    && !c.entity_name.is_empty()
                {
                    return Some((c.entity_name, path.to_string()));
                }
            }
            // The matcher pairs on body similarity, so a declaration that was
            // renamed AND rewritten comes back unpaired. One name out, one name
            // in, in the file that defined it, is then the only candidate there
            // is — and the suggestion says "if that is the rename" rather than
            // claiming it, because this is arithmetic and not evidence.
            let before: BTreeSet<&str> = old.iter().map(|e| e.name.as_str()).collect();
            let after: BTreeSet<&str> = new.iter().map(|e| e.name.as_str()).collect();
            let gone: Vec<&&str> = before.difference(&after).collect();
            let arrived: Vec<&&str> = after.difference(&before).collect();
            if gone.len() == 1 && arrived.len() == 1 && *gone[0] == name {
                return Some((arrived[0].to_string(), path.to_string()));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(pairs: &[(&str, &str)]) -> Tree {
        pairs
            .iter()
            .map(|(p, c)| (p.to_string(), c.to_string()))
            .collect()
    }

    const BASE: &str = "def a():\n    total_count = 0\n    return total_count\n\ndef keep():\n    return 'a stable helper line'\n";

    /// Runs `f` on a watchdog thread: a regression fails the test instead of
    /// hanging the suite.
    fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(std::time::Duration::from_secs(secs))
            .unwrap_or_else(|_| panic!("did not finish within {secs}s"))
    }

    /// A data file with a hundred thousand same-named elements, each with a
    /// child: the shape `sem-core`'s id disambiguation is quadratic in. At
    /// this size an unbounded parse spins for many minutes; the check must not
    /// parse it at all, and must still verify everything else.
    #[test]
    fn a_huge_data_file_elsewhere_in_the_repo_does_not_stall_the_check() {
        let mut xml = String::from("<records>\n");
        let mut i = 0;
        while xml.len() <= 3 * weave_core::merge::STRUCTURE_LIMIT_BYTES {
            xml.push_str(&format!("  <record><id>{i}</id></record>\n"));
            i += 1;
        }
        xml.push_str("</records>\n");
        let ours = BASE.replace("total_count = 0", "total_count = 1");
        let theirs = BASE.replace("return 'a stable", "return 'one stable");
        let merged = ours.replace("return 'a stable", "return 'one stable");
        let work = tree(&[("m.py", &merged), ("data/records.xml", &xml)]);
        let v = within(60, move || {
            check(
                &tree(&[("m.py", BASE)]),
                &tree(&[("m.py", &ours)]),
                &tree(&[("m.py", &theirs)]),
                &work,
                &["m.py".to_string()],
            )
        });
        assert!(v[0].ok(), "{:#?}", v[0]);
        let note = oversize_note(&tree(&[("data/records.xml", "")])).is_none();
        assert!(note, "a small file needs no note");
    }

    /// A file too large to parse still defines what it declares: the name is
    /// asked of it lexically, so a caller of it is not reported dangling.
    #[test]
    fn a_name_declared_only_in_an_oversize_file_is_not_dangling() {
        let mut big = String::from("def load_all_rows():\n    return []\n");
        while big.len() <= weave_core::merge::STRUCTURE_LIMIT_BYTES {
            big.push_str("# padding that makes this file too large to parse for structure\n");
        }
        let base = tree(&[
            ("a.py", "def load_all_rows():\n    return []\n"),
            ("b.py", "def go():\n    return load_all_rows()\n"),
        ]);
        let ours = tree(&[
            ("a.py", "def unrelated():\n    return 1\n"),
            ("b.py", "def go():\n    return load_all_rows()\n"),
        ]);
        let mut work = ours.clone();
        let v = check(&base, &ours, &base, &work, &["b.py".to_string()]);
        assert!(
            v[0].findings.iter().any(|f| f.class == "DANGLING"),
            "without a definition anywhere it dangles: {:#?}",
            v[0]
        );
        work.insert("vendor/big.py".to_string(), big);
        let v = check(&base, &ours, &base, &work, &["b.py".to_string()]);
        assert!(v[0].ok(), "{:#?}", v[0]);
        assert!(oversize_note(&work).is_some_and(|n| n.starts_with("1 file(s)")));
    }

    /// Every subject calls a name the merge renamed away. Each repo-wide fact
    /// — which names vanished, where each lived, what it became — is worked
    /// out once, not once per subject: asked per subject, this was a parse of
    /// every base file for every hit, and a two-thousand-file merge spun for
    /// minutes.
    #[test]
    fn the_dangling_pass_scales_with_the_merge_not_its_square() {
        let n = 1500;
        let file = |i: usize| format!("pkg/m{i:04}.py");
        // Files of realistic length: the per-subject cost was every vanished
        // name times every line of the file.
        let padding: String = (0..60)
            .map(|k| format!("# note {k}: an ordinary line of commentary in a module\n"))
            .collect();
        let body = |i: usize, name: &str| {
            format!(
                "def {name}(x):\n    return x + {i}\n\n\ndef use_{i}():\n    return helper_{}(1)\n{padding}",
                (i + 1) % n
            )
        };
        let base: Tree = (0..n)
            .map(|i| (file(i), body(i, &format!("helper_{i}"))))
            .collect();
        let ours: Tree = (0..n)
            .map(|i| (file(i), body(i, &format!("renamed_helper_{i}"))))
            .collect();
        let subjects: Vec<String> = (0..n).map(file).collect();
        let v = within(120, move || {
            let work = ours.clone();
            check(&base, &ours, &base, &work, &subjects)
        });
        assert_eq!(v.len(), n);
        assert!(
            v.iter()
                .all(|v| v.findings.iter().any(|f| f.class == "DANGLING")),
            "every caller of a renamed helper dangles"
        );
    }

    #[test]
    fn a_data_key_both_sides_added_is_a_duplicate_and_a_broken_file_is_parse() {
        let base = "[deps]\nalpha = \"1\"\nbravo = \"1\"\n";
        let ours = "[deps]\nalpha = \"1\"\nkit = \"0.6\"\nbravo = \"1\"\n";
        let theirs = "[deps]\nalpha = \"1\"\nbravo = \"1\"\nkit = { version = \"0.6\" }\n";
        let both =
            "[deps]\nalpha = \"1\"\nkit = \"0.6\"\nbravo = \"1\"\nkit = { version = \"0.6\" }\n";
        let run = |work: &str| {
            check(
                &tree(&[("Cargo.toml", base)]),
                &tree(&[("Cargo.toml", ours)]),
                &tree(&[("Cargo.toml", theirs)]),
                &tree(&[("Cargo.toml", work)]),
                &["Cargo.toml".to_string()],
            )
        };
        // TOML refuses a key stated twice at load, so this is a PARSE finding.
        let v = run(both);
        assert_eq!(findings_of(&v, "Cargo.toml")[0].class, "PARSE", "{v:#?}");
        assert!(run(ours).iter().all(Verdict::ok));

        let base = "{\"a\": 1,\n \"b\": 2}\n";
        let ours = "{\"a\": 1,\n \"t\": 5,\n \"b\": 2}\n";
        let theirs = "{\"a\": 1,\n \"b\": 2,\n \"t\": 9}\n";
        let both = "{\"a\": 1,\n \"t\": 5,\n \"b\": 2,\n \"t\": 9}\n";
        let v = check(
            &tree(&[("p.json", base)]),
            &tree(&[("p.json", ours)]),
            &tree(&[("p.json", theirs)]),
            &tree(&[("p.json", both)]),
            &["p.json".to_string()],
        );
        let f = findings_of(&v, "p.json");
        assert!(
            f.iter()
                .any(|f| f.class == "DUP" && f.detail.contains("`t`")),
            "{v:#?}"
        );
    }

    #[test]
    fn a_faithful_resolution_verifies_clean() {
        let ours = BASE.replace("total_count = 0", "total_count = 1");
        let theirs = BASE.replace("total_count = 0", "total_count = 2");
        let resolved = BASE.replace("total_count = 0", "total_count = 3");
        let v = check(
            &tree(&[("m.py", BASE)]),
            &tree(&[("m.py", &ours)]),
            &tree(&[("m.py", &theirs)]),
            &tree(&[("m.py", &resolved)]),
            &["m.py".to_string()],
        );
        assert!(v[0].ok(), "{:#?}", v[0]);
        assert!(v[0].line().starts_with("OK: m.py"));
    }

    #[test]
    fn a_deliberately_lossy_resolution_is_caught() {
        let ours = BASE.replace("total_count = 0", "total_count = 1");
        let theirs = BASE.replace("total_count = 0", "total_count = 2");
        // Drops `keep`, which NEITHER side touched — the unanimous case.
        let lossy = "def a():\n    total_count = 3\n    return total_count\n";
        let v = check(
            &tree(&[("m.py", BASE)]),
            &tree(&[("m.py", &ours)]),
            &tree(&[("m.py", &theirs)]),
            &tree(&[("m.py", lossy)]),
            &["m.py".to_string()],
        );
        assert!(!v[0].ok(), "{:#?}", v[0]);
        assert!(
            v[0].findings.iter().any(|f| f.class == "LOSS"),
            "{:#?}",
            v[0]
        );
    }

    #[test]
    fn markers_left_behind_are_the_first_thing_reported() {
        let conflicted = "def a():\n<<<<<<< ours — function `a`\n    total_count = 1\n=======\n    total_count = 2\n>>>>>>> theirs — function `a`\n    return total_count\n\ndef keep():\n    return 'a stable helper line'\n";
        let v = check(
            &tree(&[("m.py", BASE)]),
            &tree(&[("m.py", BASE)]),
            &tree(&[("m.py", BASE)]),
            &tree(&[("m.py", conflicted)]),
            &["m.py".to_string()],
        );
        assert_eq!(v[0].findings[0].class, "MARKERS", "{:#?}", v[0]);
    }

    #[test]
    fn a_line_stated_more_often_than_either_side_states_it_is_a_duplicate() {
        let dup = "def a():\n    total_count = 0\n    total_count = 0\n    return total_count\n\ndef keep():\n    return 'a stable helper line'\n";
        let v = check(
            &tree(&[("m.py", BASE)]),
            &tree(&[("m.py", BASE)]),
            &tree(&[("m.py", BASE)]),
            &tree(&[("m.py", dup)]),
            &["m.py".to_string()],
        );
        assert!(
            v[0].findings.iter().any(|f| f.class == "DUP"),
            "{:#?}",
            v[0]
        );
    }

    #[test]
    fn a_call_whose_definition_left_the_repo_is_dangling() {
        let base = tree(&[
            ("a.py", "def fetch_user(i):\n    return i\n"),
            ("b.py", "def go():\n    return fetch_user(1)\n"),
        ]);
        let ours = tree(&[
            ("a.py", "def get_user(i):\n    return i\n"),
            ("b.py", "def go():\n    return fetch_user(1)\n"),
        ]);
        let work = ours.clone();
        let v = check(&base, &ours, &base, &work, &["b.py".to_string()]);
        assert!(
            v[0].findings.iter().any(|f| f.class == "DANGLING"),
            "{:#?}",
            v[0]
        );
    }

    #[test]
    fn a_clean_container_co_change_is_an_advisory_not_a_finding() {
        // Both sides change DIFFERENT methods of one class; the resolution on
        // disk keeps both. The file is OK — no findings — yet the co-occupancy
        // is surfaced as a non-blocking review advisory.
        let base =
            "class C:\n    def a(self):\n        return 1\n\n    def b(self):\n        return 2\n";
        let ours = "class C:\n    def a(self):\n        return 1 + 10\n\n    def b(self):\n        return 2\n";
        let theirs = "class C:\n    def a(self):\n        return 1\n\n    def b(self):\n        return 2 + 20\n";
        let work = "class C:\n    def a(self):\n        return 1 + 10\n\n    def b(self):\n        return 2 + 20\n";
        let v = check(
            &tree(&[("m.py", base)]),
            &tree(&[("m.py", ours)]),
            &tree(&[("m.py", theirs)]),
            &tree(&[("m.py", work)]),
            &["m.py".to_string()],
        );
        assert!(
            v[0].ok(),
            "an advisory must not make the file not-OK: {:#?}",
            v[0]
        );
        assert!(
            v[0].findings.is_empty(),
            "advisory is not a finding: {:#?}",
            v[0]
        );
        assert_eq!(
            v[0].advisories.len(),
            1,
            "the co-change must be advised: {:#?}",
            v[0]
        );
        assert_eq!(v[0].advisories[0].entity, "C");
        // The report renders it under `review`, and the tally/exit are untouched.
        let report = Report {
            scope: "test".to_string(),
            verdicts: v,
        };
        assert_eq!(report.tally().1, 0, "no files with findings");
        assert!(
            report.render().contains("review (COOCCUPANCY)"),
            "{}",
            report.render()
        );
    }

    /// One side moves a definition out of top level — into a class, as a
    /// method or member — and a caller elsewhere still calls it by name.
    /// `(defining file, its base copy, its moved copy, caller file, caller)`.
    const MOVED_INTO_A_CLASS: [(&str, &str, &str, &str, &str); 4] = [
        (
            "sheet.cpp",
            "void add_row(int x) {\n  total += x;\n}\n",
            "class Sheet {\n public:\n  void add_row(int x) {\n    total += x;\n  }\n};\n",
            "fill.cpp",
            "void Sheet::fill() {\n  add_row(1);\n}\n",
        ),
        // Declared in the class, defined out of it: the definition's name is
        // qualified, and it still defines the member.
        (
            "sheet.cpp",
            "void add_row(int x) {\n  total += x;\n}\n",
            "class Sheet {\n public:\n  void add_row(int x);\n};\n\n\
             void Sheet::add_row(int x) {\n  total += x;\n}\n",
            "fill.cpp",
            "void Sheet::fill() {\n  add_row(1);\n}\n",
        ),
        (
            "rows.py",
            "def add_row(x):\n    return x + 1\n",
            "class Sheet:\n    def add_row(self, x):\n        return x + 1\n",
            "fill.py",
            "def fill():\n    return add_row(1)\n",
        ),
        (
            "sheet.ts",
            "export function addRow(x: number) {\n  return x + 1;\n}\n",
            "export class Sheet {\n  addRow(x: number) {\n    return x + 1;\n  }\n}\n",
            "fill.ts",
            "function fill() {\n  return addRow(1);\n}\n",
        ),
    ];

    fn check_move(
        defs: &str,
        base_def: &str,
        work_def: &str,
        caller: &str,
        call: &str,
    ) -> Vec<Verdict> {
        let base = tree(&[(defs, base_def), (caller, call)]);
        let ours = tree(&[(defs, work_def), (caller, call)]);
        check(
            &base,
            &ours,
            &base,
            &ours,
            &[defs.to_string(), caller.to_string()],
        )
    }

    /// A name that is still defined — only now nested in a class — is not
    /// deleted, and its callers are not dangling. Whether a definition is
    /// still there is asked of every name the file defines, not just its
    /// top-level ones.
    #[test]
    fn a_definition_moved_into_a_class_still_defines_its_name() {
        for (defs, base_def, work_def, caller, call) in MOVED_INTO_A_CLASS {
            let v = check_move(defs, base_def, work_def, caller, call);
            assert!(
                findings_of(&v, caller)
                    .iter()
                    .all(|f| f.class != "DANGLING"),
                "{defs} -> nested: {v:#?}"
            );
            // And back out again: nested in base, top level on disk.
            let v = check_move(defs, work_def, base_def, caller, call);
            assert!(
                findings_of(&v, caller)
                    .iter()
                    .all(|f| f.class != "DANGLING"),
                "{defs} -> top level: {v:#?}"
            );
        }
    }

    /// The negative control: the same definitions deleted outright — the
    /// class kept, the member gone — still leave their callers dangling.
    #[test]
    fn a_definition_deleted_outright_still_dangles() {
        let emptied = [
            (
                "sheet.cpp",
                "class Sheet {\n public:\n  void other_row(int x) {\n    total -= x;\n  }\n};\n",
                "add_row",
            ),
            (
                "rows.py",
                "class Sheet:\n    def other_row(self, x):\n        return x - 1\n",
                "add_row",
            ),
            (
                "sheet.ts",
                "export class Sheet {\n  otherRow(x: number) {\n    return x - 1;\n  }\n}\n",
                "addRow",
            ),
        ];
        for (defs, base_def, _, caller, call) in MOVED_INTO_A_CLASS {
            let (_, work_def, name) = emptied.iter().find(|(d, ..)| *d == defs).unwrap();
            let v = check_move(defs, base_def, work_def, caller, call);
            let dangling = findings_of(&v, caller)
                .into_iter()
                .find(|f| f.class == "DANGLING")
                .unwrap_or_else(|| panic!("{defs}: a deleted `{name}` must dangle: {v:#?}"));
            assert!(
                dangling.detail.contains(&format!("`{name}`")),
                "{dangling:#?}"
            );
        }
    }

    /// A data file's nested key is not a definition: one named like a deleted
    /// function does not stand in for it.
    #[test]
    fn a_nested_data_key_does_not_define_a_deleted_name() {
        let config = "{\n  \"columns\": {\n    \"add_row\": true\n  }\n}\n";
        let (defs, base_def, _, caller, call) = MOVED_INTO_A_CLASS[2];
        let gone = "class Sheet:\n    def other_row(self, x):\n        return x - 1\n";
        let base = tree(&[(defs, base_def), (caller, call), ("config.json", config)]);
        let ours = tree(&[(defs, gone), (caller, call), ("config.json", config)]);
        let v = check(
            &base,
            &ours,
            &base,
            &ours,
            &[defs.to_string(), caller.to_string()],
        );
        assert!(
            findings_of(&v, caller)
                .iter()
                .any(|f| f.class == "DANGLING"),
            "{v:#?}"
        );
    }

    fn findings_of<'a>(v: &'a [Verdict], file: &str) -> Vec<&'a Finding> {
        v.iter()
            .filter(|v| v.file == file)
            .flat_map(|v| v.findings.iter())
            .collect()
    }

    const FLOW_BASE: &str = "\
import * as Alpha from \"./widget\"
import * as Beta from \"./gadget\"
import * as Widget from \"./widget\"

export function* flow() {
  yield* Alpha.first()
  const spacer = 1
  yield* Widget.second()
}
";

    /// Each side deleted a different import; the resolution on disk kept
    /// neither, and still uses one of the two names.
    #[test]
    fn a_type_whose_import_the_merge_dropped_is_dangling() {
        // Theirs deleted the import as unused; ours started using the type,
        // as a type argument only. The resolution kept both edits.
        let base = "package p;\n\nimport a.model.Receipt;\nimport java.util.List;\n\npublic class S {\n    public List<String> all() {\n        return null;\n    }\n}\n";
        let ours = base.replace("List<String>", "List<Receipt>");
        let theirs = base.replace("import a.model.Receipt;\n", "");
        let work = ours.replace("import a.model.Receipt;\n", "");
        let f = "S.java";
        let v = check(
            &tree(&[(f, base)]),
            &tree(&[(f, &ours)]),
            &tree(&[(f, &theirs)]),
            &tree(&[(f, &work)]),
            &[f.to_string()],
        );
        assert!(
            v[0].findings.iter().any(|f| f.class == "DANGLING"),
            "{:#?}",
            v[0]
        );
        // Keeping the import is the resolution.
        let v = check(
            &tree(&[(f, base)]),
            &tree(&[(f, &ours)]),
            &tree(&[(f, &theirs)]),
            &tree(&[(f, &ours)]),
            &[f.to_string()],
        );
        assert!(
            !v[0].findings.iter().any(|f| f.class == "DANGLING"),
            "{:#?}",
            v[0]
        );
    }

    #[test]
    fn a_namespace_whose_import_the_merge_dropped_is_dangling() {
        let ours = FLOW_BASE
            .replace("import * as Widget from \"./widget\"\n", "")
            .replace("Widget.second()", "Beta.second()");
        let theirs = FLOW_BASE
            .replace("import * as Alpha from \"./widget\"\n", "")
            .replace("Alpha.first()", "Widget.first()");
        let work = FLOW_BASE
            .replace("import * as Alpha from \"./widget\"\n", "")
            .replace("import * as Widget from \"./widget\"\n", "")
            .replace("Alpha.first()", "Widget.first()")
            .replace("Widget.second()", "Beta.second()");
        let v = check(
            &tree(&[("flow.ts", FLOW_BASE)]),
            &tree(&[("flow.ts", &ours)]),
            &tree(&[("flow.ts", &theirs)]),
            &tree(&[("flow.ts", &work)]),
            &["flow.ts".to_string()],
        );
        let dangling: Vec<_> = findings_of(&v, "flow.ts")
            .into_iter()
            .filter(|f| f.class == "DANGLING")
            .collect();
        assert_eq!(dangling.len(), 1, "{v:#?}");
        assert!(dangling[0].detail.contains("`Widget`"), "{v:#?}");
        assert!(!dangling[0].detail.contains("`Alpha`"), "{v:#?}");
        assert!(
            dangling[0]
                .suggestion
                .as_deref()
                .is_some_and(|s| s.contains("import * as Widget from \"./widget\"")),
            "{v:#?}"
        );
        // A faithful resolution that keeps the import is clean.
        let fixed = format!("import * as Widget from \"./widget\"\n{work}");
        let v = check(
            &tree(&[("flow.ts", FLOW_BASE)]),
            &tree(&[("flow.ts", &ours)]),
            &tree(&[("flow.ts", &theirs)]),
            &tree(&[("flow.ts", &fixed)]),
            &["flow.ts".to_string()],
        );
        assert!(v[0].ok(), "{v:#?}");
    }

    #[test]
    fn a_file_one_side_deleted_and_the_other_left_alone_is_not_a_finding() {
        let gone = "export function unused() {\n  return 1\n}\n";
        let v = check(
            &tree(&[("gone.ts", gone)]),
            &tree(&[("gone.ts", gone)]),
            &tree(&[]),
            &tree(&[]),
            &["gone.ts".to_string()],
        );
        assert!(findings_of(&v, "gone.ts").is_empty(), "{v:#?}");
        assert!(
            v[0].line()
                .starts_with("OK: gone.ts — not in the working tree"),
            "a deletion as asked still gets its line: {v:#?}"
        );
        // …but a deletion against a modification still is.
        let edited = gone.replace("return 1", "return 2");
        let v = check(
            &tree(&[("gone.ts", gone)]),
            &tree(&[("gone.ts", &edited)]),
            &tree(&[]),
            &tree(&[]),
            &["gone.ts".to_string()],
        );
        let f = findings_of(&v, "gone.ts");
        assert_eq!(f.len(), 1, "{v:#?}");
        assert!(f[0].detail.contains("modified"), "{v:#?}");
    }

    // ---- MODDEL -----------------------------------------------------------
    //
    // One file, `f.py`; one side deletes it, the other edits it. A caller
    // `main.py` that does or does not use it; for a move, the deleter's `g.py`.

    const F: &str =
        "def helper(x):\n    return x + 1\n\n\ndef compute(y):\n    return helper(y) * 2\n";
    const MAIN_NOUSE: &str = "print(3)\n";
    const MAIN_USES: &str = "from f import compute\n\nprint(compute(3))\n";
    /// Moved and restructured, with a docstring right above the line the
    /// other side edits: diff3 cannot place both.
    const G_TOUCHING: &str = "# moved and restructured\n\n\ndef helper(x):\n    '''Increment.'''\n    return x + 1\n\n\ndef compute(y):\n    '''Double of helper.'''\n    value = helper(y)\n    return value * 2\n";
    /// Moved and restructured, leaving `helper`'s body as it was.
    const G_MOVED: &str = "# moved and restructured\n\n\ndef helper(x):\n    return x + 1\n\n\ndef compute(y):\n    '''Double of helper.'''\n    value = helper(y)\n    return value * 2\n";

    fn f_edit() -> String {
        F.replace("return x + 1", "return x + 2  # fixed off-by-one")
    }

    /// ours deletes `f.py` (and may write `ours_extra`); theirs edits it. The
    /// resolution on disk is `work`.
    fn moddel_check(
        main: &str,
        theirs_f: &str,
        ours_extra: &[(&str, &str)],
        work: &[(&str, &str)],
    ) -> Vec<Verdict> {
        let base = tree(&[("f.py", F), ("main.py", main)]);
        let mut o = vec![("main.py", main)];
        o.extend_from_slice(ours_extra);
        let ours = tree(&o);
        let theirs = tree(&[("f.py", theirs_f), ("main.py", main)]);
        let mut subjects = vec!["f.py".to_string()];
        subjects.extend(ours_extra.iter().map(|(p, _)| p.to_string()));
        check(&base, &ours, &theirs, &tree(work), &subjects)
    }

    fn verdict<'a>(v: &'a [Verdict], file: &str) -> &'a Verdict {
        v.iter()
            .find(|x| x.file == file)
            .expect("a verdict for every subject")
    }

    #[test]
    fn moddel_delete_that_loses_an_edit_is_a_finding() {
        let edit = f_edit();
        let v = moddel_check(MAIN_NOUSE, &edit, &[], &[("main.py", MAIN_NOUSE)]);
        let f = &verdict(&v, "f.py").findings;
        assert_eq!(f.len(), 1, "{v:#?}");
        assert_eq!(f[0].class, "MODDEL");
        assert!(f[0].detail.contains("exists nowhere else"), "{v:#?}");
    }

    #[test]
    fn moddel_delete_of_a_layout_only_edit_is_an_advisory() {
        let reflowed = F
            .replace("return x + 1", "return  x+1")
            .replace("\n\n\n", "\n\n\n\n");
        let v = moddel_check(MAIN_NOUSE, &reflowed, &[], &[("main.py", MAIN_NOUSE)]);
        let f = verdict(&v, "f.py");
        assert!(f.ok(), "{v:#?}");
        assert_eq!(f.advisories.len(), 1);
        assert_eq!(f.advisories[0].class, "MODDEL");
        // A comment is content: the same change with a comment is not layout.
        let commented = F.replace("def helper(x):", "def helper(x):  # adds one");
        let v = moddel_check(MAIN_NOUSE, &commented, &[], &[("main.py", MAIN_NOUSE)]);
        assert!(!verdict(&v, "f.py").ok(), "{v:#?}");
    }

    #[test]
    fn moddel_delete_whose_names_are_still_called_is_a_finding() {
        let reflowed = F.replace("return x + 1", "return  x+1");
        let v = moddel_check(MAIN_USES, &reflowed, &[], &[("main.py", MAIN_USES)]);
        let f = &verdict(&v, "f.py").findings;
        assert_eq!(f.len(), 1, "{v:#?}");
        assert_eq!(f[0].class, "MODDEL");
        assert!(f[0].detail.contains("`compute`"), "{v:#?}");
    }

    #[test]
    fn moddel_delete_after_a_move_that_carries_the_edit_is_an_advisory() {
        let edit = f_edit();
        let ported = G_MOVED.replace("return x + 1", "return x + 2  # fixed off-by-one");
        let v = moddel_check(
            MAIN_NOUSE,
            &edit,
            &[("g.py", G_MOVED)],
            &[("main.py", MAIN_NOUSE), ("g.py", &ported)],
        );
        let f = verdict(&v, "f.py");
        assert!(f.ok(), "{v:#?}");
        assert!(f.advisories[0].detail.contains("`g.py`"), "{v:#?}");
    }

    #[test]
    fn moddel_delete_after_a_move_that_dropped_the_edit_offers_the_port() {
        let edit = f_edit();
        let v = moddel_check(
            MAIN_NOUSE,
            &edit,
            &[("g.py", G_MOVED)],
            &[("main.py", MAIN_NOUSE), ("g.py", G_MOVED)],
        );
        let f = &verdict(&v, "f.py").findings;
        assert_eq!(f.len(), 1, "{v:#?}");
        assert_eq!(f[0].class, "MODDEL");
        let s = f[0].suggestion.as_deref().unwrap();
        assert!(s.contains("--- a/g.py"), "{s}");
        assert!(s.contains("+    return x + 2  # fixed off-by-one"), "{s}");
    }

    #[test]
    fn moddel_delete_after_a_move_the_edit_cannot_follow_is_a_finding() {
        let edit = f_edit();
        let v = moddel_check(
            MAIN_NOUSE,
            &edit,
            &[("g.py", G_TOUCHING)],
            &[("main.py", MAIN_NOUSE), ("g.py", G_TOUCHING)],
        );
        let f = &verdict(&v, "f.py").findings;
        assert_eq!(f.len(), 1, "{v:#?}");
        assert!(f[0].detail.contains("does not re-apply"), "{v:#?}");
    }

    #[test]
    fn moddel_keep_after_a_move_duplicates_the_definitions() {
        let edit = f_edit();
        let v = moddel_check(
            MAIN_NOUSE,
            &edit,
            &[("g.py", G_MOVED)],
            &[("main.py", MAIN_NOUSE), ("g.py", G_MOVED), ("f.py", &edit)],
        );
        let f = &verdict(&v, "f.py").findings;
        assert_eq!(f.len(), 1, "{v:#?}");
        assert_eq!(f[0].class, "MODDEL");
        assert!(f[0].detail.contains("`compute`"), "{v:#?}");
    }

    #[test]
    fn moddel_keep_without_a_move_is_an_advisory() {
        let edit = f_edit();
        let v = moddel_check(
            MAIN_NOUSE,
            &edit,
            &[],
            &[("main.py", MAIN_NOUSE), ("f.py", &edit)],
        );
        let f = verdict(&v, "f.py");
        assert!(f.ok(), "{v:#?}");
        assert_eq!(f.advisories.len(), 1);
        assert_eq!(f.advisories[0].class, "MODDEL");
        assert!(f.line().starts_with("OK: f.py"), "{v:#?}");
    }

    #[test]
    fn a_file_git_relocated_into_a_renamed_directory_is_checked_against_its_adder() {
        // theirs added `old/job.py`; ours renamed `old/` to `new/`; git wrote
        // the addition at `new/job.py`. Lines it states twice are its own.
        let job =
            "@staticmethod\ndef a():\n    return 1\n\n@staticmethod\ndef b():\n    return 2\n";
        let v = check(
            &tree(&[]),
            &tree(&[]),
            &tree(&[("old/job.py", job)]),
            &tree(&[("new/job.py", job)]),
            &["new/job.py".to_string(), "old/job.py".to_string()],
        );
        assert!(v.iter().all(Verdict::ok), "{v:#?}");
        assert!(
            verdict(&v, "old/job.py").line().contains("`new/job.py`"),
            "{v:#?}"
        );
    }

    #[test]
    fn a_local_const_arrow_resolves_the_name() {
        let base = tree(&[
            ("a.ts", "export function openThing() {\n  return 1\n}\n"),
            (
                "b.ts",
                "export function outer() {\n  const openThing = (n: number): number => {\n    return n\n  }\n  return openThing(1)\n}\n",
            ),
        ]);
        let ours = tree(&[
            ("a.ts", "export function other() {\n  return 2\n}\n"),
            (
                "b.ts",
                "export function outer() {\n  const openThing = (n: number): number => {\n    return n\n  }\n  return openThing(1)\n}\n",
            ),
        ]);
        let theirs = tree(&[
            ("a.ts", "export function openThing() {\n  return 1\n}\n"),
            (
                "b.ts",
                "export function outer() {\n  const openThing = (n: number): number => {\n    return n + 1\n  }\n  return openThing(1)\n}\n",
            ),
        ]);
        let work = tree(&[
            ("a.ts", "export function other() {\n  return 2\n}\n"),
            (
                "b.ts",
                "export function outer() {\n  const openThing = (n: number): number => {\n    return n + 1\n  }\n  return openThing(1)\n}\n",
            ),
        ]);
        let v = check(
            &base,
            &ours,
            &theirs,
            &work,
            &["a.ts".to_string(), "b.ts".to_string()],
        );
        assert!(findings_of(&v, "b.ts").is_empty(), "{v:#?}");
    }

    #[test]
    fn prose_is_not_checked_for_references() {
        let base = tree(&[
            (
                "util.ts",
                "export function clamp(n: number) {\n  return n\n}\n",
            ),
            ("notes.md", "# Notes\n\nWe clamp (roughly) the total.\n"),
        ]);
        let ours = tree(&[
            (
                "util.ts",
                "export function floor(n: number) {\n  return n\n}\n",
            ),
            ("notes.md", "# Notes\n\nWe clamp (roughly) the total.\n"),
        ]);
        let theirs = tree(&[
            (
                "util.ts",
                "export function clamp(n: number) {\n  return n\n}\n",
            ),
            (
                "notes.md",
                "# Notes\n\nWe clamp (roughly) the total, always.\n",
            ),
        ]);
        let work = tree(&[
            (
                "util.ts",
                "export function floor(n: number) {\n  return n\n}\n",
            ),
            (
                "notes.md",
                "# Notes\n\nWe clamp (roughly) the total, always.\n",
            ),
        ]);
        let v = check(
            &base,
            &ours,
            &theirs,
            &work,
            &["notes.md".to_string(), "util.ts".to_string()],
        );
        assert!(findings_of(&v, "notes.md").is_empty(), "{v:#?}");
    }

    #[test]
    fn line_counts_follow_a_rename() {
        let base_text =
            "export function* widgetFlow() {\n  yield* widget.open(gadget.name)\n  return 0\n}\n";
        let ours_text = "export function* widgetFlow() {\n  yield* widget.open(gadget.name)\n  yield* widget.open(gadget.name)\n  yield* widget.open(gadget.name)\n  return 0\n}\n";
        let v = check(
            &tree(&[("old.ts", base_text)]),
            &tree(&[("old.ts", ours_text)]),
            &tree(&[("new.ts", base_text)]),
            &tree(&[("new.ts", ours_text)]),
            &["new.ts".to_string(), "old.ts".to_string()],
        );
        assert!(findings_of(&v, "new.ts").is_empty(), "{v:#?}");
        assert!(findings_of(&v, "old.ts").is_empty(), "{v:#?}");
    }

    #[test]
    fn a_report_with_no_files_says_so_in_a_sentence_and_never_prints_nothing() {
        let r = Report {
            scope: "no merge in progress".to_string(),
            verdicts: Vec::new(),
        };
        let text = r.render();
        assert!(text.contains("NOTHING WAS VERIFIED"), "{text}");
        assert!(!text.trim().is_empty());
        assert!(!text.trim().starts_with('['));
    }
}
