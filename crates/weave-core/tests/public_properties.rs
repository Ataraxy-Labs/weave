//! A small, public set of behaviors weave's merge is expected to have.
//!
//! These are plain properties anyone merging text should be able to check for
//! themselves: doing nothing twice is the same as doing it once, a side that
//! didn't change gets out of the way, non-overlapping changes both survive,
//! and a real contradiction is reported rather than silently guessed at.
//!
//! A larger, private suite checks many more cases before every release; this
//! file is the part of it that ships in the open.

use weave_core::entity_merge;

fn base_py() -> &'static str {
    "def add(a, b):\n    return a + b\n"
}

// ---------------------------------------------------------------------------
// Merging a settled result with itself changes nothing further.
// ---------------------------------------------------------------------------

#[test]
fn merging_a_settled_result_with_itself_changes_nothing_further() {
    let base = base_py();
    let changed = "def add(a, b):\n    return a + b + 1\n";

    let result = entity_merge(base, changed, changed, "math.py");
    assert!(result.is_clean());
    assert_eq!(result.content, changed);

    // The result is now a settled state. Merging it with itself again must
    // land on exactly the same content, not drift.
    let again = entity_merge(&result.content, &result.content, &result.content, "math.py");
    assert!(again.is_clean());
    assert_eq!(again.content, result.content);
}

// ---------------------------------------------------------------------------
// A side that made no change is the identity: the other side wins cleanly.
// ---------------------------------------------------------------------------

#[test]
fn a_side_with_no_change_is_the_identity_and_the_other_side_wins_cleanly() {
    let base = base_py();
    let changed = "def add(a, b):\n    return a + b + 1\n";

    // "ours" changed, "theirs" is still exactly the base.
    let result = entity_merge(base, changed, base, "math.py");
    assert!(result.is_clean());
    assert_eq!(result.content, changed);

    // "theirs" changed, "ours" is still exactly the base.
    let result = entity_merge(base, base, changed, "math.py");
    assert!(result.is_clean());
    assert_eq!(result.content, changed);
}

// ---------------------------------------------------------------------------
// Two sides adding different, non-overlapping functions merge cleanly and
// keep both additions.
// ---------------------------------------------------------------------------

#[test]
fn two_sides_adding_different_functions_merge_cleanly_and_keep_both() {
    let base = base_py();
    let ours = format!("{base}\ndef sub(a, b):\n    return a - b\n");
    let theirs = format!("{base}\ndef mul(a, b):\n    return a * b\n");

    let result = entity_merge(base, &ours, &theirs, "math.py");
    assert!(
        result.is_clean(),
        "non-overlapping additions should not conflict"
    );
    assert!(result.content.contains("def add"));
    assert!(result.content.contains("def sub"));
    assert!(result.content.contains("def mul"));
}

// ---------------------------------------------------------------------------
// Two sides editing the same function in contradictory ways is reported as
// a conflict, not resolved by silently picking a side.
// ---------------------------------------------------------------------------

#[test]
fn two_sides_editing_the_same_function_differently_is_reported_as_a_conflict() {
    let base = base_py();
    let ours = "def add(a, b):\n    return a + b + 1\n";
    let theirs = "def add(a, b):\n    return a + b - 1\n";

    let result = entity_merge(base, ours, theirs, "math.py");
    assert!(
        !result.is_clean(),
        "a genuine contradiction must not be resolved silently"
    );
    assert!(!result.conflicts.is_empty());
}

// ---------------------------------------------------------------------------
// A merge over a many-function file still returns an answer.
// ---------------------------------------------------------------------------

#[test]
fn a_merge_over_many_functions_completes_and_keeps_every_addition() {
    let mut base = String::new();
    let mut ours = String::new();
    let mut theirs = String::new();
    for i in 0..200 {
        let line = format!("def f{i}():\n    return {i}\n\n");
        base.push_str(&line);
        ours.push_str(&line);
        theirs.push_str(&line);
    }
    ours.push_str("def extra_ours():\n    return 1\n");
    theirs.push_str("def extra_theirs():\n    return 2\n");

    let result = entity_merge(&base, &ours, &theirs, "big.py");
    assert!(result.is_clean());
    assert!(result.content.contains("def extra_ours"));
    assert!(result.content.contains("def extra_theirs"));
}
