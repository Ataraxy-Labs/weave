//! Arity evidence: how many arguments a definition takes, and how many a call
//! passes — asked only where the answer is a count, never a guess.
//!
//! The repo-scope pass can already see a *name* break across files: a rename
//! in `a.py` whose surviving caller lives in `b.py`. It could not see a
//! *signature* break, which merges just as cleanly:
//!
//! > ours changes `fetch_user(uid)` to `fetch_user(uid, tenant)` in `a.py`;
//! > theirs, which never touched `a.py`, adds `fetch_user(1)` to `b.py`. Both
//! > files merge per file without a conflict, the name still binds, and the
//! > program fails the first time `b.py` runs.
//!
//! This module answers the two counting questions that finding needs, and
//! refuses to answer whenever the count is not exact:
//!
//! * [`params`] — the number of parameters a function entity declares, or
//!   `None` when the language lets a call pass a different number and still be
//!   right: defaults (`x=1`), Python's `*args`/`**kwargs` and the `*` / `/`
//!   markers, rest and spread (`...`), TypeScript optionals (`x?`), Rust and
//!   Python receivers (`self`), or a shape this parser does not recognise.
//! * [`call_sites`] — every call of a name in a text whose argument count can
//!   be read off the call itself: a splat, a spread, a string the scan cannot
//!   close, or a parenthesis that never closes makes that call `None` rather
//!   than a number.
//!
//! "Call" means exactly what [`weave_core::binding::has_call_reference`] means
//! — the identifier at a word boundary, not after `.`, followed by `(`, never
//! on the line that defines it — so the arity pass and the dangling pass fire
//! on the same set of uses. False negatives are the accepted cost: a finding
//! here is meant to be acted on, and a miscounted call would not be.

use weave_core::binding::has_definition;

/// The languages whose parameter lists are read here. Everything else returns
/// `None`: counting a C++ overload set or a Kotlin named-argument call as one
/// number would be a guess.
fn family(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?;
    Some(match ext {
        "py" | "pyi" => "py",
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => "js",
        "rs" => "rs",
        "go" => "go",
        "java" => "java",
        _ => return None,
    })
}

/// Do two paths share a language — so a definition in one can be what a bare
/// call in the other reaches?
pub(crate) fn same_family(a: &str, b: &str) -> bool {
    matches!((family(a), family(b)), (Some(x), Some(y)) if x == y)
}

/// A definition's signature line and its exact parameter count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Signature {
    /// The text from the start of the definition through its closing `)`,
    /// whitespace-collapsed: what a finding shows a reader.
    pub text: String,
    pub params: usize,
}

/// The exact parameter count of the function entity `name` whose source is
/// `content`, or `None` when the language allows a call with a different
/// count to be correct, or when the parameter list cannot be found.
pub(crate) fn params(path: &str, name: &str, content: &str) -> Option<Signature> {
    let lang = family(path)?;
    let open = signature_open_paren(content, name)?;
    let (args, close) = split_args(content, open, true, lang == "rs")?;
    if args.len() == 1 && args[0].is_empty() {
        return Some(sig(content, open, close, 0));
    }
    for (i, a) in args.iter().enumerate() {
        let a = a.trim();
        if a.is_empty() {
            // A trailing comma is the only empty segment a valid list has.
            if i + 1 == args.len() {
                continue;
            }
            return None;
        }
        // Defaults, rest/variadics, splats and positional/keyword markers: a
        // call may legitimately pass a different number of arguments.
        if a.contains('=') || a.contains("...") || a.starts_with('*') || a == "/" {
            return None;
        }
        match lang {
            // A receiver is not passed in the parenthesised arguments.
            "py" if i == 0 && (a == "self" || a == "cls") => return None,
            "rs" if a.trim_start_matches(['&', ' ']).starts_with("self")
                || a.starts_with("mut self") =>
            {
                return None
            }
            // TypeScript `x?: T` / `x?` — optional.
            "js" if a
                .split(':')
                .next()
                .is_some_and(|n| n.trim_end().ends_with('?')) =>
            {
                return None
            }
            _ => {}
        }
    }
    let n = args.iter().filter(|a| !a.trim().is_empty()).count();
    Some(sig(content, open, close, n))
}

fn sig(content: &str, open: usize, close: usize, params: usize) -> Signature {
    // The signature reads from the line the parameter list opens on — a
    // decorator or doc comment above it is not part of what changed.
    let start = content[..open].rfind('\n').map_or(0, |i| i + 1);
    let text = content[start..=close]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    Signature { text, params }
}

/// Where the definition's own parameter list opens: the first `(` after the
/// name's first word-boundary occurrence, allowing generics (`<…>`, `[…]`)
/// and an arrow-function assignment (`= (`, `= async (`) in between.
fn signature_open_paren(content: &str, name: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut from = 0;
    let at = loop {
        let i = from + content[from..].find(name)?;
        let before = content[..i].chars().next_back();
        let after = content[i + name.len()..].chars().next();
        if before.is_none_or(|c| !ident(c) && c != '.') && after.is_none_or(|c| !ident(c)) {
            break i + name.len();
        }
        from = i + name.chars().next().map_or(1, char::len_utf8);
    };
    let mut j = skip_ws(bytes, at);
    // Generic parameters: `<T, U>` (Rust, TS, Java) or `[T any]` (Go).
    if let Some(&b) = bytes.get(j) {
        if b == b'<' || b == b'[' {
            j = skip_ws(bytes, close_of(bytes, j)? + 1);
        }
    }
    // `const name = (a, b) => …` / `= async (a) => …` / `= function (a) …`
    if bytes.get(j) == Some(&b'=') && bytes.get(j + 1) != Some(&b'=') {
        j = skip_ws(bytes, j + 1);
        for kw in ["async", "function"] {
            if content[j..].starts_with(kw) {
                j = skip_ws(bytes, j + kw.len());
            }
        }
    }
    (bytes.get(j) == Some(&b'(')).then_some(j)
}

fn ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    i
}

/// The index of the bracket closing the one at `open`, nesting `<>` too —
/// only ever called on a generic parameter list, where `<` is a bracket.
fn close_of(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (k, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'<' | b'[' | b'(' | b'{' => depth += 1,
            b'>' if k > 0 && bytes[k - 1] == b'-' => {}
            b'>' | b']' | b')' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}

/// The top-level, comma-separated segments between the `(` at `open` and its
/// matching `)`, plus the index of that `)`. `angles` also nests `<…>`, which a
/// parameter list needs (`HashMap<K, V>`) and a call must not (`a < b, c > d`).
/// `None` when the list never closes or a string in it never ends.
fn split_args(content: &str, open: usize, angles: bool, rust: bool) -> Option<(Vec<&str>, usize)> {
    let bytes = content.as_bytes();
    let mut depth = 0i32;
    let mut seg = open + 1;
    let mut out = Vec::new();
    let mut k = open;
    while k < bytes.len() {
        let b = bytes[k];
        match b {
            // A Rust lifetime (`&'a str`) opens no string; a char literal
            // (`'a'`) closes two bytes later.
            b'\''
                if rust
                    && bytes
                        .get(k + 1)
                        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
                    && bytes.get(k + 2) != Some(&b'\'') => {}
            b'"' | b'\'' | b'`' => {
                k = string_end(bytes, k)?;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b'<' if angles => depth += 1,
            b'>' if angles && k > 0 && matches!(bytes[k - 1], b'-' | b'=') => {}
            b'>' if angles => depth -= 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    out.push(content[seg..k].trim());
                    return Some((out, k));
                }
            }
            b',' if depth == 1 => {
                out.push(content[seg..k].trim());
                seg = k + 1;
            }
            _ => {}
        }
        k += 1;
    }
    None
}

/// Index of the quote closing the string opened at `open`; `None` if the
/// string runs off the end — a call whose arguments cannot be read.
fn string_end(bytes: &[u8], open: usize) -> Option<usize> {
    let q = bytes[open];
    let mut k = open + 1;
    while k < bytes.len() {
        match bytes[k] {
            b'\\' => k += 1,
            b'\n' if q != b'`' => return None,
            c if c == q => return Some(k),
            _ => {}
        }
        k += 1;
    }
    None
}

/// One call of a name: where it is, what it says, and how many arguments it
/// passes — `None` when that number cannot be read off the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallSite {
    /// 1-based line of the call's name.
    pub line: usize,
    pub text: String,
    pub args: Option<usize>,
}

/// Every call of `name` in `content`, a file at `path` (see the module doc for
/// what a call is).
pub(crate) fn call_sites(path: &str, content: &str, name: &str) -> Vec<CallSite> {
    let mut out = Vec::new();
    if name.is_empty() || !content.contains(name) {
        return out;
    }
    let rust = family(path) == Some("rs");
    let mut line_start = 0usize;
    for (n, line) in content.split_inclusive('\n').enumerate() {
        let here = line_start;
        line_start += line.len();
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
            continue;
        }
        if has_definition(line, name) {
            continue;
        }
        let mut from = 0usize;
        while let Some(rel) = line[from..].find(name) {
            let i = from + rel;
            from = i + name.chars().next().map_or(1, char::len_utf8);
            let before_ok = line[..i]
                .chars()
                .next_back()
                .is_none_or(|c| !ident(c) && c != '.');
            // Inside a string literal on this line: an odd number of quotes
            // before it. Not a call.
            let quotes = line[..i]
                .bytes()
                .filter(|b| matches!(b, b'"' | b'\'' | b'`'))
                .count();
            let after = i + name.len();
            let rest = &line[after..];
            let paren = rest.len() - rest.trim_start().len();
            if !before_ok || quotes % 2 == 1 || !rest.trim_start().starts_with('(') {
                continue;
            }
            let open = here + after + paren;
            let args = split_args(content, open, false, rust).and_then(|(segs, close)| {
                let text = &content[here + i..=close];
                let n = count_args(&segs)?;
                Some((n, text.split_whitespace().collect::<Vec<_>>().join(" ")))
            });
            out.push(CallSite {
                line: n + 1,
                text: args
                    .as_ref()
                    .map_or_else(|| line.trim().to_string(), |(_, t)| t.clone()),
                args: args.map(|(n, _)| n),
            });
        }
    }
    out
}

/// A call's exact argument count, or `None` when a splat or spread makes the
/// count a runtime fact.
fn count_args(segs: &[&str]) -> Option<usize> {
    if segs.len() == 1 && segs[0].is_empty() {
        return Some(0);
    }
    let mut n = 0;
    for (i, s) in segs.iter().enumerate() {
        if s.is_empty() {
            if i + 1 == segs.len() {
                continue;
            }
            return None;
        }
        if s.starts_with('*') || s.starts_with("...") || s.ends_with("...") {
            return None;
        }
        n += 1;
    }
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(path: &str, name: &str, src: &str) -> Option<usize> {
        params(path, name, src).map(|s| s.params)
    }

    #[test]
    fn exact_parameter_lists_count_and_flexible_ones_refuse() {
        assert_eq!(p("a.py", "f", "def f(a, b):\n    pass\n"), Some(2));
        assert_eq!(p("a.py", "f", "def f():\n    pass\n"), Some(0));
        assert_eq!(p("a.py", "f", "def f(a, b=1):\n    pass\n"), None);
        assert_eq!(p("a.py", "f", "def f(a, *rest):\n    pass\n"), None);
        assert_eq!(
            p(
                "a.ts",
                "f",
                "export function f(a: Map<K, V>, b?: number) {}"
            ),
            None
        );
        assert_eq!(
            p(
                "a.ts",
                "f",
                "const f = async (a: Map<K, V>, b: number) => a;"
            ),
            Some(2)
        );
        assert_eq!(
            p(
                "a.rs",
                "f",
                "pub fn f<T: Fn(u8, u8) -> u8>(a: T, b: Vec<(u8, u8)>) {}"
            ),
            Some(2)
        );
        assert_eq!(p("a.go", "f", "func f(a, b int, c ...string) {}"), None);
        assert_eq!(
            p("a.go", "f", "func f(a, b int) error { return nil }"),
            Some(2)
        );
        assert_eq!(p("a.rb", "f", "def f(a)\nend\n"), None);
    }

    #[test]
    fn call_sites_count_arguments_and_refuse_splats() {
        let src = "x = f(1, g(2, 3), 'a,b')\ny = obj.f(1)\nz = f(*args)\nw = f()\n";
        let calls = call_sites("a.py", src, "f");
        let counts: Vec<_> = calls.iter().map(|c| (c.line, c.args)).collect();
        assert_eq!(counts, vec![(1, Some(3)), (3, None), (4, Some(0))]);
    }
}
