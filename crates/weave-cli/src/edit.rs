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
    /// The entity's version as sem showed it (`v:…`): the edit is refused if
    /// the entity changed since, so two agents never overwrite each other's
    /// version of one function.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<String>,
    /// For `rename`: also rename the name where it appears as a whole word in
    /// comments and in docs and other text files (Markdown, reStructuredText,
    /// plain text). String literals are still left alone.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub everywhere: bool,
    /// For `rename`: the files to rename in, as git pathspec globs
    /// (`src/**`, `*.py`). Default: the whole repository.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// Put `entity` back the way it was at this git ref (`HEAD~3`, a branch,
    /// a commit): its text at that ref replaces its text now, and nothing else
    /// in the file moves. A revert is a replace whose content comes from
    /// history, so every replace guard still applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revert: Option<String>,
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
    /// For a rename: also comments, and whole words in a text file.
    everywhere: bool,
    /// For a rename: the entity is a member (a method or field), so `x.name`
    /// is a use of it rather than of something else spelled the same.
    member: bool,
    /// For a rename: the file that defines the renamed entity.
    home: Option<String>,
    expect: Option<String>,
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
            everywhere: false,
            member: false,
            home: None,
            expect: self.expect.clone(),
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
    /// Entities whose signature changed or that were deleted, with every
    /// place their name is still used: the call sites to update in the next
    /// edit rather than discover from a failing test.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub review: Vec<String>,
    /// The output of the `then` command, run once the edits were written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
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
    // Agents editing one checkout at once: each batch reads, plans and
    // writes under one lock per repository, so a batch always applies to
    // the files as the previous batch left them and no edit is lost.
    let _lock = if dry_run { None } else { lock(root) };
    let mut by_file: BTreeMap<String, Vec<(usize, Plain)>> = BTreeMap::new();
    let mut notes = Vec::new();
    let mut renamed: Vec<String> = Vec::new();
    for (index, e) in edits.iter().enumerate() {
        let fail = |message: String| EditError { index, file: e.file.clone(), message, candidates: vec![] };
        let path = normalize(root, &e.file).map_err(fail)?;
        let mut plain = e.plain().map_err(fail)?;
        if let Some(rev) = &e.revert {
            plain.content = Some(reverted(root, &path, rev, &plain).map_err(|(m, c)| EditError { index, file: e.file.clone(), message: m, candidates: c })?);
        }
        if let Some(to) = &e.rename {
            let entity = plain.entity.clone().ok_or_else(|| fail("rename needs the entity to rename".into()))?;
            let from = entity.rsplit(['.', ':']).next().unwrap_or(&entity).to_string();
            if !is_identifier(to) {
                return Err(fail(format!("{to:?} is not an identifier")));
            }
            renamed.push(from.clone());
            let home_src = std::fs::read_to_string(root.join(&path)).unwrap_or_default();
            let target = find(&path, &home_src, &entity, plain.kind.as_deref(), plain.ordinal).map_err(|(m, c)| EditError { index, file: e.file.clone(), message: m, candidates: c })?;
            let member = target.parent_id.is_some() || target.entity_type == "method";
            let (code, other) = mentions(root, &from, &e.scope);
            let mut files: Vec<String> = code;
            if !files.contains(&path) {
                files.insert(0, path.clone());
            }
            for f in files {
                let p = Plain { rename_ident: Some((from.clone(), to.clone())), entity: (f == path).then(|| entity.clone()), kind: plain.kind.clone(), home: Some(path.clone()), everywhere: e.everywhere, member, ..Default::default() };
                by_file.entry(f).or_default().push((index, p));
            }
            let other: Vec<String> = if e.everywhere {
                let (text, rest): (Vec<String>, Vec<String>) = other.into_iter().partition(|f| is_text_doc(f));
                for f in text {
                    let p = Plain { rename_ident: Some((from.clone(), to.clone())), everywhere: true, ..Default::default() };
                    by_file.entry(f).or_default().push((index, p));
                }
                rest
            } else {
                other
            };
            if member {
                let twins: Vec<String> = code_twins(root, &from, &path, target.start_line);
                if !twins.is_empty() {
                    notes.push(format!("{from} is also defined at {}: calls like x.{from}() were renamed whatever x is, so check the ones on those types", twins.join(", ")));
                }
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
    // what a rename left behind, so the agent need not grep to find out:
    // the old name in strings, comments and docs, or another entity's name
    for from in &renamed {
        let word = |line: &str| line.match_indices(from.as_str()).any(|(i, _)| {
            let before = line[..i].chars().next_back();
            let after = line[i + from.len()..].chars().next();
            !before.is_some_and(|c| c.is_alphanumeric() || c == '_') && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
        });
        let mut left: Vec<String> = Vec::new();
        for (path, _, now) in &files {
            for (i, line) in now.lines().enumerate() {
                if word(line) {
                    left.push(format!("{path}:{}: {}", i + 1, line.trim()));
                }
            }
        }
        if !left.is_empty() {
            let more = left.len().saturating_sub(12);
            left.truncate(12);
            let tail = if more > 0 { format!("\n  ... and {more} more") } else { String::new() };
            notes.push(format!("{from} is still spelled out here, in strings, comments, docs or as a different entity's name (left as they are):\n  {}{tail}", left.join("\n  ")));
        }
    }
    if !dry_run {
        for (path, _, now) in &files {
            let abs = root.join(path);
            if let Some(dir) = abs.parent() {
                std::fs::create_dir_all(dir).map_err(|e| io_error(path, e))?;
            }
            std::fs::write(&abs, now).map_err(|e| io_error(path, e))?;
        }
    }
    let review = if dry_run { Vec::new() } else { review(root, &files, &renamed) };
    let written = !dry_run && !reports.is_empty();
    Ok(Report { written, files: reports, notes, review, then: None })
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

/// The sem binary: `SEM_BIN`, else `sem` on PATH.
fn sem_bin() -> String {
    std::env::var("SEM_BIN").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "sem".to_string())
}

/// `<git common dir>/sem-run/last`: the build or test command the agent ran
/// last through `sem check --run`, and the directory it ran it in.
fn last_command(root: &Path) -> Option<(String, String)> {
    let o = std::process::Command::new("git").args(["rev-parse", "--path-format=absolute", "--git-common-dir"]).current_dir(root).output().ok()?;
    let dir = std::path::PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()).join("sem-run");
    let text = std::fs::read_to_string(dir.join("last")).ok()?;
    let (rel, cmd) = text.split_once('\n')?;
    (!cmd.trim().is_empty()).then(|| (rel.to_string(), cmd.to_string()))
}

/// Start the agent's last test command on the tree just written, in the
/// background. Agents follow an edit by rerunning the same command after
/// thinking for some seconds; by then this run is done or under way, and
/// `sem check --run` joins it instead of starting over.
pub fn speculate(root: &Path) {
    if std::env::var("WEAVE_SPECULATE").is_ok_and(|v| v == "0") {
        return;
    }
    let Some((rel, cmd)) = last_command(root) else { return };
    let cwd = root.join(rel);
    let quoted = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let line = format!("{} check --run {} --quiet >/dev/null 2>&1 &", quoted(&sem_bin()), quoted(&cmd));
    let _ = std::process::Command::new("sh").arg("-c").arg(line).current_dir(cwd).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
}

/// Run `cmd` through `sem check --run` once the edits are written, so an edit
/// and its check take one call; it joins the speculative run if there is one.
pub fn then(root: &Path, cmd: &str) -> String {
    match std::process::Command::new(sem_bin()).args(["check", "--run", cmd]).current_dir(root).stdin(std::process::Stdio::null()).output() {
        Ok(o) => format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
        Err(e) => format!("could not run sem check --run: {e} (set SEM_BIN)"),
    }
}

fn lock(root: &Path) -> Option<std::fs::File> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut h: u64 = 0xcbf29ce484222325;
    for b in root.to_string_lossy().bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(std::env::temp_dir().join(format!("weave-edit-{h:016x}.lock"))).ok()?;
    f.lock().ok()?;
    Some(f)
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
        // in the defining file, the target's own definition is renamed and
        // every other entity spelled the same is left alone
        let target = match &e.entity {
            Some(name) => Some(find(path, src, name, e.kind.as_deref(), None)?),
            None => None,
        };
        if e.home.is_none() && is_text_doc(path) {
            return Ok(replace_words(src, from, to));
        }
        let home = e.home.as_deref().unwrap_or(path);
        let module = home.rsplit('/').next().unwrap_or(home).split('.').next().unwrap_or("");
        return Ok(rename_identifiers(path, src, from, to, target.as_ref(), module, e.everywhere, e.member));
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
        let body = fit(content, &indent);
        let body = if uses_tabs(src) { tabify(&body) } else { body };
        let out = format!("{}{gap}{indent}{}{}", &src[..end], body, &src[end..]);
        placed_under(path, src, &out, Some(&parent.name))?;
        return Ok(out);
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
            Some(name) => {
                let (s, t) = span(src, &find(path, src, name, e.kind.as_deref(), e.ordinal)?);
                check_version(name, &src[s..t], e.expect.as_deref())?;
                (s, t)
            }
            None => (0, src.len()),
        };
        let new = if uses_tabs(src) && new.lines().any(|l| l.starts_with("    ")) && !old.lines().any(|l| l.starts_with("    ")) { tabify(new) } else { new.clone() };
        return replace_text(src, start, end, old, &new, e.all, e.entity.as_deref());
    }

    if let Some(anchor) = e.after.as_ref().or(e.before.as_ref()) {
        let content = e.content.as_deref().ok_or_else(|| ("after and before need content: the entity to add".to_string(), vec![]))?;
        if !exists {
            return Err(("the file does not exist; create it with content alone".into(), vec![]));
        }
        let target = find(path, src, anchor, e.kind.as_deref(), e.ordinal)?;
        let (start, end) = span(src, &target);
        let indent = indent_at(src, start);
        let body = fit(content, &indent);
        let body = if uses_tabs(src) { tabify(&body) } else { body };
        let gap = gap_before(src, start);
        let out = if e.after.is_some() {
            let end = line_end(src, end);
            format!("{}{gap}{indent}{body}{}", &src[..end], &src[end..])
        } else {
            // before the anchor's doc comments and attributes, not between
            // them and the definition they document
            let lead = leading_start(src, start);
            let at = if lead < start { lead + (src[lead..].len() - src[lead..].trim_start_matches([' ', '\t']).len()) } else { start };
            format!("{}{body}{gap}{indent}{}", &src[..at], &src[at..])
        };
        let all = entities(path, src);
        let parent = target.parent_id.as_ref().and_then(|id| all.iter().find(|p| &p.id == id)).map(|p| p.name.clone());
        placed_under(path, src, &out, parent.as_deref())?;
        return Ok(out);
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
        check_version(name, &src[start..end], e.expect.as_deref())?;
        let lead = leading_start(src, start);
        let mut from = line_start(src, lead);
        if src[from..lead].trim().is_empty() {
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
            check_version(name, &src[start..end], e.expect.as_deref())?;
            let body = if src[..start].ends_with([' ', '\t']) { fit(content, &indent_at(src, start)) } else { content.trim_matches('\n').to_string() };
            let body = if uses_tabs(src) { tabify(&body) } else { body };
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

/// `content` as text to place where `indent` already stands: the first line
/// without its indentation, the rest at the depth the file needs. Accepts the
/// three ways agents write a nested entity: every line at its final depth,
/// only the body at its final depth (sem's form), or every line relative to
/// the entity.
/// Does this file indent with tabs?
fn uses_tabs(src: &str) -> bool {
    let (tabs, spaces) = src.lines().fold((0, 0), |(t, sp), l| (t + l.starts_with('\t') as usize, sp + l.starts_with("  ") as usize));
    tabs > spaces
}

/// Leading runs of spaces as tabs, four to a tab, for a file indented with tabs.
fn tabify(text: &str) -> String {
    text.lines()
        .map(|l| {
            let lead = l.len() - l.trim_start_matches(' ').len();
            if lead == 0 { l.to_string() } else { format!("{}{}{}", "\t".repeat(lead / 4), " ".repeat(lead % 4), &l[lead..]) }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fit(content: &str, indent: &str) -> String {
    let content = content.trim_matches('\n');
    let lines: Vec<&str> = content.lines().collect();
    let width = |l: &str| l.len() - l.trim_start_matches([' ', '\t']).len();
    let Some(first) = lines.iter().position(|l| !l.trim().is_empty()) else { return String::new() };
    let base = width(lines[first]);
    let target = indent.len();
    let unit = if indent.contains('\t') { "\t" } else { " " };
    // Which form, when the first line is not indented: the last line tells.
    // A closing line (`}`, `)`, `end`) stands at the entity's own depth, so
    // at the target it is the final form and at zero the relative one; a
    // body line is in its final form only when deeper than the target.
    let last = lines.iter().rev().find(|l| !l.trim().is_empty()).copied().unwrap_or("");
    let closing = matches!(last.trim(), "}" | "};" | "})" | ")" | "]" | "end" | "});" | "},");
    let relative = base == 0
        && target > 0
        && lines.len() > first + 1
        && if closing { width(last) < target } else { width(last) <= target };
    // in the final form, lines written at column zero after the first are the
    // entity's own header (a signature after its doc comment): bring them to
    // the target depth
    let final_form = base == 0 && !relative;
    let mut out = Vec::with_capacity(lines.len());
    for (i, l) in lines.iter().enumerate() {
        if i == first {
            out.push(l.trim_start().to_string());
        } else if l.trim().is_empty() {
            out.push(String::new());
        } else if final_form && width(l) == 0 {
            out.push(format!("{indent}{}", l.trim_start()));
        } else {
            let w = width(l) as isize;
            // relative: shift by the depth; every line at its final depth:
            // re-base on the target; sem's form (base 0, body final): as is
            let shift = if relative { target as isize } else if base > 0 { target as isize - base as isize } else { 0 };
            let n = (w + shift).max(0) as usize;
            out.push(format!("{}{}", unit.repeat(n), l.trim_start()));
        }
    }
    out.join("\n")
}

/// An added entity must land where it was meant to: a sibling of its anchor,
/// or a member of the class it was added into.
fn placed_under(path: &str, before: &str, after: &str, parent: Option<&str>) -> Result<(), Fail> {
    let keys = |src: &str| {
        let all = entities(path, src);
        all.iter()
            .filter(|e| !e.name.is_empty())
            .map(|e| {
                let p = e.parent_id.as_ref().and_then(|id| all.iter().find(|x| &x.id == id)).map(|x| x.name.clone());
                (p, e.name.clone())
            })
            .collect::<Vec<_>>()
    };
    let old = keys(before);
    let mut new = keys(after);
    for k in &old {
        if let Some(i) = new.iter().position(|n| n == k) {
            new.remove(i);
        }
    }
    let added_names: Vec<&String> = new.iter().map(|(_, n)| n).collect();
    // the outermost new entities: those whose parent is not itself new
    for (p, name) in &new {
        if p.as_ref().is_some_and(|p| added_names.contains(&p)) {
            continue;
        }
        if p.as_deref() != parent {
            let wanted = parent.map(|p| format!("in {p}")).unwrap_or_else(|| "at the top level".into());
            let got = p.as_ref().map(|p| format!("inside {p}")).unwrap_or_else(|| "at the top level".into());
            return Err((format!("{name} would land {got}, not {wanted}: check the content's indentation"), vec![]));
        }
    }
    Ok(())
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

/// sem's entity version: `v:` + FNV-1a (32 bits) of the source without
/// trailing newlines.
pub fn version(source: &str) -> String {
    let mut h: u32 = 0x811c9dc5;
    for b in source.trim_end_matches('\n').bytes() {
        h = (h ^ b as u32).wrapping_mul(0x01000193);
    }
    format!("v:{h:08x}")
}

fn check_version(name: &str, source: &str, expect: Option<&str>) -> Result<(), Fail> {
    let Some(want) = expect.map(str::trim).filter(|w| !w.is_empty()) else { return Ok(()) };
    let want = if want.starts_with("v:") { want.to_string() } else { format!("v:{want}") };
    let now = version(source);
    if now == want {
        return Ok(());
    }
    Err((format!("{name} changed since you read it ({want}, now {now}): read it again and redo the edit on the current version"), vec![]))
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

/// Other definitions spelled `name` in the repository's code (not the one at
/// `home:line`), as `file:line`.
fn code_twins(root: &Path, name: &str, home: &str, line: usize) -> Vec<String> {
    let (code, _) = mentions(root, name, &[]);
    let mut out = Vec::new();
    for f in code {
        let Ok(src) = std::fs::read_to_string(root.join(&f)) else { continue };
        for e in entities(&f, &src) {
            if e.name == name && !(f == home && e.start_line == line) {
                out.push(format!("{f}:{}", e.start_line));
            }
        }
    }
    out.truncate(8);
    out
}

/// Docs and other prose files a rename with `everywhere` also updates.
fn is_text_doc(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "md" | "mdx" | "markdown" | "rst" | "txt" | "adoc")
}

/// Byte offsets of `word` in `text` where it stands as a whole word.
fn word_hits<'a>(text: &'a str, word: &'a str) -> impl Iterator<Item = (usize, &'a str)> + 'a {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).filter(move |(i, _)| {
        !text[..*i].chars().next_back().is_some_and(is_word) && !text[i + word.len()..].chars().next().is_some_and(is_word)
    })
}

fn replace_words(text: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (i, _) in word_hits(text, from).collect::<Vec<_>>() {
        out.push_str(&text[at..i]);
        out.push_str(to);
        at = i + from.len();
    }
    out.push_str(&text[at..]);
    out
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
/// Two kinds of same-spelled identifiers are someone else's and are skipped:
/// a member access (`obj.name`, unless `obj` is the defining module, as in
/// `cells.cell_len`), and the name of another definition (a method of the
/// same name in some class). `target` is the renamed entity when `src` is
/// the file that defines it.
#[allow(clippy::too_many_arguments)]
fn rename_identifiers(path: &str, src: &str, from: &str, to: &str, target: Option<&SemanticEntity>, module: &str, comments: bool, member: bool) -> String {
    use sem_core::parser::plugins::code::{language_config_for_content, parse_tree};
    let Some(tree) = language_config_for_content(src, path).and_then(|c| parse_tree(c, src)) else {
        return src.to_string();
    };
    // other definitions spelled `from`: their name tokens are not renamed
    let others: Vec<(usize, usize)> = entities(path, src)
        .iter()
        .filter(|e| e.name == from && target.is_none_or(|t| t.start_line != e.start_line))
        .map(|e| span(src, e))
        .collect();
    let target_span = target.map(|t| span(src, t));
    const MEMBER: &[&str] = &["attribute", "member_expression", "field_expression", "selector_expression", "navigation_expression", "member_access_expression", "field_access", "property_access_expression"];
    let mut hits = Vec::new();
    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if n.child_count() > 0 {
            stack.extend(n.children(&mut cursor));
            continue;
        }
        let k = n.kind();
        if comments && k.contains("comment") {
            for (i, _) in word_hits(&src[n.byte_range()], from) {
                let at = n.start_byte() + i;
                hits.push(at..at + from.len());
            }
            continue;
        }
        if !(k.contains("identifier") || k == "name" || k == "constant") || &src[n.byte_range()] != from {
            continue;
        }
        if let Some(p) = n.parent() {
            // obj.name: someone else's member unless obj is the module, or the
            // renamed entity is itself a member (then obj.name is its call)
            if !member && MEMBER.contains(&p.kind()) && p.child(0).is_some_and(|o| o.id() != n.id()) {
                let object = p.child(0).map(|o| &src[o.byte_range()]).unwrap_or("");
                if object != module {
                    continue;
                }
            }
        }
        // the name token of another definition spelled the same
        let at = n.start_byte();
        let defines_other = others.iter().any(|&(s, t)| at >= s && at <= t && src[s..at].lines().count() <= 3 && !target_span.is_some_and(|(ts, tt)| at >= ts && at <= tt && ts >= s && tt <= t));
        if defines_other && is_definition_name(&n) {
            continue;
        }
        hits.push(n.byte_range());
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


/// Is this identifier the name a definition introduces (`def name`, `fn
/// name`, `class name`, a method or property name)?
fn is_definition_name(n: &tree_sitter::Node) -> bool {
    let Some(p) = n.parent() else { return false };
    let k = p.kind();
    let definition = k.contains("definition") || k.contains("declaration") || k.contains("function_item") || k.contains("method") || k.ends_with("_item") || k == "property_signature" || k == "pair";
    definition && p.child_by_field_name("name").is_some_and(|c| c.id() == n.id())
}

/// Every entity in the file, at every depth.
fn entities(path: &str, src: &str) -> Vec<SemanticEntity> {
    REGISTRY.get_explicit_plugin(path).map(|p| p.extract_entities(src, path)).unwrap_or_default()
}

fn label(e: &SemanticEntity, all: &[SemanticEntity]) -> String {
    let parent = e.parent_id.as_ref().and_then(|id| all.iter().find(|p| &p.id == id)).map(|p| format!("{}.", p.name)).unwrap_or_default();
    format!("{} {parent}{} (line {})", e.entity_type, e.name, e.start_line)
}

/// The text `plain`'s entity had at git ref `rev`: what a revert writes back.
///
/// History is read, never trusted to still line up with the file: the entity
/// is found by name in the file as it was at `rev`, so a function that moved
/// within the file since is still reverted to the right text. The result goes
/// through the ordinary replace path, so the identity guard (same name, kind
/// and visibility unless `allow_signature_change`), `expect`, the parse guard
/// and the review of stranded callers all apply to a revert as they would to
/// a hand-written replace.
fn reverted(root: &Path, path: &str, rev: &str, plain: &Plain) -> Result<String, Fail> {
    let name = plain.entity.as_deref().ok_or_else(|| ("revert needs the entity to put back".to_string(), vec![]))?;
    if plain.content.is_some() || plain.old.is_some() || plain.delete || plain.after.is_some() || plain.before.is_some() || plain.into.is_some() || plain.append {
        return Err(("revert replaces the entity with its text at that ref: drop content, old/new, delete and the insert options".to_string(), vec![]));
    }
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["show", &format!("{rev}:./{path}")])
        .output()
        .map_err(|e| (format!("could not run git to read {path} at {rev}: {e}"), vec![]))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err((format!("{path} does not exist at {rev}: {}", why.trim()), vec![]));
    }
    let then = String::from_utf8_lossy(&out.stdout);
    find(path, &then, name, plain.kind.as_deref(), plain.ordinal)
        .map(|e| e.content)
        .map_err(|(m, c)| (format!("{name} did not exist in {path} at {rev} ({m})"), c))
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

/// Where an entity's leading doc comments, attributes and decorators begin:
/// the lines directly above `start` (no blank line between) that belong to
/// it. Inserting before an entity, or deleting it, must not split them off.
fn leading_start(src: &str, start: usize) -> usize {
    let mut at = line_start(src, start);
    if !src[at..start].trim().is_empty() {
        return start;
    }
    while at > 0 {
        let prev = line_start(src, at - 1);
        let t = src[prev..at].trim();
        let owned = t.starts_with("///") || t.starts_with("//!") || t.starts_with("#[") || t.starts_with("#![") || t.starts_with('@')
            || t.starts_with("/**") || t.starts_with("* ") || t == "*" || t.starts_with("*/") || t.starts_with("//") || t.starts_with("# ") && false;
        if t.is_empty() || !owned {
            break;
        }
        at = prev;
    }
    at
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

/// Review an edit another tool made (an agent's own edit or patch tool):
/// for each `(file, content before the edit)` (`None` before: the file is new
/// or its content at HEAD stands in), what the edit broke in the parse and
/// which callers a signature change or deletion left behind. Then start the
/// agent's last test command on the new tree, as after a weave edit.
pub fn review_external(root: &Path, files: &[(String, Option<String>)]) -> Vec<String> {
    let mut planned = Vec::new();
    let mut out = Vec::new();
    for (file, before) in files {
        let Ok(path) = normalize(root, file) else { continue };
        let Ok(after) = std::fs::read_to_string(root.join(&path)) else { continue };
        let before = before.clone().or_else(|| {
            std::process::Command::new("git").args(["show", &format!("HEAD:{path}")]).current_dir(root).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        });
        if let Some(why) = parses_worse(&path, before.as_deref().unwrap_or(""), &after) {
            out.push(why.replace("; nothing was written", ""));
        }
        planned.push((path, before, after));
    }
    out.extend(review(root, &planned, &[]));
    speculate(root);
    out
}

/// A definition's signature: its text up to where the body starts, with
/// decorators and attributes dropped and whitespace collapsed.
fn signature(content: &str) -> String {
    let lines: Vec<&str> = content.lines().skip_while(|l| { let t = l.trim_start(); t.starts_with('@') || t.starts_with("#[") }).collect();
    let text = lines.join("\n");
    let cut = ["{", ":\n", "=>", " where "].iter().filter_map(|m| text.find(m)).min().unwrap_or(text.len().min(400));
    text[..cut].split_whitespace().collect::<Vec<_>>().join(" ")
}

/// For each entity the batch deleted or gave a new signature: the places its
/// name is still used, from `git grep`, after the batch was written.
fn review(root: &Path, files: &[(String, Option<String>, String)], renamed: &[String]) -> Vec<String> {
    let mut changed: Vec<(String, String, &'static str)> = Vec::new(); // (name, file, why)
    for (path, disk, now) in files {
        let Some(disk) = disk else { continue };
        let before = entities(path, disk);
        let after = entities(path, now);
        for b in &before {
            if b.name.len() < 3 || renamed.contains(&b.name) || !matches!(b.entity_type.as_str(), "function" | "method" | "class" | "struct" | "interface" | "trait" | "type" | "enum" | "constructor") {
                continue;
            }
            match after.iter().find(|a| a.name == b.name && a.entity_type == b.entity_type) {
                None => changed.push((b.name.clone(), path.clone(), "was deleted")),
                Some(a) if signature(&a.content) != signature(&b.content) => changed.push((b.name.clone(), path.clone(), "changed its signature")),
                _ => {}
            }
        }
    }
    let edited: Vec<&str> = files.iter().map(|(p, _, _)| p.as_str()).collect();
    let mut out = Vec::new();
    for (name, home, why) in changed {
        let Ok(o) = std::process::Command::new("git").arg("-C").arg(root).args(["grep", "-n", "-w", "-I", "--untracked", "-F", "-e", &name]).output() else { continue };
        let sites: Vec<String> = String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| sem_core::parser::plugins::code::language_config_for_content("", l.split(':').next().unwrap_or("")).is_some())
            .filter(|l| {
                let text = l.splitn(3, ':').nth(2).unwrap_or("").trim_start();
                // the definition itself, and mentions in comments, are not uses
                let defines = [format!("def {name}"), format!("fn {name}"), format!("function {name}"), format!("class {name}"), format!("func {name}("), format!(") {name}(")]
                    .iter()
                    .any(|d| text.contains(d.as_str()) && (text.starts_with("def ") || text.starts_with("fn ") || text.starts_with("pub ") || text.starts_with("func ") || text.starts_with("function ") || text.starts_with("class ") || text.starts_with("export ") || text.starts_with("async ")));
                let comment = ["//", "#", "*", "/*", "--"].iter().any(|c| text.starts_with(c));
                !defines && !comment
            })
            .map(|l| {
                let mut parts = l.splitn(3, ':');
                let (f, n, t) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or("").trim());
                let mark = if edited.contains(&f) { " (file edited in this batch)" } else { "" };
                format!("{f}:{n}: {t}{mark}")
            })
            .collect();
        if sites.is_empty() {
            continue;
        }
        let more = sites.len().saturating_sub(20);
        let tail = if more > 0 { format!("\n  ... and {more} more") } else { String::new() };
        out.push(format!("{name} ({home}) {why}; still used at:\n  {}{tail}", sites.into_iter().take(20).collect::<Vec<_>>().join("\n  ")));
    }
    out
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

    /// A revert puts one entity back as it was at a ref and leaves every
    /// other entity — including ones edited since — as it is now.
    #[test]
    fn revert_restores_one_entity_from_history() {
        let d = scratch();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git").arg("-C").arg(&d).args(args).output().unwrap().status.success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "t"]);
        std::fs::write(d.join("x.py"), PY).unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        let now = PY.replace("return 1\n", "return 10\n").replace("return a()", "return a() + 1");
        std::fs::write(d.join("x.py"), &now).unwrap();
        let r = run(&d, &[edit(|e| { e.entity = Some(EntityRef::from("a")); e.revert = Some("HEAD".into()); })], false).unwrap();
        let out = std::fs::read_to_string(d.join("x.py")).unwrap();
        assert_eq!(out, PY.replace("return a()", "return a() + 1"), "only `a` goes back");
        assert_eq!(r.files[0].modified, vec!["a"]);
        let err = run(&d, &[edit(|e| { e.entity = Some(EntityRef::from("nope")); e.revert = Some("HEAD".into()); })], false).unwrap_err();
        assert!(err.message.contains("did not exist in x.py at HEAD"), "{}", err.message);
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

    #[test]
    fn inserted_content_is_reindented_whichever_way_it_is_written() {
        for content in [
            "    def z(self):\n        return 0",
            "def z(self):\n        return 0",
            "def z(self):\n    return 0",
        ] {
            let (out, _) = go(PY, vec![edit(|e| { e.after = Some("K.m".into()); e.content = Some(content.into()); })]).unwrap();
            assert!(out.contains("        return 2\n\n    def z(self):\n        return 0\n\n    def n"), "{content:?}\n{out}");
        }
    }

    #[test]
    fn expect_refuses_an_entity_changed_since_it_was_read() {
        let v = version("def a():\n    return 1");
        go(PY, vec![edit(|e| { e.entity = Some(EntityRef::from("a")); e.old = Some("1".into()); e.new = Some("2".into()); e.expect = Some(v.clone()); })]).unwrap();
        let err = go(PY, vec![edit(|e| { e.entity = Some(EntityRef::from("a")); e.old = Some("1".into()); e.new = Some("2".into()); e.expect = Some("v:00000000".into()); })]).unwrap_err();
        assert!(err.message.contains("changed since you read it"), "{}", err.message);
    }

    #[test]
    fn rename_leaves_members_and_other_definitions_alone() {
        let d = scratch();
        let git = |a: &[&str]| std::process::Command::new("git").arg("-C").arg(&d).args(a).output().unwrap();
        git(&["init", "-q"]);
        std::fs::write(d.join("cells.py"), "def cell_len(s):\n    return len(s)\n").unwrap();
        std::fs::write(d.join("text.py"), "from cells import cell_len\nimport cells\n\n\nclass Text:\n    @property\n    def cell_len(self):\n        return cell_len(self.s)\n\n\ndef w(t):\n    return t.cell_len + cells.cell_len(\"x\")\n").unwrap();
        git(&["add", "."]);
        let e: Vec<Edit> = serde_json::from_str(r#"[{"file":"cells.py","entity":"cell_len","rename":"cell_width"}]"#).unwrap();
        run(&d, &e, false).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("cells.py")).unwrap(), "def cell_width(s):\n    return len(s)\n");
        assert_eq!(
            std::fs::read_to_string(d.join("text.py")).unwrap(),
            "from cells import cell_width\nimport cells\n\n\nclass Text:\n    @property\n    def cell_len(self):\n        return cell_width(self.s)\n\n\ndef w(t):\n    return t.cell_len + cells.cell_width(\"x\")\n"
        );
    }

    #[test]
    fn signature_changes_list_the_call_sites_left() {
        let d = scratch();
        let git = |a: &[&str]| std::process::Command::new("git").arg("-C").arg(&d).args(a).output().unwrap();
        git(&["init", "-q"]);
        std::fs::write(d.join("x.py"), "def area():\n    return 1\n\n\ndef total():\n    return area()\n").unwrap();
        std::fs::write(d.join("y.py"), "from x import area\n\nprint(area())\n").unwrap();
        git(&["add", "."]);
        let e: Vec<Edit> = serde_json::from_str(r#"[{"file":"x.py","entity":"area","content":"def area(n):\n    return n"}]"#).unwrap();
        let r = run(&d, &e, false).unwrap();
        assert!(r.review.len() == 1 && r.review[0].contains("y.py:3: print(area())") && r.review[0].contains("x.py:6: return area() (file edited in this batch)"), "{:?}", r.review);
    }

    #[test]
    fn insert_before_and_delete_keep_doc_comments_with_their_entity() {
        let rs = "impl V {\n    /// Is it an array?\n    #[inline]\n    pub fn is_array(&self) -> bool {\n        true\n    }\n}\n";
        let d = scratch();
        std::fs::write(d.join("v.rs"), rs).unwrap();
        let e: Vec<Edit> = serde_json::from_str(r#"[{"file":"v.rs","before":"is_array","content":"/// Its length.\npub fn len(&self) -> usize {\n        0\n    }"}]"#).unwrap();
        run(&d, &e, false).unwrap();
        let out = std::fs::read_to_string(d.join("v.rs")).unwrap();
        assert!(out.contains("    /// Its length.\n    pub fn len(&self) -> usize {\n        0\n    }\n\n    /// Is it an array?\n    #[inline]\n    pub fn is_array"), "{out}");
        let e: Vec<Edit> = serde_json::from_str(r#"[{"file":"v.rs","entity":"is_array","delete":true}]"#).unwrap();
        run(&d, &e, false).unwrap();
        let out = std::fs::read_to_string(d.join("v.rs")).unwrap();
        assert!(!out.contains("Is it an array") && !out.contains("#[inline]") && out.contains("pub fn len"), "{out}");
    }

    #[test]
    fn rename_everywhere_also_updates_comments_and_docs() {
        let d = scratch();
        let git = |a: &[&str]| std::process::Command::new("git").arg("-C").arg(&d).args(a).output().unwrap();
        git(&["init", "-q"]);
        std::fs::write(d.join("x.py"), "def area():\n    # area is cached\n    return \"area\"\n").unwrap();
        std::fs::write(d.join("README.md"), "Call `area()` (not areas).\n").unwrap();
        git(&["add", "."]);
        let e: Vec<Edit> = serde_json::from_str(r#"[{"file":"x.py","entity":"area","rename":"size","everywhere":true}]"#).unwrap();
        run(&d, &e, false).unwrap();
        assert_eq!(std::fs::read_to_string(d.join("x.py")).unwrap(), "def size():\n    # size is cached\n    return \"area\"\n");
        assert_eq!(std::fs::read_to_string(d.join("README.md")).unwrap(), "Call `size()` (not areas).\n");
    }

    #[test]
    fn method_renames_follow_calls_and_tabbed_files_stay_tabbed() {
        let d = scratch();
        let git = |a: &[&str]| std::process::Command::new("git").arg("-C").arg(&d).args(a).output().unwrap();
        git(&["init", "-q"]);
        std::fs::write(d.join("c.go"), "package c\n\ntype C struct{}\n\nfunc (c *C) IsAvail() bool {\n\treturn true\n}\n\nfunc use(c *C) bool {\n\treturn c.IsAvail()\n}\n").unwrap();
        git(&["add", "."]);
        let e: Vec<Edit> = serde_json::from_str(r#"[{"file":"c.go","entity":"IsAvail","rename":"IsAvailable"},{"file":"c.go","after":"IsAvailable","content":"// Deprecated: use IsAvailable.\nfunc (c *C) IsAvail() bool {\n    return c.IsAvailable()\n}"}]"#).unwrap();
        run(&d, &e, false).unwrap();
        let out = std::fs::read_to_string(d.join("c.go")).unwrap();
        assert!(out.contains("\treturn c.IsAvailable()\n}\n\nfunc use"), "{out}");
        assert!(out.contains("func (c *C) IsAvail() bool {\n\treturn c.IsAvailable()\n}"), "{out}");
    }
}
