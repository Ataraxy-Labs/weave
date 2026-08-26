//! A side with no top-level entities keys its entire content as one
//! `file_only` interstitial region. That text must survive the merge whether
//! or not any entities are rendered around it — dropping it turns "one side
//! is plain statements" into silent data loss with exit 0 (#148).

use std::fs;
use std::process::Command;

#[test]
fn top_level_statements_survive_when_one_side_has_no_entities() {
    // base: an entity plus a shared top-level call.
    // ours: adds an entity, keeps everything else.
    // theirs: deletes the entity — the file is now only the call, so this
    //         side has no entities at all and becomes a `file_only` region.
    // The correct merge keeps ours' new entity, honors theirs' deletion, and
    // keeps the call every version wrote.
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let base = dir.join("base.ts");
    let current = dir.join("A.ts");
    let theirs = dir.join("theirs.ts");
    fs::write(&base, "let a=0\nb()\n").unwrap();
    fs::write(&current, "let a=0\nlet d=0\nb()\n").unwrap();
    fs::write(&theirs, "b()\n").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_weave-driver"))
        .args([
            base.to_str().unwrap(),
            current.to_str().unwrap(),
            theirs.to_str().unwrap(),
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

    let merged = fs::read_to_string(&current).unwrap();
    assert!(
        merged.contains("b()"),
        "top-level call present on all three sides was dropped: {merged:?}"
    );
    assert!(
        merged.contains("let d=0"),
        "ours' added entity missing: {merged:?}"
    );
    assert!(
        !merged.contains("let a=0"),
        "theirs' deletion not honored: {merged:?}"
    );
}
