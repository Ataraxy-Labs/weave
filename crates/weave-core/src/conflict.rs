use serde::Serialize;
use std::fmt;

use crate::merge::ResolutionStrategy;

/// Controls conflict marker format: enhanced (weave metadata) or standard (git-compatible).
///
/// When `enhanced` is true (default for git merge driver), markers include entity metadata,
/// Conflict complexity classification, and the refusal line naming which guard declined.
/// When `enhanced` is false (triggered by `-l` flag from jj/other tools), markers use
/// the standard git format that tools can parse: `<<<<<<< ours` / `=======` / `>>>>>>> theirs`.
#[derive(Debug, Clone)]
pub struct MarkerFormat {
    pub marker_length: usize,
    pub enhanced: bool,
    /// The line-comment prefix of the language the markers will land in.
    ///
    /// The enhanced marker puts a `refused_by:` line inside the artifact. `//` is not
    /// a comment in Python, YAML, SQL or Lisp, so the prefix is part of the
    /// marker's format and travels with it. It used to be hard-coded here and
    /// repaired afterwards by `weave-driver`, which scanned the rendered
    /// artifact for "this line opens a conflict" — a second owner for a fact
    /// this struct already had, deciding it AFTER `is_clean()` had been decided
    /// on different bytes.
    pub comment_prefix: String,
}

impl Default for MarkerFormat {
    fn default() -> Self {
        Self {
            marker_length: 7,
            enhanced: true,
            comment_prefix: "//".to_string(),
        }
    }
}

impl MarkerFormat {
    pub fn standard(marker_length: usize) -> Self {
        Self {
            marker_length,
            enhanced: false,
            comment_prefix: "//".to_string(),
        }
    }

    /// The same format, with the comment prefix this file's language uses.
    ///
    /// `pub` for exactly one consumer: `weave-driver` appends one comment line
    /// to a conflicted artifact (the pull channel's teach line) and must spell
    /// the comment the way the file does. Handing out a FORMAT that carries the
    /// answer is not the same as handing out the table — the table stays
    /// private, so no consumer can re-derive `//` for Python, which is the bug
    /// this arrangement exists to make unreachable.
    pub fn for_file(mut self, file_path: &str) -> Self {
        self.comment_prefix = line_comment_prefix(file_path).to_string();
        self
    }
}

/// The text of a marker refusal line, if this line is one: a comment prefix
/// made of punctuation, then `refused_by: `.
///
/// This used to read `hint: ` and carry a sentence of advice, which readers
/// treated as litter to strip along with the markers rather than as useful
/// content. What readers do use, unprompted, is the guard's name — the
/// evidence that it declined to guess, not advice about what to do instead.
/// So the one line inside the box is now that name and the evidence under it,
/// and the advice is gone.
pub(crate) fn refusal_body(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches(|c: char| c.is_ascii_punctuation());
    if rest.len() == line.len() {
        return None;
    }
    rest.trim_start().strip_prefix("refused_by: ")
}

/// The stable body of the driver's teach line — the pull channel's one
/// sentence, appended once to a CONFLICTED artifact.
///
/// It lives here, next to the refusal line, because it is the same KIND of
/// thing: text weave wrote, that no version of the file wrote, that belongs to
/// neither side's claim. [`crate::frame`] has to be able to recognise it for
/// exactly that reason — a two-button resolution must drop it with the markers,
/// and the frame check must not count it as a line weave duplicated into the
/// output. One owner for the bytes, so the writer and the two readers cannot
/// drift.
/// `pub(crate)` and not `pub`: the two readers reach it through
/// [`teach_line`] and [`is_teach_line`], which is the whole arrangement —
/// handing out the raw mark would let a consumer spell the line itself, which
/// is the drift these three items exist to prevent.
pub(crate) const TEACH_MARK: &str = "weave: run 'weave explain ";

/// The whole line, in the file's own comment syntax.
pub fn teach_line(comment_prefix: &str, file_path: &str) -> String {
    format!(
        "{comment_prefix} {TEACH_MARK}{file_path}' for per-hunk detail, \
'weave check' to verify your resolution"
    )
}

/// Is this line the teach line? A comment prefix, then the stable mark.
pub fn is_teach_line(line: &str) -> bool {
    let rest = line.trim_start_matches(|c: char| c.is_ascii_punctuation());
    rest.len() != line.len() && rest.trim_start().starts_with(TEACH_MARK)
}

/// How wide one quoted collision line may be inside the refusal line.
///
/// The whole point of the line is that it is ONE line: a reader who has to
/// scroll it has been handed a document again.
const COLLISION_QUOTE_WIDTH: usize = 64;

/// The one line inside every conflict box: which guard refused, and the lines
/// both sides had an opinion about.
///
/// ```text
/// # refused_by: container_member · collision: `    self.retries = n` +2 more
/// ```
///
/// Both halves are READ OFF facts weave already computed — the guard is the
/// rung of the ladder that declined ([`crate::merge::ResolutionStrategy::guard`]),
/// and the collision is [`crate::diagnose::diagnose`]'s intersection of the two
/// sides' deleted base lines. Neither is advice and neither is inferred: this
/// line tells the reader *why there is a box* and *what is in dispute*, which
/// is the only question the box is asking.
pub(crate) fn refusal_line(
    prefix: &str,
    guard: &str,
    base: Option<&str>,
    ours: Option<&str>,
    theirs: Option<&str>,
) -> String {
    format!(
        "{prefix} refused_by: {guard} · collision: {}\n",
        collision_summary(base, ours, theirs)
    )
}

/// The collision half of the refusal line.
///
/// Only hunks where BOTH sides acted count. A hunk one side left alone is not a
/// collision, it is an edit that happens to sit inside a box the other side's
/// work opened — counting those is how "158 hunks" gets printed for three real
/// conflicts.
fn collision_summary(base: Option<&str>, ours: Option<&str>, theirs: Option<&str>) -> String {
    let (Some(base), Some(ours), Some(theirs)) = (base, ours, theirs) else {
        return "none (no common ancestor text)".to_string();
    };
    let hunks = crate::diagnose::diagnose(base, ours, theirs);
    let contested: Vec<&crate::diagnose::Hunk> = hunks
        .iter()
        .filter(|h| {
            h.ours.action != crate::diagnose::SideAction::Untouched
                && h.theirs.action != crate::diagnose::SideAction::Untouched
        })
        .collect();
    let lines: Vec<&String> = contested.iter().flat_map(|h| h.collision.iter()).collect();
    match lines.split_first() {
        None if contested.is_empty() => "none (both sides edited, nowhere the same lines)".into(),
        None => format!(
            "none ({} contested hunk(s), neither side deleted the other's lines)",
            contested.len()
        ),
        Some((first, rest)) => {
            let mut s = format!("`{}`", quote_line(first));
            if !rest.is_empty() {
                s.push_str(&format!(" +{} more", rest.len()));
            }
            s
        }
    }
}

/// One source line, made safe to sit inside a one-line comment.
///
/// Control characters become spaces (a newline here would silently turn one
/// comment into two lines, one of which is not a comment), any run that could
/// read as a conflict marker is broken, and the result is truncated.
fn quote_line(line: &str) -> String {
    let flat: String = line
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    // Break any run of seven-or-more identical marker characters, so a quoted
    // source line can never be read as a marker by the very parsers that read
    // this file back. Stated as a rule about runs rather than as three literal
    // patterns: the literals would be a fourth place that decides what a marker
    // looks like, and `MarkerFormat::marker_length` says there is no fixed one.
    let mut flat_broken = String::with_capacity(flat.len() + 4);
    let mut run: usize = 0;
    let mut prev = '\0';
    for c in flat.chars() {
        run = if c == prev { run + 1 } else { 1 };
        prev = c;
        if run >= 7 && matches!(c, '<' | '=' | '>' | '|') {
            flat_broken.push(' ');
            run = 1;
        }
        flat_broken.push(c);
    }
    let flat = flat_broken;
    if flat.chars().count() > COLLISION_QUOTE_WIDTH {
        let head: String = flat.chars().take(COLLISION_QUOTE_WIDTH - 1).collect();
        format!("{head}…")
    } else {
        flat
    }
}

/// The line-comment prefix for a file, by extension.
///
/// One table, in the crate that renders the markers. Every consumer —
/// `weave-cli`, `weave-github`, `weave-mcp`, `weave-driver` — used to inherit
/// `//` and only the driver repaired it, so the other three emitted `//` into
/// Python.
///
/// Private, and re-exported nowhere: the table's one caller is
/// [`MarkerFormat::for_file`], one module away, and that is the whole point of
/// it — a consumer that could read the table could also *not* read it and go
/// on emitting `//` into Python, which is the bug this exists to make
/// unreachable. Handing out the answer is authority; handing out a format that
/// already carries the answer is not.
fn line_comment_prefix(file_path: &str) -> &'static str {
    let ext = std::path::Path::new(file_path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        // hash-comment languages
        "py" | "pyi" | "pyx" | "yaml" | "yml" | "rb" | "sh" | "bash" | "zsh" | "fish" | "ps1"
        | "toml" | "pl" | "pm" | "r" | "jl" | "ex" | "exs" | "nix" | "tf" | "dockerfile"
        | "makefile" | "mk" | "cmake" | "gemfile" | "conf" | "cfg" | "ini" | "properties" => "#",
        // double-dash comment languages
        "sql" | "lua" | "hs" | "elm" | "ada" | "adb" | "ads" | "vhd" | "vhdl" => "--",
        // semicolon comment languages
        "clj" | "cljs" | "cljc" | "edn" | "lisp" | "lsp" | "el" | "scm" | "ss" | "rkt" | "asm"
        | "s" => ";",
        // C-family and friends (also the safe default)
        _ => "//",
    }
}

/// The type of conflict between two branches' changes to an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictKind {
    /// Both branches modified the same entity and the changes couldn't be merged.
    BothModified,
    /// One branch modified the entity while the other deleted it.
    ModifyDelete { modified_in_ours: bool },
    /// Both branches added an entity with the same ID but different content.
    BothAdded,
    /// Both branches renamed the same entity to different names.
    RenameRename {
        base_name: String,
        ours_name: String,
        theirs_name: String,
    },
    /// One branch renamed the entity, the other modified it.
    RenameModify {
        old_name: String,
        new_name: String,
        renamed_in_ours: bool,
    },
}

impl ConflictKind {
    /// The stable wire tag: the `kind` refinement of a `class=CONFLICT`
    /// finding in `weave-findings.schema.json`.
    ///
    /// It lives here, next to the enum it projects, for one reason: the match
    /// is exhaustive, so a new `ConflictKind` variant cannot compile until
    /// someone has answered what it is called on the wire. While the two
    /// producers (`weave-cli`'s patch reporter and `weave-mcp`'s findings
    /// document) each carried their own copy of this match, a new variant
    /// would have landed in whichever catch-all arm they had grown by then —
    /// and the two documents would have named it differently.
    ///
    /// Naming is all the authority this grants. It decides nothing about the
    /// conflict, which is why weave-core may hold it without knowing anything
    /// about the schema beyond the tag.
    pub fn wire_kind(&self) -> &'static str {
        match self {
            ConflictKind::BothModified => "both_modified",
            ConflictKind::ModifyDelete { .. } => "modify_delete",
            ConflictKind::BothAdded => "both_added",
            ConflictKind::RenameRename { .. } => "rename_rename",
            ConflictKind::RenameModify { .. } => "rename_modify",
        }
    }
}

impl fmt::Display for ConflictKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictKind::BothModified => write!(f, "both modified"),
            ConflictKind::ModifyDelete {
                modified_in_ours: true,
            } => write!(f, "modified in ours, deleted in theirs"),
            ConflictKind::ModifyDelete {
                modified_in_ours: false,
            } => write!(f, "deleted in ours, modified in theirs"),
            ConflictKind::BothAdded => write!(f, "both added"),
            ConflictKind::RenameRename {
                base_name,
                ours_name,
                theirs_name,
            } => {
                write!(
                    f,
                    "both renamed: '{}' → ours '{}', theirs '{}'",
                    base_name, ours_name, theirs_name
                )
            }
            ConflictKind::RenameModify {
                old_name,
                new_name,
                renamed_in_ours: true,
            } => {
                write!(
                    f,
                    "renamed in ours ('{}' → '{}'), modified in theirs",
                    old_name, new_name
                )
            }
            ConflictKind::RenameModify {
                old_name,
                new_name,
                renamed_in_ours: false,
            } => {
                write!(
                    f,
                    "modified in ours, renamed in theirs ('{}' → '{}')",
                    old_name, new_name
                )
            }
        }
    }
}

/// Conflict complexity classification.
///
/// Helps agents and tools choose appropriate resolution strategies:
/// - Text: trivial, usually auto-resolvable (comment changes)
/// - Syntax: signature/type changes, may need type-checking
/// - Functional: body logic changes, needs careful review
/// - Composite variants indicate multiple dimensions of change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictComplexity {
    /// Only text/comment/string changes
    Text,
    /// Signature, type, or structural changes (no body changes)
    Syntax,
    /// Function body / logic changes
    Functional,
    /// Both text and syntax changes
    TextSyntax,
    /// Both text and functional changes
    TextFunctional,
    /// Both syntax and functional changes
    SyntaxFunctional,
    /// All three dimensions changed
    TextSyntaxFunctional,
    /// Could not classify (e.g., unknown entity type)
    Unknown,
}

impl ConflictComplexity {
    /// How much a reader should trust an automatic answer here. One owner: the
    /// entity-level marker and the scoped marker say the same word for the same
    /// complexity, or the two artefacts are not comparable.
    pub fn confidence(&self) -> &'static str {
        match self {
            ConflictComplexity::Text => "high",
            ConflictComplexity::Syntax
            | ConflictComplexity::Functional
            | ConflictComplexity::TextSyntax
            | ConflictComplexity::TextFunctional => "medium",
            ConflictComplexity::SyntaxFunctional | ConflictComplexity::TextSyntaxFunctional => {
                "low"
            }
            ConflictComplexity::Unknown => "unknown",
        }
    }

    /// Human-readable resolution hint for this conflict type.
    pub fn resolution_hint(&self) -> &'static str {
        match self {
            ConflictComplexity::Text => {
                "Cosmetic change on both sides. Pick either version or combine formatting."
            }
            ConflictComplexity::Syntax => {
                "Structural change (rename/retype). Check callers of this entity."
            }
            ConflictComplexity::Functional => {
                "Logic changed on both sides. Requires understanding intent of each change."
            }
            ConflictComplexity::TextSyntax => {
                "Renamed and reformatted. Prefer the structural change, verify formatting."
            }
            ConflictComplexity::TextFunctional => {
                "Logic and cosmetic changes overlap. Resolve logic first, then reformat."
            }
            ConflictComplexity::SyntaxFunctional => {
                "Structural and logic conflict. Both design and behavior differ."
            }
            ConflictComplexity::TextSyntaxFunctional => {
                "All three dimensions conflict. Manual review required."
            }
            ConflictComplexity::Unknown => "Could not classify. Compare both versions manually.",
        }
    }
}

impl fmt::Display for ConflictComplexity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictComplexity::Text => write!(f, "T"),
            ConflictComplexity::Syntax => write!(f, "S"),
            ConflictComplexity::Functional => write!(f, "F"),
            ConflictComplexity::TextSyntax => write!(f, "T+S"),
            ConflictComplexity::TextFunctional => write!(f, "T+F"),
            ConflictComplexity::SyntaxFunctional => write!(f, "S+F"),
            ConflictComplexity::TextSyntaxFunctional => write!(f, "T+S+F"),
            ConflictComplexity::Unknown => write!(f, "?"),
        }
    }
}

/// Classify conflict complexity by analyzing what changed between versions.
pub(crate) fn classify_conflict(
    base: Option<&str>,
    ours: Option<&str>,
    theirs: Option<&str>,
) -> ConflictComplexity {
    let base = base.unwrap_or("");
    let ours = ours.unwrap_or("");
    let theirs = theirs.unwrap_or("");

    // Compare ours and theirs changes vs base
    let ours_diff = classify_change(base, ours);
    let theirs_diff = classify_change(base, theirs);

    // Merge the dimensions
    let has_text = ours_diff.text || theirs_diff.text;
    let has_syntax = ours_diff.syntax || theirs_diff.syntax;
    let has_functional = ours_diff.functional || theirs_diff.functional;

    match (has_text, has_syntax, has_functional) {
        (true, false, false) => ConflictComplexity::Text,
        (false, true, false) => ConflictComplexity::Syntax,
        (false, false, true) => ConflictComplexity::Functional,
        (true, true, false) => ConflictComplexity::TextSyntax,
        (true, false, true) => ConflictComplexity::TextFunctional,
        (false, true, true) => ConflictComplexity::SyntaxFunctional,
        (true, true, true) => ConflictComplexity::TextSyntaxFunctional,
        (false, false, false) => ConflictComplexity::Unknown,
    }
}

struct ChangeDimensions {
    text: bool,
    syntax: bool,
    functional: bool,
}

/// Find the end of the signature in a function/method definition.
/// Handles multi-line parameter lists by tracking parenthesis depth.
/// Returns the index (exclusive) of the first body line after the signature.
fn find_signature_end(lines: &[&str]) -> usize {
    if lines.is_empty() {
        return 0;
    }
    let mut depth: i32 = 0;
    for (i, line) in lines.iter().enumerate() {
        for ch in line.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                '{' | ':' if depth <= 0 && i > 0 => {
                    // Body start: signature ends at this line (inclusive)
                    return i + 1;
                }
                _ => {}
            }
        }
        // If we opened parens and closed them all on this line or previous,
        // and the next line starts a body block, the signature ends here
        if depth <= 0 && i > 0 {
            // Check if this line ends the signature (has closing paren and body opener)
            let trimmed = line.trim();
            if trimmed.ends_with('{') || trimmed.ends_with(':') || trimmed.ends_with("->") {
                return i + 1;
            }
        }
    }
    // Fallback: first line is signature
    1
}

fn classify_change(base: &str, modified: &str) -> ChangeDimensions {
    if base == modified {
        return ChangeDimensions {
            text: false,
            syntax: false,
            functional: false,
        };
    }

    let base_lines: Vec<&str> = base.lines().collect();
    let modified_lines: Vec<&str> = modified.lines().collect();

    let mut has_comment_change = false;
    let mut has_signature_change = false;
    let mut has_body_change = false;

    // Find signature end for multi-line signatures
    let base_sig_end = find_signature_end(&base_lines);
    let mod_sig_end = find_signature_end(&modified_lines);

    // Check signature (may span multiple lines)
    let base_sig: Vec<&str> = base_lines.iter().take(base_sig_end).copied().collect();
    let mod_sig: Vec<&str> = modified_lines.iter().take(mod_sig_end).copied().collect();
    if base_sig != mod_sig {
        let all_comments = base_sig.iter().all(|l| is_comment_line(l))
            && mod_sig.iter().all(|l| is_comment_line(l));
        if all_comments {
            has_comment_change = true;
        } else {
            has_signature_change = true;
        }
    }

    // Check body lines
    let base_body: Vec<&str> = base_lines.iter().skip(base_sig_end).copied().collect();
    let mod_body: Vec<&str> = modified_lines.iter().skip(mod_sig_end).copied().collect();

    if base_body != mod_body {
        // Check if changes are only in comments
        let base_no_comments: Vec<&str> = base_body
            .iter()
            .filter(|l| !is_comment_line(l))
            .copied()
            .collect();
        let mod_no_comments: Vec<&str> = mod_body
            .iter()
            .filter(|l| !is_comment_line(l))
            .copied()
            .collect();

        if base_no_comments == mod_no_comments {
            has_comment_change = true;
        } else {
            has_body_change = true;
        }
    }

    ChangeDimensions {
        text: has_comment_change,
        syntax: has_signature_change,
        functional: has_body_change,
    }
}

fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("*")
        || trimmed.starts_with("#")
        || trimmed.starts_with("\"\"\"")
        || trimmed.starts_with("'''")
}

/// A conflict on a specific entity.
#[derive(Debug, Clone)]
pub struct EntityConflict {
    pub entity_name: String,
    pub entity_type: String,
    pub kind: ConflictKind,
    pub complexity: ConflictComplexity,
    pub ours_content: Option<String>,
    pub theirs_content: Option<String>,
    pub base_content: Option<String>,
}

// ---------------------------------------------------------------------------
// The one ruler: a conflict is a HOLE in the partition
// ---------------------------------------------------------------------------

/// One conflict box, cut once.
///
/// A conflict is a hole in a partition. The FRAME is that partition minus the
/// hole — text every side that has an opinion agrees on, which is why it can sit
/// outside the markers as ordinary source. Each side's marker content is exactly
/// that side's bytes of the hole, cut at the SAME offsets the frame was cut at.
///
/// The identity that makes it a partition, and the property this type exists to
/// hold: for every rendered side `S`,
///
/// ```text
///     prefix ++ hole(S) ++ suffix  ==  S
/// ```
///
/// so a reader who keeps the frame and one side's hole gets that side's text
/// back byte for byte, and no line can be both in the frame and in a hole.
///
/// **There used to be two slicers.** `EntityConflict::to_conflict_markers` and
/// `merge::scoped_conflict_marker` each carried their own copy of this cut, and
/// the copies had drifted: one of them sliced the BASE section at *ours'*
/// offsets, under the comment "use prefix/suffix from ours/theirs narrowing as
/// approximation". An approximation is exactly what a partition may not be. Base
/// lines were duplicated into the frame or dropped from the box depending on how
/// far ours and theirs happened to agree — a second ruler, measuring a different
/// thing, and text that belongs to no piece of a partition has to end up either
/// duplicated or lost.
///
/// So the cut is taken over the sides that will actually be RENDERED, and
/// nothing else. In enhanced markers base is not rendered — weave states the
/// refusal instead — so the frame is what the two claimants agree on. In standard
/// diff3 markers base IS rendered, so it is one of the sides the frame must
/// agree with, and the identity above holds for all three.
pub(crate) struct ConflictBox<'a> {
    lines: Vec<Option<Vec<&'a str>>>,
    prefix: usize,
    suffix: usize,
}

/// Which slot of a box a text belongs to. The order is the order git writes
/// them, and it is the order [`ConflictBox::side`] indexes.
#[derive(Clone, Copy)]
pub(crate) enum BoxSide {
    Ours = 0,
    Base = 1,
    Theirs = 2,
}

impl<'a> ConflictBox<'a> {
    /// Cut the hole. `base` is `None` whenever base will not be rendered — the
    /// frame is agreed among the sides a reader can see, and a side nobody
    /// renders has no vote on where the box begins.
    pub(crate) fn cut(
        ours: Option<&'a str>,
        base: Option<&'a str>,
        theirs: Option<&'a str>,
    ) -> Self {
        let lines: Vec<Option<Vec<&str>>> = [ours, base, theirs]
            .into_iter()
            .map(|s| s.map(|s| s.lines().collect::<Vec<&str>>()))
            .collect();
        let present: Vec<&Vec<&str>> = lines.iter().flatten().collect();
        // One side present is not a disagreement — a modify/delete states the
        // whole surviving text and narrowing it against nothing is meaningless.
        if present.len() < 2 {
            return ConflictBox {
                lines,
                prefix: 0,
                suffix: 0,
            };
        }
        let shortest = present.iter().map(|l| l.len()).min().unwrap_or(0);
        let prefix = (0..shortest)
            .take_while(|&i| present.iter().all(|l| l[i] == present[0][i]))
            .count();
        let room = present.iter().map(|l| l.len() - prefix).min().unwrap_or(0);
        let suffix = (0..room)
            .take_while(|&k| {
                present
                    .iter()
                    .all(|l| l[l.len() - 1 - k] == present[0][present[0].len() - 1 - k])
            })
            .count();
        ConflictBox {
            lines,
            prefix,
            suffix,
        }
    }

    /// The frame before the box: lines every rendered side opens with.
    pub(crate) fn frame_prefix(&self) -> &[&'a str] {
        self.lines
            .iter()
            .flatten()
            .next()
            .map(|l| &l[..self.prefix])
            .unwrap_or(&[])
    }

    /// The frame after the box.
    pub(crate) fn frame_suffix(&self) -> &[&'a str] {
        self.lines
            .iter()
            .flatten()
            .next()
            .map(|l| &l[l.len() - self.suffix..])
            .unwrap_or(&[])
    }

    /// One side's bytes of the hole, or `None` when that side has no text here.
    pub(crate) fn side(&self, which: BoxSide) -> Option<&[&'a str]> {
        self.lines[which as usize]
            .as_ref()
            .map(|l| &l[self.prefix..l.len() - self.suffix])
    }
}

/// Append `lines` to `out`, one newline each.
pub(crate) fn push_lines(out: &mut String, lines: &[&str]) {
    for l in lines {
        out.push_str(l);
        out.push('\n');
    }
}

impl EntityConflict {
    /// Render this conflict as conflict markers.
    ///
    /// When `fmt.enhanced` is true, includes entity metadata and the
    /// `refused_by:` line. When false, outputs standard git-compatible diff3
    /// markers.
    ///
    /// The cut is [`ConflictBox`]'s and nothing else's: whatever every rendered
    /// side agrees on at the edges becomes frame, and each side's marker content
    /// is its own bytes between those two offsets. Base is handed to the cut
    /// only in standard mode, because that is the only mode that renders it.
    pub(crate) fn to_conflict_markers(&self, fmt: &MarkerFormat, guard: &str) -> String {
        let open = "<".repeat(fmt.marker_length);
        let sep = "=".repeat(fmt.marker_length);
        let close = ">".repeat(fmt.marker_length);

        let hole = ConflictBox::cut(
            self.ours_content.as_deref().or(Some("")),
            (!fmt.enhanced).then(|| self.base_content.as_deref().unwrap_or("")),
            self.theirs_content.as_deref().or(Some("")),
        );

        let mut out = String::new();
        push_lines(&mut out, hole.frame_prefix());

        if fmt.enhanced {
            let confidence = self.complexity.confidence();
            let label = format!(
                "{} `{}` ({}, confidence: {})",
                self.entity_type, self.entity_name, self.complexity, confidence
            );
            out.push_str(&format!("{} ours \u{2014} {}\n", open, label));
            out.push_str(&refusal_line(
                &fmt.comment_prefix,
                guard,
                self.base_content.as_deref(),
                self.ours_content.as_deref(),
                self.theirs_content.as_deref(),
            ));
        } else {
            out.push_str(&format!("{} ours\n", open));
        }

        push_lines(&mut out, hole.side(BoxSide::Ours).unwrap_or(&[]));

        if !fmt.enhanced {
            out.push_str(&format!("{} base\n", "|".repeat(fmt.marker_length)));
            push_lines(&mut out, hole.side(BoxSide::Base).unwrap_or(&[]));
        }

        out.push_str(&format!("{}\n", sep));
        push_lines(&mut out, hole.side(BoxSide::Theirs).unwrap_or(&[]));

        if fmt.enhanced {
            let confidence = self.complexity.confidence();
            let label = format!(
                "{} `{}` ({}, confidence: {})",
                self.entity_type, self.entity_name, self.complexity, confidence
            );
            out.push_str(&format!("{} theirs \u{2014} {}\n", close, label));
        } else {
            out.push_str(&format!("{} theirs\n", close));
        }

        push_lines(&mut out, hole.frame_suffix());
        out
    }
}

// ---------------------------------------------------------------------------
// Coalescing: one question, one box
// ---------------------------------------------------------------------------

/// How much agreed text may sit between two boxes of the same scope before they
/// are two questions rather than one.
///
/// In Java the gap between two conflicted members of one class is `}`, a blank
/// line, and the `/**` that opens the next member's doc comment — four lines. In
/// Python it is a blank line and a decorator. Six admits an annotation on top of
/// that and stops well short of a whole member, which is the thing that must
/// never be swallowed: text nobody contests, hoisted into a marker, is a reader
/// being asked to arbitrate something both sides agree on.
const MAX_INTERSTITIAL_LINES: usize = 6;

/// One conflict box, as it appears in a rendered file.
struct RenderedBox {
    open: String,
    /// The `refused_by:` line. Marker furniture, not either side's claim —
    /// dropped when two boxes merge, which is safe precisely because no
    /// resolution keeps it.
    refusal: Option<String>,
    ours: Vec<String>,
    base: Option<Vec<String>>,
    theirs: Vec<String>,
    close: String,
    /// The text after `ours — ` on the opening line: what this box is about.
    scope: String,
}

/// A rendered file, read as the alternation it is.
enum Piece {
    Frame(Vec<String>),
    Box(RenderedBox),
}

/// Fold runs of same-scope boxes separated by a short agreed gap into one box.
///
/// A file with one real disagreement can otherwise print a wall of markers:
/// twenty boxes, every one of them labelled `scope 'then'`, each separated
/// from the next by `}` / blank / `/**`. Twenty boxes is twenty questions, and
/// there was one — the two sides had rewritten the same run of overloads.
/// Without coalescing, a reader faced with that wall reasonably routes around
/// it entirely rather than reading twenty near-duplicate boxes.
///
/// **The gap is duplicated into both halves, and that is what makes this
/// safe.** Merging boxes `B1`, `B2` separated by frame `F` gives ours =
/// `B1.ours ++ F ++ B2.ours` and theirs = `B1.theirs ++ F ++ B2.theirs`, so
/// under either standard resolution the output is byte-for-byte what it was
/// before — the reader keeps the frame and one claim each way, and `F` is in
/// both claims. Coalescing therefore cannot change the merge's content, only how
/// many decisions the reader is asked to make about it. That property is
/// asserted, not argued (`coalescing_changes_no_resolution`).
///
/// Scope equality is required, so two boxes about different members stay two
/// questions no matter how close they sit. The per-scope detail a coalesced box
/// stops spelling out in the artifact is not lost: `MergeResult::conflicts`
/// still carries one `EntityConflict` per contested entity, which is where a
/// reader who wants the breakdown should be reading it from anyway.
pub(crate) fn coalesce_adjacent_boxes(content: &str, fmt: &MarkerFormat) -> String {
    let pieces = read_pieces(content, fmt);
    if pieces.iter().filter(|p| matches!(p, Piece::Box(_))).count() < 2 {
        return content.to_string();
    }
    let trailing_newline = content.ends_with('\n');

    let mut out: Vec<Piece> = Vec::new();
    for piece in pieces {
        // A frame short enough to be a gap, sitting between two same-scope
        // boxes, is absorbed by the merge of the two; anything else is frame and
        // stays frame.
        match (out.len() >= 2, &piece) {
            (true, Piece::Box(next)) => {
                let mergeable = match (&out[out.len() - 2], &out[out.len() - 1]) {
                    (Piece::Box(prev), Piece::Frame(gap)) => {
                        prev.scope == next.scope && gap.len() <= MAX_INTERSTITIAL_LINES
                    }
                    _ => false,
                };
                if mergeable {
                    let Some(Piece::Frame(gap)) = out.pop() else {
                        unreachable!("just matched a Frame")
                    };
                    let Some(Piece::Box(prev)) = out.pop() else {
                        unreachable!("just matched a Box")
                    };
                    let Piece::Box(next) = piece else {
                        unreachable!("just matched a Box")
                    };
                    out.push(Piece::Box(merge_boxes(prev, gap, next)));
                    continue;
                }
                out.push(piece);
            }
            _ => out.push(piece),
        }
    }

    let mut s = String::new();
    for piece in &out {
        match piece {
            Piece::Frame(lines) => {
                for l in lines {
                    s.push_str(l);
                    s.push('\n');
                }
            }
            Piece::Box(b) => {
                s.push_str(&b.open);
                s.push('\n');
                if let Some(h) = &b.refusal {
                    s.push_str(h);
                    s.push('\n');
                }
                for l in &b.ours {
                    s.push_str(l);
                    s.push('\n');
                }
                if let Some(base) = &b.base {
                    s.push_str(&format!("{} base\n", "|".repeat(fmt.marker_length)));
                    for l in base {
                        s.push_str(l);
                        s.push('\n');
                    }
                }
                s.push_str(&format!("{}\n", "=".repeat(fmt.marker_length)));
                for l in &b.theirs {
                    s.push_str(l);
                    s.push('\n');
                }
                s.push_str(&b.close);
                s.push('\n');
            }
        }
    }
    if !trailing_newline {
        while s.ends_with('\n') {
            s.pop();
        }
    }
    s
}

/// `B1 ++ gap ++ B2`, with the gap in BOTH claims — the identity the whole pass
/// rests on.
fn merge_boxes(mut prev: RenderedBox, gap: Vec<String>, next: RenderedBox) -> RenderedBox {
    prev.ours.extend(gap.iter().cloned());
    prev.ours.extend(next.ours);
    prev.theirs.extend(gap.iter().cloned());
    prev.theirs.extend(next.theirs);
    prev.base = match (prev.base, next.base) {
        (Some(mut b1), Some(b2)) => {
            b1.extend(gap);
            b1.extend(b2);
            Some(b1)
        }
        (b1, b2) => b1.or(b2),
    };
    prev.close = next.close;
    prev
}

/// Read a rendered file as the alternation of frame and boxes that it is.
///
/// A marker line that does not open a well-formed box — a source line quoting
/// `<<<<<<<`, a truncated artifact — leaves the whole file untouched, because
/// this pass is not entitled to guess at text it cannot parse.
fn read_pieces(content: &str, fmt: &MarkerFormat) -> Vec<Piece> {
    let lines: Vec<&str> = content.lines().collect();
    let open_m = "<".repeat(fmt.marker_length);
    let base_m = "|".repeat(fmt.marker_length);
    let sep_m = "=".repeat(fmt.marker_length);
    let close_m = ">".repeat(fmt.marker_length);

    let mut pieces: Vec<Piece> = Vec::new();
    let mut frame: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if !lines[i].starts_with(&open_m) {
            frame.push(lines[i].to_string());
            i += 1;
            continue;
        }
        let open = lines[i].to_string();
        let scope = open
            .split_once(" \u{2014} ")
            .map(|(_, rest)| rest.to_string())
            .unwrap_or_else(|| open.clone());
        i += 1;
        let refusal = (i < lines.len() && refusal_body(lines[i]).is_some()).then(|| {
            let h = lines[i].to_string();
            i += 1;
            h
        });
        let take = |i: &mut usize, stop: &[&str]| -> Vec<String> {
            let mut v = Vec::new();
            while *i < lines.len() && !stop.iter().any(|m| lines[*i].starts_with(m)) {
                v.push(lines[*i].to_string());
                *i += 1;
            }
            v
        };
        let ours = take(&mut i, &[&base_m, &sep_m, &close_m, &open_m]);
        let base = (i < lines.len() && lines[i].starts_with(&base_m)).then(|| {
            i += 1;
            take(&mut i, &[&sep_m, &close_m, &open_m])
        });
        if i >= lines.len() || !lines[i].starts_with(&sep_m) {
            return vec![Piece::Frame(content.lines().map(str::to_string).collect())];
        }
        i += 1;
        let theirs = take(&mut i, &[&close_m, &open_m]);
        if i >= lines.len() || !lines[i].starts_with(&close_m) {
            return vec![Piece::Frame(content.lines().map(str::to_string).collect())];
        }
        let close = lines[i].to_string();
        i += 1;
        pieces.push(Piece::Frame(std::mem::take(&mut frame)));
        pieces.push(Piece::Box(RenderedBox {
            open,
            refusal,
            ours,
            base,
            theirs,
            close,
            scope,
        }));
    }
    pieces.push(Piece::Frame(frame));
    pieces
}

/// A parsed conflict extracted from weave-enhanced conflict markers.
#[derive(Debug, Clone)]
pub struct ParsedConflict {
    pub entity_name: String,
    pub entity_kind: String,
    pub complexity: ConflictComplexity,
    pub confidence: String,
    /// The body of the box's one annotation line — `<guard> · collision: …`.
    pub refusal: String,
    pub ours_content: String,
    pub theirs_content: String,
}

/// Parse weave-enhanced conflict markers from merged file content.
///
/// Returns a `Vec<ParsedConflict>` for each conflict block found.
/// Expects markers in the format produced by `EntityConflict::to_conflict_markers()`.
pub fn parse_weave_conflicts(content: &str) -> Vec<ParsedConflict> {
    let mut conflicts = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        // Look for <<<<<<< ours — <type> `<name>` (<complexity>, confidence: <conf>)
        if lines[i].starts_with("<<<<<<< ours") {
            let header = lines[i];
            let (entity_kind, entity_name, complexity, confidence) = parse_conflict_header(header);

            i += 1;

            // Read the refusal line. The prefix is whatever comment syntax the
            // file uses (`//`, `#`, `--`, `;`), so the reader matches the
            // marker, not one language's spelling of it.
            let mut refusal = String::new();
            if let Some(rest) = lines.get(i).and_then(|l| refusal_body(l)) {
                refusal = rest.to_string();
                i += 1;
            }

            // Read ours content until =======
            let mut ours_lines = Vec::new();
            while i < lines.len() && lines[i] != "=======" {
                ours_lines.push(lines[i]);
                i += 1;
            }
            i += 1; // skip =======

            // Read theirs content until >>>>>>>
            let mut theirs_lines = Vec::new();
            while i < lines.len() && !lines[i].starts_with(">>>>>>> theirs") {
                theirs_lines.push(lines[i]);
                i += 1;
            }
            i += 1; // skip >>>>>>>

            let ours_content = if ours_lines.is_empty() {
                String::new()
            } else {
                ours_lines.join("\n") + "\n"
            };
            let theirs_content = if theirs_lines.is_empty() {
                String::new()
            } else {
                theirs_lines.join("\n") + "\n"
            };

            conflicts.push(ParsedConflict {
                entity_name,
                entity_kind,
                complexity,
                confidence,
                refusal,
                ours_content,
                theirs_content,
            });
        } else {
            i += 1;
        }
    }

    conflicts
}

fn parse_conflict_header(header: &str) -> (String, String, ConflictComplexity, String) {
    // Format: "<<<<<<< ours — <type> `<name>` (<complexity>, confidence: <conf>)"
    let after_dash = header.split('\u{2014}').nth(1).unwrap_or(header).trim();

    // Extract entity type (word before backtick)
    let entity_kind = after_dash
        .split('`')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    // Extract entity name (between backticks)
    let entity_name = after_dash.split('`').nth(1).unwrap_or("").to_string();

    // Extract complexity and confidence from parenthesized section
    let paren_content = after_dash
        .rsplit('(')
        .next()
        .unwrap_or("")
        .trim_end_matches(')');

    let parts: Vec<&str> = paren_content.split(',').map(|s| s.trim()).collect();
    let complexity = match parts.first().copied().unwrap_or("") {
        "T" => ConflictComplexity::Text,
        "S" => ConflictComplexity::Syntax,
        "F" => ConflictComplexity::Functional,
        "T+S" => ConflictComplexity::TextSyntax,
        "T+F" => ConflictComplexity::TextFunctional,
        "S+F" => ConflictComplexity::SyntaxFunctional,
        "T+S+F" => ConflictComplexity::TextSyntaxFunctional,
        _ => ConflictComplexity::Unknown,
    };

    let confidence = parts
        .iter()
        .find(|p| p.starts_with("confidence:"))
        .map(|p| p.trim_start_matches("confidence:").trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    (entity_kind, entity_name, complexity, confidence)
}

/// Statistics about a merge operation.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MergeStats {
    pub entities_unchanged: usize,
    pub entities_ours_only: usize,
    pub entities_theirs_only: usize,
    pub entities_both_changed_merged: usize,
    pub entities_conflicted: usize,
    pub entities_added_ours: usize,
    pub entities_added_theirs: usize,
    pub entities_deleted: usize,
    pub used_fallback: bool,
    /// Entities that were auto-merged but reference other modified entities.
    pub semantic_warnings: usize,
    /// Entities resolved via diffy 3-way merge. Caps [`MergeStats::confidence`]
    /// at `high`.
    pub resolved_via_diffy: usize,
    /// Entities resolved via inner entity merge — the deeper decomposition, so
    /// the weaker claim. Caps [`MergeStats::confidence`] at `medium`.
    pub resolved_via_inner_merge: usize,
    /// Call references rewritten by the binding pass because the other side
    /// renamed the definition they pointed at (reference repair).
    pub references_rewritten: usize,
}

impl MergeStats {
    pub fn has_conflicts(&self) -> bool {
        self.entities_conflicted > 0
    }

    /// Tally one finished decision into the summary: which line it lands on,
    /// and which confidence bucket the strategy that reached it belongs to.
    ///
    /// This is the *other* half of giving the table one owner. Moving the
    /// multi-file fold here left the per-file fold in `v2`, so the same nine
    /// fields still had two writing modules — a smaller version of the same
    /// finding, and the check said so. The mapping from a `Tally` to a line of
    /// the summary is a fact about the summary, so it lives beside the lines it
    /// names, and `v2::summarize` now walks the decisions and calls this rather
    /// than reaching into the fields itself.
    pub(crate) fn record(&mut self, tally: &crate::v2::Tally, strategy: &ResolutionStrategy) {
        use crate::v2::Tally;
        match tally {
            Tally::Unchanged => self.entities_unchanged += 1,
            Tally::OursOnly => self.entities_ours_only += 1,
            Tally::TheirsOnly => self.entities_theirs_only += 1,
            Tally::BothMerged => self.entities_both_changed_merged += 1,
            Tally::AddedOurs => self.entities_added_ours += 1,
            Tally::AddedTheirs => self.entities_added_theirs += 1,
            Tally::Deleted => self.entities_deleted += 1,
            Tally::Conflicted => self.entities_conflicted += 1,
        }
        match strategy {
            ResolutionStrategy::DiffyMerged | ResolutionStrategy::DecoratorMerged => {
                self.resolved_via_diffy += 1
            }
            ResolutionStrategy::InnerMerged | ResolutionStrategy::StatementMerged => {
                self.resolved_via_inner_merge += 1
            }
            _ => {}
        }
    }

    /// This merge did not go through the entity model.
    ///
    /// `used_fallback` used to be a flag with four writing sites across two
    /// crates and no owner — whichever site ran last decided the value, and
    /// nothing in either signature said so. The sites are all saying the same
    /// thing, so they say it by name now instead of by assignment.
    pub(crate) fn mark_fallback(&mut self) {
        self.used_fallback = true;
    }

    /// Fold one file's summary into a multi-file total.
    ///
    /// A merge summarises one file; a pull request is many, and someone has to
    /// add them up. That someone is here, once, because the alternative is what
    /// used to happen: `weave-github` summed the table by hand, field by
    /// field, in another crate, and nine fields of this struct had two
    /// writing modules with nothing in either signature to say so. Whether the
    /// two agreed about what `used_fallback` means across a multi-file merge was
    /// a property of two `+=` lists staying in step.
    ///
    /// They were not in step. The hand-written fold covered twelve of the
    /// thirteen fields and dropped `references_rewritten`, so every call site
    /// the binding pass repaired vanished from the repository total — a silent
    /// zero, not a wrong number, which is why nobody saw it.
    ///
    /// Hence the destructuring: the pattern is exhaustive, so adding a field to
    /// the table is a compile error here rather than a line someone remembers to
    /// add. The compiler is the reviewer.
    pub fn absorb(&mut self, file: &MergeStats) {
        let MergeStats {
            entities_unchanged,
            entities_ours_only,
            entities_theirs_only,
            entities_both_changed_merged,
            entities_conflicted,
            entities_added_ours,
            entities_added_theirs,
            entities_deleted,
            used_fallback,
            semantic_warnings,
            resolved_via_diffy,
            resolved_via_inner_merge,
            references_rewritten,
        } = file;
        self.entities_unchanged += entities_unchanged;
        self.entities_ours_only += entities_ours_only;
        self.entities_theirs_only += entities_theirs_only;
        self.entities_both_changed_merged += entities_both_changed_merged;
        self.entities_conflicted += entities_conflicted;
        self.entities_added_ours += entities_added_ours;
        self.entities_added_theirs += entities_added_theirs;
        self.entities_deleted += entities_deleted;
        // Any file falling back makes the whole merge a fallback merge.
        self.used_fallback |= used_fallback;
        self.semantic_warnings += semantic_warnings;
        self.resolved_via_diffy += resolved_via_diffy;
        self.resolved_via_inner_merge += resolved_via_inner_merge;
        self.references_rewritten += references_rewritten;
    }

    /// Entities weave composed rather than handed back: the three lines that
    /// mean "a change survived that the other side did not make".
    ///
    /// One owner, because it is a claim about what the merge achieved and
    /// three consumers were each spelling the sum out for themselves (the
    /// driver's report, the lifetime stats, the GitHub comment) — three places
    /// to update the day a line is added to the table.
    pub fn auto_resolved(&self) -> usize {
        self.entities_ours_only + self.entities_theirs_only + self.entities_both_changed_merged
    }

    /// Overall merge confidence, as one of four stable strings, weakest first:
    ///
    /// * `conflict`  — something was left for a human.
    /// * `medium`    — an inner entity merge or a line-level fallback carried it.
    /// * `high`      — diffy carried at least one entity body.
    /// * `very_high` — no entity needed a body-level merge at all.
    ///
    /// The order is deliberate: the further down into an entity weave had to go
    /// to get a clean answer, the less the clean answer is worth. `stats.rs`
    /// matches these four strings by name.
    pub fn confidence(&self) -> &'static str {
        if self.entities_conflicted > 0 {
            "conflict"
        } else if self.resolved_via_inner_merge > 0 || self.used_fallback {
            "medium"
        } else if self.resolved_via_diffy > 0 {
            "high"
        } else {
            "very_high"
        }
    }
}

impl fmt::Display for MergeStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unchanged: {}", self.entities_unchanged)?;
        if self.entities_ours_only > 0 {
            write!(f, ", ours-only: {}", self.entities_ours_only)?;
        }
        if self.entities_theirs_only > 0 {
            write!(f, ", theirs-only: {}", self.entities_theirs_only)?;
        }
        if self.entities_both_changed_merged > 0 {
            write!(f, ", auto-merged: {}", self.entities_both_changed_merged)?;
        }
        if self.entities_added_ours > 0 {
            write!(f, ", added-ours: {}", self.entities_added_ours)?;
        }
        if self.entities_added_theirs > 0 {
            write!(f, ", added-theirs: {}", self.entities_added_theirs)?;
        }
        if self.entities_deleted > 0 {
            write!(f, ", deleted: {}", self.entities_deleted)?;
        }
        if self.entities_conflicted > 0 {
            write!(f, ", CONFLICTS: {}", self.entities_conflicted)?;
        }
        if self.semantic_warnings > 0 {
            write!(f, ", semantic-warnings: {}", self.semantic_warnings)?;
        }
        if self.used_fallback {
            write!(f, " (line-level fallback)")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one-ruler rule, as an identity: for every side the box RENDERS,
    ///
    /// ```text
    ///     frame_prefix ++ side(S) ++ frame_suffix  ==  S
    /// ```
    ///
    /// This is what `narrow_conflict_lines` could not state, because it handed
    /// out two `usize`s and left every caller to slice with them — and the
    /// caller that sliced BASE at ours' offsets broke exactly this identity,
    /// under a comment that called it an approximation. Asserted for both modes,
    /// because the set of rendered sides is the only thing that differs between
    /// them.
    #[test]
    fn the_frame_and_the_hole_tile_every_side_they_are_cut_over() {
        let cases: &[(&str, &str, &str)] = &[
            // no agreement at all
            ("a\n", "b\n", "c\n"),
            // common prefix and suffix, base diverging inside
            ("h\nB\nt\n", "h\nO\nO2\nt\n", "h\nT\nt\n"),
            // base agrees at the head but not the tail
            ("h\nB\nx\n", "h\nO\nt\n", "h\nT\nt\n"),
            // one side is a strict prefix of the other
            ("a\n", "a\nb\n", "a\n"),
            // whitespace-only sides, the shape that made real files move
            ("\n", "\n\n", "\n"),
            // empty base
            ("", "x\n", "y\n"),
        ];
        for (b, o, t) in cases {
            for render_base in [false, true] {
                let hole = ConflictBox::cut(Some(o), render_base.then_some(*b), Some(t));
                let mut sides = vec![(BoxSide::Ours, *o), (BoxSide::Theirs, *t)];
                if render_base {
                    sides.push((BoxSide::Base, *b));
                }
                for (which, text) in sides {
                    let mut rebuilt = String::new();
                    push_lines(&mut rebuilt, hole.frame_prefix());
                    push_lines(&mut rebuilt, hole.side(which).expect("present side"));
                    push_lines(&mut rebuilt, hole.frame_suffix());
                    let expected: String =
                        text.lines().map(|l| format!("{l}\n")).collect::<String>();
                    assert_eq!(
                        rebuilt, expected,
                        "frame + hole did not tile side of ({b:?}, {o:?}, {t:?}) \
                         with render_base={render_base}"
                    );
                }
            }
        }
    }

    /// A side that is absent states nothing, and a box with one claimant has no
    /// frame: there is nobody to agree with.
    #[test]
    fn a_modify_delete_box_hoists_nothing_out_of_the_markers() {
        let hole = ConflictBox::cut(Some("a\nb\n"), None, None);
        assert!(hole.frame_prefix().is_empty());
        assert!(hole.frame_suffix().is_empty());
        assert_eq!(hole.side(BoxSide::Ours), Some(&["a", "b"][..]));
        assert_eq!(hole.side(BoxSide::Theirs), None);
    }

    /// The cut is over the sides that will be RENDERED. In enhanced markers base
    /// is not one of them, so a base that happens to disagree at the edge cannot
    /// shrink a frame the two claimants agree on.
    #[test]
    fn a_side_nobody_renders_has_no_vote_on_where_the_box_begins() {
        let (b, o, t) = ("different\nB\n", "shared\nO\n", "shared\nT\n");
        let enhanced = ConflictBox::cut(Some(o), None, Some(t));
        assert_eq!(enhanced.frame_prefix(), &["shared"]);
        let standard = ConflictBox::cut(Some(o), Some(b), Some(t));
        assert!(standard.frame_prefix().is_empty());
    }

    // -- coalescing ------------------------------------------------------

    fn boxes(n: usize, scope: &str) -> String {
        let mut s = String::from("head();\n");
        for i in 0..n {
            s.push_str(&format!(
                "<<<<<<< ours \u{2014} scope `{scope}` (S, confidence: low)\n\
                 // refused_by: statement_fold · collision: `ours0();`\n\
                 ours{i}();\n\
                 =======\n\
                 theirs{i}();\n\
                 >>>>>>> theirs \u{2014} scope `{scope}`\n\
                 }}\n\n  /**\n"
            ));
        }
        s.push_str("tail();\n");
        s
    }

    fn count_boxes(s: &str) -> usize {
        s.lines().filter(|l| l.starts_with("<<<<<<<")).count()
    }

    /// The property the whole pass rests on: coalescing changes how many
    /// decisions the reader is asked for and nothing about what the artifact
    /// resolves to, under EITHER standard resolution. Checked against a large
    /// batch of real conflicted merges too — 535 conflicted outputs, 0
    /// resolution differences.
    #[test]
    fn coalescing_changes_no_resolution() {
        let fmt = MarkerFormat::default();
        for n in [2usize, 3, 20] {
            let before = boxes(n, "then");
            let after = coalesce_adjacent_boxes(&before, &fmt);
            assert!(
                count_boxes(&after) < count_boxes(&before),
                "{n} same-scope boxes three lines apart did not coalesce"
            );
            for take in [
                crate::frame::Resolution::Ours,
                crate::frame::Resolution::Theirs,
            ] {
                assert_eq!(
                    crate::frame::resolve_conflicts(&before, take),
                    crate::frame::resolve_conflicts(&after, take),
                    "coalescing {n} boxes changed the {take:?} resolution"
                );
            }
        }
    }

    #[test]
    fn twenty_boxes_about_one_thing_become_one_box() {
        let fmt = MarkerFormat::default();
        assert_eq!(
            count_boxes(&coalesce_adjacent_boxes(&boxes(20, "then"), &fmt)),
            1
        );
    }

    /// Two boxes about different members are two questions however close they
    /// sit. Scope equality is the whole guard against a coalescer that turns a
    /// file into one enormous unreadable claim.
    #[test]
    fn boxes_about_different_scopes_stay_separate() {
        let fmt = MarkerFormat::default();
        let mut src = boxes(1, "alpha");
        src.push_str(&boxes(1, "bravo"));
        assert_eq!(count_boxes(&coalesce_adjacent_boxes(&src, &fmt)), 2);
    }

    /// A whole uncontested member between two boxes is not a gap. Hoisting it
    /// into a marker would ask the reader to arbitrate text both sides agree on.
    #[test]
    fn a_long_agreed_run_between_two_boxes_is_not_a_gap() {
        let fmt = MarkerFormat::default();
        let one = boxes(1, "then");
        let filler: String = (0..MAX_INTERSTITIAL_LINES + 1)
            .map(|i| format!("agreed{i}();\n"))
            .collect();
        let src = format!("{one}{filler}{one}");
        assert_eq!(count_boxes(&coalesce_adjacent_boxes(&src, &fmt)), 2);
    }

    /// diff3 boxes carry a base section, and it is folded the same way the two
    /// claims are — so `prefix ++ base ++ suffix` still reproduces base.
    #[test]
    fn a_diff3_box_keeps_its_base_section_through_a_merge() {
        let fmt = MarkerFormat::standard(7);
        let one = |i: usize| {
            format!("<<<<<<< ours\no{i}\n||||||| base\nb{i}\n=======\nt{i}\n>>>>>>> theirs\ngap\n")
        };
        let src = format!("{}{}", one(1), one(2));
        let out = coalesce_adjacent_boxes(&src, &fmt);
        // Standard markers carry no scope label, so both boxes are "one thing".
        assert_eq!(count_boxes(&out), 1);
        assert!(
            out.contains("b1\ngap\nb2\n"),
            "base halves did not fold: {out}"
        );
        for take in [
            crate::frame::Resolution::Ours,
            crate::frame::Resolution::Theirs,
        ] {
            assert_eq!(
                crate::frame::resolve_conflicts(&src, take),
                crate::frame::resolve_conflicts(&out, take)
            );
        }
    }

    /// Text this pass cannot parse as boxes is text it has no business
    /// rewriting: a source line quoting a marker leaves the file alone.
    #[test]
    fn an_unparseable_marker_leaves_the_file_untouched() {
        let fmt = MarkerFormat::default();
        let src = "a();\n<<<<<<< ours \u{2014} scope `x`\nonly_half();\nb();\n";
        assert_eq!(coalesce_adjacent_boxes(src, &fmt), src);
    }

    #[test]
    fn test_classify_functional_conflict() {
        let base = "function foo() {\n    return 1;\n}\n";
        let ours = "function foo() {\n    return 2;\n}\n";
        let theirs = "function foo() {\n    return 3;\n}\n";
        assert_eq!(
            classify_conflict(Some(base), Some(ours), Some(theirs)),
            ConflictComplexity::Functional
        );
    }

    #[test]
    fn test_classify_syntax_conflict() {
        // Signature changed, body unchanged
        let base = "function foo(a: number) {\n    return a;\n}\n";
        let ours = "function foo(a: string) {\n    return a;\n}\n";
        let theirs = "function foo(a: boolean) {\n    return a;\n}\n";
        assert_eq!(
            classify_conflict(Some(base), Some(ours), Some(theirs)),
            ConflictComplexity::Syntax
        );
    }

    #[test]
    fn test_classify_text_conflict() {
        // Only comment changes
        let base = "// old comment\n    return 1;\n";
        let ours = "// ours comment\n    return 1;\n";
        let theirs = "// theirs comment\n    return 1;\n";
        assert_eq!(
            classify_conflict(Some(base), Some(ours), Some(theirs)),
            ConflictComplexity::Text
        );
    }

    #[test]
    fn test_classify_syntax_functional_conflict() {
        // Signature + body changed
        let base = "function foo(a: number) {\n    return a;\n}\n";
        let ours = "function foo(a: string) {\n    return a + 1;\n}\n";
        let theirs = "function foo(a: boolean) {\n    return a + 2;\n}\n";
        assert_eq!(
            classify_conflict(Some(base), Some(ours), Some(theirs)),
            ConflictComplexity::SyntaxFunctional
        );
    }

    #[test]
    fn test_classify_unknown_when_identical() {
        let content = "function foo() {\n    return 1;\n}\n";
        assert_eq!(
            classify_conflict(Some(content), Some(content), Some(content)),
            ConflictComplexity::Unknown
        );
    }

    #[test]
    fn test_classify_modify_delete() {
        // Theirs deleted (None), ours modified body
        // vs empty: both signature and body differ → SyntaxFunctional
        let base = "function foo() {\n    return 1;\n}\n";
        let ours = "function foo() {\n    return 2;\n}\n";
        assert_eq!(
            classify_conflict(Some(base), Some(ours), None),
            ConflictComplexity::SyntaxFunctional
        );
    }

    #[test]
    fn test_classify_both_added() {
        // No base → comparing each side against empty
        // Both signature and body differ from empty → SyntaxFunctional
        let ours = "function foo() {\n    return 1;\n}\n";
        let theirs = "function foo() {\n    return 2;\n}\n";
        assert_eq!(
            classify_conflict(None, Some(ours), Some(theirs)),
            ConflictComplexity::SyntaxFunctional
        );
    }

    #[test]
    fn test_conflict_markers_include_complexity_and_refusal() {
        let conflict = EntityConflict {
            entity_name: "foo".to_string(),
            entity_type: "function".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::Functional,
            ours_content: Some("return 1;".to_string()),
            theirs_content: Some("return 2;".to_string()),
            base_content: Some("return 0;".to_string()),
        };
        let markers =
            conflict.to_conflict_markers(&MarkerFormat::default(), "merge_ladder_exhausted");
        assert!(
            markers.contains("confidence: medium"),
            "Markers should contain confidence: {}",
            markers
        );
        assert!(
            markers.contains("// refused_by: merge_ladder_exhausted · collision:"),
            "Markers should carry the refusal line: {}",
            markers
        );
    }

    #[test]
    fn the_refusal_line_is_a_comment_in_the_files_language() {
        // The prefix is part of the marker's format, not something a
        // consumer repairs afterwards.
        let conflict = EntityConflict {
            entity_name: "f".to_string(),
            entity_type: "function".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::Text,
            ours_content: Some("def f():\n    return 1\n".to_string()),
            theirs_content: Some("def f():\n    return 2\n".to_string()),
            base_content: Some("def f():\n    return 0\n".to_string()),
        };
        for (path, prefix) in [
            ("a.py", "#"),
            ("q.sql", "--"),
            ("core.clj", ";"),
            ("a.rs", "//"),
            ("no_extension", "//"),
        ] {
            let fmt = MarkerFormat::default().for_file(path);
            let out = conflict.to_conflict_markers(&fmt, "merge_ladder_exhausted");
            let refusal = out
                .lines()
                .find(|l| l.contains("refused_by: "))
                .unwrap_or_else(|| panic!("{path}: no refusal line in:\n{out}"));
            assert!(
                refusal.starts_with(prefix),
                "{path}: refusal must start with {prefix:?}, got {refusal:?}"
            );
            // And it reads back out through the parser, whatever the prefix.
            let parsed = parse_weave_conflicts(&out);
            assert_eq!(parsed.len(), 1, "{path}: {out}");
            assert!(
                !parsed[0].refusal.is_empty(),
                "{path}: refusal lost on the way back"
            );
        }
    }

    #[test]
    fn test_resolution_hints() {
        assert!(ConflictComplexity::Text
            .resolution_hint()
            .contains("Cosmetic"));
        assert!(ConflictComplexity::Syntax
            .resolution_hint()
            .contains("Structural"));
        assert!(ConflictComplexity::Functional
            .resolution_hint()
            .contains("Logic"));
        assert!(ConflictComplexity::TextSyntax
            .resolution_hint()
            .contains("Renamed"));
        assert!(ConflictComplexity::TextFunctional
            .resolution_hint()
            .contains("Logic and cosmetic"));
        assert!(ConflictComplexity::SyntaxFunctional
            .resolution_hint()
            .contains("Structural and logic"));
        assert!(ConflictComplexity::TextSyntaxFunctional
            .resolution_hint()
            .contains("All three"));
        assert!(ConflictComplexity::Unknown
            .resolution_hint()
            .contains("Could not classify"));
    }

    #[test]
    fn test_parse_weave_conflicts() {
        let conflict = EntityConflict {
            entity_name: "process".to_string(),
            entity_type: "function".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::Functional,
            ours_content: Some("fn process() { return 1; }".to_string()),
            theirs_content: Some("fn process() { return 2; }".to_string()),
            base_content: Some("fn process() { return 0; }".to_string()),
        };
        let markers =
            conflict.to_conflict_markers(&MarkerFormat::default(), "merge_ladder_exhausted");

        let parsed = parse_weave_conflicts(&markers);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].entity_name, "process");
        assert_eq!(parsed[0].entity_kind, "function");
        assert_eq!(parsed[0].complexity, ConflictComplexity::Functional);
        assert_eq!(parsed[0].confidence, "medium");
        assert!(parsed[0].refusal.starts_with("merge_ladder_exhausted"));
        assert!(parsed[0].ours_content.contains("return 1"));
        assert!(parsed[0].theirs_content.contains("return 2"));
    }

    #[test]
    fn test_parse_weave_conflicts_multiple() {
        let c1 = EntityConflict {
            entity_name: "foo".to_string(),
            entity_type: "function".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::Text,
            ours_content: Some("// a".to_string()),
            theirs_content: Some("// b".to_string()),
            base_content: None,
        };
        let c2 = EntityConflict {
            entity_name: "Bar".to_string(),
            entity_type: "class".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::SyntaxFunctional,
            ours_content: Some("class Bar { x() {} }".to_string()),
            theirs_content: Some("class Bar { y() {} }".to_string()),
            base_content: None,
        };
        let content = format!(
            "some code\n{}\nmore code\n{}\nend",
            c1.to_conflict_markers(&MarkerFormat::default(), "merge_ladder_exhausted"),
            c2.to_conflict_markers(&MarkerFormat::default(), "both_added_divergent")
        );
        let parsed = parse_weave_conflicts(&content);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].entity_name, "foo");
        assert_eq!(parsed[0].complexity, ConflictComplexity::Text);
        assert_eq!(parsed[1].entity_name, "Bar");
        assert_eq!(parsed[1].complexity, ConflictComplexity::SyntaxFunctional);
    }

    #[test]
    fn test_standard_markers_no_metadata() {
        let conflict = EntityConflict {
            entity_name: "foo".to_string(),
            entity_type: "function".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::Functional,
            ours_content: Some("return 1;".to_string()),
            theirs_content: Some("return 2;".to_string()),
            base_content: Some("return 0;".to_string()),
        };
        let markers =
            conflict.to_conflict_markers(&MarkerFormat::standard(7), "merge_ladder_exhausted");
        assert_eq!(markers, "<<<<<<< ours\nreturn 1;\n||||||| base\nreturn 0;\n=======\nreturn 2;\n>>>>>>> theirs\n");
        // No em-dash, no refusal line, no metadata
        assert!(!markers.contains('\u{2014}'));
        assert!(!markers.contains("refused_by"));
        assert!(!markers.contains("confidence"));
    }

    #[test]
    fn test_standard_markers_custom_length() {
        let conflict = EntityConflict {
            entity_name: "foo".to_string(),
            entity_type: "function".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::Functional,
            ours_content: Some("a".to_string()),
            theirs_content: Some("b".to_string()),
            base_content: None,
        };
        let markers =
            conflict.to_conflict_markers(&MarkerFormat::standard(11), "merge_ladder_exhausted");
        assert!(markers.starts_with("<<<<<<<<<<<")); // 11 <'s
        assert!(markers.contains("===========")); // 11 ='s
        assert!(markers.contains(">>>>>>>>>>>")); // 11 >'s
    }
}
