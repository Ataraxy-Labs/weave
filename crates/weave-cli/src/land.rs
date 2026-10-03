//! `weave land` — land a merge, and say for every file git could not merge how
//! much is known about the result.
//!
//! Three labels, strongest first:
//!
//! * **PROVEN** — weave merged the file cleanly AND the independent merge
//!   certificate (`weave-certify`, which shares no merge code with weave)
//!   shows the result is the three-way selection of base, ours and theirs.
//! * **VERIFIED** — the file's answer passed the exact [`gate`], and it came
//!   from one of two places: an external resolver (any program; see
//!   [`Resolver`]) wrote it, or weave's element union did — a clean merge
//!   whose both-changed regions the certificate admits only with the
//!   `elem_union` check ([`UNION_ALLOWANCES`]). No resolver is called for the
//!   second; see [`land_unit`] for why it is VERIFIED and not PROVEN.
//! * **REFUSED** — nothing above holds. The file keeps its conflict markers and
//!   stays unmerged in the index; the resolver's answer is never written.
//!
//! In a blind evaluation, a cheap model's accepted merges behind this gate were
//! wrong 9.5% of the time, against 22.0% without it — plus one rule added
//! since, DROPPED, after an answer that kept only one side of a conflict block
//! passed it. Every check that reads `weave check` is asked of weave's own
//! verifier here ([`crate::worktree::check`]) rather than restated.
//!
//! Scope: the files git's line merge conflicts on (content, add/add,
//! modify/delete), and every other file both sides changed whose merge — the
//! answer in the tree, else git's line merge — is not git's line merge or
//! fails weave's own merge check ([`weave_core::verify::verify`]: a line both
//! kept lost, a line or a `case` label / map key stated twice). The rest are
//! counted as `not_examined`: git merged them line-cleanly and the check
//! passed. An answer the tree already holds (a file the merge driver
//! resolved; `--result`'s file) is judged before any resolver. Each file
//! is checked on its own three stages, as the gate was measured; a cross-file
//! effect (a caller in another file) is outside it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use weave_core::host::Host;
use weave_core::verify::{line_counts, significant, unanimity_floor};
use weave_core::{entity_merge_fmt, MarkerFormat};

use crate::gitscan::{self, Body, Entry};
use crate::parsers::{is_supported, REGISTRY};
use crate::repo_scope::Tree;
use crate::worktree;

type R<T> = Result<T, Box<dyn std::error::Error>>;

/// The certificate's rule set, v2: a both-changed region is certified only by
/// one of these allowances. v1 had `imp_strict` where v2 has `imp_used` —
/// `imp_strict` plus a check that every import line one side added that the
/// merge keeps binds a name the merged file uses.
pub const PROOF_ALLOWANCES: [&str; 3] = ["imp_used", "subsume_ins", "nest"];

/// What else may admit a both-changed region of weave's clean merge, for a
/// VERIFIED landing (never PROVEN): `elem_union`, the certificate's own check
/// of an element union (`weave_certify::elem`), and `nest_eu`, `nest` with
/// children it admits. Not in the proof rule set, so a file that
/// needs one of these must also pass the gate.
pub const UNION_ALLOWANCES: [&str; 2] = ["elem_union", "nest_eu"];

/// Version tag of the JSON report / certificate document.
pub const SCHEMA: &str = "weave-land/1";

// ------------------------------------------------------------------ the unit

/// How git's merge left the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Kind {
    /// Both sides edited it and git's line merge conflicts.
    #[serde(rename = "content")]
    Content,
    /// Both sides created it, differently.
    #[serde(rename = "add/add")]
    AddAdd,
    /// One side deleted it; the other modified it.
    #[serde(rename = "modify/delete")]
    ModifyDelete,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Content => "content",
            Kind::AddAdd => "add/add",
            Kind::ModifyDelete => "modify/delete",
        }
    }
}

/// One file git could not merge: its three stages (absent = `None`) and git's
/// own line merge of it, conflict markers and all.
#[derive(Debug, Clone)]
pub struct Unit {
    pub path: String,
    pub kind: Kind,
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    /// `git merge-file` output (plain `merge` style); `None` for
    /// modify/delete, where git writes no markers. For a file git merges
    /// line-cleanly that is a unit anyway (see [`plan`]), git's clean text.
    pub gitmerged: Option<String>,
    /// The answer the tree already holds, when there is one to judge: in
    /// working-tree mode a file the merge driver resolved (stage 0 in the
    /// index), with `--result <rev>` the file at that rev. It is labelled
    /// before any resolver is asked: PROVEN when it is weave's certified
    /// merge, VERIFIED when it passes the [`gate`].
    pub present: Option<Candidate>,
    /// git merged the file line-cleanly, and that merge is known bad: it
    /// failed weave's merge check, or the merge driver refused the file. An
    /// answer then has to differ from git's somewhere (to state `case 3`
    /// once, say), so the gate does not hold it to every line of git's merge
    /// (AUTOMERGED); it holds it to both sides' changes, the whole file read
    /// as one block (DROPPED / UNDELETED).
    pub unverified_git_merge: bool,
}

impl Unit {
    fn sides(&self) -> Vec<&str> {
        [self.ours.as_deref(), self.theirs.as_deref()]
            .into_iter()
            .flatten()
            .collect()
    }
}

/// A proposed resolution of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidate {
    File(String),
    Delete,
}

// ------------------------------------------------------------------ the gate

/// One reason the gate gave, in the words fed back to a resolver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateFinding {
    pub class: String,
    pub detail: String,
}

/// The gate's answer: it passes iff `reasons` is empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GateVerdict {
    /// `MALFORMED` | `MARKERS` | `PARSE` | `WEAVE` | `WEAVE_UNVERIFIED` |
    /// `LINES` | `AUTOMERGED` | `DROPPED`.
    pub reasons: Vec<&'static str>,
    pub findings: Vec<GateFinding>,
}

impl GateVerdict {
    pub fn pass(&self) -> bool {
        self.reasons.is_empty()
    }

    fn fail(&mut self, reason: &'static str, class: &str, detail: impl Into<String>) {
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
        self.findings.push(GateFinding {
            class: class.to_string(),
            detail: detail.into(),
        });
    }

    /// What a retry is told: every finding, or the bare reasons when a check
    /// failed without one.
    pub fn feedback(&self) -> Vec<String> {
        let lines: Vec<String> = self
            .findings
            .iter()
            .map(|f| format!("{}: {}", f.class, f.detail))
            .collect();
        if lines.is_empty() {
            self.reasons.iter().map(|r| r.to_string()).collect()
        } else {
            lines
        }
    }

    fn malformed(why: &str) -> GateVerdict {
        let mut v = GateVerdict::default();
        v.fail("MALFORMED", "MALFORMED", why);
        v
    }
}

/// The exact gate: check `cand` against the file's three stages.
///
/// 1. **MARKERS** — a conflict-marker line kind neither side has.
/// 2. **PARSE** — the candidate must parse (tree-sitter) when every present
///    side does.
/// 3. **WEAVE** — `weave check`'s findings on this file (markers, loss,
///    duplicates, dangling names, duplicate data keys, a data file that no
///    longer loads, modify/delete), minus duplicate/dangling names ours or
///    theirs already trigger when taken as the resolution. A crash is
///    `WEAVE_UNVERIFIED`, never a pass.
/// 4. **LINES** — for a file weave has no grammar for, weave check's line
///    rules (loss, duplication) alone.
/// 5. **AUTOMERGED** — every line git merged automatically outside the
///    conflict blocks, that base does not already have, is still there.
/// 6. **DROPPED** — inside the conflict blocks, each side's change relative to
///    base survives: every line one side added there is still there, and
///    every line one side deleted there has not come back (see [`dropped`]).
///    A resolution that keeps one side of a block verbatim fails this unless
///    the other side's change is subsumed by it.
pub fn gate(u: &Unit, cand: &Candidate) -> GateVerdict {
    gate_with(u, cand, false)
}

/// Whether a `weave check` finding is the line-multiplicity rule (a line
/// stated more often than any version states it).
fn is_line_duplication(f: &worktree::Finding) -> bool {
    f.class == "DUP"
        && f.detail
            .contains("line(s) appear more often than any version")
}

/// [`gate`]; with `merge_result`, the answer is a merge as committed — it may
/// carry the committer's own new code beside the merge (a test table with
/// one more `want any` row), so the line-multiplicity rule, which reads any
/// extra copy of a line as a resolution stating it twice, is not asked. Keys
/// stated twice, definitions stated twice, lost lines and both sides'
/// changes still are.
fn gate_with(u: &Unit, cand: &Candidate, merge_result: bool) -> GateVerdict {
    let mut v = GateVerdict::default();
    if let Candidate::File(c) = cand {
        let theirs_or_ours: BTreeSet<&str> = u.sides().into_iter().flat_map(markers).collect();
        if markers(c).iter().any(|m| !theirs_or_ours.contains(m)) {
            v.fail(
                "MARKERS",
                "MARKERS",
                "conflict marker lines are still present in the file",
            );
        }
        let sides = u.sides();
        if !sides.is_empty()
            && sides.iter().all(|s| parses(&u.path, s) == Some(true))
            && parses(&u.path, c) == Some(false)
        {
            v.fail(
                "PARSE",
                "PARSE",
                "the file does not parse (syntax error), although both sides parse",
            );
        }
    }

    let work = match cand {
        Candidate::File(c) => Some(c.as_str()),
        Candidate::Delete => None,
    };
    if is_supported(&u.path) {
        match weave_check(u, work) {
            Err(why) => v.fail("WEAVE_UNVERIFIED", "UNVERIFIED", why),
            Ok(found) if found.is_empty() => {}
            Ok(found) => {
                let pre = preexisting(u);
                for f in found {
                    if merge_result && is_line_duplication(&f) {
                        continue;
                    }
                    let names = backticked(&f.detail);
                    let key = (f.class.to_string(), finding_pattern(&f.detail));
                    let already = matches!(f.class, "DUP" | "DANGLING")
                        && !names.is_empty()
                        && names
                            .iter()
                            .all(|n| pre.contains(&(key.clone(), n.clone())));
                    if !already {
                        v.fail("WEAVE", f.class, f.detail);
                    }
                }
            }
        }
    } else if let Some(c) = work {
        // No grammar: weave check's line rules are the whole structural answer.
        if let Ok(found) = weave_check(u, Some(c)) {
            for f in found
                .into_iter()
                .filter(|f| matches!(f.class, "LOSS" | "DUP"))
                .filter(|f| !(merge_result && is_line_duplication(f)))
            {
                v.fail("LINES", f.class, f.detail);
            }
        }
    }

    if let Candidate::File(c) = cand {
        let automerged = if u.unverified_git_merge {
            None
        } else {
            automerged(u.base.as_deref(), u.gitmerged.as_deref(), c)
        };
        if let Some(detail) = automerged {
            v.fail("AUTOMERGED", "AUTOMERGED", detail);
        }
        if let (Some(o), Some(t)) = (u.ours.as_deref(), u.theirs.as_deref()) {
            let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dropped(
                    u.base.as_deref().unwrap_or(""),
                    o,
                    t,
                    c,
                    u.unverified_git_merge,
                )
                .map_err(|e| e.to_string())
            }))
            .unwrap_or_else(|_| Err("the check crashed on this file".into()));
            match run {
                Ok(found) => {
                    for f in found {
                        v.fail("DROPPED", &f.class, f.detail);
                    }
                }
                Err(why) => v.fail(
                    "DROPPED",
                    "UNVERIFIED",
                    format!("the conflict blocks could not be read: {why}"),
                ),
            }
        }
    }
    v
}

/// The marker-line kinds present in `text` (`<<<<<<<`, `=======`, `>>>>>>>`,
/// `|||||||` at the start of a line).
fn markers(text: &str) -> BTreeSet<&'static str> {
    const KINDS: [&str; 4] = ["<<<<<<<", "=======", ">>>>>>>", "|||||||"];
    text.split('\n')
        .filter_map(|l| KINDS.iter().find(|k| l.starts_with(**k)).copied())
        .collect()
}

/// `Some(true)` = parses without an error node, `None` = no grammar.
fn parses(path: &str, text: &str) -> Option<bool> {
    let (_, tree) = REGISTRY.extract_entities_with_tree(path, text)?;
    tree.map(|t| !t.root_node().has_error())
}

/// `weave check`'s findings on this one file, with only its three stages in
/// scope — the minimal repository the gate was measured with. `work = None`
/// is the file deleted.
fn weave_check(u: &Unit, work: Option<&str>) -> Result<Vec<worktree::Finding>, String> {
    let tree = |t: Option<&str>| -> Tree {
        t.map(|c| (u.path.clone(), c.to_string()))
            .into_iter()
            .collect()
    };
    let (b, o, t, w) = (
        tree(u.base.as_deref()),
        tree(u.ours.as_deref()),
        tree(u.theirs.as_deref()),
        tree(work),
    );
    let subjects = [u.path.clone()];
    let verdicts = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        worktree::check(&b, &o, &t, &w, &subjects)
    }))
    .map_err(|_| "weave check crashed on this file".to_string())?;
    verdicts
        .into_iter()
        .find(|v| v.file == u.path)
        .map(|v| v.findings)
        .ok_or_else(|| "weave check gave no verdict for this file".to_string())
}

type NameKey = ((String, String), String);

/// Names ours or theirs already trigger a duplicate/dangling finding for when
/// taken as the resolution: pre-existing, not evidence against a resolution.
fn preexisting(u: &Unit) -> BTreeSet<NameKey> {
    let mut pre = BTreeSet::new();
    for side in u.sides() {
        for f in weave_check(u, Some(side)).unwrap_or_default() {
            for n in backticked(&f.detail) {
                pre.insert(((f.class.to_string(), finding_pattern(&f.detail)), n));
            }
        }
    }
    pre
}

/// Every `` `…` `` span's contents, left to right.
fn backticked(detail: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = detail;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        out.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    out
}

/// A finding's shape with its names and numbers taken out, so two findings of
/// one rule about different names compare equal.
fn finding_pattern(detail: &str) -> String {
    let mut out = String::new();
    let mut rest = detail;
    loop {
        match rest.find('`') {
            Some(open) if rest[open + 1..].contains('`') => {
                out.push_str(&rest[..open]);
                let after = &rest[open + 1..];
                rest = &after[after.find('`').unwrap_or(0) + 1..];
            }
            _ => {
                out.push_str(rest);
                break;
            }
        }
    }
    out.chars().filter(|c| !c.is_ascii_digit()).collect()
}

/// The AUTOMERGED rule: a significant line git's merge placed outside every
/// conflict block, and that base does not already state as often, must be in
/// the candidate at least as often as git's merge states it there. For a
/// file git merged line-cleanly (a unit only because its merge failed weave's
/// merge check, or because the tree holds another answer) that is every line
/// of git's merge: each side's change must survive there too.
fn automerged(base: Option<&str>, gitmerged: Option<&str>, cand: &str) -> Option<String> {
    let gitmerged = gitmerged?;
    let mut depth = 0usize;
    let mut outside: HashMap<&str, usize> = HashMap::new();
    for l in gitmerged.lines() {
        if l.starts_with("<<<<<<<") {
            depth += 1;
        } else if l.starts_with(">>>>>>>") && depth > 0 {
            depth -= 1;
        } else if depth == 0 && significant(l) {
            *outside.entry(l.trim()).or_insert(0) += 1;
        }
    }
    let (cb, cw) = (line_counts(base.unwrap_or("")), line_counts(cand));
    let get = |m: &HashMap<&str, usize>, l: &str| m.get(l).copied().unwrap_or(0);
    let mut miss: Vec<(&str, usize)> = outside
        .iter()
        .filter(|(l, n)| **n > get(&cb, l) && get(&cw, l) < **n)
        .map(|(l, n)| (*l, n - get(&cw, l)))
        .collect();
    miss.sort();
    let first = miss.first()?;
    Some(format!(
        "{} line(s) that git merged automatically (outside the conflict blocks) are missing, \
         e.g. `{}`",
        miss.iter().map(|(_, n)| n).sum::<usize>(),
        first.0.chars().take(72).collect::<String>()
    ))
}

/// The DROPPED rule, over git's diff3 line merge of the file: each conflict
/// block, with its base section, is a region both sides changed. A faithful
/// resolution keeps BOTH changes. A change is measured in tokens (identifier,
/// number and word runs, and operator runs; brackets, commas and layout are
/// not tokens), so a resolver may re-wrap lines or fold two edits of one line
/// into one line, but may not leave a side's edit out:
///
/// * **DROPPED** — a token one side added in a block (more copies in its
///   section than in base's) must survive. Per block a token is owed
///   `max(added by ours, added by theirs)` times — the max, not the sum, is
///   what lets an identical change on both sides, or one side's change that
///   subsumes the other's, land once. Across the file the candidate must hold
///   it at least `outside + Σ owed` times (`outside` = its copies outside
///   every block).
/// * **UNDELETED** — a token a side deleted in a block must not come back:
///   the candidate may hold it at most `outside + Σ (base copies both sides
///   kept + owed)` times. This keeps an in-block delete-vs-modify (one side
///   removes what the other edits) refused whichever side is kept.
///
/// With `whole_file`, a merge git calls clean (no block at all) is read as
/// one block over the whole file — for a file whose clean git merge is known
/// bad, where each side's change must still survive in the answer.
///
/// `Ok(findings)`; empty = every block's two changes survive.
fn dropped(
    base: &str,
    ours: &str,
    theirs: &str,
    cand: &str,
    whole_file: bool,
) -> R<Vec<GateFinding>> {
    let mut blocks = diff3_blocks(base, ours, theirs)?;
    if blocks.regions.is_empty() {
        if !whole_file {
            return Ok(Vec::new());
        }
        let lines = |t: &str| t.lines().map(str::to_string).collect::<Vec<_>>();
        blocks = Diff3 {
            regions: vec![[lines(ours), lines(base), lines(theirs)]],
            outside: Vec::new(),
        };
    }
    let get = |m: &HashMap<&str, usize>, k: &str| m.get(k).copied().unwrap_or(0);

    #[derive(Default)]
    struct Tally<'a> {
        add: [usize; 2],
        owed: usize,
        kept: usize,
        deleted_by: [bool; 2],
        /// a line of each side's section that adds this token, for the finding
        line: [Option<&'a str>; 2],
    }
    let mut tally: BTreeMap<&str, Tally> = BTreeMap::new();
    for [o, b, t] in &blocks.regions {
        let (co, cb, ct) = (tokens_of(o), tokens_of(b), tokens_of(t));
        let keys: BTreeSet<&str> = co
            .keys()
            .chain(cb.keys())
            .chain(ct.keys())
            .copied()
            .collect();
        for k in keys {
            let (n, a, z) = (get(&cb, k), get(&co, k), get(&ct, k));
            let e = tally.entry(k).or_default();
            let (ao, at) = (a.saturating_sub(n), z.saturating_sub(n));
            e.add[0] += ao;
            e.add[1] += at;
            e.owed += ao.max(at);
            e.kept += unanimity_floor(n, a, z);
            e.deleted_by[0] |= a < n;
            e.deleted_by[1] |= z < n;
            for (side, (added, sec)) in [(ao, o), (at, t)].into_iter().enumerate() {
                if added > 0 && e.line[side].is_none() {
                    e.line[side] = sec
                        .iter()
                        .map(|l| l.trim())
                        .find(|l| tokens(l).any(|x| x == k));
                }
            }
        }
    }
    let outside = tokens_of(&blocks.outside);
    let whole = [cand];
    let cw = tokens_of(&whole);

    // whose addition is short: 0 = ours, 1 = theirs, 2 = both together
    let mut missing: [Vec<(&str, Option<&str>)>; 3] = Default::default();
    let mut back: Vec<(&str, &'static str)> = Vec::new();
    for (k, e) in &tally {
        let (out, found) = (get(&outside, k), get(&cw, k));
        if e.owed > 0 && found < out + e.owed {
            let short = |s: usize| e.add[s] > 0 && found < out + e.add[s];
            match (short(0), short(1)) {
                (true, false) => missing[0].push((k, e.line[0])),
                (false, true) => missing[1].push((k, e.line[1])),
                _ => missing[2].push((k, e.line[0].or(e.line[1]))),
            }
        }
        if (e.deleted_by[0] || e.deleted_by[1]) && found > out + e.kept + e.owed {
            back.push((
                k,
                match e.deleted_by {
                    [true, true] => "both sides",
                    [true, false] => "ours",
                    _ => "theirs",
                },
            ));
        }
    }

    // name the most telling tokens first: words before operators, long first
    for m in missing.iter_mut() {
        m.sort_by_key(|(k, _)| {
            let word = k
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
            (!word, std::cmp::Reverse(k.len()), *k)
        });
    }
    let clip = |l: &str| l.chars().take(72).collect::<String>();
    let names = |m: &[(&str, Option<&str>)]| {
        m.iter()
            .take(5)
            .map(|(k, _)| format!("`{k}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut out = Vec::new();
    for (side, who) in [(1usize, "theirs"), (0, "ours"), (2, "ours and theirs")] {
        let m = &missing[side];
        if m.is_empty() {
            continue;
        }
        let line = m[0]
            .1
            .map(|l| format!(" (from `{}`)", clip(l)))
            .unwrap_or_default();
        out.push(GateFinding {
            class: "DROPPED".into(),
            detail: format!(
                "{who} changed a conflict block and the resolution drops {}: {} token(s) {who} \
                 added there are missing, e.g. {}{line}; keep both sides' changes",
                if side == 2 {
                    "part of it"
                } else {
                    "that change"
                },
                m.len(),
                names(m),
            ),
        });
    }
    if !back.is_empty() {
        out.push(GateFinding {
            class: "UNDELETED".into(),
            detail: format!(
                "{} token(s) a side deleted inside a conflict block are back, e.g. `{}` \
                 (deleted by {}); keep both sides' changes",
                back.len(),
                back[0].0,
                back[0].1
            ),
        });
    }
    Ok(out)
}

/// git's diff3 merge of one file, split: every conflict block's three
/// sections `[ours, base, theirs]`, and every line outside the blocks.
struct Diff3 {
    regions: Vec<[Vec<String>; 3]>,
    outside: Vec<String>,
}

fn diff3_blocks(base: &str, ours: &str, theirs: &str) -> R<Diff3> {
    // A marker size no source line plausibly starts with, so the sides' own
    // marker-like lines are never read as structure.
    const SIZE: usize = 41;
    let merged = line_merge_with(
        base,
        ours,
        theirs,
        &["--diff3", &format!("--marker-size={SIZE}")],
    )?
    .1;
    // bytes, not chars: a marker is ASCII, and a source line need not be
    let marker = |l: &str, c: u8| {
        let b = l.as_bytes();
        b.len() >= SIZE
            && b[..SIZE].iter().all(|x| *x == c)
            && b.get(SIZE).is_none_or(|x| *x == b' ')
    };
    let mut d = Diff3 {
        regions: Vec::new(),
        outside: Vec::new(),
    };
    // 0 = outside, 1 = ours, 2 = base, 3 = theirs
    let mut section = 0usize;
    for l in merged.lines() {
        match section {
            0 if marker(l, b'<') => {
                section = 1;
                d.regions.push(Default::default());
            }
            1 if marker(l, b'|') => section = 2,
            1 | 2 if marker(l, b'=') => section = 3,
            3 if marker(l, b'>') => section = 0,
            0 => d.outside.push(l.to_string()),
            s => d.regions.last_mut().expect("in a block")[s - 1].push(l.to_string()),
        }
    }
    if section != 0 {
        return Err("git's diff3 merge has an unterminated conflict block".into());
    }
    Ok(d)
}

/// A change's tokens: word runs (letters, digits, `_`) and operator runs.
/// Brackets, commas, quotes, `:`, `;` and whitespace are not tokens, so
/// re-wrapping or re-punctuating a line changes nothing.
fn tokens(line: &str) -> impl Iterator<Item = &str> {
    const OPS: &str = "+-*/%=<>!&|^~@?";
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            1
        } else if OPS.contains(c) {
            2
        } else {
            0
        }
    };
    let mut out = Vec::new();
    let mut start: Option<(usize, u8)> = None;
    for (i, c) in line
        .char_indices()
        .chain(std::iter::once((line.len(), ' ')))
    {
        let k = class(c);
        match start {
            Some((s, sk)) if sk != k => {
                out.push(&line[s..i]);
                start = (k != 0).then_some((i, k));
            }
            None if k != 0 => start = Some((i, k)),
            _ => {}
        }
    }
    out.into_iter()
}

fn tokens_of<S: AsRef<str>>(lines: &[S]) -> HashMap<&str, usize> {
    let mut m: HashMap<&str, usize> = HashMap::new();
    for l in lines {
        for t in tokens(l.as_ref()) {
            *m.entry(t).or_insert(0) += 1;
        }
    }
    m
}

// ------------------------------------------------------------- the resolver

/// An external resolver: a shell command run once per attempt.
///
/// stdin is one JSON object: `path`, `kind`, `base`, `ours`, `theirs`,
/// `conflicted` (git's merge with markers; `null` for modify/delete),
/// `attempt` (1 or 2), `previous` (the rejected answer, or `null`) and
/// `findings` (why it was rejected). Absent sides are `null`.
///
/// stdout is the complete resolved file — or one line: `DELETE` / `KEEP`
/// (modify/delete only: delete the file / keep the modified side as it is) or
/// `CANNOT[: reason]`. A non-zero exit is a failed call.
#[derive(Debug, Clone)]
pub struct Resolver {
    pub command: String,
    pub timeout: Duration,
}

/// What the resolver said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    File(String),
    Delete,
    Keep,
    Cannot(String),
    /// Exited non-zero, timed out, or wrote something that is not UTF-8.
    Failed(String),
}

impl Resolver {
    pub fn ask(
        &self,
        u: &Unit,
        attempt: usize,
        previous: Option<&Candidate>,
        findings: &[String],
    ) -> R<Answer> {
        let job = serde_json::json!({
            "path": u.path,
            "kind": u.kind,
            "base": u.base,
            "ours": u.ours,
            "theirs": u.theirs,
            "conflicted": u.gitmerged,
            "attempt": attempt,
            "previous": previous.map(|p| match p {
                Candidate::File(c) => c.as_str(),
                Candidate::Delete => "DELETE",
            }),
            "findings": findings,
        });
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&self.command);
        let run = gitscan::run_bounded(
            cmd,
            "the resolver",
            Some(serde_json::to_vec(&job)?),
            self.timeout,
        );
        let (status, stdout, stderr) = match run {
            Ok(r) => r,
            Err(e) => return Ok(Answer::Failed(e.to_string())),
        };
        // 126/127: the shell could not run the command at all. That is the
        // invocation being wrong, not one file's answer.
        if matches!(status.code(), Some(126) | Some(127)) {
            return Err(format!(
                "the resolver command could not be run: {}",
                String::from_utf8_lossy(&stderr).trim()
            )
            .into());
        }
        if !status.success() {
            let tail = String::from_utf8_lossy(&stderr);
            let tail: String = tail.trim().chars().rev().take(300).collect();
            return Ok(Answer::Failed(format!(
                "the resolver exited {}: {}",
                status
                    .code()
                    .map_or("on a signal".into(), |c| c.to_string()),
                tail.chars().rev().collect::<String>()
            )));
        }
        let Ok(out) = String::from_utf8(stdout) else {
            return Ok(Answer::Failed("the resolver's answer is not UTF-8".into()));
        };
        Ok(parse_answer(out))
    }
}

fn parse_answer(out: String) -> Answer {
    let t = out.trim();
    if !t.contains('\n') {
        if t == "DELETE" {
            return Answer::Delete;
        }
        if t == "KEEP" {
            return Answer::Keep;
        }
        if let Some(rest) = t.strip_prefix("CANNOT") {
            if rest.is_empty() || rest.starts_with(':') || rest.starts_with(' ') {
                return Answer::Cannot(rest.trim_start_matches(':').trim().to_string());
            }
        }
    }
    Answer::File(out)
}

/// The resolved text's newline conventions follow the inputs': a trailing
/// newline when every side ends in one, none when no side does, CRLF when
/// every side is CRLF throughout.
fn follow_newlines(mut body: String, sides: &[&str]) -> String {
    if sides.is_empty() {
        return body;
    }
    if sides.iter().all(|s| s.ends_with('\n')) && !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    if !sides.iter().any(|s| s.ends_with('\n')) && body.ends_with('\n') {
        body.pop();
    }
    let crlf = |s: &str| s.contains("\r\n") && s.matches("\r\n").count() == s.matches('\n').count();
    if sides.iter().all(|s| crlf(s)) {
        body = body.replace("\r\n", "\n").replace('\n', "\r\n");
    }
    body
}

// ----------------------------------------------------------------- landing

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Status {
    Proven,
    Verified,
    Refused,
}

/// What to leave in the working tree for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landed {
    File(String),
    Deleted,
    /// Keep it conflicted: write these bytes (conflict markers), or leave the
    /// working-tree file as git left it (`None`).
    Conflicted(Option<String>),
}

/// One file's line in the report.
#[derive(Debug, Clone, Serialize)]
pub struct FileReport {
    pub path: String,
    pub kind: Kind,
    pub status: Status,
    /// `certificate` | `elem_union` | `present` | `gate` | `resolver` |
    /// `no-resolver` | `not-text` | `keys`.
    pub rule: &'static str,
    /// One sentence.
    pub reason: String,
    /// The gate's reasons (empty unless the gate ran and failed).
    pub reasons: Vec<&'static str>,
    pub findings: Vec<GateFinding>,
    /// Resolver calls made (0, 1 or 2).
    pub attempts: usize,
    /// What weave's merge did: `clean` | `conflicted` | `not-run`.
    pub weave: &'static str,
    /// sha256 of the bytes landed (PROVEN / VERIFIED file), else `null`.
    pub sha256: Option<String>,
    #[serde(skip)]
    pub landed: Landed,
}

/// Land one file: weave, then the certificate, then the resolver behind the
/// gate. `Err` only for a failure of the whole run (the resolver cannot be
/// started at all).
///
/// A clean weave merge the certificate admits with its v2 rule set is PROVEN.
/// One it admits only by also allowing [`UNION_ALLOWANCES`] — weave united
/// elements two sides inserted into one collection, and the certificate's
/// independent `elem_union` check, reading the four parse trees, found every
/// inserted element verbatim, base order kept, no key twice, a construct the
/// policy calls a set, and a result that parses — lands VERIFIED, after the
/// same gate a resolver's answer must pass (DROPPED / UNDELETED included),
/// with no resolver call. When the region-by-region verdict fails, the
/// check also reads the whole file as one statement list (both sides
/// appending named top-level declarations at one point). Not PROVEN: PROVEN means the certificate's
/// fixed proof rule set, measured before it was adopted; `elem_union` is a
/// policy about which collections are sets, not a three-way selection, and
/// it has not been measured that way. Any doubt — the check declines or
/// crashes, the gate fails — sends the file on to the resolver, as before.
pub fn land_unit(u: &Unit, host: &Host, resolver: Option<&Resolver>) -> R<FileReport> {
    let mut report = FileReport {
        path: u.path.clone(),
        kind: u.kind,
        status: Status::Refused,
        rule: "gate",
        reason: String::new(),
        reasons: Vec::new(),
        findings: Vec::new(),
        attempts: 0,
        weave: "not-run",
        sha256: None,
        landed: Landed::Conflicted(u.gitmerged.as_deref().map(|g| {
            if markers(g).is_empty() {
                whole_file_conflict(u)
            } else {
                g.to_string()
            }
        })),
    };

    // ---- 1. weave, and 2. the certificate on a clean result
    let mut unproven = String::new();
    if let (Kind::Content | Kind::AddAdd, Some(ours), Some(theirs)) =
        (u.kind, u.ours.as_deref(), u.theirs.as_deref())
    {
        let fmt = MarkerFormat::default().for_file(&u.path);
        let base = u.base.as_deref().unwrap_or("");
        let merged = entity_merge_fmt(base, ours, theirs, &u.path, &fmt, host);
        // weave's labels describe weave's merge; when the tree already holds
        // a different answer, that answer is the one to judge (step 3).
        let describes_tree = match &u.present {
            None => true,
            Some(Candidate::File(p)) => *p == merged.content,
            Some(Candidate::Delete) => false,
        };
        if merged.is_clean() && !describes_tree {
            report.weave = "clean";
            unproven = "the tree's answer is not weave's merge; ".into();
        } else if merged.is_clean() {
            report.weave = "clean";
            let cert = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                certify(u, &merged.content)
            }))
            .unwrap_or_else(|_| Err("the certificate crashed on this file".into()));
            match cert {
                Ok((admitted, true)) => {
                    let v = gate(u, &Candidate::File(merged.content.clone()));
                    if v.pass() {
                        report.status = Status::Verified;
                        report.rule = "elem_union";
                        report.reason = format!(
                            "weave's element union passed the independent element check \
                             (both-changed regions admitted by: {}) and the gate; no resolver \
                             was asked",
                            admitted.join(", ")
                        );
                        report.sha256 = Some(sha256(&merged.content));
                        report.landed = Landed::File(merged.content);
                        return Ok(report);
                    }
                    unproven = format!(
                        "weave's element union passed the element check but not the gate ({}); ",
                        v.reasons.join(", ")
                    );
                }
                Ok((admitted, false)) => {
                    report.status = Status::Proven;
                    report.rule = "certificate";
                    report.reason = if admitted.is_empty() {
                        "weave's merge is the three-way selection of every region".into()
                    } else {
                        format!(
                            "weave's merge is the three-way selection (both-changed regions \
                             admitted by: {})",
                            admitted.join(", ")
                        )
                    };
                    report.sha256 = Some(sha256(&merged.content));
                    report.landed = Landed::File(merged.content);
                    return Ok(report);
                }
                Err(why) => unproven = format!("weave's clean merge is not certified ({why}); "),
            }
        } else {
            report.weave = "conflicted";
            report.landed = Landed::Conflicted(Some(weave_core::conflict::teach_on_marker(
                &merged.content,
                &u.path,
                fmt.marker_length,
            )));
        }
    }

    // ---- 3. the answer the tree already holds, behind the gate
    if let Some(present) = &u.present {
        let v = gate_with(u, present, true);
        if v.pass() {
            report.status = Status::Verified;
            report.rule = "present";
            report.reason = format!(
                "{unproven}the merge's own result (the merge driver's, or the result \
                 revision's) passed the gate; no resolver was asked"
            );
            report.landed = match present {
                Candidate::File(c) => {
                    report.sha256 = Some(sha256(c));
                    Landed::File(c.clone())
                }
                Candidate::Delete => Landed::Deleted,
            };
            return Ok(report);
        }
        unproven = format!(
            "{unproven}the merge's own result failed the gate ({}); ",
            v.reasons.join(", ")
        );
        report.reasons = v.reasons;
        report.findings = v.findings;
    }

    // ---- 4. the resolver, behind 5. the gate, with at most one retry
    let Some(resolver) = resolver else {
        report.rule = "no-resolver";
        report.reason = format!("{unproven}no resolver was given (--resolver)");
        return Ok(report);
    };
    let mut previous: Option<Candidate> = None;
    let mut feedback: Vec<String> = Vec::new();
    for attempt in 1..=2 {
        report.attempts = attempt;
        let answer = resolver.ask(u, attempt, previous.as_ref(), &feedback)?;
        let modify_delete = u.kind == Kind::ModifyDelete;
        let (cand, verdict) = match answer {
            Answer::Cannot(why) => {
                report.rule = "resolver";
                report.reason = format!(
                    "{unproven}the resolver declined: {}",
                    if why.is_empty() {
                        "no reason given"
                    } else {
                        &why
                    }
                );
                return Ok(report);
            }
            Answer::Failed(why) => {
                report.rule = "resolver";
                report.reason = format!("{unproven}{why}");
                return Ok(report);
            }
            Answer::Delete if modify_delete => (Candidate::Delete, None),
            Answer::Keep if modify_delete => {
                let kept = u
                    .ours
                    .clone()
                    .or_else(|| u.theirs.clone())
                    .unwrap_or_default();
                (Candidate::File(kept), None)
            }
            Answer::Delete | Answer::Keep => (
                Candidate::File(String::new()),
                Some(GateVerdict::malformed(
                    "DELETE and KEEP answer a modify/delete conflict only; this file needs \
                     its complete resolved content",
                )),
            ),
            Answer::File(text) if text.trim().is_empty() => (
                Candidate::File(text),
                Some(GateVerdict::malformed("the answer was empty")),
            ),
            Answer::File(text) => (Candidate::File(follow_newlines(text, &u.sides())), None),
        };
        let verdict = verdict.unwrap_or_else(|| gate(u, &cand));
        if verdict.pass() {
            report.status = Status::Verified;
            report.rule = "gate";
            report.reason =
                format!("{unproven}the resolver's answer passed the gate on attempt {attempt}");
            report.reasons.clear();
            report.findings.clear();
            report.landed = match cand {
                Candidate::File(c) => {
                    report.sha256 = Some(sha256(&c));
                    Landed::File(c)
                }
                Candidate::Delete => Landed::Deleted,
            };
            return Ok(report);
        }
        feedback = verdict.feedback();
        report.reasons = verdict.reasons;
        report.findings = verdict.findings;
        previous = Some(cand);
    }
    report.rule = "gate";
    report.reason = format!(
        "{unproven}the resolver's answer failed the gate twice: {}",
        report.reasons.join(", ")
    );
    Ok(report)
}

/// One conflict box over the whole file, ours against theirs: what a
/// refused file git merged line-cleanly is left as, so that it reads as
/// unresolved and not as git's merge.
fn whole_file_conflict(u: &Unit) -> String {
    let side = |t: Option<&str>| {
        let t = t.unwrap_or("");
        if t.is_empty() || t.ends_with('\n') {
            t.to_string()
        } else {
            format!("{t}\n")
        }
    };
    format!(
        "<<<<<<< ours\n{}=======\n{}>>>>>>> theirs\n",
        side(u.ours.as_deref()),
        side(u.theirs.as_deref())
    )
}

/// The certificate on weave's clean merge: `Ok((allowances that admitted a
/// both-changed region, whether one needed [`UNION_ALLOWANCES`]))` when every
/// both-changed region is admitted — or, failing that, when `elem_union`
/// admits the whole file as one statement list — and `Err(why not)`
/// otherwise.
fn certify(u: &Unit, merged: &str) -> Result<(Vec<&'static str>, bool), String> {
    use weave_certify::{check, decompose, normalize};
    let version = |t: Option<&str>| t.map(|t| decompose(&REGISTRY, &u.path, &normalize(t)));
    let (o, a, b) = (
        version(u.base.as_deref()),
        version(u.ours.as_deref()),
        version(u.theirs.as_deref()),
    );
    let m = decompose(&REGISTRY, &u.path, &normalize(merged));
    let r = check(&u.path, o.as_ref(), a.as_ref(), b.as_ref(), &m);
    by_region(&r).or_else(|why| {
        // Both sides inserting top-level declarations at one point leaves the
        // text after an inserted entity a region neither side wrote; the
        // whole file read as one statement list may still be a union.
        match (a.as_ref(), b.as_ref()) {
            (Some(a), Some(b))
                if weave_certify::elem_union_file(&u.path, o.as_ref(), a, b, &m).is_ok() =>
            {
                Ok((vec!["elem_union (whole file)"], true))
            }
            _ => Err(why),
        }
    })
}

/// The certificate's verdict region by region (see [`certify`]).
fn by_region(r: &weave_certify::Report) -> Result<(Vec<&'static str>, bool), String> {
    if let Some((rule, key)) = r.hard.first() {
        return Err(format!("{rule} at `{key}`"));
    }
    let mut admitted: Vec<&'static str> = Vec::new();
    let mut union = false;
    for (key, verdicts) in &r.both {
        let admits = |set: &[&str]| {
            verdicts
                .iter()
                .find(|(a, s)| set.contains(a) && *s == "admit")
                .map(|(a, _)| *a)
        };
        let by = admits(&PROOF_ALLOWANCES).or_else(|| {
            let a = admits(&UNION_ALLOWANCES);
            union |= a.is_some();
            a
        });
        match by {
            Some(a) if !admitted.contains(&a) => admitted.push(a),
            Some(_) => {}
            None => {
                return Err(format!(
                    "both sides changed `{key}` and no allowance admits the result"
                ))
            }
        }
    }
    Ok((admitted, union))
}

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// ----------------------------------------------------------------- the plan

/// Where the answer already in the tree is read, for [`plan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Present {
    /// The working tree, for every path the index holds at stage 0 (a file
    /// the merge driver resolved, or git merged) — and, with `unmerged`, for
    /// a conflicted path too once the file on disk has no conflict-marker
    /// line left (someone resolved it in place and did not stage it).
    WorkingTree { unmerged: bool },
    /// The tree of this revision (a merge commit being re-checked).
    Rev(String),
}

/// The merge to land, read out of git.
pub struct Plan {
    pub base: String,
    pub ours: String,
    pub theirs: String,
    pub units: Vec<Unit>,
    /// Files git could not merge that are not text weave can read (binary,
    /// symlink, submodule, unreadable blob): refused before anything runs.
    pub not_text: Vec<FileReport>,
    /// Files both sides changed that git's line merge merges cleanly, and
    /// whose merge — the answer in the tree, else git's — is git's own and
    /// passes weave's merge check ([`weave_core::verify::verify`]).
    pub not_examined: Vec<String>,
    /// Each conflicted path's index entries at base / ours / theirs, for
    /// restoring a refused file's unmerged stages.
    pub stages: BTreeMap<String, [Option<(String, String)>; 3]>,
}

/// Read the merge `base` × `ours` × `theirs` (each defaulting as `weave check`
/// does: HEAD, the operation in progress, their merge base) and classify every
/// file both sides changed.
///
/// A file git's line merge merges cleanly is still examined: when the answer
/// in the tree (`present`) is git's merge, that merge must pass weave's own
/// merge check — no line both sides kept lost, nothing stated twice, no
/// `case` or map key twice ([`weave_core::verify::verify`]); git calling it
/// clean is not enough (two features each adding `case 3:` to one switch is
/// line-clean). When it fails, or the tree holds something else, the file is
/// a unit like any conflicted one.
pub fn plan(
    dir: &Path,
    base: Option<&str>,
    ours: Option<&str>,
    theirs: Option<&str>,
    present: Option<&Present>,
) -> R<Plan> {
    let (base, ours, theirs) = gitscan::resolve_revs(dir, base, ours, theirs)?;
    let oid = |rev: &str| -> R<String> {
        Ok(gitscan::git(dir, &["rev-parse", "--verify", rev])?
            .trim()
            .to_string())
    };
    let (base, ours, theirs) = (oid(&base)?, oid(&ours)?, oid(&theirs)?);
    let changed = |side: &str| -> R<BTreeSet<String>> {
        Ok(gitscan::git(
            dir,
            &["diff", "--name-only", "--no-renames", "-z", &base, side],
        )?
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect())
    };
    let both: Vec<String> = changed(&ours)?
        .intersection(&changed(&theirs)?)
        .cloned()
        .collect();
    let [eb, eo, et] = [&base, &ours, &theirs].map(|rev| gitscan::entries_at_rev(dir, rev, &both));
    let (eb, eo, et) = (eb?, eo?, et?);
    let (present_of, unmerged) = present_answers(dir, present, &both)?;

    let mut plan = Plan {
        base,
        ours,
        theirs,
        units: Vec::new(),
        not_text: Vec::new(),
        not_examined: Vec::new(),
        stages: BTreeMap::new(),
    };
    for path in both {
        let (b, o, t) = (eb.get(&path), eo.get(&path), et.get(&path));
        let same = |x: Option<&Entry>, y: Option<&Entry>| {
            x.map(|e| (&e.mode, &e.oid)) == y.map(|e| (&e.mode, &e.oid))
        };
        if same(o, t) {
            continue; // both sides made the same change
        }
        let kind = match (b, o, t) {
            (_, None, _) | (_, _, None) => Kind::ModifyDelete,
            (None, _, _) => Kind::AddAdd,
            _ => Kind::Content,
        };
        let entry = |e: Option<&Entry>| e.map(|e| (e.mode.clone(), e.oid.clone()));
        let stages = [entry(b), entry(o), entry(t)];
        let odd = [b, o, t].into_iter().flatten().find_map(|e| match &e.body {
            Body::Text(_) => None,
            Body::Binary => Some("a merge stage is binary (not UTF-8)".to_string()),
            Body::Irregular => Some("a merge stage is a symlink or a submodule".to_string()),
            Body::Unreadable(why) => Some(format!("a merge stage could not be read: {why}")),
        });
        if let Some(why) = odd {
            plan.stages.insert(path.clone(), stages);
            plan.not_text.push(FileReport {
                path,
                kind,
                status: Status::Refused,
                rule: "not-text",
                reason: format!("{why}; not attempted"),
                reasons: Vec::new(),
                findings: Vec::new(),
                attempts: 0,
                weave: "not-run",
                sha256: None,
                landed: Landed::Conflicted(None),
            });
            continue;
        }
        let text = |e: Option<&Entry>| match e.map(|e| &e.body) {
            Some(Body::Text(t)) => Some(t.clone()),
            _ => None,
        };
        let (base_text, ours_text, theirs_text) = (text(b), text(o), text(t));
        let present_answer = present_of.get(&path).cloned();
        let mut unverified_git_merge = false;
        let gitmerged = match (kind, &ours_text, &theirs_text) {
            (Kind::ModifyDelete, _, _) => None,
            (_, Some(o), Some(t)) => {
                let base_t = base_text.as_deref().unwrap_or("");
                let (clean, merged) = line_merge(base_t, o, t)?;
                if clean {
                    // A path the index holds unmerged was refused by the
                    // merge driver: never waved through on git's line merge.
                    let holds =
                        !unmerged.contains(&path) && line_clean_holds(&path, base_t, o, t, &merged);
                    let answer = match &present_answer {
                        Some(Candidate::File(p)) => Some(p.as_str()),
                        Some(Candidate::Delete) => None,
                        None => Some(merged.as_str()),
                    };
                    if holds && answer == Some(merged.as_str()) {
                        plan.not_examined.push(path);
                        continue;
                    }
                    unverified_git_merge = !holds;
                }
                Some(merged)
            }
            _ => None,
        };
        plan.stages.insert(path.clone(), stages);
        plan.units.push(Unit {
            path,
            kind,
            base: base_text,
            ours: ours_text,
            theirs: theirs_text,
            gitmerged,
            present: present_answer,
            unverified_git_merge,
        });
    }
    Ok(plan)
}

/// git's clean line merge of a file passes the check weave's own merge must
/// pass before it may call a merge clean. Files too large or binary for
/// structure keep git's guarantee, as weave's merge does.
fn line_clean_holds(path: &str, base: &str, ours: &str, theirs: &str, merged: &str) -> bool {
    if [base, ours, theirs, merged]
        .iter()
        .any(|t| t.len() > weave_core::merge::STRUCTURE_LIMIT_BYTES)
    {
        return true;
    }
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        weave_core::verify::verify(base, ours, theirs, merged, path, &REGISTRY, &[])
    }));
    matches!(run, Ok(None))
}

/// The answer already in the tree for each path, per [`Present`]: a file's
/// text, or `Delete` when the tree does not have it. A path the working
/// tree's index holds unmerged has no answer here unless `unmerged` asks for
/// in-place resolutions — a conflicted file is the resolver's to answer.
/// Also: the paths the index holds unmerged (working-tree mode only).
fn present_answers(
    dir: &Path,
    present: Option<&Present>,
    paths: &[String],
) -> R<(BTreeMap<String, Candidate>, BTreeSet<String>)> {
    let mut out = BTreeMap::new();
    let mut unmerged_paths = BTreeSet::new();
    match present {
        None => {}
        Some(Present::Rev(rev)) => {
            let at = gitscan::entries_at_rev(dir, rev, paths)?;
            for p in paths {
                match at.get(p).map(|e| &e.body) {
                    Some(Body::Text(t)) => {
                        out.insert(p.clone(), Candidate::File(t.clone()));
                    }
                    Some(_) => {}
                    None => {
                        out.insert(p.clone(), Candidate::Delete);
                    }
                }
            }
        }
        Some(Present::WorkingTree {
            unmerged: take_unmerged,
        }) => {
            let unmerged: BTreeSet<String> =
                gitscan::git(dir, &["diff", "--name-only", "--diff-filter=U", "-z"])?
                    .split('\0')
                    .filter(|p| !p.is_empty())
                    .map(str::to_string)
                    .collect();
            unmerged_paths = unmerged.clone();
            for p in paths
                .iter()
                .filter(|p| *take_unmerged || !unmerged.contains(*p))
            {
                let on_disk = dir.join(p);
                match std::fs::read(&on_disk) {
                    Ok(bytes) => {
                        if let Ok(t) = String::from_utf8(bytes) {
                            if !unmerged.contains(p) || markers(&t).is_empty() {
                                out.insert(p.clone(), Candidate::File(t));
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        out.insert(p.clone(), Candidate::Delete);
                    }
                    Err(_) => {}
                }
            }
        }
    }
    Ok((out, unmerged_paths))
}

/// git's own line merge of one file, as `git merge` writes it (plain `merge`
/// conflict style): `(clean, text)`.
fn line_merge(base: &str, ours: &str, theirs: &str) -> R<(bool, String)> {
    line_merge_with(base, ours, theirs, &[])
}

/// [`line_merge`] with extra `git merge-file` options (a conflict style).
fn line_merge_with(base: &str, ours: &str, theirs: &str, opts: &[&str]) -> R<(bool, String)> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "weave-land-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir)?;
    let result = (|| -> R<(bool, String)> {
        for (name, text) in [("ours", ours), ("base", base), ("theirs", theirs)] {
            std::fs::write(dir.join(name), text)?;
        }
        let mut cmd = Command::new("git");
        cmd.args([
            "merge-file",
            "-p",
            "-L",
            "ours",
            "-L",
            "base",
            "-L",
            "theirs",
        ])
        .args(opts)
        .args(["ours", "base", "theirs"])
        .current_dir(&dir)
        // No user config: `merge.conflictStyle` would change which lines
        // count as merged automatically.
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
        let (status, stdout, stderr) =
            gitscan::run_bounded(cmd, "git merge-file", None, Duration::from_secs(120))?;
        match status.code() {
            Some(c @ 0..=127) => Ok((c == 0, String::from_utf8(stdout)?)),
            _ => Err(format!(
                "git merge-file failed: {}",
                String::from_utf8_lossy(&stderr).trim()
            )
            .into()),
        }
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

// ------------------------------------------------------------ the write-back

/// Write every file's outcome into the working tree and index at `dir`:
/// PROVEN / VERIFIED files are written and staged (a verified DELETE is
/// removed); a REFUSED file keeps its conflict markers and is put back to
/// unmerged, so `git commit` refuses until someone resolves it.
pub fn write_back(dir: &Path, plan: &Plan, reports: &[FileReport]) -> R<()> {
    for r in reports {
        let on_disk = dir.join(&r.path);
        match &r.landed {
            Landed::File(text) => {
                if let Some(parent) = on_disk.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&on_disk, text)?;
                gitscan::git(dir, &["add", "--", &r.path])?;
            }
            Landed::Deleted => {
                gitscan::git(
                    dir,
                    &["rm", "-q", "--cached", "--ignore-unmatch", "--", &r.path],
                )?;
                if on_disk.symlink_metadata().is_ok() {
                    std::fs::remove_file(&on_disk)?;
                }
            }
            Landed::Conflicted(text) => {
                if let Some(text) = text {
                    if let Some(parent) = on_disk.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&on_disk, text)?;
                }
                let Some(stages) = plan.stages.get(&r.path) else {
                    continue;
                };
                // Drop whatever stage-0 entry a merge driver left, and put the
                // three merge stages back.
                let mut info = format!("0 {}\t{}\n", "0".repeat(40), r.path);
                for (n, entry) in stages.iter().enumerate() {
                    if let Some((mode, oid)) = entry {
                        info.push_str(&format!("{mode} {oid} {}\t{}\n", n + 1, r.path));
                    }
                }
                gitscan::git_input(
                    dir,
                    &["update-index", "--index-info"],
                    Some(info.into_bytes()),
                )?;
            }
        }
    }
    Ok(())
}

// ----------------------------------------------------------------- the report

/// The whole run, as `--json` prints it and `--certificate` writes it.
pub fn document(plan: &Plan, reports: &[FileReport], mode: &str) -> serde_json::Value {
    let count = |s: Status| reports.iter().filter(|r| r.status == s).count();
    serde_json::json!({
        "schema": SCHEMA,
        "merge": {
            "base": plan.base,
            "ours": plan.ours,
            "theirs": plan.theirs,
            "mode": mode,
        },
        "proof_allowances": PROOF_ALLOWANCES,
        "union_allowances": UNION_ALLOWANCES,
        "summary": {
            "proven": count(Status::Proven),
            "verified": count(Status::Verified),
            "refused": count(Status::Refused),
            "not_examined": plan.not_examined.len(),
        },
        "files": reports,
        "not_examined": plan.not_examined,
    })
}

/// The human summary.
pub fn render(plan: &Plan, reports: &[FileReport]) -> String {
    let count = |s: Status| reports.iter().filter(|r| r.status == s).count();
    let mut out = format!(
        "weave land: {} file(s) git could not merge — {} PROVEN, {} VERIFIED, {} REFUSED",
        reports.len(),
        count(Status::Proven),
        count(Status::Verified),
        count(Status::Refused),
    );
    if !plan.not_examined.is_empty() {
        out.push_str(&format!(
            " ({} more that both sides changed git merged line-cleanly, and the merge check passed)",
            plan.not_examined.len()
        ));
    }
    out.push('\n');
    let width = reports.iter().map(|r| r.path.len()).max().unwrap_or(0);
    for r in reports {
        let status = match r.status {
            Status::Proven => "PROVEN",
            Status::Verified => "VERIFIED",
            Status::Refused => "REFUSED",
        };
        out.push_str(&format!(
            "  {status:<8}  {:<width$}  {}: {}\n",
            r.path, r.rule, r.reason
        ));
        for f in &r.findings {
            out.push_str(&format!("              - {}: {}\n", f.class, f.detail));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(path: &str, base: &str, ours: &str, theirs: &str, gitmerged: &str) -> Unit {
        Unit {
            path: path.into(),
            kind: Kind::Content,
            base: Some(base.into()),
            ours: Some(ours.into()),
            theirs: Some(theirs.into()),
            gitmerged: Some(gitmerged.into()),
            present: None,
            unverified_git_merge: false,
        }
    }

    #[test]
    fn finding_pattern_strips_names_and_digits() {
        assert_eq!(
            finding_pattern("2 name(s) are defined more than once: `f` (2x)"),
            " name(s) are defined more than once:  (x)"
        );
        assert_eq!(backticked("a `x` b `y` c `z"), vec!["x", "y"]);
    }

    #[test]
    fn markers_are_a_set_difference_with_the_sides() {
        let u = unit("a.txt", "x\n", "=======\n", "y\n", "x\n");
        // ours verbatim: no MARKERS (ours has the line), though theirs' `y`
        // is DROPPED
        let v = gate(&u, &Candidate::File("=======\n".into()));
        assert_eq!(v.reasons, vec!["DROPPED"], "{v:?}");
        let v = gate(&u, &Candidate::File("<<<<<<< ours\n".into()));
        assert!(v.reasons.contains(&"MARKERS"));
    }

    #[test]
    fn automerged_lines_must_survive() {
        let gm =
            "keep_this_line_git_merged()\n<<<<<<< ours\na = 1\n=======\na = 2\n>>>>>>> theirs\n";
        assert!(automerged(Some("a = 0\n"), Some(gm), "a = 3\n").is_some());
        assert!(automerged(
            Some("a = 0\n"),
            Some(gm),
            "keep_this_line_git_merged()\na = 3\n"
        )
        .is_none());
        // a clean git merge (a unit only because the tree holds another
        // answer, or git's failed the merge check): every line it wrote that
        // base does not have must survive
        assert!(automerged(None, Some("keep_this_line_git_merged()\n"), "").is_some());
        assert!(automerged(
            Some("keep_this_line_git_merged()\n"),
            Some("keep_this_line_git_merged()\n"),
            ""
        )
        .is_none());
    }

    /// The gate on a real three-way merge of `m.py`.
    fn gate_py(base: &str, ours: &str, theirs: &str, cand: &str) -> GateVerdict {
        let (clean, gm) = line_merge(base, ours, theirs).unwrap();
        assert!(!clean, "the fixture must conflict");
        gate(
            &unit("m.py", base, ours, theirs, &gm),
            &Candidate::File(cand.into()),
        )
    }

    fn classes(v: &GateVerdict) -> Vec<&str> {
        v.findings.iter().map(|f| f.class.as_str()).collect()
    }

    // Both sides append a different entry to one dict, at the same place.
    const D_BASE: &str = "T = {\n    'alpha': 1,\n    'omega': 9,\n}\n";
    const D_OURS: &str = "T = {\n    'alpha': 1,\n    'beta': 2,\n    'omega': 9,\n}\n";
    const D_THEIRS: &str = "T = {\n    'alpha': 1,\n    'gamma': 3,\n    'omega': 9,\n}\n";

    #[test]
    fn one_sided_resolutions_are_dropped() {
        for (cand, side) in [(D_OURS, "theirs"), (D_THEIRS, "ours")] {
            let v = gate_py(D_BASE, D_OURS, D_THEIRS, cand);
            assert_eq!(v.reasons, vec!["DROPPED"], "{v:?}");
            assert!(v.findings[0].detail.starts_with(side), "{v:?}");
        }
    }

    #[test]
    fn a_union_in_either_order_or_rewrapped_passes() {
        for cand in [
            "T = {\n    'alpha': 1,\n    'beta': 2,\n    'gamma': 3,\n    'omega': 9,\n}\n",
            "T = {\n    'alpha': 1,\n    'gamma': 3,\n    'beta': 2,\n    'omega': 9,\n}\n",
            "T = {\n    'alpha': 1,\n    'beta': 2, 'gamma': 3,\n    'omega': 9,\n}\n",
        ] {
            let v = gate_py(D_BASE, D_OURS, D_THEIRS, cand);
            assert!(v.pass(), "{cand}: {v:?}");
        }
    }

    #[test]
    fn an_identical_change_inside_a_block_is_owed_once() {
        // both add `'beta': 2`; each adds one more entry of its own
        let ours = "T = {\n    'alpha': 1,\n    'beta': 2,\n    'delta': 4,\n    'omega': 9,\n}\n";
        let theirs =
            "T = {\n    'alpha': 1,\n    'beta': 2,\n    'gamma': 3,\n    'omega': 9,\n}\n";
        let union = "T = {\n    'alpha': 1,\n    'beta': 2,\n    'delta': 4,\n    'gamma': 3,\n    'omega': 9,\n}\n";
        assert!(gate_py(D_BASE, ours, theirs, union).pass());
        let v = gate_py(D_BASE, ours, theirs, ours);
        assert_eq!(classes(&v), vec!["DROPPED"], "{v:?}");
        assert!(v.findings[0].detail.contains("`gamma`"), "{v:?}");
    }

    #[test]
    fn a_side_that_subsumes_the_other_passes_verbatim() {
        // theirs adds `beta`; ours adds `beta` and `gamma`: ours holds both changes
        let ours = "T = {\n    'alpha': 1,\n    'beta': 2,\n    'gamma': 3,\n    'omega': 9,\n}\n";
        let v = gate_py(D_BASE, ours, D_OURS, ours);
        assert!(v.pass(), "{v:?}");
        // the subsumed side is not enough
        let v = gate_py(D_BASE, ours, D_OURS, D_OURS);
        assert_eq!(v.reasons, vec!["DROPPED"], "{v:?}");
    }

    #[test]
    fn two_edits_of_one_line_folded_into_one_line_pass() {
        let base = "def f():\n    return os.sep\n";
        let ours = "def f():\n    return os.sep + 'a'\n";
        let theirs = "def f():\n    return os.sep + 'b'\n";
        assert!(gate_py(
            base,
            ours,
            theirs,
            "def f():\n    return os.sep + 'a' + 'b'\n"
        )
        .pass());
        // incompatible edits of one value: picking one is a dropped change
        let (o, t) = ("X = 2\n", "X = 3\n");
        let v = gate_py("X = 1\n", o, t, o);
        assert_eq!(v.reasons, vec!["DROPPED"], "{v:?}");
    }

    #[test]
    fn delete_vs_modify_inside_a_block_is_refused_either_way() {
        let base = "def keep():\n    return 0\n\n\ndef f():\n    return 1\n";
        let ours = "def keep():\n    return 0\n";
        let theirs = "def keep():\n    return 0\n\n\ndef f():\n    return 2\n";
        // keep theirs' edit: ours' deletion is undone
        let v = gate_py(base, ours, theirs, theirs);
        assert!(classes(&v).contains(&"UNDELETED"), "{v:?}");
        // keep ours' deletion: theirs' edit is dropped
        let v = gate_py(base, ours, theirs, ours);
        assert!(classes(&v).contains(&"DROPPED"), "{v:?}");
    }

    #[test]
    fn a_one_sided_deletion_must_stay_deleted() {
        // ours deletes `old`, theirs adds `new` next to it
        let base = "A = [\n    'first_entry',\n    'old_entry',\n]\n";
        let ours = "A = [\n    'first_entry',\n]\n";
        let theirs = "A = [\n    'first_entry',\n    'old_entry',\n    'new_entry',\n]\n";
        let v = gate_py(base, ours, theirs, theirs);
        assert_eq!(classes(&v), vec!["UNDELETED"], "{v:?}");
        assert!(v.findings[0].detail.contains("`old_entry`"), "{v:?}");
        let both = "A = [\n    'first_entry',\n    'new_entry',\n]\n";
        assert!(gate_py(base, ours, theirs, both).pass());
    }

    #[test]
    fn add_add_keeps_both_files_content() {
        let ours = "def a():\n    return 'from ours'\n";
        let theirs = "def b():\n    return 'from theirs'\n";
        let (_, gm) = line_merge("", ours, theirs).unwrap();
        let u = Unit {
            kind: Kind::AddAdd,
            base: None,
            ..unit("m.py", "", ours, theirs, &gm)
        };
        let v = gate(&u, &Candidate::File(ours.into()));
        assert_eq!(v.reasons, vec!["DROPPED"], "{v:?}");
        let both = format!("{ours}\n\n{theirs}");
        assert!(gate(&u, &Candidate::File(both)).pass());
    }

    #[test]
    fn non_ascii_lines_near_a_block_are_read() {
        // a 41-byte prefix that ends inside a multi-byte char must not panic
        let base = "x = 1\n        # Test NULL literal: Snowflake \u{2192} DuckDB\n";
        let ours = "x = 2\n        # Test NULL literal: Snowflake \u{2192} DuckDB\n";
        let theirs = "x = 3\n        # Test NULL literal: Snowflake \u{2192} DuckDB\n";
        let v = gate_py(base, ours, theirs, ours);
        assert_eq!(v.reasons, vec!["DROPPED"], "{v:?}");
        let marker_like = "=".repeat(41) + " \u{2192}\n";
        let d = diff3_blocks(
            &marker_like,
            &format!("a\n{marker_like}"),
            &format!("b\n{marker_like}"),
        )
        .unwrap();
        assert_eq!(d.regions.len(), 1);
    }

    #[test]
    fn tokens_ignore_layout_and_brackets() {
        let t: Vec<&str> = tokens("  exp.IsInf(this=e.this.copy()), x >= 10_0").collect();
        assert_eq!(
            t,
            ["exp", "IsInf", "this", "=", "e", "this", "copy", "x", ">=", "10_0"]
        );
    }

    #[test]
    fn newlines_follow_the_sides() {
        assert_eq!(follow_newlines("a".into(), &["x\n", "y\n"]), "a\n");
        assert_eq!(follow_newlines("a\n".into(), &["x", "y"]), "a");
        assert_eq!(
            follow_newlines("a\nb\n".into(), &["x\r\n", "y\r\n"]),
            "a\r\nb\r\n"
        );
    }

    // An element union inside a class: both sides add an entry at one point of a
    // registry dict. The union is admitted only by the `elem_union` check; a
    // result with one side's entry removed, or base order changed, is not
    // admitted by anything, so it could never land without the resolver.
    #[test]
    fn certify_admits_an_element_union_and_refuses_a_tampered_one() {
        let base = "class Gen:\n    TRANSFORMS = {\n        exp.Abs: rename_func(\"ABS\"),\n        exp.IntDiv: rename_func(\"DIV\"),\n        exp.Mod: rename_func(\"MOD\"),\n    }\n";
        let at = "        exp.IntDiv: rename_func(\"DIV\"),\n";
        let ins = |x: &str| base.replace(at, &format!("{at}{x}"));
        let (fin, unix) = (
            "        exp.IsFinite: rename_func(\"IS_FINITE\"),\n",
            "        exp.TimeToUnix: rename_func(\"UNIX_SECONDS\"),\n",
        );
        let (ours, theirs) = (ins(fin), ins(unix));
        let u = unit("gen.py", base, &ours, &theirs, "");
        let union = ins(&format!("{fin}{unix}"));
        assert_eq!(certify(&u, &union), Ok((vec!["elem_union"], true)));
        // and weave's own merge is that union, or the other order
        let host = Host::default();
        let merged = entity_merge_fmt(
            base,
            &ours,
            &theirs,
            "gen.py",
            &MarkerFormat::default(),
            &host,
        );
        assert!(merged.is_clean());
        assert_eq!(certify(&u, &merged.content), Ok((vec!["elem_union"], true)));
        let abs = "        exp.Abs: rename_func(\"ABS\"),\n";
        for (what, bad) in [
            ("ours dropped", union.replace(fin, "")),
            ("theirs dropped", union.replace(unix, "")),
            (
                "base order changed",
                union.replace(&format!("{abs}{at}"), &format!("{at}{abs}")),
            ),
            (
                "an entry rewritten",
                union.replace("UNIX_SECONDS", "UNIX_MILLIS"),
            ),
        ] {
            assert!(
                certify(&u, &bad).is_err(),
                "{what}: {:?}",
                certify(&u, &bad)
            );
        }
    }

    // Two features each add `case 3:` to one switch. git
    // merges it line-cleanly, the result does not compile.
    const SW_BASE: &str = "package p\n\nfunc F(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 2:\n\t\treturn 20\n\t}\n\treturn 0\n}\n";
    const SW_OURS: &str = "package p\n\nfunc F(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 3:\n\t\treturn 31\n\tcase 2:\n\t\treturn 20\n\t}\n\treturn 0\n}\n";
    const SW_THEIRS: &str = "package p\n\nfunc F(x int) int {\n\tswitch x {\n\tcase 1:\n\t\treturn 10\n\tcase 2:\n\t\treturn 20\n\tcase 3:\n\t\treturn 32\n\t}\n\treturn 0\n}\n";

    #[test]
    fn a_line_clean_duplicate_case_fails_the_merge_check_and_the_gate() {
        let (clean, merged) = line_merge(SW_BASE, SW_OURS, SW_THEIRS).unwrap();
        assert!(clean, "git merges it line-cleanly");
        assert!(!line_clean_holds(
            "p.go", SW_BASE, SW_OURS, SW_THEIRS, &merged
        ));
        let u = Unit {
            present: Some(Candidate::File(merged.clone())),
            unverified_git_merge: true,
            ..unit("p.go", SW_BASE, SW_OURS, SW_THEIRS, &merged)
        };
        let v = gate(&u, &Candidate::File(merged));
        assert!(
            v.findings
                .iter()
                .any(|f| f.class == "DUP" && f.detail.contains("case `3`")),
            "{v:?}"
        );
        let r = land_unit(&u, &Host::default(), None).unwrap();
        assert_eq!(r.status, Status::Refused, "{r:?}");
        // left conflicted, with markers, not as git's (compiling-looking) merge
        match &r.landed {
            Landed::Conflicted(Some(t)) => assert!(!markers(t).is_empty(), "{t}"),
            other => panic!("{other:?}"),
        }
        // A resolution must state `case 3` once, so it differs from git's
        // merge; keeping both sides' cases (one renumbered) passes, dropping
        // one side's case does not.
        let renumbered = SW_OURS
            .replace("case 3:\n\t\treturn 31", "case 4:\n\t\treturn 31")
            .replace(
                "\t}\n\treturn 0",
                "\tcase 3:\n\t\treturn 32\n\t}\n\treturn 0",
            );
        assert!(gate(&u, &Candidate::File(renumbered)).pass());
        let v = gate(&u, &Candidate::File(SW_OURS.into()));
        assert_eq!(v.reasons, vec!["DROPPED"], "{v:?}");
    }

    #[test]
    fn the_answer_in_the_tree_is_judged_before_the_resolver() {
        // a both-sides edit git conflicts on; the tree already holds a merge
        // that keeps both changes
        let base = "def f():\n    return 1\n\n\ndef g():\n    return 2\n";
        let ours = "def f():\n    return 10\n\n\ndef g():\n    return 2\n";
        let theirs = "def f():\n    return 1\n\n\ndef g():\n    return 20\n";
        let (_, gm) = line_merge(base, ours, theirs).unwrap();
        let both = "def f():\n    return 10\n\n\ndef g():\n    return 20\n";
        let u = Unit {
            present: Some(Candidate::File(both.into())),
            ..unit("m.py", base, ours, theirs, &gm)
        };
        let r = land_unit(&u, &Host::default(), None).unwrap();
        assert_ne!(r.status, Status::Refused, "{r:?}");
        assert_eq!(r.landed, Landed::File(both.into()));
        // one side's file as the answer drops the other's change (git merged
        // this file line-cleanly: the change outside any block must survive)
        let u = Unit {
            present: Some(Candidate::File(ours.into())),
            ..unit("m.py", base, ours, theirs, &gm)
        };
        let r = land_unit(&u, &Host::default(), None).unwrap();
        assert_eq!(r.status, Status::Refused, "{r:?}");
    }

    #[test]
    fn answers() {
        assert_eq!(parse_answer("DELETE\n".into()), Answer::Delete);
        assert_eq!(parse_answer(" KEEP ".into()), Answer::Keep);
        assert_eq!(
            parse_answer("CANNOT: too big\n".into()),
            Answer::Cannot("too big".into())
        );
        assert_eq!(
            parse_answer("CANNOT\n".into()),
            Answer::Cannot(String::new())
        );
        assert_eq!(
            parse_answer("CANNOTATION\n".into()),
            Answer::File("CANNOTATION\n".into())
        );
        assert_eq!(
            parse_answer("DELETE\nx\n".into()),
            Answer::File("DELETE\nx\n".into())
        );
    }
}
