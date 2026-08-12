//! The repo-scope binding pass — weave's answer to the per-file driver's one
//! structural blind spot.
//!
//! A git merge driver is invoked **once per file**, with exactly three blobs,
//! so it only ever sees one file's contents. A rename in one file whose caller
//! lives in another can merge cleanly per file and still leave the program
//! broken:
//!
//! > `a.py` renames `fetch_user` → `get_user`; `b.py` — untouched by that side,
//! > edited by the other — still calls `fetch_user(1)`. Both files merge
//! > cleanly. The program is broken. No per-file driver can see it, because
//! > neither invocation is given both files.
//!
//! This module closes that gap by running the *same* def/use evidence rules
//! ([`weave_core::binding`] — literally the same functions the per-file pass
//! calls, not a restatement of them) over the whole repo instead of one file.
//! It changes nothing about the rule; it only gives it the domain it always
//! assumed. One owner for that evidence is what makes the two passes'
//! verdicts comparable at all.
//!
//! What it does **not** do: it never resolves, rewrites or merges anything. It
//! is a read-side pass and emits findings only — `DANGLING` (a use lost its
//! definition) and `SHADOW` (a use may rebind to a different definition).
//! `DUP` and `ORDER` need a module/import model this pass does not have,
//! so it does not emit them.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use sem_core::model::change::ChangeType;
use sem_core::model::entity::SemanticEntity;
use sem_core::model::identity::match_entities;

use crate::parsers::entities_of;
use crate::wire::{Bindings, Finding, Findings, Reference, RelatedRef, Rename, SCHEMA_VERSION_1_1};
use weave_core::binding::{has_call_reference, has_definition};

/// One version of the repository: path → content. Only files that survived
/// into that version appear.
pub type Tree = BTreeMap<String, String>;

/// Which version of the merge triple a fact came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Ours,
    Theirs,
}

impl Side {
    fn wire(self) -> &'static str {
        match self {
            Side::Ours => "ours",
            Side::Theirs => "theirs",
        }
    }
    fn other(self) -> Side {
        match self {
            Side::Ours => Side::Theirs,
            Side::Theirs => Side::Ours,
        }
    }
}

/// A definition weave can see: a top-level entity in a supported file.
#[derive(Debug, Clone)]
struct Def {
    name: String,
    entity_type: String,
}

/// Per-file, per-version def sets plus the raw text (the text is what the
/// evidence rules actually run on — the parser tells us *what* is defined, the
/// text tells us *what is used*).
#[derive(Default)]
struct FileVersions {
    base: Option<String>,
    ours: Option<String>,
    theirs: Option<String>,
    base_defs: Vec<Def>,
    ours_defs: Vec<Def>,
    theirs_defs: Vec<Def>,
    /// base name → new name, per side, from sem-core's matcher.
    ours_renames: BTreeMap<String, String>,
    theirs_renames: BTreeMap<String, String>,
}

impl FileVersions {
    fn version(&self, side: Side) -> Option<&String> {
        match side {
            Side::Ours => self.ours.as_ref(),
            Side::Theirs => self.theirs.as_ref(),
        }
    }
    fn defs(&self, side: Side) -> &[Def] {
        match side {
            Side::Ours => &self.ours_defs,
            Side::Theirs => &self.theirs_defs,
        }
    }
    fn renames(&self, side: Side) -> &BTreeMap<String, String> {
        match side {
            Side::Ours => &self.ours_renames,
            Side::Theirs => &self.theirs_renames,
        }
    }
    /// Did `side` change (or add) this file relative to base?
    fn touched(&self, side: Side) -> bool {
        self.version(side) != self.base.as_ref()
    }
}

fn defs_of(entities: &[SemanticEntity]) -> Vec<Def> {
    entities
        .iter()
        .map(|e| Def {
            name: e.name.clone(),
            entity_type: e.entity_type.clone(),
        })
        .collect()
}

/// base name → new name, for entities sem-core's matcher paired across a
/// rename. This is the *same* matcher the per-file pass uses, so the two
/// passes agree about what counts as a rename: findings are relative to how
/// renames are matched, so both passes must use the one matcher or their
/// verdicts are not comparable.
fn rename_map(
    before: &[SemanticEntity],
    after: &[SemanticEntity],
    path: &str,
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for c in match_entities(before, after, path, None, None, None).changes {
        if c.change_type == ChangeType::Renamed {
            if let Some(old) = c.old_entity_name.clone() {
                if old != c.entity_name && !old.is_empty() && !c.entity_name.is_empty() {
                    map.insert(old, c.entity_name.clone());
                }
            }
        }
    }
    map
}

pub(crate) use crate::parsers::is_supported;

/// Run the repo-scope binding pass over a merge triple of whole trees.
///
/// Returns one findings document per file that has something to say, each
/// conforming to `weave-findings.schema.json` with `scope: "repo"`. Empty means
/// weave found nothing — never that nothing is wrong.
pub fn check(base: &Tree, ours: &Tree, theirs: &Tree) -> Vec<Findings> {
    // ---- 1. Build per-file def sets for the WHOLE repo, all three versions.
    let mut paths: BTreeSet<&String> = BTreeSet::new();
    paths.extend(base.keys());
    paths.extend(ours.keys());
    paths.extend(theirs.keys());

    let mut files: BTreeMap<String, FileVersions> = BTreeMap::new();
    for path in paths {
        if !is_supported(path) {
            continue;
        }
        let mut fv = FileVersions {
            base: base.get(path).cloned(),
            ours: ours.get(path).cloned(),
            theirs: theirs.get(path).cloned(),
            ..Default::default()
        };
        let base_e = entities_of(path, fv.base.as_deref().unwrap_or(""));
        let ours_e = entities_of(path, fv.ours.as_deref().unwrap_or(""));
        let theirs_e = entities_of(path, fv.theirs.as_deref().unwrap_or(""));
        fv.ours_renames = rename_map(&base_e, &ours_e, path);
        fv.theirs_renames = rename_map(&base_e, &theirs_e, path);
        fv.base_defs = defs_of(&base_e);
        fv.ours_defs = defs_of(&ours_e);
        fv.theirs_defs = defs_of(&theirs_e);
        files.insert(path.clone(), fv);
    }

    // ---- 2. The post-merge definition set, repo-wide.
    //
    // A name is still bound after the merge if *some* file still defines it.
    // Per file, the merged version is whichever side changed it; when both
    // changed it we take the union, which is the conservative direction: it can
    // only suppress findings, never invent them.
    let mut merged_defs: HashSet<String> = HashSet::new();
    let mut base_defs_all: HashSet<String> = HashSet::new();
    for fv in files.values() {
        for d in &fv.base_defs {
            base_defs_all.insert(d.name.clone());
        }
        let (o, t) = (fv.touched(Side::Ours), fv.touched(Side::Theirs));
        let survivors: Vec<&Def> = match (o, t) {
            (true, true) => fv.ours_defs.iter().chain(fv.theirs_defs.iter()).collect(),
            (true, false) => fv.ours_defs.iter().collect(),
            (false, true) => fv.theirs_defs.iter().collect(),
            (false, false) => fv.base_defs.iter().collect(),
        };
        for d in survivors {
            merged_defs.insert(d.name.clone());
        }
    }

    // Definitions each side can see anywhere in its own tree — used to decide
    // whether a caller's name was already bound on that side.
    let side_defs = |side: Side| -> HashSet<String> {
        files
            .values()
            .filter(|fv| fv.version(side).is_some())
            .flat_map(|fv| fv.defs(side))
            .map(|def| def.name.clone())
            .collect()
    };
    let ours_defs_all = side_defs(Side::Ours);
    let theirs_defs_all = side_defs(Side::Theirs);

    let mut out: FindingSink = FindingSink::default();

    // ---- 3. DANGLING: one side removed a definition, the other side's changed
    //         code still calls it, and nothing in the repo redefines it.
    for actor in [Side::Ours, Side::Theirs] {
        let other = actor.other();
        for (path, fv) in &files {
            if !fv.touched(actor) || fv.base.is_none() {
                continue;
            }
            let actor_names: HashSet<&str> =
                fv.defs(actor).iter().map(|d| d.name.as_str()).collect();
            let renames = fv.renames(actor);
            for d in &fv.base_defs {
                if actor_names.contains(d.name.as_str()) {
                    continue;
                }
                // Still defined somewhere post-merge: the use is bound, so this
                // is not DANGLING (it may be DUP, which is not decidable here).
                if merged_defs.contains(&d.name) {
                    continue;
                }
                let renamed_to = renames.get(&d.name).cloned();
                for (other_path, other_fv) in &files {
                    if other_path == path || !other_fv.touched(other) {
                        continue;
                    }
                    let Some(content) = other_fv.version(other) else {
                        continue;
                    };
                    // The caller's own file defines it: bound locally, skip —
                    // the same guard the per-file pass applies.
                    if has_definition(content, &d.name) {
                        continue;
                    }
                    if !has_call_reference(content, &d.name) {
                        continue;
                    }
                    out.push(
                        other_path,
                        dangling_finding(
                            &d.name,
                            &d.entity_type,
                            actor,
                            path,
                            renamed_to.as_deref(),
                        ),
                        renamed_to.as_ref().map(|to| Rename {
                            from: d.name.clone(),
                            to: to.clone(),
                            side: actor.wire().to_string(),
                        }),
                        Reference {
                            entity: d.name.clone(),
                            defined_in_output: false,
                            referenced_in_output: true,
                        },
                    );
                }
            }
        }
    }

    // ---- 4. SHADOW: one side introduced a definition of a name that did not
    //         exist in base, and the other side's changed code calls that name
    //         while binding it to something else (an import, a builtin).
    for actor in [Side::Ours, Side::Theirs] {
        let other = actor.other();
        let other_defs_all = match other {
            Side::Ours => &ours_defs_all,
            Side::Theirs => &theirs_defs_all,
        };
        for (path, fv) in &files {
            if !fv.touched(actor) {
                continue;
            }
            let base_names: HashSet<&str> = fv.base_defs.iter().map(|d| d.name.as_str()).collect();
            let rename_targets: HashSet<&str> =
                fv.renames(actor).values().map(|s| s.as_str()).collect();
            for d in fv.defs(actor) {
                if base_names.contains(d.name.as_str()) || rename_targets.contains(d.name.as_str())
                {
                    continue;
                }
                // Not new to the repo: the name already resolved somewhere.
                if base_defs_all.contains(&d.name) {
                    continue;
                }
                // The other side defines it too — a collision, not a capture.
                if other_defs_all.contains(&d.name) {
                    continue;
                }
                for (other_path, other_fv) in &files {
                    if other_path == path || !other_fv.touched(other) {
                        continue;
                    }
                    let Some(content) = other_fv.version(other) else {
                        continue;
                    };
                    if has_definition(content, &d.name) || !has_call_reference(content, &d.name) {
                        continue;
                    }
                    out.push(
                        other_path,
                        shadow_finding(&d.name, &d.entity_type, actor, path),
                        None,
                        Reference {
                            entity: d.name.clone(),
                            defined_in_output: false,
                            referenced_in_output: true,
                        },
                    );
                }
            }
        }
    }

    out.into_documents()
}

fn dangling_finding(
    name: &str,
    entity_type: &str,
    actor: Side,
    defining_file: &str,
    renamed_to: Option<&str>,
) -> Finding {
    let mut f = Finding::derived("DANGLING", name, entity_type);
    f.side = Some(actor.wire().to_string());
    f.confidence = Some("high".to_string());
    f.suggestion = Some(match renamed_to {
        Some(to) => format!(
            "`{name}` is still called here but {} renamed it to `{to}` in `{defining_file}`, \
             and nothing else in the repo defines it. Rewrite these call sites to `{to}`.",
            actor.wire()
        ),
        None => format!(
            "`{name}` is still called here but {} deleted it from `{defining_file}`, and nothing \
             else in the repo defines it. Restore the definition or remove the call sites.",
            actor.wire()
        ),
    });
    f.related = vec![RelatedRef {
        entity: renamed_to.unwrap_or(name).to_string(),
        entity_type: Some(entity_type.to_string()),
        file: Some(defining_file.to_string()),
    }];
    f
}

fn shadow_finding(name: &str, entity_type: &str, actor: Side, defining_file: &str) -> Finding {
    let mut f = Finding::derived("SHADOW", name, entity_type);
    f.side = Some(actor.wire().to_string());
    // Cross-file capture depends on the language's import/module rules, which
    // weave does not model. Medium is the honest ceiling here.
    f.confidence = Some("medium".to_string());
    f.suggestion = Some(format!(
        "`{name}` is called here and was resolving outside this merge (an import or a builtin), \
         but {} added a new definition of `{name}` in `{defining_file}`. Check which one this \
         call is meant to reach.",
        actor.wire()
    ));
    f.related = vec![RelatedRef {
        entity: name.to_string(),
        entity_type: Some(entity_type.to_string()),
        file: Some(defining_file.to_string()),
    }];
    f
}

/// Everything one document accumulates: its findings and the binding facts
/// that justify them.
#[derive(Default)]
struct DocumentParts {
    findings: Vec<Finding>,
    renames: Vec<Rename>,
    references: Vec<Reference>,
}

/// Accumulates findings per file and folds them into schema documents.
#[derive(Default)]
struct FindingSink {
    per_file: BTreeMap<String, DocumentParts>,
    /// (file, class, entity) — one finding per breakage, however many callers
    /// or sides witness it.
    seen: HashSet<(String, String, String)>,
}

impl FindingSink {
    fn push(&mut self, file: &str, finding: Finding, rename: Option<Rename>, reference: Reference) {
        let key = (
            file.to_string(),
            finding.class.clone(),
            finding.entity.clone(),
        );
        if !self.seen.insert(key) {
            return;
        }
        let slot = self.per_file.entry(file.to_string()).or_default();
        slot.findings.push(finding);
        if let Some(r) = rename {
            if !slot.renames.contains(&r) {
                slot.renames.push(r);
            }
        }
        if !slot.references.contains(&reference) {
            slot.references.push(reference);
        }
    }

    fn into_documents(self) -> Vec<Findings> {
        self.per_file
            .into_iter()
            .map(|(file, parts)| {
                // A repo-scope document never describes a text merge, so
                // `clean` is about markers and is vacuously true; the whole
                // point of the schema is that clean and safe differ.
                let confidence = if parts.findings.iter().any(|f| f.class == "DANGLING") {
                    "low"
                } else {
                    "medium"
                };
                Findings {
                    schema_version: SCHEMA_VERSION_1_1.to_string(),
                    file,
                    scope: Some("repo".to_string()),
                    clean: true,
                    confidence: confidence.to_string(),
                    findings: parts.findings,
                    // A check performs no merge, so there is no per-entity
                    // composition record to report. Empty, never absent.
                    ops: Vec::new(),
                    bindings: Bindings {
                        renames: parts.renames,
                        references: parts.references,
                    },
                }
            })
            .collect()
    }
}

/// Convenience: the merge triple as directory trees on disk.
pub fn read_dir_tree(root: &std::path::Path) -> std::io::Result<Tree> {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Tree) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".git" || name == "target" || name == "node_modules" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out)?;
            } else if let Ok(rel) = path.strip_prefix(root) {
                let rel = rel.to_string_lossy().replace('\\', "/");
                if !is_supported(&rel) {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    out.insert(rel, content);
                }
            }
        }
        Ok(())
    }
    let mut out = Tree::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

/// Total finding count across documents — the exit-code decision.
pub fn total_findings(docs: &[Findings]) -> usize {
    docs.iter().map(|d| d.findings.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(pairs: &[(&str, &str)]) -> Tree {
        pairs
            .iter()
            .map(|(p, c)| (p.to_string(), c.to_string()))
            .collect()
    }

    /// THE blind spot: a rename in one file, a caller in another, both merging
    /// cleanly per-file.
    #[test]
    fn a_rename_in_one_file_dangles_a_caller_in_another() {
        let base = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        // ours renames the definition
        let ours = tree(&[
            ("a.py", "def get_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        // theirs adds a caller of the OLD name
        let theirs = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            (
                "b.py",
                "def report():\n    return 1\n\n\ndef summary():\n    return fetch_user(1)\n",
            ),
        ]);

        let docs = check(&base, &ours, &theirs);
        assert_eq!(docs.len(), 1, "the caller's file is the one at risk");
        assert_eq!(docs[0].file, "b.py");
        let f = &docs[0].findings[0];
        assert_eq!(f.class, "DANGLING");
        assert_eq!(f.entity, "fetch_user");
        assert_eq!(f.side.as_deref(), Some("ours"));
        assert_eq!(f.related[0].file.as_deref(), Some("a.py"));
        assert_eq!(docs[0].bindings.renames[0].to, "get_user");
    }

    #[test]
    fn a_delete_in_one_file_dangles_a_caller_in_another() {
        let base = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        let ours = tree(&[("a.py", "\n"), ("b.py", "def report():\n    return 1\n")]);
        let theirs = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return fetch_user(1)\n"),
        ]);
        let docs = check(&base, &ours, &theirs);
        let f = &docs[0].findings[0];
        assert_eq!(f.class, "DANGLING");
        assert!(f.suggestion.as_ref().unwrap().contains("deleted"));
    }

    #[test]
    fn a_surviving_definition_elsewhere_is_not_dangling() {
        // c.py still defines fetch_user, so the caller is bound.
        let base = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            ("c.py", "def fetch_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        let ours = tree(&[
            ("a.py", "def get_user(uid):\n    return uid\n"),
            ("c.py", "def fetch_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        let theirs = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            ("c.py", "def fetch_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return fetch_user(1)\n"),
        ]);
        assert!(check(&base, &ours, &theirs).is_empty());
    }

    #[test]
    fn an_untouched_caller_is_not_reported() {
        // b.py references the old name but NEITHER side touched b.py: that is a
        // pre-existing repo state, not something this merge composed.
        let base = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return fetch_user(1)\n"),
        ]);
        let ours = tree(&[
            ("a.py", "def get_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return fetch_user(1)\n"),
        ]);
        let theirs = base.clone();
        assert!(check(&base, &ours, &theirs).is_empty());
    }

    #[test]
    fn attribute_access_and_bare_mentions_do_not_count_as_calls() {
        let base = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        let ours = tree(&[
            ("a.py", "def get_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        let theirs = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            (
                "b.py",
                "def report():\n    log('fetch_user')\n    return db.fetch_user(1)\n",
            ),
        ]);
        assert!(check(&base, &ours, &theirs).is_empty());
    }

    #[test]
    fn a_new_definition_capturing_another_sides_call_is_shadow() {
        let base = tree(&[
            ("a.py", "def existing():\n    return 0\n"),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        // ours defines a brand-new `render`
        let ours = tree(&[
            (
                "a.py",
                "def existing():\n    return 0\n\n\ndef render(x):\n    return x\n",
            ),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        // theirs adds code that calls `render` — meaning something imported
        let theirs = tree(&[
            ("a.py", "def existing():\n    return 0\n"),
            ("b.py", "def report():\n    return render(1)\n"),
        ]);
        let docs = check(&base, &ours, &theirs);
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].findings[0].class, "SHADOW");
        assert_eq!(docs[0].findings[0].entity, "render");
    }

    #[test]
    fn unsupported_files_are_skipped_rather_than_guessed_at() {
        let base = tree(&[("a.bin", "def fetch_user(uid): pass\n")]);
        let ours = tree(&[("a.bin", "def get_user(uid): pass\n")]);
        let theirs = tree(&[("a.bin", "def fetch_user(uid): pass\nfetch_user(1)\n")]);
        assert!(check(&base, &ours, &theirs).is_empty());
    }

    #[test]
    fn documents_conform_to_the_schemas_required_keys() {
        let base = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        let ours = tree(&[
            ("a.py", "def get_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return 1\n"),
        ]);
        let theirs = tree(&[
            ("a.py", "def fetch_user(uid):\n    return uid\n"),
            ("b.py", "def report():\n    return fetch_user(1)\n"),
        ]);
        let docs = check(&base, &ours, &theirs);
        let v = serde_json::to_value(&docs[0]).unwrap();
        for key in [
            "schema_version",
            "file",
            "clean",
            "confidence",
            "findings",
            "ops",
            "bindings",
        ] {
            assert!(v.get(key).is_some(), "missing required key {key}");
        }
        assert_eq!(v["scope"], "repo");
        assert_eq!(v["schema_version"], "1.1.0");
    }
}
