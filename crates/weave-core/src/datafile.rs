//! Structured data files — JSON, TOML, YAML — read the way their consumers
//! read them: as tables of keys.
//!
//! A line merge of a data file can be clean line by line and still be invalid
//! data: two sides each add `tower-http = …` to `[dependencies]` at different
//! lines, both lines survive, and the file no longer loads. No line is stated
//! twice, so the line rulers never see it. What is stated twice is a KEY, at
//! one table path — which is the ruler here.
//!
//! [`keys`] answers, per file, either "this does not parse" or the most times
//! any one table/object states each key. TOML and YAML parsers already reject
//! a key stated twice in a mapping; JSON parsers
//! accept one (the last wins), so JSON is walked here and its keys counted.

use std::collections::HashMap;

/// `"<table path>\u{1f}<key>"` -> the most times one table at that path
/// states the key. JSON array elements share the path `[]`. YAML root
/// sequence IDs have a bare name and a ranked entry for each document
/// containing duplicates, ordered by decreasing multiplicity.
pub type KeyTally = HashMap<String, usize>;

/// The data format of `path`, if it is one weave reads keys out of.
fn format(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?;
    if ext.eq_ignore_ascii_case("json") || ext.eq_ignore_ascii_case("jsonc") {
        Some("JSON")
    } else if ext.eq_ignore_ascii_case("toml") {
        Some("TOML")
    } else if ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml") {
        Some("YAML")
    } else {
        None
    }
}

/// `None` when `path` is not a data file. Otherwise the key tally, or why the
/// text does not parse as its format.
pub fn keys(path: &str, text: &str) -> Option<Result<KeyTally, String>> {
    Some(match format(path)? {
        "JSON" => json_keys(text),
        "TOML" => text
            .parse::<toml::Table>()
            .map(|_| KeyTally::new())
            .map_err(|e| e.message().to_string()),
        _ => yaml_keys(text),
    })
}

/// The format's name, for a sentence about it.
pub fn format_name(path: &str) -> Option<&'static str> {
    format(path)
}

pub(crate) fn yaml_documents(
    text: &str,
) -> impl Iterator<Item = Result<oneyaml::Value, oneyaml::Error>> + '_ {
    use serde::Deserialize;
    let settings = oneyaml::Settings::new().honor_version_directive(true);
    oneyaml::Deserializer::from_str_with(text, &settings).map(oneyaml::Value::deserialize)
}

/// Mapping duplicates are load errors. Count sequence duplicates per document
/// so an inherited duplicate cannot mask one added in another document.
fn yaml_keys(text: &str) -> Result<KeyTally, String> {
    use oneyaml::Value;

    let mut keys = KeyTally::new();
    let mut duplicates = HashMap::<String, Vec<usize>>::new();
    let mut local = HashMap::new();
    for doc in yaml_documents(text) {
        let doc = doc.map_err(|e| e.to_string())?;
        let root = doc.untag_ref();
        let mut pending = vec![root];
        while let Some(value) = pending.pop() {
            match value.untag_ref() {
                Value::Mapping(mapping) => {
                    for (key, child) in mapping {
                        if let Value::String(name) = key {
                            keys.insert(format!("\u{1f}{name}"), 1);
                        } else {
                            let name = typed_yaml_key(key).map_err(|e| e.to_string())?;
                            keys.insert(format!("typed\u{1f}{name}"), 1);
                        }
                        pending.push(child);
                    }
                }
                Value::Sequence(items) => pending.extend(items),
                _ => {}
            }
        }
        let Some(items) = root.as_sequence() else {
            continue;
        };
        for item in items {
            // Other fields (e.g. Kubernetes `kind`) can be shared by distinct items.
            let Some(id @ (Value::String(_) | Value::Number(_) | Value::Bool(_))) =
                item.untag_ref().get("id")
            else {
                continue;
            };
            let label = oneyaml::to_string(id).map_err(|e| e.to_string())?;
            *local.entry(label).or_insert(0) += 1;
        }
        for (id, count) in local.drain() {
            if count > 1 {
                duplicates.entry(id.clone()).or_default().push(count);
            }
            keys.insert(id, 1);
        }
    }
    // Ranks survive reordering or adding documents that have no duplicates.
    for (id, mut counts) in duplicates {
        counts.sort_unstable_by(|a, b| b.cmp(a));
        for (index, count) in counts.into_iter().enumerate() {
            keys.insert(format!("{}\u{1f}{id}", index + 1), count);
        }
    }
    Ok(keys)
}

fn typed_yaml_key(key: &oneyaml::Value) -> Result<String, oneyaml::Error> {
    use oneyaml::{TaggedValue, Value};

    fn ordered(value: &Value) -> Result<Value, oneyaml::Error> {
        Ok(match value {
            Value::Mapping(mapping) => {
                let mut entries = Vec::with_capacity(mapping.len());
                for (key, value) in mapping {
                    let key = ordered(key)?;
                    entries.push((oneyaml::to_string(&key)?, key, ordered(value)?));
                }
                entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));
                Value::Mapping(
                    entries
                        .into_iter()
                        .map(|(_, key, value)| (key, value))
                        .collect(),
                )
            }
            Value::Sequence(items) => {
                Value::Sequence(items.iter().map(ordered).collect::<Result<_, _>>()?)
            }
            Value::Tagged(tagged) => Value::Tagged(Box::new(TaggedValue {
                tag: tagged.tag.clone(),
                value: ordered(&tagged.value)?,
            })),
            _ => value.clone(),
        })
    }

    // Map order is immaterial to key equality, including inside a sequence key.
    if matches!(key.untag_ref(), Value::Mapping(_) | Value::Sequence(_)) {
        oneyaml::to_string(&ordered(key)?)
    } else {
        oneyaml::to_string(key)
    }
}

/// JSON as a person writes it for a tool: comments and trailing commas are
/// allowed (tsconfig, VS Code settings), anything else malformed is not.
fn json_keys(text: &str) -> Result<KeyTally, String> {
    let mut walk = Json {
        s: text.as_bytes(),
        i: 0,
        tally: KeyTally::new(),
    };
    walk.skip();
    if walk.i < walk.s.len() {
        walk.value("", 0)?;
        walk.skip();
    }
    if walk.i < walk.s.len() {
        return Err(format!("unexpected text at byte {}", walk.i));
    }
    Ok(walk.tally)
}

struct Json<'a> {
    s: &'a [u8],
    i: usize,
    tally: KeyTally,
}

/// Deeper than this is not a file a person merges; refuse to guess.
const MAX_DEPTH: usize = 256;

impl Json<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    /// Whitespace and comments.
    fn skip(&mut self) {
        loop {
            while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"//") {
                while self.peek().is_some_and(|b| b != b'\n') {
                    self.i += 1;
                }
            } else if self.s[self.i..].starts_with(b"/*") {
                match self.s[self.i + 2..].windows(2).position(|w| w == b"*/") {
                    Some(end) => self.i += 2 + end + 2,
                    None => self.i = self.s.len(),
                }
            } else {
                return;
            }
        }
    }

    fn error(&self, what: &str) -> String {
        match self.peek() {
            Some(b) => format!("{what} at byte {}, found `{}`", self.i, b as char),
            None => format!("{what}, found end of file"),
        }
    }

    fn value(&mut self, path: &str, depth: usize) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err("nested too deeply".to_string());
        }
        match self.peek() {
            Some(b'{') => self.object(path, depth),
            Some(b'[') => self.array(path, depth),
            Some(b'"') => self.string().map(|_| ()),
            Some(b) if b == b'-' || b.is_ascii_alphanumeric() => {
                let start = self.i;
                while self
                    .peek()
                    .is_some_and(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
                {
                    self.i += 1;
                }
                let word = &self.s[start..self.i];
                let number = word[0] == b'-' || word[0].is_ascii_digit();
                if number || matches!(word, b"true" | b"false" | b"null") {
                    Ok(())
                } else {
                    self.i = start;
                    Err(self.error("expected a value"))
                }
            }
            _ => Err(self.error("expected a value")),
        }
    }

    /// A string's raw bytes between the quotes, escapes left as written: two
    /// spellings of one key are rare enough that exact text is the identity.
    fn string(&mut self) -> Result<String, String> {
        self.i += 1; // opening quote
        let start = self.i;
        loop {
            match self.peek() {
                Some(b'"') => {
                    let raw = String::from_utf8_lossy(&self.s[start..self.i]).to_string();
                    self.i += 1;
                    return Ok(raw);
                }
                Some(b'\\') => self.i += 2,
                Some(b'\n') | None => return Err(self.error("unterminated string")),
                Some(_) => self.i += 1,
            }
        }
    }

    fn object(&mut self, path: &str, depth: usize) -> Result<(), String> {
        self.i += 1; // {
        let mut here: HashMap<String, usize> = HashMap::new();
        loop {
            self.skip();
            match self.peek() {
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                Some(b'"') => {}
                _ => return Err(self.error("expected a key or `}`")),
            }
            let key = self.string()?;
            self.skip();
            if self.peek() != Some(b':') {
                return Err(self.error("expected `:`"));
            }
            self.i += 1;
            self.skip();
            self.value(&format!("{path}.{key}"), depth + 1)?;
            *here.entry(key).or_insert(0) += 1;
            self.skip();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {}
                _ => return Err(self.error("expected `,` or `}`")),
            }
        }
        for (key, n) in here {
            let slot = self.tally.entry(format!("{path}\u{1f}{key}")).or_insert(0);
            *slot = (*slot).max(n);
        }
        Ok(())
    }

    fn array(&mut self, path: &str, depth: usize) -> Result<(), String> {
        self.i += 1; // [
        let inner = format!("{path}[]");
        loop {
            self.skip();
            if self.peek() == Some(b']') {
                self.i += 1;
                return Ok(());
            }
            self.value(&inner, depth + 1)?;
            self.skip();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {}
                _ => return Err(self.error("expected `,` or `]`")),
            }
        }
    }
}

/// `"a.b\u{1f}k"` as the reader writes it: `a.b.k`.
pub fn display_key(entry: &str) -> String {
    match entry.split_once('\u{1f}') {
        Some(("", key)) => key.to_string(),
        Some((path, key)) => format!("{}.{key}", path.trim_start_matches('.')),
        None => entry.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tally(path: &str, text: &str) -> KeyTally {
        keys(path, text).expect("a data file").expect("parses")
    }

    #[test]
    fn a_json_key_stated_twice_in_one_object_is_counted_there() {
        let t = tally("p.json", "{\"a\": {\"k\": 1, \"k\": 2}, \"b\": {\"k\": 3}}");
        assert_eq!(t[".a\u{1f}k"], 2);
        assert_eq!(t[".b\u{1f}k"], 1);
        assert_eq!(t["\u{1f}a"], 1);
        assert_eq!(display_key(".a\u{1f}k"), "a.k");
    }

    #[test]
    fn json_array_elements_share_one_path() {
        let t = tally("p.json", "[{\"id\": 1}, {\"id\": 2, \"id\": 3}]");
        assert_eq!(t["[]\u{1f}id"], 2, "the most one element states it");
    }

    #[test]
    fn json_for_tools_may_carry_comments_and_trailing_commas() {
        let t = tally(
            "tsconfig.json",
            "// settings\n{\n  /* strict */ \"strict\": true,\n  \"paths\": [\"a\",],\n}\n",
        );
        assert_eq!(t["\u{1f}strict"], 1);
    }

    #[test]
    fn malformed_data_does_not_parse() {
        for (path, text) in [
            ("p.json", "{\"a\": 1 \"b\": 2}"),
            ("p.json", "{\"a\": }"),
            ("p.json", "{\"a\": 1"),
            (
                "Cargo.toml",
                "[dependencies]\nserde = \"1\"\nserde = \"2\"\n",
            ),
            ("c.yaml", "a: 1\na: 2\n"),
            ("c.yaml", "a: [1, 2\n"),
            (
                "c.yaml",
                "---\nok: true\n---\nkey: \"value\"# missing space\n",
            ),
            ("c.yaml", "flow: [a,\nb,\nc]"),
            ("c.yaml", "outer: {kind: Pod, \"\\u006bind\": Service}"),
        ] {
            assert!(
                keys(path, text).expect("data").is_err(),
                "{path}: {text:?} must not parse"
            );
        }
    }

    #[test]
    fn wellformed_data_parses() {
        for (path, text) in [
            ("p.json", "{\"a\": [1, -2.5e3, true, null, \"x\\\"y\"]}"),
            ("p.json", ""),
            (
                "Cargo.toml",
                "[dependencies]\nserde = { version = \"1\" }\n",
            ),
            ("c.yml", "---\na: 1\n---\na: 2\n"),
            ("c.yaml", "ref: !Ref Thing\n"),
            ("c.yml", "---\n{\"multi\n  line\": a}\n---\n{multi line: b}"),
        ] {
            assert!(
                keys(path, text).expect("data").is_ok(),
                "{path}: {text:?} must parse"
            );
        }
        assert!(keys("m.py", "x = 1").is_none());
    }
}
