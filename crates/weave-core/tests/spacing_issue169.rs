//! Regression coverage for #169: an empty gap is source layout, not missing data.

use pretty_assertions::assert_eq;
use proptest::prelude::*;
use weave_core::entity_merge;

const BASE: &str = "\
const PARAMS_VAR = js`params`
const SNIPPETS_VAR = js`snippets`
type Option = string
type Query = number

function first() { return 1 }

function second() { return 2 }

function third() { return 3 }

function fourth() { return 4 }
";

fn assert_merge(base: &str, ours: &str, theirs: &str, expected: &str, path: &str) {
    for (left, right) in [(ours, theirs), (theirs, ours)] {
        let result = entity_merge(base, left, right, path);
        assert!(result.is_clean(), "{path}: {:?}", result.conflicts);
        assert!(
            !result.audit.is_empty(),
            "{path}: exercise entity reconstruction, not a whole-file fast path"
        );
        assert_eq!(result.content, expected, "{path}: preserve source layout");
    }
}

fn assert_unrelated_edits(base: &str) {
    let ours = base.replace("return 1 }", "return 10 }");
    let theirs = base.replace("return 4 }", "return 40 }");
    let expected = ours.replace("return 4 }", "return 40 }");
    assert_merge(base, &ours, &theirs, &expected, "fixture.ts");
}

#[test]
fn unrelated_edits_preserve_adjacent_const_and_type_declarations() {
    assert_unrelated_edits(BASE);
}

#[test]
fn an_intentionally_deleted_blank_line_stays_deleted() {
    let base = BASE
        .replace("\nconst ", "\n\nconst ")
        .replace("\ntype ", "\n\ntype ");
    let ours = base.replace("params`\n\nconst", "params`\nconst");
    let theirs = base.replace("return 4 }", "return 40 }");
    let expected = ours.replace("return 4 }", "return 40 }");
    assert_merge(&base, &ours, &theirs, &expected, "fixture.ts");
}

#[test]
fn both_sides_can_delete_a_gap_while_editing_different_entities() {
    let base = BASE.replace("params`\nconst", "params`\n\nconst");
    let ours = BASE.replace("return 1 }", "return 10 }");
    let theirs = BASE.replace("return 4 }", "return 40 }");
    let expected = ours.replace("return 4 }", "return 40 }");
    assert_merge(&base, &ours, &theirs, &expected, "fixture.ts");
}

#[test]
fn mixed_gap_widths_and_comments_keep_their_own_bytes() {
    for comment in [
        "/** Options accepted by the loader. */",
        "// Keep this group together.",
    ] {
        let base = BASE
            .replace("type Option", &format!("{comment}\ntype Option"))
            .replace("\n\nfunction second", "\n\n\nfunction second")
            .replace("\n\nfunction third", "\n \t\nfunction third");
        assert_unrelated_edits(&base);
    }
}

#[test]
fn adjacent_declarations_stay_adjacent_with_crlf_and_bom() {
    for bom in ["", "\u{feff}"] {
        for newline in ["\n", "\r\n"] {
            assert_unrelated_edits(&format!("{bom}{}", BASE.replace('\n', newline)));
        }
    }
}

#[test]
fn additions_deletions_and_renames_preserve_compact_groups() {
    let theirs = BASE.replace("return 4 }", "return 40 }");
    for ours in [
        BASE.replace("type Query", "type Extra = boolean\ntype Query"),
        BASE.replace("type Option = string\n", ""),
        BASE.replace("type Option = string", "type Selection = string"),
    ] {
        let expected = ours.replace("return 4 }", "return 40 }");
        assert_merge(BASE, &ours, &theirs, &expected, "fixture.ts");
    }
}

#[test]
fn successive_two_sided_merges_do_not_reformat_untouched_declarations() {
    let mut base = BASE.to_string();
    let mut first = 1;
    let mut fourth = 4;
    for round in 0..8 {
        let next_first = 100 + round;
        let next_fourth = 400 + round;
        let ours = base.replace(
            &format!("function first() {{ return {first} }}"),
            &format!("function first() {{ return {next_first} }}"),
        );
        let old_fourth = format!("function fourth() {{ return {fourth} }}");
        let new_fourth = format!("function fourth() {{ return {next_fourth} }}");
        let theirs = base.replace(&old_fourth, &new_fourth);
        let expected = ours.replace(&old_fourth, &new_fourth);
        assert_merge(&base, &ours, &theirs, &expected, "fixture.ts");
        base = expected;
        first = next_first;
        fourth = next_fourth;
    }
}

#[test]
fn compact_function_groups_are_not_specific_to_typescript_declarations() {
    let fixtures = [
        ("fixture.js", "function fN() { return VALUE; }\n"),
        ("fixture.ts", "function fN(): number { return VALUE; }\n"),
        ("fixture.py", "def fN():\n    return VALUE\n"),
        ("fixture.rs", "fn fN() -> i32 { VALUE }\n"),
    ];
    for (path, template) in fixtures {
        let mut base = String::new();
        for index in 0..6 {
            if index > 1 {
                base.push('\n');
            }
            base.push_str(
                &template
                    .replace('N', &index.to_string())
                    .replace("VALUE", &(100 + index).to_string()),
            );
        }
        let ours = base.replace("100", "200");
        let theirs = base.replace("105", "305");
        let expected = ours.replace("105", "305");
        assert_merge(&base, &ours, &theirs, &expected, path);
    }
}

#[test]
fn genuinely_new_boundaries_still_inherit_the_file_spacing() {
    let base = "function first() { return 1 }\n\nfunction last() { return 2 }\n";
    let added_ours = "function alpha() { return 10 }\n";
    let added_theirs = "function beta() { return 20 }\n";
    let ours = format!("{added_ours}\n{base}");
    let theirs = format!("{added_theirs}\n{base}");
    let result = entity_merge(base, &ours, &theirs, "fixture.ts");
    assert!(result.is_clean(), "{:?}", result.conflicts);
    assert!(!result.audit.is_empty());
    assert!(
        result.content == format!("{added_ours}\n{added_theirs}\n{base}")
            || result.content == format!("{added_theirs}\n{added_ours}\n{base}"),
        "the two new declarations have no shared input boundary: {:?}",
        result.content
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn arbitrary_existing_gap_widths_survive_disjoint_edits(
        widths in prop::collection::vec(0usize..4, 7),
    ) {
        let declarations = [
            "const PARAMS_VAR = js`params`\n",
            "const SNIPPETS_VAR = js`snippets`\n",
            "type Option = string\n",
            "type Query = number\n",
            "function first() { return 1 }\n",
            "function second() { return 2 }\n",
            "function third() { return 3 }\n",
            "function fourth() { return 4 }\n",
        ];
        let mut base = String::new();
        for (index, declaration) in declarations.iter().enumerate() {
            if index > 0 {
                base.push_str(&"\n".repeat(widths[index - 1]));
            }
            base.push_str(declaration);
        }
        let ours = base.replace("return 1 }", "return 10 }");
        let theirs = base.replace("return 4 }", "return 40 }");
        let expected = ours.replace("return 4 }", "return 40 }");
        for (left, right) in [(&ours, &theirs), (&theirs, &ours)] {
            let result = entity_merge(&base, left, right, "fixture.ts");
            prop_assert!(result.is_clean());
            prop_assert!(!result.audit.is_empty());
            prop_assert_eq!(&result.content, &expected);
        }
    }
}
