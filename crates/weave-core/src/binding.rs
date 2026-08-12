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
//! is deliberately four functions: the two predicates, the whole-content
//! definition query, and the boundary-respecting rewrite. Nothing here knows
//! about merges, wire formats or files.
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
    content.lines().any(|line| {
        if is_definition_line(line, name) {
            return false;
        }
        let bytes = line.as_bytes();
        let mut from = 0usize;
        while let Some(rel) = line[from..].find(name) {
            let i = from + rel;
            let before_ok = i == 0 || (!is_ident_char(bytes[i - 1]) && bytes[i - 1] != b'.');
            let after = i + name.len();
            let call_ok = line[after..].trim_start().starts_with('(');
            if before_ok && call_ok {
                return true;
            }
            from = i + 1;
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
        let bytes = line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            let starts_ident = (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_')
                && (i == 0 || (!is_ident_char(bytes[i - 1]) && bytes[i - 1] != b'.'));
            if !starts_ident {
                i += 1;
                continue;
            }
            let start = i;
            while i < bytes.len() && is_ident_char(bytes[i]) {
                i += 1;
            }
            let name = &line[start..i];
            if line[i..].trim_start().starts_with('(') && !is_definition_line(line, name) {
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
    let bytes = content.as_bytes();
    // A rejected match is retried one *char* later, not one needle later:
    // `needle` may overlap itself, and only the boundary test decides.
    let first_char_len = needle.chars().next().map_or(1, char::len_utf8);

    let mut result = String::with_capacity(content.len());
    // Everything before `copied` is already in `result`.
    let mut copied = 0;
    let mut search = 0;
    while let Some(rel) = content[search..].find(needle) {
        let i = search + rel;
        let before_ok = i == 0 || {
            let prev_idx = content[..i]
                .char_indices()
                .next_back()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            !is_ident_char(bytes[prev_idx])
        };
        let after_idx = i + needle.len();
        let after_ok = after_idx >= content.len() || !is_ident_char(bytes[after_idx]);
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
