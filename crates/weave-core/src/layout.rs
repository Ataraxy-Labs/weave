//! Layout: what a text says, with how it is laid out taken away.
//!
//! Two texts are *layout-equal* when they differ only in whitespace — spaces,
//! blank lines, where a line is broken, a byte-order mark — and in nothing a
//! reader of the program could observe. Comments are content: a doc comment
//! generates documentation, a directive comment (`// @ts-ignore`,
//! `# type: ignore`) changes what a tool does, and an edit to one is an edit.
//!
//! Where the file's grammar is available the comparison is over the syntax
//! tree: every leaf in order, with its kind and its text, and any text a node
//! holds between its children. That is what makes a line break layout in one
//! place and content in another — `return\nx` and `return x` are different
//! programs in a language with automatic semicolons, and they are different
//! trees. A text that does not parse is never layout-equal to anything but
//! itself: an unparseable side says nothing a tree can vouch for. Where there
//! is no grammar the comparison is lexical: strings verbatim, every other
//! token by itself, whitespace dropped.
//!
//! Indentation is structure in some languages (Python, YAML), so there every
//! line's indentation is part of what the text says.

use crate::merge::{PARSER_REGISTRY, STRUCTURE_LIMIT_BYTES};

/// Extensions whose indentation is syntax.
fn indentation_is_syntax(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    matches!(
        ext,
        "py" | "pyi" | "yaml" | "yml" | "haml" | "pug" | "coffee" | "nim" | "sass" | "styl"
    )
}

/// `a` and `b` differ only in layout. See the module docs.
pub fn layout_equal(a: &str, b: &str, path: &str) -> bool {
    if a == b {
        return true;
    }
    let (a, b) = (strip_bom(a), strip_bom(b));
    if a == b {
        return true;
    }
    if a.len() > STRUCTURE_LIMIT_BYTES || b.len() > STRUCTURE_LIMIT_BYTES {
        return false;
    }
    match (syntax(a, path), syntax(b, path)) {
        (Syntax::Tree(x), Syntax::Tree(y)) => {
            x == y && indentation(a, path) == indentation(b, path)
        }
        (Syntax::NoGrammar, Syntax::NoGrammar) => {
            lexical(a) == lexical(b) && indentation(a, path) == indentation(b, path)
        }
        _ => false,
    }
}

/// `a` and `b` state the same tokens (and, where it is syntax, the same
/// indentation), without asking a grammar: for fragments of a file — one
/// declaration's text — that need not parse on their own.
pub fn tokens_equal(a: &str, b: &str, path: &str) -> bool {
    let (a, b) = (strip_bom(a), strip_bom(b));
    a == b
        || (lexical(a) == lexical(b)
            && relative_indentation(a, path) == relative_indentation(b, path))
}

/// [`indentation`], less the first line's: a fragment's own nesting.
fn relative_indentation(text: &str, path: &str) -> Vec<isize> {
    let all = indentation(text, path);
    let first = all.first().copied().unwrap_or(0) as isize;
    all.into_iter().map(|n| n as isize - first).collect()
}

fn strip_bom(t: &str) -> &str {
    t.strip_prefix('\u{feff}').unwrap_or(t)
}

#[derive(Debug, PartialEq, Eq)]
enum Syntax {
    /// The preorder leaves and in-node text of an error-free parse.
    Tree(Vec<(String, String)>),
    /// The grammar found an error.
    Broken,
    /// No tree-sitter grammar for this path.
    NoGrammar,
}

fn syntax(text: &str, path: &str) -> Syntax {
    let Some((_, Some(tree))) = PARSER_REGISTRY.extract_entities_with_tree(path, text) else {
        return Syntax::NoGrammar;
    };
    if tree.root_node().has_error() {
        return Syntax::Broken;
    }
    let bytes = text.as_bytes();
    let slice = |s: usize, e: usize| String::from_utf8_lossy(&bytes[s..e.min(bytes.len())]);
    let squash = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        let kind = node.kind();
        // Whitespace inside a literal is the literal's value.
        let literal = ["string", "char", "template", "heredoc", "raw"]
            .iter()
            .any(|k| kind.contains(k));
        if node.child_count() == 0 {
            let t = slice(node.start_byte(), node.end_byte());
            let t = if literal { t.into_owned() } else { squash(&t) };
            out.push((kind.to_string(), t));
        } else {
            out.push((kind.to_string(), String::new()));
            // Text a node holds between its children (some grammars keep
            // inline content there) is content too, and must not hide.
            let mut pos = node.start_byte();
            let mut child = node.walk();
            for c in node.children(&mut child) {
                let gap = slice(pos, c.start_byte());
                if literal && !gap.is_empty() || !gap.trim().is_empty() {
                    let g = if literal {
                        gap.into_owned()
                    } else {
                        squash(&gap)
                    };
                    out.push(("__gap__".to_string(), g));
                }
                pos = pos.max(c.end_byte());
            }
            let gap = slice(pos, node.end_byte());
            if literal && !gap.is_empty() || !gap.trim().is_empty() {
                let g = if literal {
                    gap.into_owned()
                } else {
                    squash(&gap)
                };
                out.push(("__gap__".to_string(), g));
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return Syntax::Tree(out);
            }
        }
    }
}

/// Strings verbatim; identifiers, numbers and single punctuation characters as
/// tokens; whitespace dropped. Comments are tokenized like everything else, so
/// their words count and their spacing does not.
fn lexical(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        if c == b'"' || c == b'\'' || c == b'`' {
            i += 1;
            while i < bytes.len() && bytes[i] != c && (c == b'`' || bytes[i] != b'\n') {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(bytes.len());
        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80 {
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric()
                    || bytes[i] == b'_'
                    || bytes[i] == b'$'
                    || bytes[i] >= 0x80)
            {
                i += 1;
            }
        } else {
            i += 1;
        }
        // Never split a UTF-8 sequence: widen to the next char boundary.
        while i < bytes.len() && !text.is_char_boundary(i) {
            i += 1;
        }
        out.push(&text[start..i]);
    }
    out
}

/// Every non-blank line's indentation, where indentation is syntax.
fn indentation(text: &str, path: &str) -> Vec<usize> {
    if !indentation_is_syntax(path) {
        return Vec::new();
    }
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_and_line_breaks_are_layout() {
        let a = "function f(a, b) {\n  return a + b;\n}\n";
        let b = "function f(a,b){\n\n    return a+b;\n}";
        assert!(layout_equal(a, b, "m.ts"));
    }

    #[test]
    fn a_byte_order_mark_is_layout() {
        let a = "using System;\nclass C {}\n";
        let b = "\u{feff}using System;\nclass C {}\n";
        assert!(layout_equal(a, b, "C.cs"));
    }

    #[test]
    fn a_comment_is_content() {
        let a = "function f() {\n  return 1;\n}\n";
        let b = "// the answer\nfunction f() {\n  return 1;\n}\n";
        assert!(!layout_equal(a, b, "m.ts"));
        let c = "function f() {\n  // @ts-ignore\n  return 1;\n}\n";
        assert!(!layout_equal(a, c, "m.ts"));
    }

    #[test]
    fn whitespace_inside_a_string_is_content() {
        let a = "const s = \"a b\";\n";
        let b = "const s = \"a  b\";\n";
        assert!(!layout_equal(a, b, "m.ts"));
    }

    #[test]
    fn a_line_break_that_changes_the_program_is_content() {
        let a = "function f() {\n  return x;\n}\n";
        let b = "function f() {\n  return\n  x;\n}\n";
        assert!(!layout_equal(a, b, "m.ts"));
    }

    #[test]
    fn indentation_is_content_in_python() {
        let a = "def f():\n    if x:\n        a()\n    b()\n";
        let b = "def f():\n    if x:\n        a()\n        b()\n";
        assert!(!layout_equal(a, b, "m.py"));
        let c = "def f():\n    if x:\n        a()\n\n\n    b()\n";
        assert!(layout_equal(a, c, "m.py"));
    }

    #[test]
    fn a_text_that_does_not_parse_vouches_for_nothing() {
        let a = "function f() {\n  return 1;\n}\n";
        let b = "function f() {\n  return 1;\n";
        assert!(!layout_equal(a, b, "m.ts"));
    }

    #[test]
    fn without_a_grammar_the_comparison_is_lexical() {
        assert!(layout_equal(
            "a  =  1\n\nb=2\n",
            "a = 1\nb = 2\n",
            "x.unknownext"
        ));
        assert!(!layout_equal("a = 1\n", "a = 2\n", "x.unknownext"));
        assert!(!layout_equal("# note\na = 1\n", "a = 1\n", "x.unknownext"));
    }
}
