//! One parser registry for the whole process.
//!
//! `create_default_registry()` builds every tree-sitter grammar weave knows;
//! doing that per file (or, worse, per path in a repo walk) is the difference
//! between a check that runs and one that does not.

use std::sync::LazyLock;

use sem_core::model::entity::SemanticEntity;
use sem_core::parser::plugins::create_default_registry;
use sem_core::parser::registry::ParserRegistry;
use weave_core::merge::STRUCTURE_LIMIT_BYTES;

pub(crate) static REGISTRY: LazyLock<ParserRegistry> = LazyLock::new(create_default_registry);

/// Does weave have a real grammar for this path?
///
/// `get_explicit_plugin` deliberately refuses the fallback (20-line chunk)
/// plugin: entity reasoning over arbitrary chunks is worse than no entity
/// reasoning, so unsupported files inherit git's guarantees wholesale
/// instead of a guess dressed up as structure.
pub(crate) fn is_supported(path: &str) -> bool {
    REGISTRY.get_explicit_plugin(path).is_some()
}

/// Is this path a programming language, where names are bound and referenced?
///
/// Weave also parses prose and data formats (Markdown, JSON, YAML, TOML, CSV,
/// LaTeX) into entities for merging, but a word followed by `(` in prose is
/// not a call. Reference checks ask this, not [`is_supported`].
pub(crate) fn is_code(path: &str) -> bool {
    REGISTRY.get_explicit_plugin(path).is_some_and(|p| {
        !matches!(
            p.id(),
            "markdown" | "json" | "yaml" | "toml" | "csv" | "latex"
        )
    })
}

/// Top-level entities of `content`, as parsed for `path`. Empty when the file
/// has no supported grammar.
///
/// Top-level only: cross-file name resolution and file-level patching are both
/// about names another file can reach by a bare reference. A method body's
/// local name is not one, and counting it would claim bindings the program does
/// not have.
///
/// Also empty above [`STRUCTURE_LIMIT_BYTES`], the ceiling the merge itself
/// reads structure under: past it, extraction can be quadratic, and a check
/// that parses an unbounded file is a check that may never answer. Callers
/// that need a name out of such a file ask it lexically ([`oversize`]).
pub(crate) fn entities_of(path: &str, content: &str) -> Vec<SemanticEntity> {
    if oversize(content) {
        return Vec::new();
    }
    let Some(plugin) = REGISTRY.get_explicit_plugin(path) else {
        return Vec::new();
    };
    top_level(plugin.extract_entities(content, path))
}

/// Every name `content` defines, at any depth: functions and classes, and the
/// methods and members nested inside them. Empty when [`entities_of`] is.
///
/// This answers "is this name still defined here?", which is not the question
/// [`entities_of`] answers. A definition can move between top level and a
/// class — or the parser can see it at a different depth in two versions of a
/// file — and a call to it still resolves. Asked of top-level names only, the
/// moved name looks deleted and its callers look dangling.
pub(crate) fn defined_names_of(path: &str, content: &str) -> Vec<String> {
    if oversize(content) {
        return Vec::new();
    }
    let Some(plugin) = REGISTRY.get_explicit_plugin(path) else {
        return Vec::new();
    };
    plugin
        .extract_entities(content, path)
        .into_iter()
        .map(|e| e.name)
        .filter(|n| !n.is_empty())
        .collect()
}

/// Too large to read for structure; see [`entities_of`].
pub(crate) fn oversize(content: &str) -> bool {
    content.len() > STRUCTURE_LIMIT_BYTES
}

fn top_level(entities: Vec<SemanticEntity>) -> Vec<SemanticEntity> {
    let spans: Vec<(usize, usize)> = entities
        .iter()
        .filter(|e| e.parent_id.is_none())
        .map(|e| (e.start_line, e.end_line))
        .collect();
    let mut out: Vec<SemanticEntity> = entities
        .into_iter()
        .filter(|e| {
            if e.parent_id.is_some() || e.name.is_empty() {
                return false;
            }
            // Some plugins report nesting by line range rather than parent_id.
            !spans
                .iter()
                .any(|&(s, t)| s < e.start_line && e.end_line <= t)
        })
        .collect();
    out.sort_by_key(|e| e.start_line);
    out
}
