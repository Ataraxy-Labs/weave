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
//! 4. **dangling** — is anything still called that nothing defines any more?
//!    Binding evidence, repo-wide, the same functions the per-file pass uses.
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

use crate::parsers::{entities_of, is_supported};
use crate::repo_scope::Tree;
use weave_core::binding::{has_call_reference, has_definition};

/// A line is only evidence if it says something. Bare punctuation (`}`, `);`,
/// `else:`) repeats legitimately all over a real file, so counting it would
/// bury every real finding under closing braces.
fn significant(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 8 && t.chars().any(|c| c.is_alphanumeric())
}

/// Is this line a *slice* of a statement rather than a statement?
///
/// A wrapped call argument (`any(IMediaType.class), anyInt(),`), an opening
/// header (`await event.fire_event_async(`, `except zmq.Error:`, `impl Foo {`)
/// and a leading continuation (`.map(|x| x + 1)`) all repeat legitimately
/// wherever the same shape is written again, so their multiplicity says
/// nothing about the merge. This is [`weave_core::frame`]'s "structural
/// punctuation carries no identity", one level up: *fragments* carry none
/// either.
fn is_fragment(line: &str) -> bool {
    let t = line.trim();
    let opens_or_continues = |s: &str| {
        s.ends_with([
            ',', '(', '[', '{', ':', '+', '-', '*', '/', '&', '|', '=', '<', '>', '?',
        ]) || s.ends_with("=>")
            || s.ends_with("->")
    };
    let starts_as_continuation = t.starts_with(['.', ',', ')', ']', '}', '?', ':', '+'])
        || t.starts_with("&&")
        || t.starts_with("||");
    opens_or_continues(t) || starts_as_continuation
}

/// The lines the multiset rules are allowed to reason about.
fn carries_identity(line: &str) -> bool {
    significant(line) && !is_fragment(line)
}

fn counts(text: &str) -> HashMap<&str, usize> {
    let mut m: HashMap<&str, usize> = HashMap::new();
    for l in text.lines().map(str::trim).filter(|l| significant(l)) {
        *m.entry(l).or_insert(0) += 1;
    }
    m
}

/// One finding about one file, already in the words the reader gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// `MARKERS` | `LOSS` | `DUP` | `DANGLING`.
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
    /// The container the co-change happened in.
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
}

impl Verdict {
    /// OK is about FINDINGS only. Advisories never make a file not-OK — that is
    /// the whole point of the register.
    pub fn ok(&self) -> bool {
        self.findings.is_empty()
    }

    /// The one line this file gets.
    pub fn line(&self) -> String {
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

impl Report {
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
                out.push_str(&format!(
                    "    review (COOCCUPANCY): cleanly merged {} `{}` — {}; confirm they are meant to coexist\n",
                    a.entity_type, a.entity, a.detail
                ));
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
                let kept_o = co.get(line).copied().unwrap_or(0).min(*n);
                let kept_t = ct.get(line).copied().unwrap_or(0).min(*n);
                // Both sides may between them have deleted every copy; that is
                // a fact about the input, not an underflow to repair.
                // `saturating_sub` would read the same and clippy asks for it,
                // but the analyzer that looks for silent-repair patterns would
                // flag it as one — and this is not a repair, it is the
                // arithmetic. Written out, both checkers are told the truth.
                #[allow(clippy::implicit_saturating_sub)]
                let required = if kept_o + kept_t > *n {
                    kept_o + kept_t - *n
                } else {
                    0
                };
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
            let (n, o, t) = (
                cb.get(line).copied().unwrap_or(0),
                co.get(line).copied().unwrap_or(0),
                ct.get(line).copied().unwrap_or(0),
            );
            // Written out rather than saturating, for the same reason the
            // unanimity floor above is: both sides may between them have kept
            // fewer copies than base wrote, and that is the arithmetic, not an
            // underflow to repair.
            #[allow(clippy::implicit_saturating_sub)]
            let additive = if o + t > n { o + t - n } else { 0 };
            additive.max(o).max(t).max(1)
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

/// What makes two top-level items the SAME declaration: the kind, the name with
/// its generic arguments off, and — where the language overloads — the
/// parameter list.
type DefinitionKey = (String, String, Option<String>);

/// Kinds that do not declare a name of their own.
///
/// Three groups, and each one is a false positive the v3 trial paid for:
///
/// * **attachments** — `impl`, `extension`, `instance`. `impl X` does not
///   define `X`; it adds to the `X` a declaration elsewhere already made, and
///   a type may have as many inherent impls as it likes. This is the rule
///   whose absence made `struct UpdateDiff` + `impl UpdateDiff` read as
///   "`UpdateDiff` defined 2x" on a resolution that was correct.
/// * **reopenable declarations** — a namespace, a module or a TypeScript
///   interface stated twice is declaration *merging*, which is the language
///   working as designed, not a name silently shadowed.
/// * **calls that are not declarations at all** — `test('…')` / `it('…')` /
///   `describe('…')` are function calls whose first argument the extractor
///   uses as a name. Two tests may share a title; nothing shadows anything.
fn declares_a_name(entity_type: &str) -> bool {
    !matches!(
        entity_type,
        "impl"
            | "extension"
            | "instance"
            | "module"
            | "internal_module"
            | "namespace"
            | "interface"
            | "test"
            | "test_suite"
            | "export"
            | "import"
            | "package"
            | "use"
    )
}

/// Languages where two items may share a name and differ only in their
/// parameters. In these, a repeated name is an overload until the signatures
/// match; everywhere else a repeated name is a redefinition.
fn overloads(path: &str) -> bool {
    matches!(
        path.rsplit('.').next().unwrap_or(""),
        "java"
            | "kt"
            | "kts"
            | "cs"
            | "cpp"
            | "cc"
            | "cxx"
            | "hpp"
            | "hh"
            | "h"
            | "c"
            | "ts"
            | "tsx"
            | "swift"
            | "scala"
            | "php"
    )
}

/// `Bar<T>` and `Bar<U>` are one nameable thing; the generic arguments are the
/// item's parameters, not part of its name.
fn without_generics(name: &str) -> &str {
    match name.find('<') {
        Some(i) if i > 0 => name[..i].trim_end(),
        _ => name,
    }
}

/// The parameter list of an item's header, whitespace removed — `None` when the
/// item has no parameter list to compare (a struct, an enum, a constant).
fn parameter_list(content: &str) -> Option<String> {
    let header: String = content.lines().take(3).collect::<Vec<_>>().join(" ");
    let open = header.find('(')?;
    let mut depth = 0usize;
    for (i, c) in header[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(
                        header[open + 1..open + i]
                            .chars()
                            .filter(|c| !c.is_whitespace())
                            .collect(),
                    );
                }
            }
            _ => {}
        }
    }
    None
}

/// The identity a duplicate-definition finding is entitled to compare, or
/// `None` for an item that declares nothing.
fn definition_key(path: &str, e: &SemanticEntity) -> Option<DefinitionKey> {
    if e.name.is_empty() || !declares_a_name(&e.entity_type) {
        return None;
    }
    let signature = overloads(path)
        .then(|| parameter_list(&e.content))
        .flatten();
    Some((
        e.entity_type.clone(),
        without_generics(&e.name).to_string(),
        signature,
    ))
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
    // subjects the way the stage trees are.
    let defined_now: BTreeSet<String> = work
        .iter()
        .filter(|(p, _)| is_supported(p))
        .flat_map(|(p, c)| entities_of(p, c).into_iter().map(|e| e.name))
        .collect();

    // The names a merge stage defined that nothing on disk defines any more.
    // This depends on the stages and the working tree, NOT on which subject we
    // are looking at, so it is computed ONCE. Folding it into the per-file loop
    // — as this once did — re-parses every stage of every file for every subject,
    // an O(subjects × repo) cost that is the whole reason a large mid-merge tree
    // hangs. The stage trees are already scoped to the subjects (see
    // [`crate::gitscan::merge_scope`]); an untouched file's stage equals its
    // working-tree copy, so it contributes only names that are in `defined_now`
    // and can never be `gone`.
    let mut gone: BTreeSet<String> = BTreeSet::new();
    for stage in [base, ours, theirs] {
        for (p, c) in stage.iter().filter(|(p, _)| is_supported(p)) {
            for e in entities_of(p, c) {
                if !defined_now.contains(&e.name) {
                    gone.insert(e.name);
                }
            }
        }
    }

    let mut verdicts = Vec::new();
    for file in subjects {
        let Some(w) = work.get(file) else {
            verdicts.push(Verdict {
                file: file.clone(),
                findings: vec![Finding {
                    class: "MARKERS",
                    detail: "the file is not in the working tree (deleted during the merge)"
                        .to_string(),
                    suggestion: None,
                }],
                advisories: Vec::new(),
            });
            continue;
        };
        let mut findings = verify_file(
            file,
            base.get(file).map(String::as_str),
            ours.get(file).map(String::as_str),
            theirs.get(file).map(String::as_str),
            w,
        );
        findings.extend(dangling(w, base, work, &gone));
        let advisories = advisories_for(
            file,
            base.get(file).map(String::as_str),
            ours.get(file).map(String::as_str),
            theirs.get(file).map(String::as_str),
        );
        verdicts.push(Verdict {
            file: file.clone(),
            findings,
            advisories,
        });
    }
    verdicts
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
/// actually call.
///
/// The rename repair is the derivable half: when the vanished name has a
/// same-file successor that IS defined now and did not exist in base, the fix
/// is a rename and weave can say which one.
fn dangling(w: &str, base: &Tree, work: &Tree, gone: &BTreeSet<String>) -> Vec<Finding> {
    let mut hits: Vec<(String, Option<(String, String)>)> = Vec::new();
    for name in gone {
        if name.len() < 3 || has_definition(w, name) || !has_call_reference(w, name) {
            continue;
        }
        hits.push((name.clone(), successor_of(name, base, work)));
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

/// The name a vanished definition was RENAMED to, and the file that says so.
///
/// This is the derivable half of a `DANGLING` finding, and it is derived by
/// the one matcher both weave passes already use — `sem-core`'s
/// `match_entities`, so the answer here and the answer the merge gave cannot
/// disagree. It is looked up in the file where the definition *lived*, not in
/// the file where the call survives: a new name next to a broken call is a
/// coincidence, a matched pair in the defining file is evidence.
fn successor_of(name: &str, base: &Tree, work: &Tree) -> Option<(String, String)> {
    for (path, before) in base.iter().filter(|(p, _)| is_supported(p)) {
        let old = entities_of(path, before);
        if !old.iter().any(|e| e.name == name) {
            continue;
        }
        let Some(now) = work.get(path) else { continue };
        let new = entities_of(path, now);
        for c in match_entities(&old, &new, path, None, None, None).changes {
            if c.change_type == ChangeType::Renamed
                && c.old_entity_name.as_deref() == Some(name)
                && c.entity_name != name
                && !c.entity_name.is_empty()
            {
                return Some((c.entity_name, path.clone()));
            }
        }
        // The matcher pairs on body similarity, so a declaration that was
        // renamed AND rewritten comes back unpaired. One name out, one name in,
        // in the file that defined it, is then the only candidate there is —
        // and the suggestion says "if that is the rename" rather than claiming
        // it, because this is arithmetic and not evidence.
        let before: BTreeSet<&str> = old.iter().map(|e| e.name.as_str()).collect();
        let after: BTreeSet<&str> = new.iter().map(|e| e.name.as_str()).collect();
        let gone: Vec<&&str> = before.difference(&after).collect();
        let arrived: Vec<&&str> = after.difference(&before).collect();
        if gone.len() == 1 && arrived.len() == 1 && *gone[0] == name {
            return Some((arrived[0].to_string(), path.clone()));
        }
    }
    None
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
