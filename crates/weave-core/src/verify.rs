//! Fail closed: what a merge must satisfy before weave may call it clean.
//!
//! Every composition rung in the merge is a claim that two edits are
//! independent. A claim can be wrong, and when it is, the merge driver used to
//! say "clean" anyway — which, for anyone who trusts the exit code, is worse
//! than the conflict git would have raised. So a clean answer is checked
//! against the three inputs before it is returned, and one that fails any check
//! here is refused: the caller reports a conflict instead.
//!
//! Each check is a NECESSARY condition on a faithful composition of the two
//! sides — something a line-level diff3 of disjoint hunks satisfies by
//! construction — so a refusal is evidence that the rung composed something it
//! had no licence to, never a matter of taste:
//!
//! 1. **markers** — a clean result never contains a conflict marker. (Inputs
//!    that carry markers are refused before the merge runs, so any marker here
//!    was written by the merge.)
//! 2. **lines** — every line both sides kept is still there, and no line is
//!    stated more often than applying both sides' edits states it:
//!    `o + t − n` copies when either side added, `min(o, t)` when both only
//!    deleted (the two may have deleted the same copy). This is where a
//!    deletion that came back, or an edit that landed twice, shows up. A
//!    whole hunk both sides inserted is agreement, and agreement is
//!    idempotent: such a block may not be stated more often than the side
//!    that states it most, wherever the two insertions landed.
//! 3. **bindings** — no name is bound, no member declared, no match/switch
//!    arm stated in one match, and no singular declaration (`package`)
//!    written more often than either side wrote it. A count above both sides'
//!    means both sides introduced the same name differently and the merge
//!    kept both: a duplicate import, a `const` that redeclares an import, a
//!    second `package` line, a doubled enum entry, two arms for one case.
//! 4. **parse** — if ours and theirs both parse, the merge must too.
//! 5. **data** — a JSON, TOML or YAML merge must still load if either side
//!    does, and may not state a key at one table path more often than either
//!    side does. Line-disjoint additions of the same key are clean to a line
//!    merge and fatal to every consumer of the file.
//! 6. **keys** — no switch, match, map / object / dict literal or struct
//!    literal states one key (a `case` label, a map key, a field) more often
//!    than either side states it in one container ([`duplicate_keys`]). Two
//!    sides each adding `case 3:` to one switch is line-disjoint, clean to a
//!    line merge, and does not compile; `case 3:` is too short a line for
//!    rule 2 to count. The merge also asks this of every clean answer as its
//!    last step, whichever rung wrote it.
//!
//! The line primitives here are shared with `weave check`, which asks the same
//! questions of a resolution a person wrote. A person may write a line no side
//! wrote, and may state it once more than the arithmetic allows; the merge may
//! not. That is the one parameter the two callers differ on
//! ([`duplication_ceiling`]).

use std::collections::{HashMap, HashSet};

use sem_core::model::entity::SemanticEntity;
use sem_core::parser::registry::ParserRegistry;

/// Why a clean answer was refused: which check, and the evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unverified {
    /// `markers` | `lines` | `bindings` | `parse` | `keys` | `data` | `iota`.
    pub check: &'static str,
    pub detail: String,
}

impl std::fmt::Display for Unverified {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.check, self.detail)
    }
}

// ---------------------------------------------------------------------------
// Line identity — shared with `weave check`
// ---------------------------------------------------------------------------

/// A line is only evidence if it says something. Bare punctuation (`}`, `);`,
/// `else:`) repeats legitimately all over a real file, so counting it would
/// bury every real finding under closing braces.
pub fn significant(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 8 && t.chars().any(|c| c.is_alphanumeric())
}

/// Is this line a *slice* of a statement rather than a statement?
///
/// A wrapped call argument (`any(IMediaType.class), anyInt(),`), an opening
/// header (`await event.fire_event_async(`, `except zmq.Error:`, `impl Foo {`)
/// and a leading continuation (`.map(|x| x + 1)`) all repeat legitimately
/// wherever the same shape is written again, so their multiplicity says
/// nothing about the merge on its own.
pub fn is_fragment(line: &str) -> bool {
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

/// The lines the multiset rules may reason about on their own.
pub fn carries_identity(line: &str) -> bool {
    significant(line) && !is_fragment(line)
}

/// How many times each significant line is stated, trimmed.
pub fn line_counts(text: &str) -> HashMap<&str, usize> {
    let mut m: HashMap<&str, usize> = HashMap::new();
    for l in text.lines().map(str::trim).filter(|l| significant(l)) {
        *m.entry(l).or_insert(0) += 1;
    }
    m
}

/// How many of base's `n` copies of a line any faithful merge keeps, given
/// that ours kept `o` and theirs kept `t`.
///
/// Deletions from the two sides are additive: of base's `n` copies, ours kept
/// `min(n, o)` and theirs `min(n, t)`, and nothing says those are the same
/// copies, so the survivors are the sum minus `n`, floored at zero.
#[allow(clippy::implicit_saturating_sub)]
pub fn unanimity_floor(n: usize, o: usize, t: usize) -> usize {
    let kept = o.min(n) + t.min(n);
    if kept > n {
        kept - n
    } else {
        0
    }
}

/// The most copies of a line a merge may state, given base `n`, ours `o`,
/// theirs `t`.
///
/// Applying both sides' edits to base states `o + t − n` copies. When both
/// sides only deleted, they may have deleted the same copy, so `min(o, t)` is
/// the allowance; when either added, the two additions are additive. That is
/// the merge's budget (`resolver_may_write == false`).
///
/// A person resolving by hand may also write one copy of anything, and may
/// keep all of either side's copies whatever the other deleted:
/// `resolver_may_write == true` is `weave check`'s ruler.
// Written out, not saturating: both sides may between them have kept fewer
// copies than base wrote, and that is the arithmetic, not an underflow to
// repair (the same reason `weave check` spelled it this way).
#[allow(clippy::implicit_saturating_sub)]
pub fn duplication_ceiling(n: usize, o: usize, t: usize, resolver_may_write: bool) -> usize {
    let applied = if o + t > n { o + t - n } else { 0 };
    if resolver_may_write {
        return applied.max(o).max(t).max(1);
    }
    if o <= n && t <= n {
        o.min(t)
    } else {
        applied
    }
}

// ---------------------------------------------------------------------------
// Declarations — shared with `weave check`
// ---------------------------------------------------------------------------

/// What makes two items the SAME declaration: the kind, the name with its
/// generic arguments off, and — where the language overloads — the parameter
/// list.
pub type DefinitionKey = (String, String, Option<String>);

/// Kinds that do not declare a name of their own: attachments (`impl`,
/// `extension`, `instance`), reopenable declarations (a namespace, module or
/// TypeScript interface stated twice is declaration merging), calls that are
/// not declarations at all (`test('…')`), and the import family, whose names
/// are read off the statement instead ([`import_names`]).
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
            // The no-grammar fallback's fixed-size slices of lines: named by
            // their position, which a merge moves without declaring anything.
            | "chunk"
    )
}

/// Languages where two items may share a name and differ only in their
/// parameters.
fn overloads(path: &str) -> bool {
    matches!(
        extension(path),
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

fn extension(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or("")
}

/// `Bar<T>` and `Bar<U>` are one nameable thing.
fn without_generics(name: &str) -> &str {
    match name.find('<') {
        Some(i) if i > 0 => name[..i].trim_end(),
        _ => name,
    }
}

/// The parameter list of an item's header, whitespace removed — `None` when the
/// item has no parameter list to compare.
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
pub fn definition_key(path: &str, e: &SemanticEntity) -> Option<DefinitionKey> {
    if e.name.is_empty() || !declares_a_name(&e.entity_type) {
        return None;
    }
    let signature = overloads(path)
        .then(|| parameter_list(&e.content))
        .flatten();
    // An item of a YAML sequence is named by its first key (`- id`), which
    // every item of a list of hooks shares: it binds nothing. Its identity is
    // its first line (`- id: lint`), so two items that differ there are two
    // items, and one item stated twice is still counted twice.
    let yaml_item =
        e.name.starts_with('-') && matches!(crate::datafile::format_name(path), Some("YAML"));
    let name = if yaml_item {
        e.content
            .lines()
            .next()
            .unwrap_or(&e.name)
            .trim()
            .to_string()
    } else {
        without_generics(&e.name).to_string()
    };
    Some((e.entity_type.clone(), name, signature))
}

// ---------------------------------------------------------------------------
// Import bindings
// ---------------------------------------------------------------------------

/// Import statements of `text`, each on one line: a multi-line `import { … }`
/// or `from m import ( … )` is joined, so every statement can be read the way
/// a single-line one is. Only unindented statements — an indented import is
/// local to a block and binds nothing at file scope.
fn import_statements(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.starts_with([' ', '\t']) {
            continue;
        }
        let t = line.trim();
        let opener = ["import ", "from ", "use ", "using "]
            .iter()
            .any(|kw| t.starts_with(kw));
        if !opener {
            continue;
        }
        let close = if t.contains('{') && !t.contains('}') {
            Some('}')
        } else if t.ends_with('(') && !t.starts_with("import (") {
            Some(')')
        } else {
            None
        };
        let mut joined = t.to_string();
        if let Some(close) = close {
            for inner in lines.by_ref() {
                joined.push(' ');
                joined.push_str(inner.trim());
                if inner.contains(close) {
                    break;
                }
            }
        }
        out.push(joined);
    }
    out
}

/// The last path segment of a dotted/`::` path, or its alias, or `None` for a
/// glob.
fn tail_or_alias(path: &str, sep: &str) -> Option<String> {
    let path = path.trim().trim_end_matches(';').trim();
    let (path, alias) = match path.rsplit_once(" as ") {
        Some((p, a)) => (p.trim(), Some(a.trim())),
        None => (path, None),
    };
    let name = alias.unwrap_or_else(|| path.rsplit(sep).next().unwrap_or(path));
    let ok = !name.is_empty()
        && name != "*"
        && name != "_"
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$');
    ok.then(|| name.to_string())
}

/// Go: the name one import spec binds — its alias, else the package's last
/// path segment (skipping a `/vN` major-version suffix). `_` and `.` bind
/// nothing nameable.
fn go_import_name(spec: &str) -> Option<String> {
    let spec = spec.trim();
    let quote = spec.find('"')?;
    let alias = spec[..quote].trim();
    if !alias.is_empty() {
        return (alias != "_" && alias != ".").then(|| alias.to_string());
    }
    let path = spec[quote..].trim_matches('"');
    let mut segs = path.rsplit('/');
    let last = segs.next()?;
    let is_version =
        last.len() > 1 && last.starts_with('v') && last[1..].chars().all(|c| c.is_ascii_digit());
    let name = if is_version { segs.next()? } else { last };
    Some(name.replace(['-', '.'], "_"))
}

/// The file-scope names `text`'s import statements bind, one entry per
/// binding (so a name imported twice appears twice), plus one `(package)` or
/// `(namespace)` entry per singular file declaration.
///
/// Language-aware, because the same line binds different names in different
/// languages: `import a.b.C` binds `a` in Python and `C` in Java. A language
/// with no rule here binds nothing, which can only make the check refuse less.
pub fn import_names(path: &str, text: &str) -> Vec<String> {
    let ext = extension(path);
    let mut out = Vec::new();
    // A file states its package once.
    let singular = match ext {
        "java" | "kt" | "kts" | "scala" | "groovy" | "go" => Some("package "),
        _ => None,
    };
    for line in text.lines() {
        if singular.is_some_and(|kw| line.starts_with(kw)) {
            out.push("(package)".to_string());
        }
        if ext == "cs" && line.starts_with("namespace ") && line.trim_end().ends_with(';') {
            out.push("(namespace)".to_string());
        }
    }
    match ext {
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "py" | "pyi" => {
            for s in import_statements(text) {
                out.extend(crate::binding::import_bindings(&s));
            }
        }
        "java" | "kt" | "kts" | "scala" | "groovy" => {
            for s in import_statements(text) {
                let Some(rest) = s.strip_prefix("import ") else {
                    continue;
                };
                let rest = rest.trim().strip_prefix("static ").unwrap_or(rest.trim());
                if rest.contains('{') {
                    continue;
                }
                out.extend(tail_or_alias(rest, "."));
            }
        }
        "rs" => {
            for s in import_statements(text) {
                let Some(rest) = s
                    .strip_prefix("use ")
                    .or_else(|| s.strip_prefix("pub use "))
                else {
                    continue;
                };
                let rest = rest.trim().trim_end_matches(';');
                match (rest.find('{'), rest.rfind('}')) {
                    (Some(o), Some(c)) if o < c && !rest[o + 1..c].contains('{') => {
                        for part in rest[o + 1..c].split(',') {
                            let part = part.trim();
                            if part == "self" {
                                out.extend(tail_or_alias(rest[..o].trim_end_matches("::"), "::"));
                            } else {
                                out.extend(tail_or_alias(part, "::"));
                            }
                        }
                    }
                    (None, None) => out.extend(tail_or_alias(rest, "::")),
                    _ => {}
                }
            }
        }
        "cs" => {
            for s in import_statements(text) {
                let Some(rest) = s.strip_prefix("using ") else {
                    continue;
                };
                if let Some((alias, _)) = rest.split_once('=') {
                    out.extend(tail_or_alias(alias, "."));
                }
            }
        }
        // `import 'u' as p` binds `p`; `import 'u' show a, b` binds `a`, `b`;
        // a plain or `hide` import binds whatever the library exports, which
        // is nothing this line can name.
        "dart" => {
            for s in import_statements(text) {
                let Some(rest) = s.strip_prefix("import ") else {
                    continue;
                };
                let rest = rest.trim().trim_end_matches(';');
                let Some(close) = rest.rfind(['\'', '"']).filter(|&i| i > 0) else {
                    continue;
                };
                let words: Vec<&str> = rest[close + 1..]
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|w| !w.is_empty())
                    .collect();
                if let Some(i) = words.iter().position(|w| *w == "as") {
                    out.extend(words.get(i + 1).and_then(|p| tail_or_alias(p, ".")));
                } else if let Some(i) = words.iter().position(|w| *w == "show") {
                    let shown = words[i + 1..].iter().take_while(|w| **w != "hide");
                    out.extend(shown.filter_map(|w| tail_or_alias(w, ".")));
                }
            }
        }
        "go" => {
            let mut in_block = false;
            for line in text.lines() {
                let t = line.trim();
                if in_block {
                    if t.starts_with(')') {
                        in_block = false;
                    } else if !t.is_empty() && !t.starts_with("//") {
                        out.extend(go_import_name(t));
                    }
                } else if line.starts_with("import (") {
                    in_block = true;
                } else if let Some(spec) = line.strip_prefix("import ") {
                    out.extend(go_import_name(spec));
                }
            }
        }
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// The checks
// ---------------------------------------------------------------------------

/// Check a CLEAN merge result against its three inputs. `None` means every
/// check passed; `Some` names the first one that did not.
///
/// `renames` are the renames the merge itself carried out (old name → new
/// name). The line arithmetic is taken modulo those, because rewriting the
/// other side's call sites is the merge doing its job, not stating a line
/// twice.
pub fn verify(
    base: &str,
    ours: &str,
    theirs: &str,
    merged: &str,
    file_path: &str,
    registry: &ParserRegistry,
    renames: &[(String, String)],
) -> Option<Unverified> {
    markers(merged)
        .or_else(|| {
            let rename = |text: &str| {
                renames.iter().fold(text.to_string(), |acc, (from, to)| {
                    crate::binding::replace_at_word_boundaries(&acc, from, to)
                })
            };
            let (b, o, t) = (rename(base), rename(ours), rename(theirs));
            let kept = crate::binding::imports_kept_for_new_uses(&b, &o, &t);
            lines(&b, &o, &t, merged, &kept)
        })
        .or_else(|| structured_data(Some(ours), Some(theirs), merged, file_path))
        .or_else(|| prose(base, ours, theirs, file_path))
        .or_else(|| declarations(base, ours, theirs, merged, file_path, registry))
}

/// A data file's keys: `merged` must load if either side does, and may state
/// a key at one table path no more often than the side that states it most.
///
/// Shared with `weave check`, which asks it of a person's resolution with the
/// same bounds: nobody may write a data file that no longer loads, or a key
/// twice where neither author did. A side that is absent (the file was added
/// on the other) or does not parse bounds nothing.
pub fn structured_data(
    ours: Option<&str>,
    theirs: Option<&str>,
    merged: &str,
    file_path: &str,
) -> Option<Unverified> {
    let format = crate::datafile::format_name(file_path)?;
    let side = |text: Option<&str>| text.and_then(|t| crate::datafile::keys(file_path, t));
    let (o, t) = (side(ours), side(theirs));
    let m = crate::datafile::keys(file_path, merged)?;
    let parses = |r: &Option<Result<_, _>>| matches!(r, Some(Ok(_)));
    let m = match m {
        Ok(m) => m,
        Err(e) if parses(&o) || parses(&t) => {
            return Some(Unverified {
                check: "data",
                detail: format!("the merged {format} does not load ({e}), and a side's does"),
            })
        }
        Err(_) => return None,
    };
    let count = |r: &Option<Result<crate::datafile::KeyTally, String>>, k: &str| match r {
        Some(Ok(tally)) => tally.get(k).copied().unwrap_or(0),
        _ => 0,
    };
    let mut over: Vec<(&String, usize, usize, usize)> = m
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(k, n)| (k, *n, count(&o, k), count(&t, k)))
        .filter(|(_, n, o, t)| *n > (*o).max(*t).max(1))
        .collect();
    over.sort();
    over.first().map(|(k, n, o, t)| Unverified {
        check: "data",
        detail: format!(
            "key `{}` is stated {n}x in one table; ours states it {o}x, theirs {t}x",
            crate::datafile::display_key(k)
        ),
    })
}

/// Documents whose meaning is their order: every paragraph is read after the
/// one above it.
fn is_prose(path: &str) -> bool {
    matches!(
        extension(&path.to_ascii_lowercase()),
        "md" | "markdown" | "mdx" | "rst" | "txt" | "adoc" | "asciidoc" | "org" | "tex"
    )
}

/// In prose, a section boundary is not an independence boundary. When both
/// sides rewrote the same passage — deleted or replaced the same base lines,
/// differently — the sections each side wrote in its place are two versions
/// of one text, and interleaving them by section produces a document neither
/// author wrote, however cleanly its sections compose. That is a conflict.
///
/// Two sides that only ADDED at one place (a section each, appended) are not
/// rewriting anything of each other's, and stay the entity merge's to place.
fn prose(base: &str, ours: &str, theirs: &str, file_path: &str) -> Option<Unverified> {
    if !is_prose(file_path) || diffy::merge(base, ours, theirs).is_ok() {
        return None;
    }
    crate::diagnose::diagnose(base, ours, theirs)
        .into_iter()
        .find(|h| !h.collision.is_empty() && h.ours.added != h.theirs.added)
        .map(|h| Unverified {
            check: "prose",
            detail: format!(
                "both sides rewrote the passage at base line {}, differently",
                h.base_start
            ),
        })
}

/// A name some version bound at file scope — imported, or declared at top
/// level — that the merge still mentions and no longer binds: one side deleted
/// a declaration or an import, and the other side's code that uses it
/// survived. Returns the first such name.
///
/// `texts` and `entities` are base, ours, theirs and the merge, in that order;
/// the entities are sem-core's parse of each (empty where there is none).
///
/// * A use is any reference position ([`has_mention`]): an argument, a type
///   argument, an operand, not only a call.
/// * A binding is an import, read per language ([`import_names`], so a Java
///   import binds its last segment), a top-level item sem-core extracts, or a
///   declaration the lexical reader sees — a top-level `const x = …` of a
///   grammar whose entity set leaves variables out.
///
/// A name no version bound in this file is external — a definition in another
/// file, a parameter, a global — and never fires. Neither does a name some
/// version already mentions unbound: that version asserts the binding lives
/// elsewhere (a definition moved to a module the file imports whole), and the
/// merge did not create the use. The merge is refused only for a dangling use
/// it created.
///
/// [`has_mention`]: crate::binding::has_mention
pub fn dangling_use(
    file_path: &str,
    texts: [&str; 4],
    entities: [&[SemanticEntity]; 4],
) -> Option<String> {
    use crate::binding::{declared_name, has_declaration, has_mention};
    // What each text binds at file scope, and whether it binds `n` at all.
    let bound: Vec<HashSet<String>> = texts
        .iter()
        .zip(entities)
        .map(|(text, ents)| {
            let top = text
                .lines()
                .filter(|l| !l.starts_with([' ', '\t']))
                .filter_map(declared_name);
            // A language with no import rule of its own (Vue, Svelte, Swift…)
            // is read the ES/Python way, as it was before the rules existed.
            let imports = if import_names_has_rule(file_path) {
                import_names(file_path, text)
            } else {
                import_statements(text)
                    .iter()
                    .flat_map(|s| crate::binding::import_bindings(s))
                    .collect()
            };
            imports
                .into_iter()
                .filter(|n| !n.starts_with('('))
                .chain(file_scope_names(file_path, ents))
                .chain(top)
                .collect()
        })
        .collect();
    let loose = |i: usize, n: &str| {
        !bound[i].contains(n) && has_mention(texts[i], n) && !has_declaration(texts[i], n)
    };
    let merged_ids = crate::binding::identifiers(texts[3]);
    let mut names: Vec<&String> = bound[..3]
        .iter()
        .flatten()
        .filter(|n| n.chars().count() >= 3 && merged_ids.contains(n.as_str()))
        .collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .find(|n| loose(3, n) && !(0..3).any(|i| loose(i, n)))
        .cloned()
}

/// Does [`import_names`] have a rule for this file's language?
fn import_names_has_rule(path: &str) -> bool {
    matches!(
        extension(path),
        "js" | "jsx"
            | "mjs"
            | "cjs"
            | "ts"
            | "tsx"
            | "mts"
            | "cts"
            | "py"
            | "pyi"
            | "java"
            | "kt"
            | "kts"
            | "scala"
            | "groovy"
            | "rs"
            | "cs"
            | "go"
            | "dart"
    )
}

/// The names `entities` declare at file scope — the top-level items, by the
/// identity [`definition_key`] gives them.
fn file_scope_names<'a>(
    file_path: &str,
    entities: impl IntoIterator<Item = &'a SemanticEntity>,
) -> Vec<String> {
    entities
        .into_iter()
        .filter(|e| e.parent_id.is_none() && definition_key(file_path, e).is_some())
        .map(|e| without_generics(&e.name).to_string())
        .collect()
}

/// An import one side deleted, back in the merge with nothing to use it.
///
/// The shape: base imports `N`; one side's refactor stopped using `N` and
/// deleted the import (it binds `N` nowhere); the other side edited the import
/// line (a new module path) and kept its old uses. Read line by line, that is
/// two different edits of one line, and a composition that keeps the edited
/// line undoes the deletion. If the merge's code then uses `N` the import is
/// needed and keeping it is right; if no code of the merge mentions `N`, the
/// line is the deletion reverted, and the merge is refused.
fn revived_import(versions: [&str; 3], merged: &str) -> Option<Unverified> {
    use crate::binding::{has_binding, identifiers, import_bindings};
    let [base, ours, theirs] = versions;
    let imported = |text: &str| -> HashSet<String> {
        import_statements(text)
            .iter()
            .flat_map(|s| import_bindings(s))
            .collect()
    };
    let code: String = merged
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            import_bindings(l).is_empty() && !t.starts_with("import ") && !t.starts_with("from ")
        })
        .map(|l| format!("{l}\n"))
        .collect();
    let used = identifiers(&code);
    let mut revived: Vec<String> = imported(merged)
        .intersection(&imported(base))
        .filter(|n| {
            n.chars().count() >= 3
                && !used.contains(n.as_str())
                && (!has_binding(ours, n) || !has_binding(theirs, n))
        })
        .cloned()
        .collect();
    revived.sort();
    revived.into_iter().next().map(|n| Unverified {
        check: "bindings",
        detail: format!(
            "`{n}` is imported again, one side deleted the import, and nothing uses it"
        ),
    })
}

/// Check one import region the import rung composed: it may not bind a name,
/// or state a package, more often than either side's region does.
pub fn region_binds_twice(path: &str, ours: &str, theirs: &str, merged: &str) -> Option<String> {
    over_either(
        &tally(import_names(path, ours)),
        &tally(import_names(path, theirs)),
        &tally(import_names(path, merged)),
    )
    .map(|(name, m, o, t)| format!("`{name}` is bound {m}x; ours binds it {o}x, theirs {t}x"))
}

fn markers(merged: &str) -> Option<Unverified> {
    let n = merged
        .lines()
        .filter(|l| l.starts_with("<<<<<<<") || l.starts_with(">>>>>>>"))
        .count();
    (n > 0).then(|| Unverified {
        check: "markers",
        detail: format!("{n} conflict marker line(s) in a result the merge called clean"),
    })
}

/// Line counts with a trailing list separator ignored. `"a": 1,` and `"a": 1`
/// are one member written at two positions in its list: appending an element
/// adds a comma to the line above, and dropping the last one removes it, so a
/// merge that keeps both sides' list edits writes lines no side wrote only in
/// their commas.
fn member_counts(text: &str) -> HashMap<&str, usize> {
    let mut m: HashMap<&str, usize> = HashMap::new();
    for l in text.lines() {
        let l = l.trim().trim_end_matches(',').trim_end();
        if significant(l) {
            *m.entry(l).or_insert(0) += 1;
        }
    }
    m
}

fn lines(
    base: &str,
    ours: &str,
    theirs: &str,
    merged: &str,
    kept: &[String],
) -> Option<Unverified> {
    let (cb, co, ct, cm) = (
        member_counts(base),
        member_counts(ours),
        member_counts(theirs),
        member_counts(merged),
    );
    let get = |m: &HashMap<&str, usize>, l: &str| m.get(l).copied().unwrap_or(0);

    let mut lost: Vec<&str> = cb
        .iter()
        .filter(|(l, n)| get(&cm, l) < unanimity_floor(**n, get(&co, l), get(&ct, l)))
        .map(|(l, _)| *l)
        .collect();
    lost.sort_unstable();
    if let Some(l) = lost.first() {
        return Some(Unverified {
            check: "lines",
            detail: format!("a line both sides kept is missing: `{}`", clip(l)),
        });
    }

    let over: HashSet<&str> = cm
        .iter()
        .filter(|(l, m)| {
            let (n, o, t) = (get(&cb, l), get(&co, l), get(&ct, l));
            // A line no input states was written by the merge on purpose —
            // a composed import, a rewritten call site. Its count is not this
            // rule's to judge.
            if n + o + t == 0 {
                return false;
            }
            let mut cap = duplication_ceiling(n, o, t, false);
            if kept.iter().any(|k| k == *l) {
                cap = cap.max(1);
            }
            **m > cap
        })
        .map(|(l, _)| *l)
        .collect();
    // A fragment is only evidence in company: a doubled edit lands as a RUN of
    // over-budget lines.
    let trimmed: Vec<&str> = merged
        .lines()
        .map(|l| l.trim().trim_end_matches(',').trim_end())
        .collect();
    // Adjacency is found in one pass over the file, not one pass per line.
    let in_run: HashSet<&str> = trimmed
        .windows(2)
        .filter(|p| over.contains(p[0]) && over.contains(p[1]))
        .flat_map(|p| [p[0], p[1]])
        .collect();
    let mut dup: Vec<&str> = over
        .iter()
        .copied()
        .filter(|l| carries_identity(l) || in_run.contains(l))
        .collect();
    dup.sort_unstable();
    dup.first()
        .map(|l| {
            let (n, o, t) = (get(&cb, l), get(&co, l), get(&ct, l));
            Unverified {
                check: "lines",
                detail: format!(
                    "`{}` is stated {}x; base {n}x, ours {o}x, theirs {t}x",
                    clip(l),
                    get(&cm, l)
                ),
            }
        })
        .or_else(|| joint_insertion_repeated(base, ours, theirs, merged))
}

/// How many significant lines a block both sides inserted must hold before
/// its repetition is read as one change written twice, not a coincidence.
pub(crate) const JOINT_BLOCK_LINES: usize = 3;

/// A block both sides inserted, whole, stated more often than either side
/// states it.
///
/// The arithmetic above reads two insertions of one line as two edits and
/// grants `o + t − n` copies: one side's `return err` in one function and the
/// other's in another are both kept, and so are the lines two new tests share.
/// A whole inserted hunk that the other side also inserted, line for line, is
/// different: it is one change both sides made (a cherry-pick, a rebase, two
/// agents given one task), and agreement is idempotent — `merge(b, x, x) = x`
/// — whether or not the two insertions landed at one place. A line merge of
/// disjoint hunks writes such a block twice and calls it clean.
///
/// So: a hunk one side wrote, of at least [`JOINT_BLOCK_LINES`] significant
/// lines, that appears contiguously among the lines a hunk of the other side
/// wrote, may not be stated (as a contiguous run of significant lines) more
/// often than the side that states it most.
fn joint_insertion_repeated(
    base: &str,
    ours: &str,
    theirs: &str,
    merged: &str,
) -> Option<Unverified> {
    let key = |l: &str| l.trim().trim_end_matches(',').trim_end().to_string();
    let sig = |lines: &mut dyn Iterator<Item = &str>| -> Vec<String> {
        lines.map(key).filter(|l| significant(l)).collect()
    };
    let hunks = |side: &str| -> Vec<Vec<String>> {
        crate::subsumption::edits(base, side)
            .iter()
            .map(|e| sig(&mut e.ins.iter().map(String::as_str)))
            .collect()
    };
    let contains = |big: &[String], small: &[String]| {
        !small.is_empty() && big.windows(small.len()).any(|w| w == small)
    };
    let count = |text: &[String], block: &[String]| {
        text.windows(block.len()).filter(|w| *w == block).count()
    };
    let (ho, ht) = (hunks(ours), hunks(theirs));
    let (so, st, sm) = (
        sig(&mut ours.lines()),
        sig(&mut theirs.lines()),
        sig(&mut merged.lines()),
    );
    for (mine, other) in [(&ho, &ht), (&ht, &ho)] {
        for block in mine.iter().filter(|h| h.len() >= JOINT_BLOCK_LINES) {
            if !other.iter().any(|h| contains(h, block)) {
                continue;
            }
            let (o, t, m) = (count(&so, block), count(&st, block), count(&sm, block));
            if m > o.max(t) {
                return Some(Unverified {
                    check: "lines",
                    detail: format!(
                        "a block both sides inserted is stated {m}x, ours {o}x, theirs {t}x: `{}` …",
                        clip(&block[0])
                    ),
                });
            }
        }
    }
    None
}

/// A hunk of `larger` against `smaller` that writes nothing but another copy
/// of a block `smaller` already states — at least [`JOINT_BLOCK_LINES`]
/// significant lines, contiguous in `smaller` — if there is one.
///
/// A new test that shares three lines of boilerplate with an old one also
/// writes its own name and its own assertions; a block pasted again writes
/// nothing of its own.
pub(crate) fn block_restated(smaller: &str, larger: &str) -> Option<String> {
    let key = |l: &str| l.trim().trim_end_matches(',').trim_end().to_string();
    let sig = |lines: &mut dyn Iterator<Item = &str>| -> Vec<String> {
        lines.map(key).filter(|l| significant(l)).collect()
    };
    let small = sig(&mut smaller.lines());
    crate::subsumption::edits(smaller, larger)
        .iter()
        .map(|e| sig(&mut e.ins.iter().map(String::as_str)))
        .find(|block| {
            block.len() >= JOINT_BLOCK_LINES && small.windows(block.len()).any(|w| w == &block[..])
        })
        .map(|block| block.join(" / "))
}

/// Names bound, members declared, and packages stated, per version — and the
/// parse check, which needs the same trees.
fn declarations(
    base: &str,
    ours: &str,
    theirs: &str,
    merged: &str,
    file_path: &str,
    registry: &ParserRegistry,
) -> Option<Unverified> {
    // One parse per version gives both the entities and whether the grammar
    // had to recover from an error. `None` = no tree (not a tree-sitter
    // language), which says nothing either way.
    let parse = |text: &str| {
        let (entities, tree) = registry
            .extract_entities_with_tree(file_path, text)
            .unwrap_or_default();
        let arms = tree
            .as_ref()
            .map(|t| arm_keys(text, t.root_node()))
            .unwrap_or_default();
        let keyed = tree
            .as_ref()
            .map(|t| keyed_elements(file_path, text, t.root_node()))
            .unwrap_or_default();
        (
            entities,
            tree.map(|t| t.root_node().has_error()),
            arms,
            keyed,
        )
    };
    let (merged_entities, merged_broken, merged_arms, merged_keyed) = parse(merged);
    let (ours_entities, ours_broken, ours_arms, ours_keyed) = parse(ours);
    let (theirs_entities, theirs_broken, theirs_arms, theirs_keyed) = parse(theirs);

    if merged_broken == Some(true) && ours_broken == Some(false) && theirs_broken == Some(false) {
        return Some(Unverified {
            check: "parse",
            detail: "the result does not parse, and both sides do".to_string(),
        });
    }
    if let Some(u) = introduced_duplicate(&ours_keyed, &theirs_keyed, &merged_keyed) {
        return Some(u);
    }

    let names = |entities: &[SemanticEntity], text: &str| {
        let mut all = import_names(file_path, text);
        all.extend(declared(file_path, entities));
        tally(all)
    };
    over_either(
        &names(&ours_entities, ours),
        &names(&theirs_entities, theirs),
        &names(&merged_entities, merged),
    )
    .map(|(name, m, o, t)| Unverified {
        check: "bindings",
        detail: format!("`{name}` is declared {m}x; ours declares it {o}x, theirs {t}x"),
    })
    .or_else(|| {
        over_either(&tally(ours_arms), &tally(theirs_arms), &tally(merged_arms)).map(
            |(arm, m, o, t)| Unverified {
                check: "bindings",
                detail: format!("the arm `{arm}` is stated {m}x; ours states it {o}x, theirs {t}x"),
            },
        )
    })
    .or_else(|| {
        // Dangling uses are a question about programs: only where there is a
        // grammar tree, and never about Markdown headings or YAML keys.
        merged_broken?;
        let (base_entities, _, _, _) = parse(base);
        dangling_use(
            file_path,
            [base, ours, theirs, merged],
            [
                &base_entities,
                &ours_entities,
                &theirs_entities,
                &merged_entities,
            ],
        )
        .map(|n| Unverified {
            check: "bindings",
            detail: format!("`{n}` is used but no longer declared or imported"),
        })
        .or_else(|| revived_import([base, ours, theirs], merged))
    })
}

/// The arms of every `match` (Rust) and `switch` (Swift) in a tree, each
/// keyed by its pattern and the heads of the expression and items it belongs
/// to.
///
/// An arm binds nothing, so the declaration tally never saw one; but a
/// pattern is a key of its match exactly as a name is a key of its scope, and
/// two arms with one pattern are two answers to one question: the second is
/// unreachable (a compile error for an enum variant bound twice in Rust, a
/// dead branch in Swift). Both sides adding an arm for one new case, each with
/// its own body, is the shape, and a line merge writes both arms.
fn arm_keys(text: &str, root: tree_sitter::Node) -> Vec<String> {
    let squash = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let src = text.as_bytes();
    let head = |n: tree_sitter::Node| -> String {
        let t = n.utf8_text(src).unwrap_or("");
        squash(t.lines().next().unwrap_or(""))
    };
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let pattern = match n.kind() {
            // `pat [if guard] => body`
            "match_arm" => n
                .child_by_field_name("pattern")
                .and_then(|p| p.utf8_text(src).ok())
                .map(squash),
            // `case pat[, pat]:` / `default:`
            "switch_entry" => {
                let t = n.utf8_text(src).unwrap_or("");
                let mut depth = 0i32;
                let end = t
                    .char_indices()
                    .find(|(_, c)| {
                        match c {
                            '(' | '[' | '{' => depth += 1,
                            ')' | ']' | '}' => depth -= 1,
                            ':' if depth == 0 => return true,
                            _ => {}
                        }
                        false
                    })
                    .map_or(t.len(), |(i, _)| i);
                Some(squash(&t[..end]))
            }
            _ => None,
        };
        if let Some(p) = pattern.filter(|p| !p.is_empty()) {
            // Scoped by the match and every item around it: `0x02 =>` in two
            // types' `decode` is two keys.
            let owner = std::iter::successors(n.parent(), |x| x.parent())
                .filter(|x| {
                    let k = x.kind();
                    matches!(k, "match_expression" | "switch_statement")
                        || k.ends_with("_item")
                        || k.ends_with("_declaration")
                })
                .map(head)
                .collect::<Vec<_>>()
                .join(" / ");
            out.push(format!("{owner} … {p}"));
        }
        let mut cursor = n.walk();
        stack.extend(n.children(&mut cursor));
    }
    out
}

// ---------------------------------------------------------------------------
// Keyed elements — the post-merge key invariant
// ---------------------------------------------------------------------------

/// One element a container looks up by key: a `case` label of a switch, a
/// key of a map / object / dict literal, a field of a struct literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyed {
    /// The container instance (a tree node id; meaningful within one tree).
    pub container: usize,
    /// What the container is, for the reader: its first line, squashed.
    pub container_head: String,
    /// The element's node kind (`expression_case`, `keyed_element`, `pair`…):
    /// keys are compared across versions per kind.
    pub kind: &'static str,
    /// The key, squashed (quotes off where the language treats `a` and `"a"`
    /// as one key).
    pub key: String,
}

/// Every keyed element in a tree. Within one container a key is an answer
/// to one question — which branch runs for `3`, what `"a"` maps to — and two
/// elements with one key are two answers: a compile error in Go (`duplicate
/// case 3 in expression switch`, `duplicate key "a" in map literal`), dead
/// code or a silently overwritten value elsewhere. Both sides adding an
/// element under one new key, each with its own body, is the shape, and a
/// line merge of disjoint hunks writes both. A `case 3:` line is too short
/// for the line rules to count, so the key itself has to be read.
pub fn keyed_elements(path: &str, text: &str, root: tree_sitter::Node) -> Vec<Keyed> {
    let squash = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let unquote_keys = matches!(
        extension(path),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "py" | "pyi"
    );
    let src = text.as_bytes();
    let txt = |n: tree_sitter::Node| n.utf8_text(src).unwrap_or("").to_string();
    let head = |n: tree_sitter::Node| -> String {
        let t = n.utf8_text(src).unwrap_or("");
        let first = squash(t.lines().next().unwrap_or(""));
        first.chars().take(60).collect()
    };
    let key_of = |s: &str| -> String {
        let k = squash(s);
        if unquote_keys {
            let q = k.trim_matches(|c| c == '"' || c == '\'' || c == '`');
            if q.len() + 2 == k.len() {
                return q.to_string();
            }
        }
        k
    };
    // The switch / match / literal a container node belongs to, for the head.
    fn owner(c: tree_sitter::Node<'_>) -> tree_sitter::Node<'_> {
        match c.kind() {
            "switch_body"
            | "switch_block"
            | "match_block"
            | "block"
            | "compound_statement"
            | "literal_value"
            | "field_initializer_list" => c.parent().unwrap_or(c),
            _ => c,
        }
    }
    let mut out = Vec::new();
    let mut push = |container: tree_sitter::Node, kind: &'static str, key: String| {
        if key.is_empty() {
            return;
        }
        out.push(Keyed {
            container: container.id(),
            container_head: head(owner(container)),
            kind,
            key,
        });
    };
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let parent = n.parent();
        match (n.kind(), parent) {
            // Go: `case a, b:` — each expression is its own key.
            ("expression_case", Some(p)) => {
                if let Some(v) = n.child_by_field_name("value") {
                    let mut c = v.walk();
                    for e in v.named_children(&mut c) {
                        push(p, "case", key_of(&txt(e)));
                    }
                }
            }
            // Go type switch: `case int, string:`.
            ("type_case", Some(p)) => {
                let mut c = n.walk();
                for t in n.children_by_field_name("type", &mut c) {
                    push(p, "case", key_of(&txt(t)));
                }
            }
            ("default_case" | "switch_default", Some(p)) => push(p, "case", "default".into()),
            // JS / TS `case v:`; C / C++ / PHP `case v:` (no value = default).
            ("switch_case" | "case_statement", Some(p)) => {
                let k = n
                    .child_by_field_name("value")
                    .map(|v| key_of(&txt(v)))
                    .unwrap_or_else(|| "default".into());
                push(p, "case", k);
            }
            // Java: `case 1, 2:` / `case 1 ->` / `default`, grouped under one
            // `switch_block`.
            ("switch_label", Some(p)) => {
                let block = p
                    .parent()
                    .filter(|g| g.kind() == "switch_block")
                    .unwrap_or(p);
                let t = squash(&txt(n));
                match t.strip_prefix("case ") {
                    Some(rest) => {
                        for k in split_top_level(rest) {
                            push(block, "case", key_of(&k));
                        }
                    }
                    None => push(block, "case", t),
                }
            }
            // Python `match` (and Scala): the patterns and the guard.
            ("case_clause", Some(p)) => {
                let mut c = n.walk();
                let pats: Vec<String> = n
                    .named_children(&mut c)
                    .filter(|x| x.kind() == "case_pattern")
                    .map(|x| squash(&txt(x)))
                    .collect();
                if !pats.is_empty() {
                    let guard = n
                        .child_by_field_name("guard")
                        .map(|g| format!(" {}", squash(&txt(g))))
                        .unwrap_or_default();
                    push(p, "case", format!("{}{guard}", pats.join(", ")));
                }
            }
            // Go composite literals: map keys, struct fields, array indexes.
            ("keyed_element", Some(p)) => {
                let k = n.child_by_field_name("key").or_else(|| n.named_child(0));
                if let Some(k) = k {
                    push(p, "key", key_of(&txt(k)));
                }
            }
            // JS / TS object literal and Python dict entries. A computed key
            // (`[k]: v`, `**d`) names nothing statically.
            ("pair", Some(p)) if matches!(p.kind(), "object" | "dictionary") => {
                if let Some(k) = n.child_by_field_name("key") {
                    if k.kind() != "computed_property_name" {
                        push(p, "key", key_of(&txt(k)));
                    }
                }
            }
            ("shorthand_property_identifier", Some(p)) if p.kind() == "object" => {
                push(p, "key", key_of(&txt(n)));
            }
            ("method_definition", Some(p)) if p.kind() == "object" => {
                if let Some(k) = n.child_by_field_name("name") {
                    if k.kind() != "computed_property_name" {
                        push(p, "key", key_of(&txt(k)));
                    }
                }
            }
            // Rust struct expressions.
            ("field_initializer", Some(p)) => {
                if let Some(k) = n.child_by_field_name("field") {
                    push(p, "key", key_of(&txt(k)));
                }
            }
            ("shorthand_field_initializer", Some(p)) => push(p, "key", key_of(&txt(n))),
            _ => {}
        }
        let mut cursor = n.walk();
        stack.extend(n.children(&mut cursor));
    }
    out
}

/// `a, f(b, c), d` → `a`, `f(b, c)`, `d`.
fn split_top_level(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    out.push(cur.trim().to_string());
    out.retain(|k| !k.is_empty());
    out
}

/// The post-merge key invariant: no container of `merged` states one key
/// more often than any single container of ours or of theirs states it.
///
/// A key stated twice in one container that neither side states twice
/// anywhere was put there twice by the merge — both sides added it, each
/// with its own element. Per container, not per file: the same `case 1:` in
/// two switches is two keys. The comparison across versions is by element
/// kind and key and ignores which container, so a duplicate one side
/// already had (which no merge introduced) never counts against the merge.
pub fn introduced_duplicate(
    ours: &[Keyed],
    theirs: &[Keyed],
    merged: &[Keyed],
) -> Option<Unverified> {
    fn most(els: &[Keyed]) -> HashMap<(&'static str, &str), usize> {
        let mut per: HashMap<(usize, &'static str, &str), usize> = HashMap::new();
        for e in els {
            *per.entry((e.container, e.kind, e.key.as_str()))
                .or_insert(0) += 1;
        }
        let mut m: HashMap<(&'static str, &str), usize> = HashMap::new();
        for ((_, kind, key), n) in per {
            let slot = m.entry((kind, key)).or_insert(0);
            *slot = (*slot).max(n);
        }
        m
    }
    let (o, t) = (most(ours), most(theirs));
    let mut per: HashMap<(usize, &str, &str), (usize, &str)> = HashMap::new();
    for e in merged {
        per.entry((e.container, e.kind, e.key.as_str()))
            .or_insert((0, e.container_head.as_str()))
            .0 += 1;
    }
    let mut over: Vec<(&str, &str, usize, usize, usize, &str)> = per
        .into_iter()
        .filter_map(|((_, kind, key), (m, head))| {
            let (a, b) = (
                o.get(&(kind, key)).copied().unwrap_or(0),
                t.get(&(kind, key)).copied().unwrap_or(0),
            );
            (m >= 2 && m > a.max(b)).then_some((kind, key, m, a, b, head))
        })
        .collect();
    over.sort();
    over.first().map(|(kind, key, m, a, b, head)| Unverified {
        check: "keys",
        detail: format!(
            "the {} `{}` is stated {m}x in one `{}`; ours states it {a}x there, theirs {b}x",
            if *kind == "case" { "case" } else { "key" },
            clip(key),
            clip(head),
        ),
    })
}

/// [`introduced_duplicate`] on three texts: `None` when there is no grammar
/// tree for the file, or no key the merge stated twice.
pub fn duplicate_keys(
    file_path: &str,
    ours: &str,
    theirs: &str,
    merged: &str,
    registry: &ParserRegistry,
) -> Option<Unverified> {
    let keyed = |text: &str| -> Option<Vec<Keyed>> {
        let (_, tree) = registry.extract_entities_with_tree(file_path, text)?;
        let tree = tree?;
        Some(keyed_elements(file_path, text, tree.root_node()))
    };
    let merged = keyed(merged)?;
    if merged.is_empty() {
        return None;
    }
    introduced_duplicate(
        &keyed(ours).unwrap_or_default(),
        &keyed(theirs).unwrap_or_default(),
        &merged,
    )
}

/// One key per declaration: a top-level item by its bare name, so that it
/// collides with an import of the same name; a nested one by its parent, kind,
/// name and signature, so that two methods of two classes do not.
fn declared(path: &str, entities: &[SemanticEntity]) -> Vec<String> {
    let parent_name: HashMap<&str, &str> = entities
        .iter()
        .map(|e| (e.id.as_str(), e.name.as_str()))
        .collect();
    entities
        .iter()
        .filter_map(|e| {
            let (kind, name, sig) = definition_key(path, e)?;
            Some(match e.parent_id.as_deref() {
                None => name,
                Some(p) => format!(
                    "{}::{kind} {name}{}",
                    parent_name.get(p).copied().unwrap_or(p),
                    sig.map(|s| format!("({s})")).unwrap_or_default()
                ),
            })
        })
        .collect()
}

fn tally(names: Vec<String>) -> HashMap<String, usize> {
    let mut m: HashMap<String, usize> = HashMap::new();
    for n in names {
        *m.entry(n).or_insert(0) += 1;
    }
    m
}

/// The first name (in sorted order) the merge states more often than either
/// side: `(name, merged, ours, theirs)`.
fn over_either(
    ours: &HashMap<String, usize>,
    theirs: &HashMap<String, usize>,
    merged: &HashMap<String, usize>,
) -> Option<(String, usize, usize, usize)> {
    let mut over: Vec<(String, usize, usize, usize)> = merged
        .iter()
        .filter_map(|(name, m)| {
            let o = ours.get(name).copied().unwrap_or(0);
            let t = theirs.get(name).copied().unwrap_or(0);
            (*m > o.max(t)).then(|| (name.clone(), *m, o, t))
        })
        .collect();
    over.sort();
    over.into_iter().next()
}

fn clip(l: &str) -> String {
    let t = l.trim();
    if t.chars().count() > 72 {
        t.chars().take(71).chain(['…']).collect()
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(
        base: &str,
        ours: &str,
        theirs: &str,
        merged: &str,
        path: &str,
    ) -> Option<&'static str> {
        verify(
            base,
            ours,
            theirs,
            merged,
            path,
            &crate::merge::PARSER_REGISTRY,
            &[],
        )
        .map(|u| u.check)
    }

    #[test]
    fn a_yaml_sequence_item_is_named_by_its_first_line() {
        // Every item of a list of hooks has the key `id`: distinct items are
        // not one name bound three times…
        let base = "- id: alpha\n  entry: alpha\n";
        let ours = "- id: alpha\n  entry: alpha\n- id: beta\n  entry: beta\n";
        let theirs = "- id: alpha\n  entry: alpha\n- id: gamma\n  entry: gamma\n";
        let merged =
            "- id: alpha\n  entry: alpha\n- id: beta\n  entry: beta\n- id: gamma\n  entry: gamma\n";
        assert_eq!(check(base, ours, theirs, merged, "hooks.yaml"), None);
        // …but one item stated twice still is.
        let theirs = "- id: alpha\n  entry: alpha\n- id: beta\n  entry: other\n";
        let twice =
            "- id: alpha\n  entry: alpha\n- id: beta\n  entry: beta\n- id: beta\n  entry: other\n";
        assert_eq!(
            check(base, ours, theirs, twice, "hooks.yaml"),
            Some("bindings")
        );
    }

    #[test]
    fn the_merge_budget_is_what_applying_both_sides_states() {
        // both added one copy: two copies, or one if they converged
        assert_eq!(duplication_ceiling(0, 1, 1, false), 2);
        // one side deleted, the other kept: none
        assert_eq!(duplication_ceiling(1, 0, 1, false), 0);
        // both deleted one of two copies: maybe the same one
        assert_eq!(duplication_ceiling(2, 1, 1, false), 1);
        // one added a copy, the other deleted base's
        assert_eq!(duplication_ceiling(1, 2, 0, false), 1);
        // a person may write one copy of anything
        assert_eq!(duplication_ceiling(1, 0, 1, true), 1);
        assert_eq!(unanimity_floor(1, 1, 1), 1);
        assert_eq!(unanimity_floor(2, 1, 1), 0);
    }

    #[test]
    fn a_disjoint_line_merge_verifies() {
        let base = "def a():\n    return 'first value'\n\n\ndef b():\n    return 'second value'\n";
        let ours = base.replace("first value", "first value, ours");
        let theirs = base.replace("second value", "second value, theirs");
        let merged = diffy::merge(base, &ours, &theirs).expect("disjoint");
        assert_eq!(check(base, &ours, &theirs, &merged, "m.py"), None);
    }

    #[test]
    fn markers_in_a_clean_result_are_refused() {
        let base = "x = 1\n";
        let merged = "<<<<<<< ours\nx = 2\n=======\nx = 3\n>>>>>>> theirs\n";
        assert_eq!(
            check(base, "x = 2\n", "x = 3\n", merged, "m.py"),
            Some("markers")
        );
    }

    #[test]
    fn a_deleted_line_that_comes_back_is_refused() {
        let base = "import os\nimport sys\n\n\ndef f():\n    return sys.argv\n";
        let ours = "import sys\n\n\ndef f():\n    return sys.argv\n";
        let theirs = "import os\nimport sys\n\n\ndef f():\n    return sys.argv[1:]\n";
        let resurrected = "import os\nimport sys\n\n\ndef f():\n    return sys.argv[1:]\n";
        assert_eq!(
            check(base, ours, theirs, resurrected, "m.py"),
            Some("lines")
        );
        let faithful = "import sys\n\n\ndef f():\n    return sys.argv[1:]\n";
        assert_eq!(check(base, ours, theirs, faithful, "m.py"), None);
    }

    #[test]
    fn a_name_bound_twice_is_refused() {
        let base = "import { load } from \"./a\"\n\nexport const f = () => load()\n";
        let ours = "import { load } from \"./b\"\n\nexport const f = () => load()\n";
        let theirs = "import { load } from \"./c\"\n\nexport const f = () => load()\n";
        let merged = "import { load } from \"./b\"\nimport { load } from \"./c\"\n\nexport const f = () => load()\n";
        assert_eq!(check(base, ours, theirs, merged, "m.ts"), Some("bindings"));
    }

    #[test]
    fn a_data_key_stated_twice_at_one_path_is_refused() {
        let base = "{\n  \"a\": {\n    \"x\": 1,\n    \"y\": 2\n  }\n}\n";
        let ours = "{\n  \"a\": {\n    \"t\": 5,\n    \"x\": 1,\n    \"y\": 2\n  }\n}\n";
        let theirs = "{\n  \"a\": {\n    \"x\": 1,\n    \"y\": 2,\n    \"t\": 9\n  }\n}\n";
        let both =
            "{\n  \"a\": {\n    \"t\": 5,\n    \"x\": 1,\n    \"y\": 2,\n    \"t\": 9\n  }\n}\n";
        let refusal = structured_data(Some(ours), Some(theirs), both, "cfg.json").expect("refused");
        assert_eq!(refusal.check, "data");
        assert!(refusal.detail.contains("`a.t`"), "{}", refusal.detail);
        assert_eq!(check(base, ours, theirs, both, "cfg.json"), Some("data"));
        // The same key once, and a key the two sides share at different paths:
        let one = "{\n  \"a\": {\n    \"t\": 5,\n    \"x\": 1,\n    \"y\": 2\n  }\n}\n";
        assert_eq!(
            structured_data(Some(ours), Some(theirs), one, "cfg.json"),
            None
        );
        // A side that already states it twice licenses twice, not three times.
        assert_eq!(
            structured_data(Some(both), Some(theirs), both, "cfg.json"),
            None
        );
    }

    #[test]
    fn a_data_file_that_stops_loading_is_refused_only_if_a_side_loads() {
        let ok = "[a]\nx = 1\n";
        let broken = "[a]\nx = 1\nx = 2\n";
        assert_eq!(
            structured_data(Some(ok), Some(ok), broken, "c.toml").map(|u| u.check),
            Some("data")
        );
        assert_eq!(
            structured_data(Some(broken), Some(broken), broken, "c.toml"),
            None
        );
        assert_eq!(structured_data(None, None, broken, "c.toml"), None);
        assert_eq!(structured_data(Some(ok), Some(ok), broken, "c.py"), None);
    }

    #[test]
    fn a_list_separator_is_not_a_line() {
        let base = "{\n  \"alpha\": 10000,\n  \"beta\": 10100,\n  \"gamma\": 10200\n}\n";
        let ours = "{\n  \"alpha\": 10000,\n  \"beta\": 10100\n}\n";
        let theirs = "{\n  \"alpha\": 10000,\n  \"beta\": 10100,\n  \"gamma\": 10200,\n  \"delta\": 10300\n}\n";
        let merged = "{\n  \"alpha\": 10000,\n  \"beta\": 10100,\n  \"delta\": 10300\n}\n";
        assert_eq!(check(base, ours, theirs, merged, "m.json"), None);
    }

    #[test]
    fn a_use_the_merge_left_unbound_is_refused_but_an_outside_name_is_not() {
        let base = "import { fetchAll } from \"./api\"\n\nexport function a() {\n  return fetchAll()\n}\n\nexport function b() {\n  return helper(1)\n}\n";
        let ours = "export function a() {\n  return []\n}\n\nexport function b() {\n  return helper(1)\n}\n";
        let theirs = format!("{base}\nexport function c() {{\n  return fetchAll()\n}}\n");
        // ours dropped the import, theirs' new `c` still calls it
        let merged = format!("{ours}\nexport function c() {{\n  return fetchAll()\n}}\n");
        assert_eq!(
            check(base, ours, &theirs, &merged, "m.ts"),
            Some("bindings")
        );
        // `helper` is defined in another file: every version calls it unbound,
        // so the merge calling it unbound is not the merge's doing
        let ours2 = base.replace("helper(1)", "helper(2)");
        let merged2 = base.replace("helper(1)", "helper(2)");
        assert_eq!(check(base, &ours2, base, &merged2, "m.ts"), None);
    }

    #[test]
    fn import_names_follow_the_language() {
        let names = |path: &str, text: &str| import_names(path, text);
        assert_eq!(
            names(
                "A.java",
                "package a.b;\n\nimport java.util.List;\nimport static a.B.max;\nimport a.c.*;\n"
            ),
            vec!["(package)", "List", "max"]
        );
        assert_eq!(
            names(
                "A.kt",
                "package a\n\nimport a.b.Window\nimport a.c.Window as Pane\n"
            ),
            vec!["(package)", "Window", "Pane"]
        );
        assert_eq!(
            names(
                "a.go",
                "package x\n\nimport (\n\t\"fmt\"\n\tyaml \"gopkg.in/yaml.v3\"\n\t_ \"embed\"\n\t\"example.com/mod/v2\"\n)\n"
            ),
            vec!["(package)", "fmt", "yaml", "mod"]
        );
        assert_eq!(
            names(
                "a.dart",
                "import 'package:a/b.dart' show openBox, Store;\nimport 'package:flutter_test/flutter_test.dart' as ft;\nimport 'package:a/c.dart';\nimport 'package:a/d.dart' hide x;\n"
            ),
            vec!["openBox", "Store", "ft"]
        );
        assert_eq!(
            names(
                "a.rs",
                "use std::io::{self, Read};\nuse crate::x::Y as Z;\n"
            ),
            vec!["io", "Read", "Z"]
        );
        assert_eq!(
            names(
                "a.py",
                "from m import (\n    a,\n    b as c,\n)\nimport os.path\n"
            ),
            vec!["a", "c", "os"]
        );
        assert_eq!(
            names("a.ts", "import {\n  type A,\n  B,\n} from \"./x\"\n"),
            vec!["A", "B"]
        );
    }
}
