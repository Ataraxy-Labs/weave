//! One parser registry for the whole process.
//!
//! `create_default_registry()` builds every tree-sitter grammar weave knows;
//! doing that per file (or, worse, per path in a repo walk) is the difference
//! between a check that runs and one that does not.

use std::sync::LazyLock;

use sem_core::model::entity::SemanticEntity;
use sem_core::parser::plugins::create_default_registry;
use sem_core::parser::registry::ParserRegistry;

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

/// Top-level entities of `content`, as parsed for `path`. Empty when the file
/// has no supported grammar.
///
/// Top-level only: cross-file name resolution and file-level patching are both
/// about names another file can reach by a bare reference. A method body's
/// local name is not one, and counting it would claim bindings the program does
/// not have.
pub(crate) fn entities_of(path: &str, content: &str) -> Vec<SemanticEntity> {
    let Some(plugin) = REGISTRY.get_explicit_plugin(path) else {
        return Vec::new();
    };
    top_level(plugin.extract_entities(content, path))
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
