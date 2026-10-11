//! `weave edit`: how an agent changes code, addressed by entity.
//!
//! An edit names a function, class or method and says what becomes of it:
//! replace it, change one piece of text inside it, delete it, or add a new
//! entity before or after it. Text edits without an entity cover everything
//! else (imports, module-level statements, files weave has no grammar for),
//! and an edit to a file that does not exist creates it.
//!
//! Every edit is resolved against the file as it is on disk now, so nothing
//! has to be read first and nothing another agent changed elsewhere in the
//! file is overwritten. A text edit's `old` must match exactly once (or set
//! `all`), which is what keeps a stale edit from landing somewhere else.
//!
//! A batch is all or nothing: every edit is planned in memory, in order, and
//! files are written only when all of them resolved.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use sem_core::model::entity::SemanticEntity;

use crate::parsers::REGISTRY;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    /// The file, relative to the repository root.
    pub file: String,
    /// The entity to change: a function, class, method or key, by name.
    /// `Parent.name` (or `Parent::name`) picks a member of one class. Also
    /// takes pi-sem's form, `{name, entity_type?, parent_name?, ordinal?}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<EntityRef>,
    /// pi-sem's verb: `replace`, `insert_after`, `insert_before` or `delete`,
    /// each about `entity`. The same edits are also spelled with `content`,
    /// `after`, `before` and `delete` directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
    /// The entity's kind (function, method, class...) when names collide.
    #[serde(default, alias = "entity_type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// 0-based occurrence among entities that still match, in file order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<usize>,
    /// The new source. With `entity`: the entity's whole new text. With
    /// `after`, `before` or `append`: the entity to add. Alone, for a file that
    /// does not exist: the new file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Text to replace, inside `entity` when one is named, else in the whole
    /// file. Must match exactly once unless `all` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    /// What `old` becomes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
    /// Replace every occurrence of `old`, not exactly one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub all: bool,
    /// Add `content` as a new entity right after this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Add `content` as a new entity right before this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Add `content` at the end of the file.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub append: bool,
    /// Add `content` as the last member of this class (or other container),
    /// so a new method needs no anchor and nothing read first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub into: Option<String>,
    /// Delete `entity`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub delete: bool,
    /// Let a replace rename the entity, change its kind or its visibility
    /// (`export`, `pub`, `public`...). Refused otherwise, since that is how an
    /// edit meant for one entity silently becomes a different one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_signature_change: bool,
    /// Rename `entity` (defined in `file`) to this name, and every identifier
    /// spelled like it in the code files of `scope`. Strings and comments are
    /// left alone; files without a grammar that mention it are listed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename: Option<String>,
    /// For `rename`: the files to rename in, as git pathspec globs
    /// (`src/**`, `*.py`). Default: the whole repository.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EntityRef {
    Name(String),
    Ref {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entity_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ordinal: Option<usize>,
    },
}

impl From<&str> for EntityRef {
    fn from(s: &str) -> Self {
        EntityRef::Name(s.to_string())
    }
}

/// An edit in one spelling: the entity as `Parent.name`, `op` folded into
/// `content`/`after`/`before`/`delete`.
#[derive(Debug, Clone, Default)]
struct Plain {
    entity: Option<String>,
    kind: Option<String>,
    ordinal: Option<usize>,
    content: Option<String>,
    old: Option<String>,
    new: Option<String>,
    all: bool,
    after: Option<String>,
    before: Option<String>,
    append: bool,
    into: Option<String>,
    delete: bool,
    allow_signature_change: bool,
    /// Rename every identifier `.0` to `.1` in this file.
    rename_ident: Option<(String, String)>,
}

impl Edit {
    fn plain(&self) -> Result<Plain, String> {
        let (mut name, mut kind, mut ordinal) = (None, self.kind.clone(), self.ordinal);
        match &self.entity {
            Some(EntityRef::Name(n)) => name = Some(n.clone()),
            Some(EntityRef::Ref { name: n, entity_type, parent_name, ordinal: o }) => {
                name = Some(match parent_name {
                    Some(p) => format!("{p}.{n}"),
                    None => n.clone(),
                });
                kind = kind.or_else(|| entity_type.clone());
                ordinal = ordinal.or(*o);
            }
            None => {}
        }
        let mut p = Plain {
            entity: name,
            kind,
            ordinal,
            content: self.content.clone(),
            old: self.old.clone(),
            new: self.new.clone(),
            all: self.all,
            after: self.after.clone(),
            before: self.before.clone(),
            append: self.append,
            into: self.into.clone(),
            delete: self.delete,
            allow_signature_change: self.allow_signature_change,
            rename_ident: None,
        };
        match self.op.as_deref() {
            None | Some("replace") => {}
            Some("insert_after") => p.after = Some(p.entity.take().ok_or("insert_after needs the entity to insert after")?),
            Some("insert_before") => p.before = Some(p.entity.take().ok_or("insert_before needs the entity to insert before")?),
            Some("delete") => p.delete = true,
            Some(other) => return Err(format!("unknown op {other:?}: use replace, insert_after, insert_before or delete")),
        }
        Ok(p)
    }
}

/// What a batch did to one file, entity by entity.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FileReport {
    pub file: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub created: bool,
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub written: bool,
    pub files: Vec<FileReport>,
    /// Things the edits did not do that the agent may want to: e.g. files
    /// without a grammar that still mention a renamed name.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Why a batch was refused. Nothing was written.
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("edit {index} ({file}): {message}")]
pub struct EditError {
    /// Which edit of the batch, 0-based.
    pub index: usize,
    pub file: String,
    pub message: String,
    /// When a name was ambiguous or missing: the entities that could be meant.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
}

/// Apply `edits` under `root`. With `dry_run`, plan and report only.
///
/// Edits to one file apply in order; different files are planned in
/// parallel. Nothing is written unless every edit resolved and no edited
/// file parses worse than it did before.
pub fn run(root: &Path, edits: &[Edit], dry_run: bool) -> Result<Report, EditError> {
    let mut by_file: BTreeMap<String, Vec<(usize, Plain)>> = BTreeMap::new();
    let mut notes = Vec::new();
    for (index, e) in edits.iter().enumerate() {
        let fail = |message: String| EditError { index, file: e.file.clone(), message, candidates: vec![] };
        let path = normalize(root, &e.file).map_err(fail)?;
        let plain = e.plain().map_err(fail)?;
        if let Some(to) = &e.rename {
            let entity = plain.entity.clone().ok_or_else(|| fail("rename needs the entity to rename".into()))?;
            let from = entity.rsplit(['.', ':']).next().unwrap_or(&entity).to_string();
            if !is_identifier(to) {
                return Err(fail(format!("{to:?} is not an identifier")));
            }
            let (code, other) = mentions(root, &from, &e.scope);
            let mut files: Vec<String> = code;
            if !files.contains(&path) {
                files.insert(0, path.clone());
            }
            for f in files {
                let p = Plain { rename_ident: Some((from.clone(), to.clone())), entity: (f == path).then(|| entity.clone()), kind: plain.kind.clone(), ..Default::default() };
                by_file.entry(f).or_default().push((index, p));
            }
            if !other.is_empty() {
                notes.push(format!("{} also appears in files weave cannot parse, left as they are: {}", from, other.join(", ")));
            }
            continue;
        }
        by_file.entry(path).or_default().push((index, plain));
    }

    type Planned = Result<(String, Option<String>, String), EditError>;
    let planned: Vec<Planned> = std::thread::scope(|scope| {
        let jobs: Vec<_> = by_file
            .iter()
            .map(|(path, list)| scope.spawn(move || plan_file(root, path, list)))
            .collect();
        jobs.into_iter().map(|j| j.join().unwrap_or_else(|_| Err(EditError { index: 0, file: String::new(), message: "planning panicked".into(), candidates: vec![] }))).collect()
    });
    // the earliest failing edit is the one to report
    if let Some(e) = planned.iter().filter_map(|r| r.as_ref().err()).min_by_key(|e| e.index) {
        return Err(e.clone());
    }
    let files: Vec<(String, Option<String>, String)> = planned.into_iter().filter_map(Result::ok).filter(|(_, disk, now)| disk.as_deref() != Some(now.as_str())).collect();

    let reports: Vec<FileReport> = files.iter().map(|(path, disk, now)| report(path, disk.as_deref(), now)).collect();
    if !dry_run {
        for (path, _, now) in &files {
            let abs = root.join(path);
            if let Some(dir) = abs.parent() {
                std::fs::create_dir_all(dir).map_err(|e| io_error(path, e))?;
            }
            std::fs::write(&abs, now).map_err(|e| io_error(path, e))?;
        }
    }
    Ok(Report { written: !dry_run && !reports.is_empty(), files: reports, notes })
}

/// One file's edits, in order: (path, content on disk, content after).
fn plan_file(root: &Path, path: &str, list: &[(usize, Plain)]) -> Result<(String, Option<String>, String), EditError> {
    let first = list.first().map(|(i, _)| *i).unwrap_or(0);
    let fail = |index: usize, message: String, candidates: Vec<String>| EditError { index, file: path.to_string(), message, candidates };
    let disk = match std::fs::read(root.join(path)) {
        Ok(b) => Some(String::from_utf8(b).map_err(|_| fail(first, "not a UTF-8 text file".into(), vec![]))?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(fail(first, err.to_string(), vec![])),
    };
    let mut now = disk.clone().unwrap_or_default();
    for (index, e) in list {
        let exists = disk.is_some() || !now.is_empty();
        now = apply_one(path, &now, exists, e).map_err(|(m, c)| fail(*index, m, c))?;
    }
    if disk.as_deref() != Some(now.as_str()) {
        if let Some(why) = parses_worse(path, disk.as_deref().unwrap_or(""), &now) {
            let last = list.last().map(|(i, _)| *i).unwrap_or(first);
            return Err(fail(last, why, vec![]));
        }
    }
    Ok((path.to_string(), disk, now))
}

/// Does `after` have parse errors `before` did not? Only for languages with
/// a grammar; the answer names the first new error's line.
fn parses_worse(path: &str, before: &str, after: &str) -> Option<String> {
    use sem_core::parser::plugins::code::{language_config_for_content, parse_tree};
    let config = language_config_for_content(after, path)?;
    let errors = |src: &str| -> Vec<usize> {
        let Some(tree) = parse_tree(config, src) else { return vec![] };
        let mut lines = Vec::new();
        let mut cursor = tree.walk();
        let mut stack = vec![tree.root_node()];
        while let Some(n) = stack.pop() {
            if !n.has_error() && !n.is_missing() {
                continue;
            }
            if n.is_error() || n.is_missing() {
                lines.push(n.start_position().row + 1);
                continue;
            }
            stack.extend(n.children(&mut cursor));
        }
        lines.sort_unstable();
        lines
    };
    let (b, a) = (errors(before), errors(after));
    if a.len() <= b.len() {
        return None;
    }
    let line = a.iter().find(|l| !b.contains(l)).or(a.first()).copied().unwrap_or(0);
    Some(format!("the edits leave {path} unparseable near line {line}; nothing was written"))
}

fn io_error(path: &str, e: std::io::Error) -> EditError {
    EditError { index: 0, file: path.to_string(), message: format!("writing: {e}"), candidates: vec![] }
}

/// `file` as a path relative to `root`, refusing anything outside it.
fn normalize(root: &Path, file: &str) -> Result<String, String> {
    let p = Path::new(file);
    let rel: PathBuf = if p.is_absolute() {
        p.strip_prefix(root).map_err(|_| format!("{file} is outside the repository"))?.to_path_buf()
    } else {
        p.to_path_buf()
    };
    let mut out: Vec<String> = Vec::new();
    for c in rel.components() {
        match c {
            std::path::Component::Normal(s) => out.push(s.to_string_lossy().to_string()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if out.pop().is_none() {
                    return Err(format!("{file} is outside the repository"));
                }
            }
            _ => return Err(format!("{file} is not a relative path")),
        }
    }
    if out.is_empty() {
        return Err("no file named".into());
    }
    Ok(out.join("/"))
}

type Fail = (String, Vec<String>);

fn apply_one(path: &str, src: &str, exists: bool, e: &Plain) -> Result<String, Fail> {
    if let Some((from, to)) = &e.rename_ident {
        if let Some(name) = &e.entity {
            find(path, src, name, e.kind.as_deref(), None)?;
        }
        return Ok(rename_identifiers(path, src, from, to));
    }
    let anchors = [e.after.is_some(), e.before.is_some(), e.append, e.into.is_some()].iter().filter(|b| **b).count();
    if anchors > 1 {
        return Err(("use only one of after, before, append and into".into(), vec![]));
    }
    if let Some(container) = &e.into {
        let content = e.content.as_deref().ok_or_else(|| ("into needs content: the member to add".to_string(), vec![]))?;
        let parent = find(path, src, container, None, None)?;
        let all = entities(path, src);
        let last = all.iter().filter(|m| m.parent_id.as_deref() == Some(parent.id.as_str())).max_by_key(|m| m.end_line);
        let Some(last) = last else {
            return Err((format!("{container} has no members to add after; use after or old/new"), vec![]));
        };
        let (start, end) = span(src, last);
        let indent = indent_at(src, start);
        let gap = gap_before(src, start);
        let end = line_end(src, end);
        return Ok(format!("{}{gap}{indent}{}{}", &src[..end], content.trim_matches('\n'), &src[end..]));
    }
    if e.old.is_some() != e.new.is_some() {
        return Err(("old and new go together".into(), vec![]));
    }
    if e.old.is_some() && (e.content.is_some() || e.delete || anchors > 0) {
        return Err(("a text edit (old/new) takes no content, delete, after, before or append".into(), vec![]));
    }

    if let (Some(old), Some(new)) = (&e.old, &e.new) {
        if !exists {
            return Err(("the file does not exist; create it with content alone".into(), vec![]));
        }
        let (start, end) = match &e.entity {
            Some(name) => span(src, &find(path, src, name, e.kind.as_deref(), e.ordinal)?),
            None => (0, src.len()),
        };
        return replace_text(src, start, end, old, new, e.all, e.entity.as_deref());
    }

    if let Some(anchor) = e.after.as_ref().or(e.before.as_ref()) {
        let content = e.content.as_deref().ok_or_else(|| ("after and before need content: the entity to add".to_string(), vec![]))?;
        if !exists {
            return Err(("the file does not exist; create it with content alone".into(), vec![]));
        }
        let target = find(path, src, anchor, e.kind.as_deref(), e.ordinal)?;
        let (start, end) = span(src, &target);
        let indent = indent_at(src, start);
        let body = content.trim_matches('\n');
        let gap = gap_before(src, start);
        return Ok(if e.after.is_some() {
            let end = line_end(src, end);
            format!("{}{gap}{indent}{body}{}", &src[..end], &src[end..])
        } else {
            format!("{}{body}{gap}{indent}{}", &src[..start], &src[start..])
        });
    }

    if e.append {
        let content = e.content.as_deref().ok_or_else(|| ("append needs content".to_string(), vec![]))?;
        let body = content.trim_matches('\n');
        if src.trim().is_empty() {
            return Ok(format!("{body}\n"));
        }
        let head = src.trim_end_matches('\n');
        return Ok(format!("{head}\n\n{body}\n"));
    }

    if e.delete {
        let name = e.entity.as_ref().ok_or_else(|| ("delete needs the entity to delete".to_string(), vec![]))?;
        let target = find(path, src, name, e.kind.as_deref(), e.ordinal)?;
        let (start, end) = span(src, &target);
        let mut from = line_start(src, start);
        if src[from..start].trim().is_empty() {
            // whole lines: drop the entity's lines and one blank line around it
            let mut to = line_end(src, end);
            if to < src.len() {
                to += 1;
            }
            while src[to..].starts_with('\n') && (from == 0 || src[..from].ends_with("\n\n")) {
                to += 1;
            }
            if to >= src.len() && from > 0 {
                while from > 0 && src[..from].ends_with("\n\n") {
                    from -= 1;
                }
            }
            return Ok(format!("{}{}", &src[..from], &src[to..]));
        }
        from = start;
        return Ok(format!("{}{}", &src[..from], &src[end..]));
    }

    let content = e.content.as_deref().ok_or_else(|| ("nothing to do: give content, old/new, or delete".to_string(), vec![]))?;
    match &e.entity {
        Some(name) => {
            if !exists {
                return Err(("the file does not exist; create it with content alone".into(), vec![]));
            }
            let target = find(path, src, name, e.kind.as_deref(), e.ordinal)?;
            let (start, end) = span(src, &target);
            let body = content.strip_suffix('\n').unwrap_or(content);
            let body = if src[..start].ends_with(|c: char| c == ' ' || c == '\t') { body.trim_start() } else { body };
            let out = format!("{}{body}{}", &src[..start], &src[end..]);
            if !e.allow_signature_change {
                identity_kept(path, &target, &src[start..end], &out, start, body.len())?;
            }
            Ok(out)
        }
        None if !exists => Ok(content.to_string()),
        None => Err((
            "the file exists: name the entity to replace, use old/new, or after/before/append to add".into(),
            vec![],
        )),
    }
}

/// A replace must leave an entity of the same name, kind and visibility
/// where the old one was.
fn identity_kept(path: &str, old: &SemanticEntity, old_text: &str, out: &str, start: usize, len: usize) -> Result<(), Fail> {
    let all = entities(path, out);
    let here = all.iter().filter(|n| {
        let (s, t) = span(out, n);
        s >= start.saturating_sub(1) && s <= start + 1 && t <= start + len + 1
    }).find(|n| n.parent_id.is_some() == old.parent_id.is_some());
    let advice = "set allow_signature_change if that is intended";
    match here {
        None if !all.iter().any(|n| n.name == old.name && n.entity_type == old.entity_type) => {
            Err((format!("the new content no longer defines {} {}; {advice}", old.entity_type, old.name), vec![]))
        }
        Some(n) if n.name != old.name || n.entity_type != old.entity_type => Err((
            format!("the replace turns {} {} into {} {}; {advice}", old.entity_type, old.name, n.entity_type, n.name),
            vec![],
        )),
        Some(n) if visibility(old_text) != visibility(&n.content) => Err((
            format!("the replace changes {}'s visibility ({:?} to {:?}); {advice}", old.name, visibility(old_text), visibility(&n.content)),
            vec![],
        )),
        _ => Ok(()),
    }
}

/// The visibility words in front of a definition.
fn visibility(text: &str) -> Vec<&str> {
    const WORDS: &[&str] = &["export", "pub", "pub(crate)", "public", "private", "protected", "internal"];
    let head = text.lines().find(|l| { let t = l.trim_start(); !t.is_empty() && !t.starts_with('@') && !t.starts_with("#[") && !t.starts_with("//") && !t.starts_with('#') }).unwrap_or("");
    head.split_whitespace().take_while(|w| !w.contains('(') || w.starts_with("pub(")).filter(|w| WORDS.contains(w)).collect()
}

fn replace_text(src: &str, start: usize, end: usize, old: &str, new: &str, all: bool, entity: Option<&str>) -> Result<String, Fail> {
    if old.is_empty() {
        return Err(("old is empty".into(), vec![]));
    }
    let scope = &src[start..end];
    let n = scope.matches(old).count();
    let place = entity.map(|e| format!(" in {e}")).unwrap_or_default();
    if n == 0 {
        let hint = if scope.contains(old.trim()) { " (it matches with different surrounding whitespace)" } else { "" };
        return Err((format!("old text not found{place}{hint}"), vec![]));
    }
    if n > 1 && !all {
        return Err((format!("old text matches {n} times{place}: add context to make it unique, or set all"), vec![]));
    }
    let replaced = if all { scope.replace(old, new) } else { scope.replacen(old, new, 1) };
    Ok(format!("{}{replaced}{}", &src[..start], &src[end..]))
}

fn is_identifier(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|f| f.is_alphabetic() || f == '_' || f == '$') && c.all(|x| x.is_alphanumeric() || x == '_' || x == '$')
}

/// Files that mention `word`: (code files weave parses, other files).
fn mentions(root: &Path, word: &str, scope: &[String]) -> (Vec<String>, Vec<String>) {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C").arg(root).args(["grep", "-l", "-w", "-I", "--untracked", "-F", "-e", word, "--"]);
    for g in scope {
        cmd.arg(format!(":(glob){g}"));
    }
    let Ok(out) = cmd.output() else { return (vec![], vec![]) };
    let (mut code, mut other) = (Vec::new(), Vec::new());
    for f in String::from_utf8_lossy(&out.stdout).lines() {
        let f = f.to_string();
        if sem_core::parser::plugins::code::language_config_for_content("", &f).is_some() {
            code.push(f);
        } else {
            other.push(f);
        }
    }
    (code, other)
}

/// Rename identifier tokens spelled `from`: tree-sitter leaves whose kind is
/// an identifier, so strings, comments and longer names are never touched.
fn rename_identifiers(path: &str, src: &str, from: &str, to: &str) -> String {
    use sem_core::parser::plugins::code::{language_config_for_content, parse_tree};
    let Some(tree) = language_config_for_content(src, path).and_then(|c| parse_tree(c, src)) else {
        return src.to_string();
    };
    let mut hits = Vec::new();
    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if n.child_count() == 0 {
            let k = n.kind();
            if (k.contains("identifier") || k == "name" || k == "constant") && &src[n.byte_range()] == from {
                hits.push(n.byte_range());
            }
            continue;
        }
        stack.extend(n.children(&mut cursor));
    }
    hits.sort_by_key(|r| r.start);
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    for r in hits {
        out.push_str(&src[at..r.start]);
        out.push_str(to);
        at = r.end;
    }
    out.push_str(&src[at..]);
    out
}

/// Every entity in the file, at every depth.
fn entities(path: &str, src: &str) -> Vec<SemanticEntity> {
    REGISTRY.get_explicit_plugin(path).map(|p| p.extract_entities(src, path)).unwrap_or_default()
}

fn label(e: &SemanticEntity, all: &[SemanticEntity]) -> String {
    let parent = e.parent_id.as_ref().and_then(|id| all.iter().find(|p| &p.id == id)).map(|p| format!("{}.", p.name)).unwrap_or_default();
    format!("{} {parent}{} (line {})", e.entity_type, e.name, e.start_line)
}

fn find(path: &str, src: &str, name: &str, kind: Option<&str>, ordinal: Option<usize>) -> Result<SemanticEntity, Fail> {
    let all = entities(path, src);
    if all.is_empty() {
        let why = if REGISTRY.get_explicit_plugin(path).is_some() { "has no entities" } else { "is not a language weave parses" };
        return Err((format!("{path} {why}: edit it with old/new"), vec![]));
    }
    let parent_of = |e: &SemanticEntity| e.parent_id.as_ref().and_then(|id| all.iter().find(|p| &p.id == id)).map(|p| p.name.clone());
    let mut matches: Vec<&SemanticEntity> = all.iter().filter(|e| e.name == name).collect();
    if matches.is_empty() {
        if let Some((parent, member)) = name.rsplit_once("::").or_else(|| name.rsplit_once('.')) {
            matches = all.iter().filter(|e| e.name == member && parent_of(e).as_deref() == Some(parent)).collect();
        }
    }
    if let Some(k) = kind {
        matches.retain(|e| e.entity_type == k);
    }
    let pick = match (matches.len(), ordinal) {
        (0, _) => {
            let leaf = name.rsplit(['.', ':']).next().unwrap_or(name).to_lowercase();
            let mut near: Vec<String> = all.iter().filter(|e| e.name.to_lowercase().contains(&leaf) || leaf.contains(&e.name.to_lowercase())).map(|e| label(e, &all)).collect();
            near.truncate(10);
            return Err((format!("no entity named {name} in {path}"), near));
        }
        (_, Some(i)) => *matches.get(i).ok_or_else(|| (format!("{name}: ordinal {i} but only {} match", matches.len()), vec![]))?,
        (1, None) => matches[0],
        (n, None) => {
            return Err((
                format!("{name} matches {n} entities in {path}: pick one with kind, Parent.name or ordinal"),
                matches.iter().map(|e| label(e, &all)).collect(),
            ))
        }
    };
    Ok(pick.clone())
}

/// The entity's byte range in `src`.
fn span(src: &str, e: &SemanticEntity) -> (usize, usize) {
    if let (Some(s), Some(t)) = (e.start_byte, e.end_byte) {
        if s <= t && t <= src.len() && src.is_char_boundary(s) && src.is_char_boundary(t) {
            return (s, t);
        }
    }
    let starts: Vec<usize> = std::iter::once(0).chain(src.match_indices('\n').map(|(i, _)| i + 1)).collect();
    let s = starts.get(e.start_line.saturating_sub(1)).copied().unwrap_or(src.len());
    let s = s + (src[s..].len() - src[s..].trim_start_matches([' ', '\t']).len());
    let t = starts.get(e.end_line).map(|i| i - 1).unwrap_or(src.len());
    (s, t.max(s))
}

fn line_start(src: &str, at: usize) -> usize {
    src[..at].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn line_end(src: &str, at: usize) -> usize {
    src[at..].find('\n').map(|i| at + i).unwrap_or(src.len())
}

fn indent_at(src: &str, at: usize) -> String {
    let ls = line_start(src, at);
    let lead = &src[ls..at];
    if lead.trim().is_empty() { lead.to_string() } else { String::new() }
}

/// The line breaks to put between two neighbouring entities: as many blank
/// lines as already sit in front of `start` (one or two), plus the newline
/// that ends the previous line.
fn gap_before(src: &str, start: usize) -> String {
    let before = &src[..line_start(src, start)];
    let newlines = before.len() - before.trim_end_matches('\n').len();
    "\n".repeat(newlines.clamp(2, 3))
}

fn report(path: &str, before: Option<&str>, after: &str) -> FileReport {
    let key = |e: &SemanticEntity, all: &[SemanticEntity]| {
        let parent = e.parent_id.as_ref().and_then(|id| all.iter().find(|p| &p.id == id)).map(|p| format!("{}.", p.name)).unwrap_or_default();
        format!("{parent}{}", e.name)
    };
    let index = |src: &str| {
        let all = entities(path, src);
        let mut m: BTreeMap<String, String> = BTreeMap::new();
        for e in &all {
            if !e.name.is_empty() {
                m.insert(key(e, &all), e.content.clone());
            }
        }
        m
    };
    let a = before.map(index).unwrap_or_default();
    let b = index(after);
    let mut r = FileReport { file: path.to_string(), created: before.is_none(), added: vec![], modified: vec![], deleted: vec![] };
    for (k, v) in &b {
        match a.get(k) {
            None => r.added.push(k.clone()),
            Some(old) if old != v => r.modified.push(k.clone()),
            _ => {}
        }
    }
    r.deleted = a.keys().filter(|k| !b.contains_key(*k)).cloned().collect();
    // a changed member changes its class's text too: name the member only
    let touched: Vec<String> = r.added.iter().chain(&r.modified).chain(&r.deleted).cloned().collect();
    r.modified.retain(|k| !touched.iter().any(|t| t.starts_with(&format!("{k}."))));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    const PY: &str = "import os\n\n\ndef a():\n    return 1\n\n\nclass K:\n    def m(self):\n        return 2\n\n    def n(self):\n        return 3\n\n\ndef b():\n    return a()\n";

    fn scratch() -> PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!("weave-edit-test-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn edit(f: impl FnOnce(&mut Edit)) -> Edit {
        let mut e = Edit { file: "x.py".into(), ..Default::default() };
        f(&mut e);
        e
    }

    fn go(src: &str, edits: Vec<Edit>) -> Result<(String, Report), EditError> {
        let d = scratch();
        if !src.is_empty() {
            std::fs::write(d.join("x.py"), src).unwrap();
        }
        let r = run(&d, &edits, false)?;
        Ok((std::fs::read_to_string(d.join("x.py")).unwrap_or_default(), r))
    }

    #[test]
    fn replaces_entities_and_members_by_name() {
        let (out, r) = go(PY, vec![
            edit(|e| { e.entity = Some(EntityRef::from("a")); e.content = Some("def a():\n    return 10\n".into()); }),
            edit(|e| { e.entity = Some(EntityRef::from("K.n")); e.content = Some("def n(self):\n        return 30".into()); }),
        ]).unwrap();
        assert_eq!(out, PY.replace("return 1\n", "return 10\n").replace("return 3", "return 30"));
        assert_eq!(r.files[0].modified, vec!["K.n", "a"]);
    }

    #[test]
    fn text_edits_are_scoped_and_must_be_unique() {
        let (out, _) = go(PY, vec![edit(|e| { e.entity = Some(EntityRef::from("m")); e.old = Some("return".into()); e.new = Some("yield".into()); })]).unwrap();
        assert!(out.contains("yield 2") && out.contains("return 3"));
        let err = go(PY, vec![edit(|e| { e.old = Some("return".into()); e.new = Some("x".into()); })]).unwrap_err();
        assert!(err.message.contains("4 times"), "{}", err.message);
    }

    #[test]
    fn adds_deletes_and_creates() {
        let (out, r) = go(PY, vec![
            edit(|e| { e.after = Some("a".into()); e.content = Some("def c():\n    return 0".into()); }),
            edit(|e| { e.after = Some("m".into()); e.content = Some("def p(self):\n        return 5".into()); }),
            edit(|e| { e.entity = Some(EntityRef::from("b")); e.delete = true; }),
        ]).unwrap();
        assert!(out.contains("    return 1\n\n\ndef c():\n    return 0\n\n\nclass K:"), "{out}");
        assert!(out.contains("        return 2\n\n    def p(self):\n        return 5\n\n    def n"), "{out}");
        assert!(out.ends_with("        return 3\n"), "{out:?}");
        assert_eq!(r.files[0].added, vec!["K.p", "c"]);
        assert_eq!(r.files[0].deleted, vec!["b"]);
        let (out, r) = go("", vec![edit(|e| e.content = Some("x = 1\n".into()))]).unwrap();
        assert_eq!(out, "x = 1\n");
        assert!(r.files[0].created);
    }

    #[test]
    fn a_failing_edit_writes_nothing() {
        let d = scratch();
        std::fs::write(d.join("x.py"), PY).unwrap();
        let edits = vec![
            edit(|e| { e.entity = Some(EntityRef::from("a")); e.content = Some("def a():\n    pass".into()); }),
            edit(|e| { e.entity = Some(EntityRef::from("nope")); e.delete = true; }),
        ];
        let err = run(&d, &edits, false).unwrap_err();
        assert_eq!(err.index, 1);
        assert_eq!(std::fs::read_to_string(d.join("x.py")).unwrap(), PY);
        assert!(normalize(&d, "../etc/passwd").is_err());
    }

    #[test]
    fn guards_refuse_broken_or_renamed_entities() {
        let pi: Vec<Edit> = serde_json::from_str(r#"[{"file":"x.py","op":"insert_after","entity":{"name":"m","parent_name":"K"},"content":"def q(self):\n        return 9"}]"#).unwrap();
        let (out, _) = go(PY, pi).unwrap();
        assert!(out.contains("    def q(self):\n        return 9\n"), "{out}");
        let broken = go(PY, vec![edit(|e| { e.entity = Some(EntityRef::from("a")); e.content = Some("def a(:\n    return 1".into()); })]).unwrap_err();
        assert!(broken.message.contains("unparseable"), "{}", broken.message);
        let renamed = go(PY, vec![edit(|e| { e.entity = Some(EntityRef::from("a")); e.content = Some("def z():\n    return 1".into()); })]).unwrap_err();
        assert!(renamed.message.contains("allow_signature_change"), "{}", renamed.message);
        let (out, _) = go(PY, vec![edit(|e| { e.entity = Some(EntityRef::from("a")); e.content = Some("def z():\n    return 1".into()); e.allow_signature_change = true; })]).unwrap();
        assert!(out.contains("def z():"));
    }

    #[test]
    fn rename_touches_identifiers_not_strings() {
        let d = scratch();
        let git = |a: &[&str]| std::process::Command::new("git").arg("-C").arg(&d).args(a).output().unwrap();
        git(&["init", "-q"]);
        std::fs::write(d.join("x.py"), PY).unwrap();
        std::fs::write(d.join("y.py"), "from x import a\n\nprint(a(), \"a\", aa)  # a\n").unwrap();
        std::fs::write(d.join("notes.md"), "call a() first\n").unwrap();
        git(&["add", "."]);
        let e: Vec<Edit> = serde_json::from_str(r#"[{"file":"x.py","entity":"a","rename":"first"}]"#).unwrap();
        let r = run(&d, &e, false).unwrap();
        assert!(std::fs::read_to_string(d.join("x.py")).unwrap().contains("def first():") && std::fs::read_to_string(d.join("x.py")).unwrap().contains("return first()"));
        assert_eq!(std::fs::read_to_string(d.join("y.py")).unwrap(), "from x import first\n\nprint(first(), \"a\", aa)  # a\n");
        assert!(r.notes[0].contains("notes.md"), "{:?}", r.notes);
    }

    #[test]
    fn into_adds_a_last_member() {
        let (out, r) = go(PY, vec![edit(|e| { e.into = Some("K".into()); e.content = Some("def z(self):\n        return 0".into()); })]).unwrap();
        assert!(out.contains("        return 3\n\n    def z(self):\n        return 0\n\n\ndef b"), "{out}");
        assert_eq!(r.files[0].added, vec!["K.z"]);
    }
}
