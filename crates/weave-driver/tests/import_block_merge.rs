//! Multi-line TS import statements through the interstitial import merge
//! (#24). The reported failure was a merged import block coming back with its
//! `import {` opener line dropped — the import-union rebuild predates the
//! ladder's no-loss backstops (`keeps_everything` / `keeps_additions`, 0.5.0),
//! which now refuse any rung that loses a line the versions wrote. This pins
//! the issue's shape: folding a one-line import into an existing multi-line
//! block must produce one well-formed block, never a decapitated one.

use std::fs;
use std::process::Command;

fn run_driver(base: &str, ours: &str, theirs: &str) -> String {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let base_path = dir.join("base.ts");
    let current = dir.join("A.ts");
    let theirs_path = dir.join("theirs.ts");
    fs::write(&base_path, base).unwrap();
    fs::write(&current, ours).unwrap();
    fs::write(&theirs_path, theirs).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_weave-driver"))
        .args([
            base_path.to_str().unwrap(),
            current.to_str().unwrap(),
            theirs_path.to_str().unwrap(),
            "7",
            "f.ts",
        ])
        .output()
        .expect("failed to run weave-driver");
    assert!(
        out.status.success(),
        "driver failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::read_to_string(&current).unwrap()
}

const BASE: &str = "\
import type { Foo } from \"./foo\"
import {
    type a,
    type b,
    type c,
} from \"./foo\"

export function f(): number { return 1 }
";

const THEIRS_FOLDED: &str = "\
import {
    type Foo,
    type a,
    type b,
    type c,
} from \"./foo\"

export function f(): number { return 1 }
";

/// Every `import` opener in the output must be balanced: a line ending in `{`
/// eventually followed by a `} from` closer, and no closer without an opener.
fn assert_import_blocks_well_formed(merged: &str) {
    let mut open = false;
    for line in merged.lines() {
        let t = line.trim();
        if t.starts_with("import") && t.ends_with('{') {
            assert!(!open, "nested import opener in:\n{merged}");
            open = true;
        } else if t.starts_with("} from") {
            assert!(
                open,
                "`}} from` closer with no `import {{` opener — decapitated \
                 import block (#24) in:\n{merged}"
            );
            open = false;
        }
    }
    assert!(!open, "unclosed import block in:\n{merged}");
}

#[test]
fn folding_an_import_into_a_multiline_block_keeps_the_block_well_formed() {
    // ours adds an unrelated import; theirs folds the `Foo` type import into
    // the existing multi-line block. Both header edits must compose.
    let ours = "\
import type { Foo } from \"./foo\"
import {
    type a,
    type b,
    type c,
} from \"./foo\"
import { z } from \"./z\"

export function f(): number { return 1 }
";
    let merged = run_driver(BASE, ours, THEIRS_FOLDED);
    assert_import_blocks_well_formed(&merged);
    for needle in ["type Foo", "type a", "type b", "type c", "import { z }"] {
        assert!(merged.contains(needle), "{needle:?} missing from:\n{merged}");
    }
}

#[test]
fn members_added_on_both_sides_of_a_multiline_block_compose() {
    // ours adds a member inside the block; theirs folds `Foo` in. The merged
    // block must contain both additions and stay well formed.
    let ours = "\
import type { Foo } from \"./foo\"
import {
    type a,
    type b,
    type c,
    type d,
} from \"./foo\"

export function f(): number { return 1 }
";
    let merged = run_driver(BASE, ours, THEIRS_FOLDED);
    assert_import_blocks_well_formed(&merged);
    for needle in ["type Foo", "type a", "type b", "type c", "type d"] {
        assert!(merged.contains(needle), "{needle:?} missing from:\n{merged}");
    }
}
