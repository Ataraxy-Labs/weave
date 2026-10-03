//! An import one side deletes, while the other side's NEW code uses the name it
//! binds, is kept.
//!
//! Field shape: base imports one module twice under two names. Each side drops
//! a different one of the two imports and rewrites its own call sites to the
//! name it kept. Taken line by line, both deletions apply, the file keeps
//! calling one of the names, and nothing binds it any more: a compile error
//! the merge introduced, with no marker and exit 0.
//!
//! A deletion claims "nothing needs this import". The other side's new use
//! shows that claim is false, so the merge keeps the import. The deleting side
//! does not use the name, so the kept line changes nothing it wrote.

use weave_core::entity_merge;

const BASE: &str = "\
import * as Kit from \"./kit\"
import * as Alpha from \"./widget\"
import * as Beta from \"./gadget\"
import * as Widget from \"./widget\"

export function* flow() {
  yield* Alpha.first()
  const spacer = 1
  const spacer2 = 2
  const spacer3 = 3
  yield* Widget.second()
}
";

/// Ours drops the `Widget` import and stops using `Widget`.
fn ours() -> String {
    BASE.replace("import * as Widget from \"./widget\"\n", "")
        .replace("Widget.second()", "Beta.second()")
}

fn binds(content: &str, name: &str) -> bool {
    content
        .lines()
        .any(|l| l.starts_with("import ") && l.contains(&format!(" as {name} ")))
}

fn uses(content: &str, name: &str) -> bool {
    content.contains(&format!("{name}."))
}

/// Every name the merged file uses as `Name.` must be bound by an import.
fn assert_no_dangling_namespace(content: &str) {
    for name in ["Kit", "Alpha", "Beta", "Widget"] {
        assert!(
            !uses(content, name) || binds(content, name),
            "`{name}` is used but no longer imported:\n{content}"
        );
    }
}

#[test]
fn each_side_drops_a_different_import_the_other_side_now_uses() {
    // Theirs drops `Alpha` and moves its call onto `Widget`.
    let theirs = BASE
        .replace("import * as Alpha from \"./widget\"\n", "")
        .replace("Alpha.first()", "Widget.first()");
    let ours = ours();
    for (left, right) in [(&ours, &theirs), (&theirs, &ours)] {
        let result = entity_merge(BASE, left, right, "flow.ts");
        assert!(result.is_clean(), "{:?}", result.conflicts);
        assert_no_dangling_namespace(&result.content);
        assert!(uses(&result.content, "Widget"), "{}", result.content);
        // Alpha's deletion is not contested: nothing uses it any more.
        assert!(!binds(&result.content, "Alpha"), "{}", result.content);
    }
}

#[test]
fn a_side_that_rewrites_one_import_into_the_other_keeps_it() {
    // Theirs rewrites the `Alpha` line into a copy of the `Widget` line and
    // deletes the original `Widget` line: textually, `Widget` MOVED up.
    let theirs = BASE
        .replace(
            "import * as Alpha from \"./widget\"\nimport * as Beta from \"./gadget\"\nimport * as Widget from \"./widget\"\n",
            "import * as Widget from \"./widget\"\nimport * as Beta from \"./gadget\"\n",
        )
        .replace("Alpha.first()", "Widget.first()");
    let ours = ours();
    for (left, right) in [(&ours, &theirs), (&theirs, &ours)] {
        let result = entity_merge(BASE, left, right, "flow.ts");
        assert!(result.is_clean(), "{:?}", result.conflicts);
        assert_no_dangling_namespace(&result.content);
        assert_eq!(
            result.content.matches("import * as Widget").count(),
            1,
            "{}",
            result.content
        );
    }
}

#[test]
fn a_new_use_keeps_the_import_even_when_the_block_is_one_sided() {
    // Theirs never touches the import block; it only adds a use of `Widget`.
    let theirs = BASE.replace(
        "  const spacer = 1\n",
        "  const spacer = 1\n  Widget.third()\n",
    );
    let ours = ours();
    for (left, right) in [(&ours, &theirs), (&theirs, &ours)] {
        let result = entity_merge(BASE, left, right, "flow.ts");
        assert!(result.is_clean(), "{:?}", result.conflicts);
        assert_no_dangling_namespace(&result.content);
        assert!(binds(&result.content, "Widget"), "{}", result.content);
    }
}

#[test]
fn an_unused_import_deleted_by_one_side_stays_deleted() {
    // Theirs edits something unrelated and adds no use of `Widget`, so ours'
    // deletion is uncontested: merge(b, x, y) must still take it. (Theirs'
    // edit sits two lines clear of ours' edit to the same body: edits to
    // ADJACENT lines of one body are a conflict, as they are for git.)
    let theirs = BASE.replace("const spacer = 1\n", "const spacer = 10\n");
    let ours = ours();
    let expected = ours.replace("const spacer = 1\n", "const spacer = 10\n");
    for (left, right) in [(&ours, &theirs), (&theirs, &ours)] {
        let result = entity_merge(BASE, left, right, "flow.ts");
        assert!(result.is_clean(), "{:?}", result.conflicts);
        assert_eq!(result.content, expected);
    }
}

#[test]
fn an_import_rebound_by_the_deleting_side_is_not_restored() {
    // Ours replaces the `Widget` namespace import with a named import of the
    // same name from elsewhere: `Widget` is still bound, nothing to keep.
    let ours = BASE.replace(
        "import * as Widget from \"./widget\"\n",
        "import { Widget } from \"./widget-next\"\n",
    );
    let theirs = BASE.replace(
        "  const spacer = 1\n",
        "  const spacer = 1\n  Widget.third()\n",
    );
    for (left, right) in [(&ours, &theirs), (&theirs, &ours)] {
        let result = entity_merge(BASE, left, right, "flow.ts");
        assert!(result.is_clean(), "{:?}", result.conflicts);
        assert!(
            !result.content.contains("import * as Widget"),
            "{}",
            result.content
        );
    }
}
