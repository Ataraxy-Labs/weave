//! Validate each Markdown YAML example as its own document stream.

use std::borrow::Cow;
use std::collections::HashMap;

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};

pub fn supports(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, ext)| {
        ["md", "mdx", "markdown"]
            .iter()
            .any(|e| ext.eq_ignore_ascii_case(e))
    })
}

struct Fault {
    block: (&'static str, String),
    detail: String,
}

fn faults(text: &str) -> Vec<Fault> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    // CommonMark accepts CR, LF and CRLF as the same line ending.
    let normalized = text.split_once('\r').map(|(first, rest)| {
        let mut buf = String::with_capacity(text.len());
        buf.push_str(first);
        for line in rest.split('\r') {
            buf.push('\n');
            buf.push_str(line.strip_prefix('\n').unwrap_or(line));
        }
        buf
    });
    let text = normalized.as_deref().unwrap_or(text);
    let mut faults = Vec::new();
    let mut cursor = 0;
    let mut line = 1;
    let mut validate = |offset: usize, kind: &'static str, source: Cow<'_, str>| {
        let error = crate::datafile::yaml_documents(&source).find_map(Result::err);
        if let Some(error) = error {
            line += text[cursor..offset].bytes().filter(|&b| b == b'\n').count();
            cursor = offset;
            let detail = format!("{kind} starting at line {line}: {error}");
            faults.push(Fault {
                block: (kind, source.into_owned()),
                detail,
            });
        }
    };

    // A later horizontal rule is part of the Markdown body.
    let mut body = text;
    let mut lines = text.split_inclusive('\n');
    if let Some(open) = lines
        .next()
        .filter(|line| line.trim_end_matches([' ', '\t', '\n']) == "---")
    {
        let mut end = open.len();
        for line in lines {
            if matches!(line.trim_end_matches([' ', '\t', '\n']), "---" | "...") {
                validate(0, "YAML front matter", text[open.len()..end].into());
                body = &text[end + line.len()..];
                break;
            }
            end += line.len();
        }
    }

    if !body.contains("```") && !body.contains("~~~") {
        return faults;
    }
    let mut events = Parser::new(body).into_offset_iter();
    while let Some((event, range)) = events.next() {
        let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) = event else {
            continue;
        };
        let lang = info.split_whitespace().next().unwrap_or("");
        if !lang.eq_ignore_ascii_case("yaml") && !lang.eq_ignore_ascii_case("yml") {
            continue;
        }
        let mut source = Cow::Borrowed("");
        // Fenced blocks contain text events followed by the closing event.
        while let Some((Event::Text(chunk), _)) = events.next() {
            if source.is_empty() {
                source = chunk.into();
            } else {
                source.to_mut().push_str(&chunk);
            }
        }
        validate(text.len() - body.len() + range.start, "YAML fence", source);
    }
    faults
}

/// Report invalid YAML blocks beyond the copies already present on either side.
pub fn introduced_errors(
    path: &str,
    text: &str,
    ours: Option<&str>,
    theirs: Option<&str>,
) -> impl Iterator<Item = String> {
    let broken = if supports(path) && ours != Some(text) && theirs != Some(text) {
        faults(text)
    } else {
        Vec::new()
    };
    let mut remaining = HashMap::new();
    if !broken.is_empty() {
        for (index, side) in [ours, theirs].into_iter().enumerate() {
            for fault in side.into_iter().flat_map(faults) {
                remaining.entry(fault.block).or_insert([0usize; 2])[index] += 1;
            }
        }
    }
    broken.into_iter().filter_map(move |fault| {
        if let Some(counts) = remaining.get_mut(&fault.block) {
            if *counts != [0, 0] {
                *counts = counts.map(|n| n.saturating_sub(1));
                return None;
            }
        }
        Some(fault.detail)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn errors(path: &str, text: &str) -> Vec<String> {
        introduced_errors(path, text, None, None).collect()
    }

    #[test]
    fn quoted_keys_follow_yaml_rules_in_each_fenced_document() {
        let doc = "```yaml\nkind: Pod\n---\n{\"multi\n  line\": first}\n```\n";
        assert!(errors("example.md", doc).is_empty());
        let broken = doc.replace("first}", "first, 'multi line': second}");
        assert_eq!(errors("example.md", &broken).len(), 1);
    }

    #[test]
    fn markdown_scopes_each_yaml_stream_and_reports_each_broken_block() {
        let good = "\u{feff}---\r\n\r\nid: 1\r\n...\r\n\n~~~YmL example\n---\nid: 1\n---\nid: 2\na: {id: 3}\n~~~\n";
        let ignored = "~~~text\n```yaml\nid: 1\nid: 2\n```\n~~~\n\n    ```yaml\n    x: [\n\n---\nparagraph\n---\n";
        assert!(errors("doc.MDX", &format!("{good}{ignored}")).is_empty());
        let broken = "---\nx: 1\nx: 2\n---\n\n> ````YAML\n> nested: {id: 1, id: 2}\n> ````\n\n- ~~~yml\n  list: [\n  ~~~\n";
        let got = errors("doc.markdown", broken);
        for (message, expected) in got.iter().zip([
            "front matter starting at line 1",
            "fence starting at line 6",
            "fence starting at line 10",
        ]) {
            assert!(message.contains(expected), "{message}");
        }
        assert_eq!(got.len(), 3);
        assert!(errors("doc.txt", broken).is_empty());
    }

    #[test]
    fn markdown_line_endings_do_not_change_block_identity_or_location() {
        let doc = "---\na: 1\na: 2\n---\n\n~~~yaml\nb: [\n~~~\n";
        let expected = errors("a.md", doc);
        assert_eq!(expected.len(), 2);
        for newline in ["\r\n", "\r"] {
            let converted = doc.replace('\n', newline);
            assert_eq!(errors("a.md", &converted), expected);
            assert!(crate::verify::structured_data(None, Some(doc), &converted, "a.md").is_none());
        }
        assert!(errors("a.md", "---\u{a0}\na: 1\na: 2\n---\n").is_empty());
    }

    #[test]
    fn merge_only_refuses_new_or_duplicated_broken_examples() {
        let broken = "~~~yaml\nx: [\n~~~\n";
        let moved = format!("# Examples\n\n{broken}");
        let verify = |merged: &str, side: &str| {
            crate::verify::structured_data(Some(side), None, merged, "x.md")
        };
        assert!(verify(&moved, broken).is_none());
        assert!(verify(&moved, "").is_some());
        assert!(verify("---\nx: [\n---\n", broken).is_some());
        let twice = broken.repeat(2);
        let both = |theirs| introduced_errors("x.md", &twice, Some(broken), Some(theirs)).next();
        assert!(both(broken).is_some());
        assert!(both(&twice).is_none());
    }
}
