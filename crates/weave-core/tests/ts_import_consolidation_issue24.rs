//! Regression coverage for issue #24 (imports getting mangled in TS).
//!
//! The original report was a merge that consolidated two `from "./foo"` imports
//! into one multi-line block and dropped the opening `import {` line. The
//! multi-line import merge was since reworked; these pin the behaviour so it
//! cannot regress: the block stays well formed and no specifier is lost.
use weave_core::merge::entity_merge;

const BODY: &str = "\n\nexport function keep() {\n    return 1\n}\n";

fn assert_well_formed(tag: &str, content: &str) {
    assert!(
        content.contains("import {"),
        "{tag}: opening `import {{` dropped:\n{content}"
    );
    for spec in ["type a", "type b", "type c"] {
        assert!(content.contains(spec), "{tag}: lost `{spec}`:\n{content}");
    }
    assert!(content.contains("Foo"), "{tag}: lost `Foo`:\n{content}");
    // Every multi-line `import {` block must be closed by a `} from`.
    assert!(
        content.matches("import {").count() <= content.matches("} from").count(),
        "{tag}: an `import {{` block was left unclosed:\n{content}"
    );
}

#[test]
fn ts_import_consolidation_keeps_the_block_well_formed() {
    let two = format!(
        "import type {{ Foo }} from \"./foo\"\nimport {{\n    type a,\n    type b,\n    type c,\n}} from \"./foo\"{BODY}"
    );
    let consolidated = format!(
        "import {{\n    type Foo,\n    type a,\n    type b,\n    type c,\n}} from \"./foo\"{BODY}"
    );
    assert_well_formed(
        "theirs-consolidates",
        &entity_merge(&two, &two, &consolidated, "x.ts").content,
    );
    assert_well_formed(
        "ours-consolidates",
        &entity_merge(&two, &consolidated, &two, "x.ts").content,
    );
    assert_well_formed(
        "both-consolidate",
        &entity_merge(&two, &consolidated, &consolidated, "x.ts").content,
    );
    assert_well_formed(
        "base-consolidated-theirs-splits",
        &entity_merge(&consolidated, &consolidated, &two, "x.ts").content,
    );
}
