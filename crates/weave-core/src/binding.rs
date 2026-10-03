//! Binding evidence: the one place weave decides what a *definition* and a
//! *use* are.
//!
//! Name resolution is a function of (definition set, use set, order), so every
//! stage that reasons about the DANGLING / DUP / SHADOW classes has to answer
//! two questions: "does this text define `name`?" and "does this text call it?".
//! Before this module there were four answers to those questions — v2's binding
//! pass, the CLI's repo-scope pass, the CLI's patch rewriter and the MCP
//! findings producer each carried their own — and they disagreed. A repo-scope
//! check could report a dangling reference the per-file pass had already
//! repaired, because the two passes did not mean the same thing by "call".
//!
//! One owner, so the passes cannot disagree about what a "call" is. The public surface
//! is the two predicates, the whole-content definition query, the
//! boundary-respecting rewrite, and the import-binding group: what an import
//! binds, what binds a name, what uses it as a value, and which deleted imports
//! a merge must keep. Nothing here knows about wire formats or files.
//!
//! Deliberately narrow, in both directions:
//!
//! * a *definition* is a definer keyword (after visibility/modifier prefixes)
//!   immediately followed by the name at a word boundary;
//! * a *use* is the identifier at a word boundary, not preceded by `.`
//!   (attribute access binds elsewhere), followed by `(` — and never on a line
//!   that defines it.
//!
//! Bare mentions, string occurrences and attribute accesses do not count, so no
//! pass can fire on a coincidence. The cost is false negatives (a reference
//! passed as a bare function value is missed); that is the right trade for a
//! tool whose findings are meant to be acted on.

use std::collections::{BTreeSet, HashSet};

/// Bytes that may appear inside an identifier. `$` counts: it is an identifier
/// character in JavaScript and PHP, so `$name(` is a call to `$name`, not to
/// `name`.
pub(crate) fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// The `char` counterpart of [`is_ident_char`]. Python 3, JavaScript and
/// TypeScript all allow non-ASCII letters in identifiers, so an identifier
/// boundary has to be decided per *character*. Deciding it on the first byte of
/// a multi-byte character classifies that byte as a non-identifier (every UTF-8
/// continuation and lead byte is `>= 0x80`), which both mis-detects boundaries
/// and, when a scan then advances by one byte, slices inside a character and
/// panics.
pub(crate) fn is_ident_char_c(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Every maximal identifier in `content`, split exactly where
/// [`has_call_reference`] and friends draw a word boundary.
///
/// A name that is not in this set cannot be called, declared or used in
/// `content`, so asking the set first turns "every vanished name × every file
/// × every line" into one pass per file.
pub fn identifiers(content: &str) -> std::collections::HashSet<&str> {
    content
        .split(|c: char| !is_ident_char_c(c))
        .filter(|w| !w.is_empty())
        .collect()
}

/// Could `content` mention `name` at all? Exact for an identifier via
/// [`identifiers`]; a substring test for anything else (`operator+`, `a::b`).
pub fn may_mention(ids: &std::collections::HashSet<&str>, content: &str, name: &str) -> bool {
    if name.chars().all(is_ident_char_c) {
        ids.contains(name)
    } else {
        content.contains(name)
    }
}

/// Visibility / modifier keywords that can precede a definer keyword.
const MODIFIERS: [&str; 11] = [
    "export ",
    "public ",
    "private ",
    "protected ",
    "static ",
    "pub ",
    "async ",
    "default ",
    "abstract ",
    "final ",
    "override ",
];

/// Keywords that introduce a definition.
const DEFINERS: [&str; 11] = [
    "def ",
    "function ",
    "fn ",
    "class ",
    "func ",
    "interface ",
    "struct ",
    "trait ",
    "impl ",
    "type ",
    "enum ",
];

/// Is `line` a *definition* of `name` rather than a use of it?
pub(crate) fn is_definition_line(line: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut t = line.trim_start();
    while let Some(rest) = MODIFIERS.iter().find_map(|kw| t.strip_prefix(kw)) {
        t = rest.trim_start();
    }
    for kw in DEFINERS {
        if let Some(rest) = t.strip_prefix(kw) {
            let rest = rest.trim_start();
            return rest.starts_with(name)
                && rest[name.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !is_ident_char(c as u8));
        }
    }
    false
}

/// Does `content` define `name` anywhere?
pub fn has_definition(content: &str, name: &str) -> bool {
    !name.is_empty() && content.lines().any(|l| is_definition_line(l, name))
}

/// Does `content` call `name`? The identifier at a word boundary, not preceded
/// by `.` (attribute access binds elsewhere), followed by `(` — and never on a
/// line that defines it.
pub fn has_call_reference(content: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // A rejected match is retried one *char* later, not one byte later: with a
    // multi-byte first character `i + 1` lands inside it and the next slice
    // panics (issue #165). `needle` may also overlap itself, so a whole-needle
    // skip would miss matches; one char is the correct minimal advance.
    let first_char_len = name.chars().next().map_or(1, char::len_utf8);
    content.lines().any(|line| {
        if is_definition_line(line, name) {
            return false;
        }
        let mut from = 0usize;
        while let Some(rel) = line[from..].find(name) {
            let i = from + rel;
            // The character before the match, Unicode-aware. A match glued to an
            // identifier character (ASCII or not) is part of a longer identifier,
            // and one preceded by `.` is attribute access that binds elsewhere.
            let before_ok = i == 0
                || line[..i]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !is_ident_char_c(c) && c != '.');
            let after = i + name.len();
            let call_ok = line[after..].trim_start().starts_with('(');
            if before_ok && call_ok {
                return true;
            }
            from = i + first_char_len;
            if from >= line.len() {
                break;
            }
        }
        false
    })
}

/// Every name `content` calls: an identifier at a word boundary, not an
/// attribute access, immediately followed by `(`, and not a definition site.
///
/// The inverted index form of [`has_call_reference`]: asking every text about
/// every name is a quadratic scan, and this answers the same question in one
/// pass. Crate-internal — the repair pass is the only caller that needs the
/// whole set rather than one predicate.
pub(crate) fn called_names(content: &str) -> HashSet<&str> {
    let mut out: HashSet<&str> = HashSet::new();
    for line in content.lines() {
        // Walk by character so a non-ASCII call site (a legal identifier in
        // Python 3 / JS / TS) enters the index instead of being skipped, which
        // is what let rename repair silently miss non-ASCII renames (issue #165).
        let mut it = line.char_indices().peekable();
        while let Some((start, c)) = it.next() {
            let starts_ident = (c.is_alphabetic() || c == '_')
                && line[..start]
                    .chars()
                    .next_back()
                    .is_none_or(|p| !is_ident_char_c(p) && p != '.');
            if !starts_ident {
                continue;
            }
            let mut end = start + c.len_utf8();
            while let Some(&(j, cc)) = it.peek() {
                if is_ident_char_c(cc) {
                    end = j + cc.len_utf8();
                    it.next();
                } else {
                    break;
                }
            }
            let name = &line[start..end];
            if line[end..].trim_start().starts_with('(') && !is_definition_line(line, name) {
                out.insert(name);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// What a block reads and writes: safe composition, restricted to what a reader can see
// ---------------------------------------------------------------------------

/// What a block of code writes and what it reads, as far as *this* module can
/// tell without a semantics for the callee.
///
/// Two commands compose in either order, to the same state, when
/// each one's writes are disjoint from the other's reads and from the other's
/// writes. That is the strongest statement about two edits weave could want,
/// and it is exactly as usable as those read/write sets are honest — which is why
/// [`footprint`] answers `None` far more often than it answers `Some`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Footprint {
    /// Names this text binds — a declaration's left-hand side, or a definer
    /// keyword's subject.
    pub defines: BTreeSet<String>,
    /// Every identifier this text mentions that it does not itself bind.
    pub uses: BTreeSet<String>,
}

/// The name a single *declaration* statement binds, if that is what it is.
///
/// A declaration for this purpose is an assignment whose effect is exactly one
/// binding a reader could observe in isolation: `x = …`, `let x = …`,
/// `const x: T = …`, `private static final T x = …`, `x: T = …`.
///
/// Anything with a call, a control-flow keyword, an index or a field target on
/// the left is not a declaration, because "what does it do" then has an answer
/// that depends on when it runs.
///
/// This lives here rather than in the statement fold because it answers the
/// module's own question — what is a definition — and having two answers to
/// that is the divergence this module was created to end.
pub(crate) fn declared_name(text: &str) -> Option<String> {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty() && !is_trivia_line(l))?
        .trim();
    // Split at the first `=` that is an assignment, not a comparison.
    let bytes = line.as_bytes();
    let mut eq = None;
    for (i, c) in bytes.iter().enumerate() {
        if *c == b'=' {
            let prev = if i > 0 { bytes[i - 1] } else { b' ' };
            let next = *bytes.get(i + 1).unwrap_or(&b' ');
            if next == b'='
                || matches!(
                    prev,
                    b'=' | b'!'
                        | b'<'
                        | b'>'
                        | b'+'
                        | b'-'
                        | b'*'
                        | b'/'
                        | b'%'
                        | b'&'
                        | b'|'
                        | b'^'
                        | b':'
                )
            {
                return None;
            }
            eq = Some(i);
            break;
        }
    }
    let lhs = &line[..eq?];
    if lhs.contains('(') || lhs.contains('[') || lhs.contains('.') {
        return None;
    }
    // Strip a type annotation and the modifier keywords in front of the name.
    let lhs = lhs.split(':').next().unwrap_or(lhs);
    let words = word_tokens(lhs);
    let name = words.last()?;
    const NOT_A_NAME: [&str; 14] = [
        "if", "while", "for", "return", "match", "switch", "case", "else", "elif", "yield",
        "assert", "with", "do", "try",
    ];
    if NOT_A_NAME.contains(name) || name.chars().next()?.is_ascii_digit() {
        return None;
    }
    Some(name.to_string())
}

/// The name a *definer keyword* introduces on this line — `def f`, `class C`,
/// `pub fn g`, `interface I` — after the modifier prefixes.
pub(crate) fn defined_name(line: &str) -> Option<String> {
    let mut t = line.trim_start();
    while let Some(rest) = MODIFIERS.iter().find_map(|kw| t.strip_prefix(kw)) {
        t = rest.trim_start();
    }
    let rest = DEFINERS.iter().find_map(|kw| t.strip_prefix(kw))?;
    let rest = rest.trim_start();
    let end = rest
        .find(|c: char| !is_ident_char(c as u8))
        .unwrap_or(rest.len());
    let name = &rest[..end];
    (!name.is_empty() && !name.starts_with(|c: char| c.is_ascii_digit())).then(|| name.to_string())
}

/// A decorator, annotation or comment line — text that belongs to whatever it
/// precedes rather than standing on its own.
pub(crate) fn is_trivia_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('@')
        || t.starts_with("#[")
        || t.starts_with("//")
        || t.starts_with("/*")
        || t.starts_with("* ")
        || t == "*"
        || t == "*/"
        || (t.starts_with('#') && !t.starts_with("#["))
}

/// Maximal runs of identifier characters — `b(2);` is `b`, `2`, not `b(2);`.
pub(crate) fn word_tokens(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            out.push(&s[start..i]);
        } else {
            i += 1;
        }
    }
    out
}

/// The read/write set of one block of code, or `None` if any part of it has an
/// effect this module cannot see.
///
/// `None` is the common answer, and deliberately so. A bare call `flush()` reads
/// and writes whatever `flush` touches; an `if`, a `return`, a `+=`, an
/// assignment through a field or an index — each is an effect on state this
/// reader has no name for. Only two shapes have a read/write set a line-level reader
/// can state honestly:
///
///  * a **declaration** — `x = <expr>`: it writes `x` and reads the names in
///    `<expr>`; and
///  * a **definition** — `def f`, `class C`, `fn g`: it writes `f` and reads
///    the names in its body. A *decorated* definition is not one of these:
///    `@app.route(...)` registers the function with something at definition
///    time, and that is an effect on the decorator's state, in the order the
///    definitions run.
///
/// Comments and blank lines contribute nothing and disqualify nothing.
pub(crate) fn footprint(text: &str) -> Option<Footprint> {
    let mut fp = Footprint::default();
    let mut in_definition_body: Option<usize> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // A decorator or annotation registers its subject with something at
        // definition time, in the order the definitions run. That is an effect
        // on state this reader cannot name, so the block has no visible read/write set.
        if trimmed.starts_with('@') || trimmed.starts_with("#[") {
            return None;
        }
        if is_trivia_line(line) {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        // Inside a definition's body, every name is a use; the body's own
        // bindings are local to it and cannot collide with the other side's.
        if let Some(head_indent) = in_definition_body {
            if indent > head_indent || trimmed == "}" || trimmed.starts_with('}') {
                fp.uses
                    .extend(word_tokens(line).into_iter().map(str::to_string));
                continue;
            }
            in_definition_body = None;
        }
        if let Some(name) = defined_name(line) {
            fp.defines.insert(name);
            fp.uses
                .extend(word_tokens(line).into_iter().map(str::to_string));
            in_definition_body = Some(indent);
            continue;
        }
        let name = declared_name(line)?;
        fp.defines.insert(name);
        fp.uses
            .extend(word_tokens(line).into_iter().map(str::to_string));
    }
    if fp.defines.is_empty() {
        return None;
    }
    for d in &fp.defines {
        fp.uses.remove(d);
    }
    Some(fp)
}

/// The disjointness test, as a decision about two blocks the merge is being asked to
/// place in the same gap: neither writes what the other writes, and neither
/// writes what the other reads.
///
/// Under this condition the two orders are the same program, so emitting both
/// is not a choice between them — it is the observation that there was nothing
/// to choose.
pub(crate) fn footprints_disjoint(a: &Footprint, b: &Footprint) -> bool {
    a.defines.is_disjoint(&b.defines)
        && a.defines.is_disjoint(&b.uses)
        && b.defines.is_disjoint(&a.uses)
}

/// Replace `needle` with `replacement` only at word boundaries — the rewrite
/// half of the same identifier model the predicates above read with.
///
/// Every entity's body goes through here once per merge (`build_arena`
/// normalises each body by blanking its own name), so the scan is on the hot
/// path for every file weave parses. It walks with `str::find` and copies the
/// gaps as slices; the earlier version tested every byte position by hand and
/// pushed the non-matching text back one `char` at a time.
pub fn replace_at_word_boundaries(content: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return content.to_string();
    }
    // A rejected match is retried one *char* later, not one needle later:
    // `needle` may overlap itself, and only the boundary test decides.
    let first_char_len = needle.chars().next().map_or(1, char::len_utf8);

    let mut result = String::with_capacity(content.len());
    // Everything before `copied` is already in `result`.
    let mut copied = 0;
    let mut search = 0;
    while let Some(rel) = content[search..].find(needle) {
        let i = search + rel;
        // Boundaries are tested on the adjacent *characters*, not the first
        // byte of a character. Testing the byte classified every non-ASCII
        // neighbour as a non-identifier and split identifiers like `获取积分`
        // into `获取total` (issue #165).
        let before_ok = i == 0
            || content[..i]
                .chars()
                .next_back()
                .is_none_or(|c| !is_ident_char_c(c));
        let after_idx = i + needle.len();
        let after_ok = after_idx >= content.len()
            || content[after_idx..]
                .chars()
                .next()
                .is_none_or(|c| !is_ident_char_c(c));
        if before_ok && after_ok {
            result.push_str(&content[copied..i]);
            result.push_str(replacement);
            copied = after_idx;
            search = after_idx;
        } else {
            search = i + first_char_len;
        }
    }
    result.push_str(&content[copied..]);
    result
}

// ---------------------------------------------------------------------------
// Import bindings
// ---------------------------------------------------------------------------

/// Is `s` one identifier and nothing else?
fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// The local name one import specifier binds: `a` → `a`, `a as b` → `b`,
/// `type a` → `a`, `* as N` → `N`.
fn specifier_binding(spec: &str) -> Option<String> {
    let spec = spec.trim().trim_end_matches(';').trim();
    let spec = spec.strip_prefix("type ").unwrap_or(spec).trim();
    let local = match spec.rsplit_once(" as ") {
        Some((_, local)) => local.trim(),
        None => spec,
    };
    is_identifier(local).then(|| local.to_string())
}

/// The names one single-line import statement binds in its file, for the
/// import forms whose binding can be read off the line: ES modules
/// (`import * as N`, `import D`, `import { a, b as c }`, `import type …`) and
/// Python (`import a.b`, `import a as b`, `from m import a, b as c`).
///
/// Empty for anything else, including multi-line statements and side-effect
/// imports (`import "./x"`). Callers treat empty as "binds nothing I know
/// of", which can only make them report less.
pub fn import_bindings(line: &str) -> Vec<String> {
    let t = line.trim().trim_end_matches(';').trim();
    if let Some(rest) = t.strip_prefix("import ") {
        let rest = rest.trim();
        // ES module: everything before ` from ` is the clause.
        if let Some((clause, _)) = rest.rsplit_once(" from ") {
            let clause = clause.trim();
            let clause = clause.strip_prefix("type ").unwrap_or(clause).trim();
            let mut out = Vec::new();
            let (head, braces) = match (clause.find('{'), clause.rfind('}')) {
                (Some(o), Some(c)) if o < c => (&clause[..o], Some(&clause[o + 1..c])),
                (Some(_), None) => return Vec::new(), // multi-line clause
                _ => (clause, None),
            };
            for part in head.split(',') {
                if let Some(n) = specifier_binding(part) {
                    out.push(n);
                }
            }
            if let Some(inner) = braces {
                for part in inner.split(',') {
                    if let Some(n) = specifier_binding(part) {
                        out.push(n);
                    }
                }
            }
            return out;
        }
        // Python `import a.b, c as d`. A quote means an ES side-effect import.
        if rest.contains(['"', '\'', '{', '(']) {
            return Vec::new();
        }
        return rest
            .split(',')
            .filter_map(|part| {
                let part = part.trim();
                match part.rsplit_once(" as ") {
                    Some((_, local)) => specifier_binding(local),
                    None => specifier_binding(part.split('.').next().unwrap_or("")),
                }
            })
            .collect();
    }
    if let Some(rest) = t.strip_prefix("from ") {
        if let Some((_, names)) = rest.split_once(" import ") {
            let names = names.trim();
            if names.starts_with('(') && !names.ends_with(')') {
                return Vec::new(); // multi-line
            }
            let names = names.trim_start_matches('(').trim_end_matches(')');
            return names.split(',').filter_map(specifier_binding).collect();
        }
    }
    Vec::new()
}

/// Does any import statement in `content` bind `name`?
pub fn has_import_binding(content: &str, name: &str) -> bool {
    content
        .lines()
        .any(|l| import_bindings(l).iter().any(|b| b == name))
}

/// Does `content` DECLARE `name` in the file itself — a definer keyword, or a
/// `const` / `let` / `var` / `val` declaration anywhere, nested ones included?
///
/// This is [`has_definition`] widened by the variable declarations, which bind
/// a name exactly as much as `function` does: a `const f = () => …` inside a
/// function body is a definition of `f`. Imports are not declarations: an
/// import binds a name that some OTHER file must still define.
pub fn has_declaration(content: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    has_definition(content, name)
        || content.lines().any(|line| {
            let mut t = line.trim_start();
            while let Some(rest) = MODIFIERS.iter().find_map(|kw| t.strip_prefix(kw)) {
                t = rest.trim_start();
            }
            let Some(rest) = ["const ", "let ", "var ", "val "]
                .iter()
                .find_map(|kw| t.strip_prefix(kw))
            else {
                return false;
            };
            let rest = rest.trim_start();
            let rest = rest.strip_prefix("mut ").unwrap_or(rest);
            // `const { a, b: c } = …` / `const [a, b] = …` destructure several.
            if rest.starts_with(['{', '[']) {
                let head = rest.split('=').next().unwrap_or(rest);
                return head
                    .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
                    .any(|w| w == name);
            }
            rest.starts_with(name)
                && rest[name.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !is_ident_char_c(c) && c != '$')
        })
}

/// Is `name` bound in `content` at all: declared there, or imported?
pub fn has_binding(content: &str, name: &str) -> bool {
    has_declaration(content, name) || has_import_binding(content, name)
}

/// Does `content` USE `name` as a value — a call `name(`, a member access
/// `name.x` or a generic `name<` — at a word boundary, not itself preceded by
/// `.`, outside import statements, comments and string-looking text?
///
/// Wider than [`has_call_reference`] by exactly the member access, which is
/// how a namespace import (`import * as N`) is used and the shape the call
/// rule cannot see.
pub fn has_value_reference(content: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let first_char_len = name.chars().next().map_or(1, char::len_utf8);
    content.lines().any(|line| {
        if is_trivia_line(line) || !import_bindings(line).is_empty() {
            return false;
        }
        let t = line.trim_start();
        if t.starts_with("import ") || t.starts_with("from ") {
            return false;
        }
        let mut from = 0usize;
        while let Some(rel) = line[from..].find(name) {
            let i = from + rel;
            let before = line[..i].chars().next_back();
            let before_ok = before.is_none_or(|c| !is_ident_char_c(c) && c != '.' && c != '$');
            // Outside a string: an even number of quotes before the match.
            let quotes = line[..i]
                .chars()
                .filter(|c| matches!(c, '"' | '\'' | '`'))
                .count();
            let after = &line[i + name.len()..];
            let after_ok = after.starts_with('.') && !after.starts_with("..")
                || after.trim_start().starts_with('(')
                || after.starts_with('<');
            if before_ok && after_ok && quotes % 2 == 0 && !is_definition_line(line, name) {
                return true;
            }
            from = i + first_char_len;
            if from >= line.len() {
                break;
            }
        }
        false
    })
}

/// Does `content` MENTION `name` as a reference: at a word boundary, in any
/// expression or type position, outside import statements, comments and
/// string-looking text, and not on a line that defines it?
///
/// Wider than [`has_value_reference`] by every position a name is read in
/// without a `.`, `(` or `<` after it: an argument `f(name)`, a type argument
/// `List<Name>`, an operand `a + name`, a return `return name;`. Narrower in
/// the positions where the word is not a reference to a file-scope binding
/// at all:
///
/// * after `.`, `::` or `->` — a member, resolved in its receiver;
/// * before a single `:` — a key, a named argument, a label, or a parameter
///   with its type (`name: T`) — or before a single `=` or `=>` — a named
///   argument, an assignment target, an arrow parameter. Those introduce the
///   word, or name a slot; they do not read the binding.
pub fn has_mention(content: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let first_char_len = name.chars().next().map_or(1, char::len_utf8);
    content.lines().any(|line| {
        if is_trivia_line(line) || !import_bindings(line).is_empty() {
            return false;
        }
        let t = line.trim_start();
        // Import, re-export and package statements bind or name; they read
        // nothing of this file's.
        let statement = ["import ", "from ", "use ", "using ", "package "]
            .iter()
            .any(|kw| t.starts_with(kw))
            || (t.starts_with("export ") && t.contains(['"', '\'']));
        if statement {
            return false;
        }
        if is_definition_line(line, name) {
            return false;
        }
        let code = &without_comments(line);
        let mut from = 0usize;
        while let Some(rel) = code[from..].find(name) {
            let i = from + rel;
            let lead = &code[..i];
            let before = lead.chars().next_back();
            let member = lead.ends_with('.') || lead.ends_with("::") || lead.ends_with("->");
            let before_ok = !member && before.is_none_or(|c| !is_ident_char_c(c) && c != '$');
            let quotes = lead
                .chars()
                .filter(|c| matches!(c, '"' | '\'' | '`'))
                .count();
            let rest = &code[i + name.len()..];
            let boundary = rest.chars().next().is_none_or(|c| !is_ident_char_c(c));
            let next = rest.trim_start();
            let slot = (next.starts_with(':') && !next.starts_with("::"))
                || (next.starts_with('=') && !next.starts_with("=="));
            if before_ok && boundary && !slot && quotes % 2 == 0 {
                return true;
            }
            from = i + first_char_len;
            if from >= code.len() {
                break;
            }
        }
        false
    })
}

/// `line` with its comments blanked out: a `/* … */` span (JSX writes its
/// comments as `{/* … */}`) and a trailing `//` comment. Text inside a string
/// is left alone, so a URL's `//` is not a comment.
fn without_comments(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
                out.push(c);
            }
            None if c == '/' && chars.peek() == Some(&'/') => break,
            None if c == '/' && chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = ' ';
                for d in chars.by_ref() {
                    if prev == '*' && d == '/' {
                        break;
                    }
                    prev = d;
                }
                out.push(' ');
            }
            None => {
                if matches!(c, '"' | '\'' | '`') {
                    quote = Some(c);
                }
                out.push(c);
            }
        }
    }
    out
}

/// Import lines one side deleted that the merge must keep because the OTHER
/// side's new code depends on them.
///
/// The shape: base imports `N`. Side X deletes that import (its own code no
/// longer uses `N`, and nothing else in X binds `N`). Side Y keeps the import
/// line verbatim and ADDS a line that uses `N`. A deletion is a claim that
/// nothing needs the import any more, and Y's addition falsifies it, so taking
/// the deletion silently breaks Y's addition — the name is used and no longer
/// bound, a compile error the merge introduced with no marker to point at it.
/// Keeping the line honours both sides: X's code does not use `N`, so the kept
/// import changes nothing X wrote.
///
/// Symmetric in the two sides, and empty whenever one side equals base
/// (a side that changed nothing added no dependency), so `merge(b, x, b) = x`
/// and idempotence are untouched.
pub fn imports_kept_for_new_uses(base: &str, ours: &str, theirs: &str) -> Vec<String> {
    let base_lines: HashSet<&str> = base.lines().map(str::trim).collect();
    let mut keep: Vec<String> = Vec::new();
    for (deleter, keeper) in [(ours, theirs), (theirs, ours)] {
        let deleter_lines: HashSet<&str> = deleter.lines().map(str::trim).collect();
        let keeper_lines: HashSet<&str> = keeper.lines().map(str::trim).collect();
        // The lines the keeper ADDED: its new code.
        let added: String = keeper
            .lines()
            .filter(|l| !base_lines.contains(l.trim()))
            .map(|l| format!("{l}\n"))
            .collect();
        if added.is_empty() {
            continue;
        }
        for line in base.lines() {
            let t = line.trim();
            if deleter_lines.contains(t) || !keeper_lines.contains(t) {
                continue;
            }
            let names = import_bindings(t);
            let needed = names
                .iter()
                .any(|n| has_value_reference(&added, n) && !has_binding(deleter, n));
            if needed && !keep.iter().any(|k| k == t) {
                keep.push(t.to_string());
            }
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_bindings_read_the_local_names() {
        let b = |l: &str| import_bindings(l);
        assert_eq!(b("import * as Widget from \"./w\""), vec!["Widget"]);
        assert_eq!(
            b("import Foo, { bar, baz as qux } from 'x';"),
            vec!["Foo", "bar", "qux"]
        );
        assert_eq!(b("import type { Gadget } from \"./g\""), vec!["Gadget"]);
        assert_eq!(b("import { type Gizmo } from \"./g\""), vec!["Gizmo"]);
        assert_eq!(b("from pkg.mod import a, b as c"), vec!["a", "c"]);
        assert_eq!(b("import os.path, numpy as np"), vec!["os", "np"]);
        assert!(b("import \"./side-effect\"").is_empty());
        assert!(b("import {").is_empty());
        assert!(b("const widget = 1").is_empty());
    }

    #[test]
    fn a_local_const_is_a_binding() {
        let body = "function outer() {\n  const openThing = (x: number): void => {\n    x\n  }\n  openThing(1)\n}\n";
        assert!(has_binding(body, "openThing"));
        assert!(has_binding("  let { a, b: c } = obj\n", "c"));
        assert!(!has_binding("  openThing(1)\n", "openThing"));
    }

    #[test]
    fn a_member_access_is_a_value_reference() {
        assert!(has_value_reference("  yield* Widget.first()\n", "Widget"));
        assert!(has_value_reference("  gadget(1)\n", "gadget"));
        assert!(!has_value_reference("  obj.Widget.first()\n", "Widget"));
        assert!(!has_value_reference("  log(\"Widget.first\")\n", "Widget"));
        assert!(!has_value_reference(
            "import * as Widget from \"./w\"\n",
            "Widget"
        ));
        assert!(!has_value_reference("  // Widget.first()\n", "Widget"));
    }

    #[test]
    fn a_mention_is_any_reference_position() {
        // Reads of the binding, in every position.
        assert!(has_mention("    openBox(wordsBox);\n", "wordsBox"));
        assert!(has_mention("  public List<Receipt> all() {\n", "Receipt"));
        assert!(has_mention("  return a + wordsBox\n", "wordsBox"));
        assert!(has_mention("  Store::open(wordsBox)\n", "Store"));
        // Not a read of a file-scope binding: a member, a key, a named
        // argument, an assignment target, a comment, a string, an import.
        assert!(!has_mention("  box.wordsBox()\n", "wordsBox"));
        assert!(!has_mention("  Store::wordsBox\n", "wordsBox"));
        assert!(!has_mention("  open(wordsBox: 1)\n", "wordsBox"));
        assert!(!has_mention("  f(wordsBox=1)\n", "wordsBox"));
        assert!(!has_mention("  wordsBox = 2\n", "wordsBox"));
        assert!(!has_mention("  go() // wordsBox\n", "wordsBox"));
        assert!(!has_mention("        {/* Heap wordsBox */}\n", "wordsBox"));
        assert!(has_mention("  go(/* a */ wordsBox)\n", "wordsBox"));
        assert!(has_mention("  fetch(\"http://x\", wordsBox)\n", "wordsBox"));
        assert!(!has_mention("  log(\"wordsBox\")\n", "wordsBox"));
        assert!(!has_mention("import a.b.Receipt;\n", "Receipt"));
        assert!(!has_mention("class Receipt {\n", "Receipt"));
        assert!(has_mention("  x == wordsBox\n", "wordsBox"));
    }

    #[test]
    fn a_definition_line_is_not_a_call_site() {
        assert!(is_definition_line("def fetch_user(id):", "fetch_user"));
        assert!(is_definition_line(
            "    pub async fn fetch_user(id: u32) {",
            "fetch_user"
        ));
        assert!(!has_call_reference(
            "def fetch_user(id):\n    pass\n",
            "fetch_user"
        ));
    }

    #[test]
    fn a_call_needs_a_word_boundary_and_an_open_paren() {
        assert!(has_call_reference("x = fetch_user(1)\n", "fetch_user"));
        // prefix of a longer identifier
        assert!(!has_call_reference("x = fetch_user_id(1)\n", "fetch_user"));
        // attribute access binds elsewhere
        assert!(!has_call_reference("x = db.fetch_user(1)\n", "fetch_user"));
        // a bare mention is not a call
        assert!(!has_call_reference("x = 'fetch_user'\n", "fetch_user"));
    }

    #[test]
    fn definers_survive_modifier_prefixes() {
        assert!(is_definition_line("export default class Foo {", "Foo"));
        assert!(!is_definition_line("export default class FooBar {", "Foo"));
    }

    /// The divergence this module exists to remove: the CLI's copy treated `$`
    /// as an identifier character and weave-core's did not, so the two passes
    /// disagreed about whether `$fetch(` calls `fetch`.
    #[test]
    fn a_dollar_prefixed_identifier_is_its_own_name() {
        assert!(!has_call_reference("x = $fetch(1)\n", "fetch"));
        assert!(has_call_reference("x = $fetch(1)\n", "$fetch"));
        assert_eq!(
            replace_at_word_boundaries("$fetch(1); fetch(2)", "fetch", "grab"),
            "$fetch(1); grab(2)"
        );
    }

    /// A read/write set is only ever `Some` when the reader can see the whole
    /// effect. Everything else answers `None`, and `None` is what makes the
    /// disjoint-composition license safe to hand out.
    #[test]
    fn a_footprint_exists_only_where_the_effect_is_visible() {
        // Declarations: writes the name, reads the expression.
        let fp = footprint("timeout = settings.timeout\n").expect("a declaration has one");
        assert!(fp.defines.contains("timeout"));
        assert!(fp.uses.contains("settings"));
        assert!(!fp.uses.contains("timeout"), "a name is not its own use");

        // Definitions: writes the name, reads its body.
        let fp = footprint("def scale(v):\n    return v * factor\n").expect("a def has one");
        assert!(fp.defines.contains("scale"));
        assert!(fp.uses.contains("factor"));

        // Everything whose effect lives somewhere this reader cannot look.
        for opaque in [
            "flush()\n",                                // a call
            "return total\n",                           // control flow
            "if ready:\n    go()\n",                    // a branch
            "total += 1\n",                             // augmented assignment
            "self.cache[key] = v\n",                    // a field/index target
            "@register\ndef scale(v):\n    return v\n", // definition-time registration
            "",                                         // nothing bound at all
        ] {
            assert!(
                footprint(opaque).is_none(),
                "{opaque:?} should have no read/write set"
            );
        }
    }

    /// Disjoint composition itself: disjoint writes, and neither writing what the
    /// other reads.
    #[test]
    fn disjoint_composition_is_symmetric_and_fails_on_either_edge() {
        let a = footprint("def scale(v):\n    return v * 2\n").unwrap();
        let b = footprint("def offset(v):\n    return v + 3\n").unwrap();
        assert!(footprints_disjoint(&a, &b));
        assert!(footprints_disjoint(&b, &a));

        // theirs reads what ours writes
        let uses_a = footprint("def offset(v):\n    return scale(v) + 3\n").unwrap();
        assert!(!footprints_disjoint(&a, &uses_a));
        assert!(!footprints_disjoint(&uses_a, &a));

        // both write the same name
        let rival = footprint("def scale(v):\n    return v + 3\n").unwrap();
        assert!(!footprints_disjoint(&a, &rival));
    }

    #[test]
    fn the_call_index_and_the_call_predicate_agree() {
        let text = "x = alpha(1)\ny = db.beta(2)\nz = 'gamma('\ndef delta():\n    delta_helper()\n";
        let indexed = called_names(text);
        for name in ["alpha", "beta", "gamma", "delta", "delta_helper"] {
            assert_eq!(
                indexed.contains(name),
                has_call_reference(text, name),
                "index and predicate disagree about {name}"
            );
        }
    }

    /// Issue #165. A non-ASCII identifier used to slice inside a multi-byte
    /// character and panic (exit 101), aborting the surrounding `git merge`
    /// instead of producing a verdict. These must all return a value, not panic.
    #[test]
    fn non_ascii_identifiers_do_not_panic() {
        // The exact unit form from the report.
        assert!(!has_call_reference("class C:\n    self.积分 = 1\n", "积分"));
        // A bare mention on a line, not a call.
        assert!(!has_call_reference(
            "def helper():\n    x = 积分\n    return x\n",
            "积分"
        ));
        // The definition line itself is never a call.
        assert!(!has_call_reference("def 积分():\n    return 1\n", "积分"));
    }

    /// A non-ASCII call is detected, and a non-ASCII name that only appears as a
    /// substring of a longer identifier is not a call to it.
    #[test]
    fn non_ascii_call_boundaries_are_by_character() {
        assert!(has_call_reference("x = 积分()\n", "积分"));
        assert!(has_call_reference("total = 获取积分(2)\n", "获取积分"));
        // `积分` is a suffix of `获取积分`, not a call on its own.
        assert!(!has_call_reference("total = 获取积分(2)\n", "积分"));
    }

    /// The call index sees non-ASCII call sites too, so rename repair no longer
    /// silently skips a non-ASCII rename, and it stays consistent with the
    /// predicate.
    #[test]
    fn the_call_index_covers_non_ascii() {
        let text = "x = 积分()\ny = 获取积分(2)\n";
        let indexed = called_names(text);
        assert!(indexed.contains("积分"));
        assert!(indexed.contains("获取积分"));
        for name in ["积分", "获取积分"] {
            assert_eq!(
                indexed.contains(name),
                has_call_reference(text, name),
                "index and predicate disagree about {name}"
            );
        }
    }

    /// The rewrite respects character boundaries: it replaces a whole non-ASCII
    /// identifier but never splits a longer one that merely contains it.
    #[test]
    fn replace_respects_non_ascii_boundaries() {
        assert_eq!(
            replace_at_word_boundaries("积分(2)", "积分", "total"),
            "total(2)"
        );
        // The reported split: `获取积分` must stay whole, not become `获取total`.
        assert_eq!(
            replace_at_word_boundaries("y = 获取积分(2)", "积分", "total"),
            "y = 获取积分(2)"
        );
        assert_eq!(
            replace_at_word_boundaries("获取(积分)", "积分", "total"),
            "获取(total)"
        );
    }
}
