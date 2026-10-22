//! One tree-sitter parse per text, however many checks read it.
//!
//! A merge reads the same few texts again and again: the subsumption check,
//! the gate (`verify`), the key backstop (`keys_hold`), the layout and
//! determinate rules each parse ours, theirs and the result on their own.
//! Profiled over 1,040 real merges that was a third of all merge time spent
//! re-parsing text it had already parsed. A parse is a pure function of the
//! registry, the path and the text, so the answer can be kept and shared:
//! nothing reads a different tree than it would have built itself.
//!
//! The memo lives only inside [`scope`] — one merge, or one check — and is
//! dropped when the outermost scope ends, so the registry it was filled from
//! is the registry every reader in the scope passed. Outside a scope `parse`
//! is a plain parse. It holds the last few texts, which is all one merge
//! touches, and an entry is matched on the full text, never on a hash alone.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use sem_core::model::entity::SemanticEntity;
use sem_core::parser::registry::ParserRegistry;
use tree_sitter::Tree;

/// What `ParserRegistry::extract_entities_with_tree` answered, with no answer
/// (no plugin for the path) read as no entities and no tree — which is how
/// every caller already read it.
#[derive(Default)]
pub(crate) struct Parsed {
    pub(crate) entities: Vec<SemanticEntity>,
    pub(crate) tree: Option<Tree>,
}

const KEEP: usize = 8;

struct Entry {
    registry: usize,
    path: String,
    text: String,
    parsed: Rc<Parsed>,
}

thread_local! {
    static RECENT: RefCell<VecDeque<Entry>> = const { RefCell::new(VecDeque::new()) };
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Run `f` with the memo on, and empty it when the outermost scope returns
/// (or unwinds).
pub(crate) fn scope<T>(f: impl FnOnce() -> T) -> T {
    struct Exit;
    impl Drop for Exit {
        fn drop(&mut self) {
            let depth = DEPTH.with(|d| {
                d.set(d.get() - 1);
                d.get()
            });
            if depth == 0 {
                RECENT.with(|r| r.borrow_mut().clear());
            }
        }
    }
    DEPTH.with(|d| d.set(d.get() + 1));
    let _exit = Exit;
    f()
}

pub(crate) fn parse(registry: &ParserRegistry, path: &str, text: &str) -> Rc<Parsed> {
    let fresh = || {
        Rc::new(
            registry
                .extract_entities_with_tree(path, text)
                .map(|(entities, tree)| Parsed { entities, tree })
                .unwrap_or_default(),
        )
    };
    if DEPTH.with(Cell::get) == 0 {
        return fresh();
    }
    let id = registry as *const ParserRegistry as usize;
    let hit = RECENT.with(|r| {
        let mut r = r.borrow_mut();
        let at = r
            .iter()
            .position(|e| e.registry == id && e.path == path && e.text == text)?;
        let entry = r.remove(at)?;
        let parsed = Rc::clone(&entry.parsed);
        r.push_front(entry);
        Some(parsed)
    });
    if let Some(parsed) = hit {
        return parsed;
    }
    let parsed = fresh();
    RECENT.with(|r| {
        let mut r = r.borrow_mut();
        r.truncate(KEEP - 1);
        r.push_front(Entry {
            registry: id,
            path: path.to_string(),
            text: text.to_string(),
            parsed: Rc::clone(&parsed),
        });
    });
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;
    use sem_core::parser::plugins::create_default_registry;

    #[test]
    fn reuse_is_scoped_and_content_sensitive() {
        let registry = create_default_registry();
        let text = "fn first() {}";
        let previous = scope(|| {
            let first = parse(&registry, "a.rs", text);
            assert!(Rc::ptr_eq(&first, &parse(&registry, "a.rs", text)));
            scope(|| assert!(Rc::ptr_eq(&first, &parse(&registry, "a.rs", text))));
            assert!(!Rc::ptr_eq(&first, &parse(&registry, "b.rs", text)));
            assert!(!Rc::ptr_eq(
                &first,
                &parse(&registry, "a.rs", "fn other() {}")
            ));
            first
        });
        scope(|| assert!(!Rc::ptr_eq(&previous, &parse(&registry, "a.rs", text))));
        assert!(RECENT.with(|r| r.borrow().is_empty()));
    }

    #[test]
    fn panic_clears_scope() {
        let result = std::panic::catch_unwind(|| {
            scope(|| {
                let registry = create_default_registry();
                parse(&registry, "a.rs", "fn first() {}");
                panic!("test unwind");
            })
        });
        assert!(result.is_err());
        assert_eq!(DEPTH.with(Cell::get), 0);
        assert!(RECENT.with(|r| r.borrow().is_empty()));
    }
}
