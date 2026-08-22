//! Address-based entity resolution within a single file.
//!
//! [`super::sync::resolve_entity_id`] resolved targets by NAME ONLY, so two
//! same-named entities in one file (a method `run` on two classes, or an
//! overload) silently resolved to whichever the parser happened to extract
//! first. This module replaces that with a progressive filter over the
//! vocabulary of weave-core v2's `Key`: `parent` + `type` + `name` +
//! `ordinal`. Callers supply only the fields they know; if more than one
//! entity survives filtering, the result is [`Resolution::Ambiguous`] — never
//! a silent pick-first.

use sem_core::model::entity::SemanticEntity;
use sem_core::parser::registry::ParserRegistry;

/// How to locate one entity inside a single file.
///
/// Only `name` is required. Every other field narrows the candidate set when
/// present (`None` = don't filter on it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityAddress<'a> {
    /// Filter by entity kind (e.g. `"function"`, `"class"`, `"method"`).
    pub entity_type: Option<&'a str>,
    /// Filter by the name of the enclosing entity (e.g. the class owning a
    /// method). Entities without a parent never match this filter.
    pub parent_name: Option<&'a str>,
    /// The entity's name. Always required.
    pub name: &'a str,
    /// 0-based occurrence index among the candidates that survive the other
    /// filters, ordered by position in the file (first occurrence = 0).
    pub ordinal: Option<u32>,
}

impl<'a> EntityAddress<'a> {
    /// Address matching any entity called `name`, however many there are.
    pub fn by_name(name: &'a str) -> Self {
        Self {
            entity_type: None,
            parent_name: None,
            name,
            ordinal: None,
        }
    }

    /// Narrow the address to a specific entity kind.
    pub fn with_type(mut self, entity_type: &'a str) -> Self {
        self.entity_type = Some(entity_type);
        self
    }

    /// Narrow the address to entities enclosed by `parent_name`.
    pub fn with_parent(mut self, parent_name: &'a str) -> Self {
        self.parent_name = Some(parent_name);
        self
    }

    /// Select the `ordinal`-th (0-based) surviving candidate.
    pub fn with_ordinal(mut self, ordinal: u32) -> Self {
        self.ordinal = Some(ordinal);
        self
    }
}

/// One surviving candidate reported when resolution is ambiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveCandidate {
    /// The CRDT/parsed entity ID that would be selected.
    pub entity_id: String,
    pub entity_type: String,
    /// Name of the enclosing entity, if any.
    pub parent: Option<String>,
    /// 0-based index of this candidate among the filtered set (file order).
    pub ordinal: u32,
    /// Short excerpt of the entity's source content.
    pub snippet: String,
}

/// Outcome of resolving an [`EntityAddress`] against a file's entities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Exactly one entity matched.
    Resolved(String),
    /// No entity matched (name unknown, or supplied filters/ordinal excluded
    /// every candidate).
    NotFound,
    /// More than one entity matched. Never guess: the caller must narrow the
    /// address (add `parent_name`, `entity_type`, or `ordinal`).
    Ambiguous(Vec<ResolveCandidate>),
}

impl Resolution {
    /// The resolved ID, if resolution was unambiguous.
    pub fn resolved(&self) -> Option<&str> {
        match self {
            Resolution::Resolved(id) => Some(id),
            _ => None,
        }
    }

    /// Human-readable explanation of a failed resolution, listing the
    /// candidates on ambiguity and telling the caller which address field to
    /// add.
    pub fn describe_failure(&self, address: &EntityAddress<'_>, file_path: &str) -> String {
        let mut qualifiers = Vec::new();
        match address.entity_type {
            Some(t) => qualifiers.push(format!("type=`{}`", t)),
            None => qualifiers.push("type=<any>".to_string()),
        }
        match address.parent_name {
            Some(p) => qualifiers.push(format!("parent=`{}`", p)),
            None => qualifiers.push("parent=<any>".to_string()),
        }
        let qualifier_str = qualifiers.join(", ");

        match self {
            Resolution::Resolved(id) => {
                format!(
                    "Entity '{}' resolved to '{}' in '{}'",
                    address.name, id, file_path
                )
            }
            Resolution::NotFound => format!(
                "Entity '{}' ({}) not found in '{}'",
                address.name, qualifier_str, file_path
            ),
            Resolution::Ambiguous(candidates) => {
                let mut msg = format!(
                    "Entity '{}' ({}) is ambiguous in '{}': {} candidates match.\nCandidates (file order):",
                    address.name,
                    qualifier_str,
                    file_path,
                    candidates.len()
                );
                for c in candidates {
                    msg.push_str(&format!(
                        "\n  [{}] {} `{}` (parent: {}) — \"{}\"",
                        c.ordinal,
                        c.entity_type,
                        address.name,
                        c.parent.as_deref().unwrap_or("<none>"),
                        c.snippet
                    ));
                }
                msg.push_str(
                    "\nDisambiguate by adding one of: parent_name (e.g. the enclosing class), \
                     entity_type, or ordinal (0-based, see candidate indexes above).",
                );
                msg
            }
        }
    }
}

/// Resolve an [`EntityAddress`] against the entities of one file.
///
/// Filters the file's parsed entities progressively by each field the caller
/// supplied (`entity_type`, then `parent_name`; `name` always applies), orders
/// survivors by their position in the file, and applies `ordinal` as a 0-based
/// index into that order. Returns [`Resolution::Ambiguous`] whenever more than
/// one entity survives — it never picks the first.
pub fn resolve_entity(
    content: &str,
    file_path: &str,
    registry: &ParserRegistry,
    address: &EntityAddress<'_>,
) -> Resolution {
    let plugin = match registry.get_plugin(file_path) {
        Some(p) => p,
        None => return Resolution::NotFound,
    };

    let entities = plugin.extract_entities(content, file_path);

    let mut candidates: Vec<(usize, &SemanticEntity)> = entities
        .iter()
        .filter(|e| e.name == address.name)
        .filter(|e| match address.entity_type {
            Some(t) => e.entity_type == t,
            None => true,
        })
        .filter(|e| match address.parent_name {
            Some(p) => parent_name_of(e).as_deref() == Some(p),
            None => true,
        })
        // Deterministic file order for ordinal assignment (stable sort keeps
        // the parser's order for equal start lines).
        .map(|e| (e.start_line, e))
        .collect();
    candidates.sort_by_key(|(line, _)| *line);

    if candidates.is_empty() {
        return Resolution::NotFound;
    }

    let selected: Vec<ResolveCandidate> = candidates
        .into_iter()
        .enumerate()
        .filter(|(i, _)| match address.ordinal {
            Some(o) => *i == o as usize,
            None => true,
        })
        .map(|(i, (_, e))| ResolveCandidate {
            entity_id: e.id.clone(),
            entity_type: e.entity_type.clone(),
            parent: parent_name_of(e),
            ordinal: i as u32,
            snippet: short_snippet(&e.content),
        })
        .collect();

    match selected.len() {
        0 => Resolution::NotFound,
        1 => Resolution::Resolved(selected[0].entity_id.clone()),
        _ => Resolution::Ambiguous(selected),
    }
}

/// Like [`resolve_entity`], but turns every outcome into a `Result` with a
/// human-readable error ([`Resolution::describe_failure`]) suitable for
/// surfacing directly over MCP or CLI.
pub fn resolve_entity_or_error(
    content: &str,
    file_path: &str,
    registry: &ParserRegistry,
    address: &EntityAddress<'_>,
) -> std::result::Result<String, String> {
    match resolve_entity(content, file_path, registry, address) {
        Resolution::Resolved(id) => Ok(id),
        resolution => Err(resolution.describe_failure(address, file_path)),
    }
}

/// Derive the enclosing entity's display name from its ID.
///
/// Parent IDs look like `src/lib.rs::class::Animal` (or carry disambiguator
/// suffixes such as `...::class::C@L1#1`); the parent's name is the last `::`
/// segment with any `@...` disambiguator stripped.
fn parent_name_of(entity: &SemanticEntity) -> Option<String> {
    let parent_id = entity.parent_id.as_deref()?;
    let last = parent_id.rsplit("::").next().unwrap_or(parent_id);
    let name = match last.split_once('@') {
        Some((name, _)) => name,
        None => last,
    };
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// First meaningful line of the entity's source, trimmed and capped for
/// display in ambiguity errors.
fn short_snippet(content: &str) -> String {
    const MAX_LEN: usize = 60;
    let mut snippet = content
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string();
    if snippet.chars().count() > MAX_LEN {
        snippet = format!("{}…", snippet.chars().take(MAX_LEN).collect::<String>());
    }
    snippet
}
