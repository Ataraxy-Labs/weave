//! The deterministic rules, at the public boundary: a conflict whose answer
//! does not depend on intent is settled, the answer does not depend on which
//! side is called ours, and a conflict about intent stays a conflict.

use weave_core::{entity_merge, ResolutionStrategy};

fn settled_by(base: &str, ours: &str, theirs: &str, path: &str) -> (String, Vec<String>) {
    let r = entity_merge(base, ours, theirs, path);
    assert!(
        r.is_clean(),
        "expected a settled merge, got:\n{}",
        r.content
    );
    let rules = r
        .audit
        .iter()
        .find_map(|a| match &a.resolution {
            ResolutionStrategy::RuleSettled { rules } => Some(rules.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let swapped = entity_merge(base, theirs, ours, path);
    assert!(swapped.is_clean(), "side-swapped merge conflicted");
    assert_eq!(
        swapped.content, r.content,
        "the answer must not depend on which side is called ours"
    );
    (r.content, rules)
}

#[test]
fn a_reformat_against_an_edit_is_the_edit() {
    let base = "export function f(a: number, b: number): number {\n  return a + b;\n}\n";
    let ours = "export function f(a: number,b: number): number {\n    return a+b;\n}\n";
    let theirs = "export function f(a: number, b: number): number {\n  return a - b;\n}\n";
    let (content, rules) = settled_by(base, ours, theirs, "m.ts");
    assert_eq!(content, theirs);
    assert!(rules.iter().any(|r| r.starts_with("D6")), "{rules:?}");
}

#[test]
fn a_comment_is_not_a_reformat() {
    let base = "export function f(a: number, b: number): number {\n  return a + b;\n}\n";
    let ours = "export function f(a: number, b: number): number {\n  // sums\n  return a + b;\n}\n";
    let theirs = "export function f(a: number, b: number): number {\n  return a - b;\n}\n";
    let r = entity_merge(base, ours, theirs, "m.ts");
    assert!(!r.is_clean(), "{}", r.content);
}

#[test]
fn two_additions_to_an_ignore_file_are_a_union() {
    let base = "target/\n*.log\n";
    let ours = "target/\n*.log\n.env\n";
    let theirs = "target/\n*.log\nnode_modules/\n";
    let (content, rules) = settled_by(base, ours, theirs, ".gitignore");
    assert_eq!(content, "target/\n*.log\n.env\nnode_modules/\n");
    assert_eq!(rules, vec!["D3 set union"]);
}

#[test]
fn two_imports_of_one_name_from_different_modules_stay_a_conflict() {
    let base = "import os\n\n\ndef f():\n    return os.sep\n";
    let ours = "import os\nfrom a import thing\n\n\ndef f():\n    return os.sep\n";
    let theirs = "import os\nfrom b import thing\n\n\ndef f():\n    return os.sep\n";
    assert!(!entity_merge(base, ours, theirs, "m.py").is_clean());
}

#[test]
fn creations_equal_up_to_layout_are_one_file() {
    let ours = "public class A {\n  int x;\n}\n";
    let theirs = "public class A {\n    int x;\n}\n";
    let (content, rules) = settled_by("", ours, theirs, "A.java");
    assert!(content == ours || content == theirs);
    assert!(rules.iter().any(|r| r.starts_with("D4")), "{rules:?}");
}

#[test]
fn a_statement_deleted_against_its_edit_stays_a_conflict() {
    // Line-level modify/delete: one side removes a statement, the other fixes
    // it. Neither contains the other.
    let base = "class K {\n  void f() {\n    put(OLD_NAME, false);\n    go();\n  }\n}\n";
    let ours = "class K {\n  void f() {\n    go();\n  }\n}\n";
    let theirs = "class K {\n  void f() {\n    put(NEW_NAME, false);\n    go();\n  }\n}\n";
    assert!(!entity_merge(base, ours, theirs, "K.java").is_clean());
}

#[test]
fn a_declaration_deleted_against_its_edit_stays_a_conflict() {
    let base = "def a():\n    return 1\n\n\ndef b():\n    return 2\n";
    let ours = "def b():\n    return 2\n";
    let theirs = "def a():\n    return 10\n\n\ndef b():\n    return 2\n";
    let r = entity_merge(base, ours, theirs, "m.py");
    assert!(!r.is_clean(), "{}", r.content);
}

#[test]
fn a_file_without_a_final_newline_is_read_as_its_lines() {
    let base = "target/\n*.log";
    let ours = "target/\n*.log\n.env";
    let theirs = "target/\n*.log\nnode_modules/";
    let (content, _) = settled_by(base, ours, theirs, ".gitignore");
    assert_eq!(content, "target/\n*.log\n.env\nnode_modules/");
}

// ---------------------------------------------------------------------------
// D3 changelog union — only where the file's `weave-set` declaration asks
// ---------------------------------------------------------------------------

const CHANGELOG_BASE: &str =
    "# Changelog\n\n## Unreleased\n\n### Added\n\n## 1.0.0\n\n- first release\n";

fn with_entries(entries: &str) -> String {
    CHANGELOG_BASE.replace("### Added\n\n", &format!("### Added\n\n{entries}\n"))
}

fn declare_unreleased(_: &str) -> Option<String> {
    Some("Unreleased".to_string())
}

fn declare_fixed(_: &str) -> Option<String> {
    Some("Fixed".to_string())
}

/// `settled_by`, under a host whose `weave-set` reader is `reader`.
fn settled_declared(
    reader: weave_core::host::AttributeReader,
    base: &str,
    ours: &str,
    theirs: &str,
    path: &str,
) -> (String, Vec<String>) {
    let r = with(Some(reader), base, ours, theirs, path);
    assert!(
        r.is_clean(),
        "expected a settled merge, got:\n{}",
        r.content
    );
    let swapped = with(Some(reader), base, theirs, ours, path);
    assert!(swapped.is_clean(), "side-swapped merge conflicted");
    assert_eq!(
        swapped.content, r.content,
        "the answer must not depend on which side is called ours"
    );
    let rules = r
        .audit
        .iter()
        .find_map(|a| match &a.resolution {
            ResolutionStrategy::RuleSettled { rules } => Some(rules.clone()),
            _ => None,
        })
        .unwrap_or_default();
    (r.content, rules)
}

/// Two differently worded entries can describe one change: textual
/// distinctness is not distinct intent, so by default the entries two sides
/// added at one point of a changelog section are a conflict.
#[test]
fn changelog_entries_are_a_conflict_by_default() {
    let ours = with_entries("- `walk`: new subcommand.\n");
    let reworded = with_entries("- `walk`: a new subcommand that walks the tree.\n");
    assert!(!entity_merge(CHANGELOG_BASE, &ours, &reworded, "CHANGELOG.md").is_clean());
    let theirs = with_entries("- `count`: new subcommand.\n");
    assert!(!entity_merge(CHANGELOG_BASE, &ours, &theirs, "CHANGELOG.md").is_clean());
    // A declaration naming another section does not cover this one.
    let r = with(
        Some(declare_fixed),
        CHANGELOG_BASE,
        &ours,
        &theirs,
        "CHANGELOG.md",
    );
    assert!(!r.is_clean(), "{}", r.content);
}

#[test]
fn two_entries_added_to_one_declared_section_are_a_union() {
    let ours = with_entries("- `walk`: new subcommand.\n");
    let theirs = with_entries("- `count`: new subcommand.\n");
    let readers: [weave_core::host::AttributeReader; 2] = [declare_all, declare_unreleased];
    for reader in readers {
        let (content, rules) =
            settled_declared(reader, CHANGELOG_BASE, &ours, &theirs, "CHANGELOG.md");
        assert_eq!(
            content,
            with_entries("- `count`: new subcommand.\n- `walk`: new subcommand.\n")
        );
        assert_eq!(rules, vec!["D3 changelog union"]);
    }
}

#[test]
fn a_run_of_entries_stays_together_and_a_shared_entry_is_written_once() {
    let ours = with_entries("- zeta one\n  continued\n- shared\n- omega\n");
    let theirs = with_entries("- alpha\n- shared\n");
    let (content, _) = settled_declared(declare_all, CHANGELOG_BASE, &ours, &theirs, "CHANGES.md");
    assert_eq!(
        content,
        with_entries("- alpha\n- shared\n- zeta one\n  continued\n- omega\n")
    );
    // An entry both wrote last is agreement, and stays last.
    let ours = with_entries("- zeta\n- shared\n");
    let (content, _) = settled_declared(declare_all, CHANGELOG_BASE, &ours, &theirs, "CHANGES.md");
    assert_eq!(content, with_entries("- alpha\n- zeta\n- shared\n"));
}

#[test]
fn a_section_both_sides_created_holds_both_entries() {
    let base = "# Changelog\n\n## 1.0.0\n\n- first release\n";
    let section = |e: &str| {
        base.replace(
            "## 1.0.0",
            &format!("## Unreleased\n\n### Added\n\n{e}\n## 1.0.0"),
        )
    };
    let (ours, theirs) = (section("- `walk`\n"), section("- `count`\n"));
    assert!(!entity_merge(base, &ours, &theirs, "CHANGELOG.md").is_clean());
    let (content, _) = settled_declared(declare_all, base, &ours, &theirs, "CHANGELOG.md");
    assert_eq!(content, section("- `count`\n- `walk`\n"));
}

#[test]
fn entries_added_to_a_list_that_is_not_a_changelog_stay_a_conflict() {
    let r = entity_merge(
        CHANGELOG_BASE,
        &with_entries("- one\n"),
        &with_entries("- two\n"),
        "README.md",
    );
    assert!(!r.is_clean(), "{}", r.content);
}

// ---------------------------------------------------------------------------
// D3 data key union
// ---------------------------------------------------------------------------

#[test]
fn toml_keys_added_after_one_line_are_a_union() {
    let base =
        "[package]\nname = \"demo\"\n\n[dependencies]\nalpha = \"1\"\n\n[features]\ndefault = []\n";
    let ours = base.replace(
        "alpha = \"1\"\n",
        "alpha = \"1\"\nzeta = { version = \"2\", features = [\"x\"] } # pinned\n",
    );
    let theirs = base.replace("alpha = \"1\"\n", "alpha = \"1\"\nbeta = \"3\"\n");
    let (content, rules) = settled_by(base, &ours, &theirs, "Cargo.toml");
    assert_eq!(
        content,
        base.replace(
            "alpha = \"1\"\n",
            "alpha = \"1\"\nbeta = \"3\"\nzeta = { version = \"2\", features = [\"x\"] } # pinned\n"
        )
    );
    assert_eq!(rules, vec!["D3 data key union"]);
}

#[test]
fn json_keys_appended_to_one_object_get_their_comma() {
    let base = "{\n  \"name\": \"demo\",\n  \"scripts\": {\n    \"build\": \"make\"\n  }\n}\n";
    let ours = base.replace("\"make\"\n", "\"make\",\n    \"test\": \"make test\"\n");
    let theirs = base.replace("\"make\"\n", "\"make\",\n    \"lint\": \"make lint\"\n");
    let (content, _) = settled_by(base, &ours, &theirs, "package.json");
    assert_eq!(
        content,
        base.replace(
            "\"make\"\n",
            "\"make\",\n    \"lint\": \"make lint\",\n    \"test\": \"make test\"\n"
        )
    );
}

#[test]
fn toml_dependency_array_items_are_a_union() {
    let base = "[project]\nname = \"demo\"\ndependencies = [\n    \"alpha>=1\"\n]\n";
    let ours = base.replace("\"alpha>=1\"\n", "\"alpha>=1\",\n    \"zeta>=2\"\n");
    let theirs = base.replace("\"alpha>=1\"\n", "\"alpha>=1\",\n    \"beta>=3\"\n");
    let (content, _) = settled_by(base, &ours, &theirs, "pyproject.toml");
    assert_eq!(
        content,
        base.replace(
            "\"alpha>=1\"\n",
            "\"alpha>=1\",\n    \"beta>=3\",\n    \"zeta>=2\"\n"
        )
    );
}

#[test]
fn yaml_keys_added_to_one_mapping_are_a_union() {
    let base = "service:\n  name: demo\n  port: 80\nother: 1\n";
    let ours = base.replace("  port: 80\n", "  port: 80\n  replicas: 2 # scaled\n");
    let theirs = base.replace("  port: 80\n", "  port: 80\n  image: demo:1\n");
    let (content, _) = settled_by(base, &ours, &theirs, "deploy.yaml");
    assert_eq!(
        content,
        base.replace(
            "  port: 80\n",
            "  port: 80\n  image: demo:1\n  replicas: 2 # scaled\n"
        )
    );
}

#[test]
fn a_side_whose_value_already_says_the_merge_is_the_answer() {
    // Theirs added an item on one line; ours added the same item while
    // re-wrapping the array, and a table besides.
    let base = "[workspace]\nmembers = [\"a\", \"c\"]\n\n[workspace.dependencies]\nx = \"1\"\n";
    let theirs = base.replace("[\"a\", \"c\"]", "[\"a\", \"b\", \"c\"]");
    let ours = base.replace(
        "[\"a\", \"c\"]\n",
        "[\n    \"a\",\n    \"b\",\n    \"c\",\n]\n\n[workspace.package]\nlicense = \"MIT\"\n",
    );
    let (content, rules) = settled_by(base, &ours, &theirs, "Cargo.toml");
    assert_eq!(content, ours);
    assert!(rules.iter().any(|r| r.starts_with("D1 data")), "{rules:?}");
}

#[test]
fn one_key_given_two_values_stays_a_conflict() {
    let base = "[dependencies]\nalpha = \"1\"\n";
    let ours = "[dependencies]\nalpha = \"1\"\nbeta = \"2\"\n";
    let theirs = "[dependencies]\nalpha = \"1\"\nbeta = \"3\"\n";
    assert!(!entity_merge(base, ours, theirs, "Cargo.toml").is_clean());
}

#[test]
fn one_package_required_twice_in_two_spellings_stays_a_conflict() {
    let base = "[project]\ndependencies = [\n]\n";
    let ours = "[project]\ndependencies = [\n    \"demo-lib>=1\"\n]\n";
    let theirs = "[project]\ndependencies = [\n    \"Demo_Lib>=1\"\n]\n";
    assert!(!entity_merge(base, ours, theirs, "pyproject.toml").is_clean());
}

#[test]
fn a_comment_the_other_side_wrote_is_not_carried_by_value() {
    let base = "[workspace]\nmembers = [\"a\", \"c\"]\n\n[workspace.dependencies]\nx = \"1\"\n";
    let theirs = base.replace(
        "[\"a\", \"c\"]",
        "[\"a\", \"b\", \"c\"] # b is the new crate",
    );
    let ours = base.replace(
        "[\"a\", \"c\"]\n",
        "[\n    \"a\",\n    \"b\",\n    \"c\",\n]\n\n[workspace.package]\nlicense = \"MIT\"\n",
    );
    assert!(!entity_merge(base, &ours, &theirs, "Cargo.toml").is_clean());
    assert!(!entity_merge(base, &theirs, &ours, "Cargo.toml").is_clean());
}

#[test]
fn a_key_the_other_side_deleted_is_not_carried_by_value() {
    let base = "[dependencies]\nalpha = \"1\"\nbeta = \"1\"\n";
    let ours = "[dependencies]\nalpha = \"1\"\nbeta = \"1\"\ngamma = \"1\"\n";
    let theirs = "[dependencies]\nalpha = \"1\"\n";
    for (o, t) in [(ours, theirs), (theirs, ours)] {
        let r = entity_merge(base, o, t, "Cargo.toml");
        assert_ne!(r.content, ours, "the deletion must not be dropped");
        if r.is_clean() {
            assert!(!r.content.contains("beta") && r.content.contains("gamma"));
        }
    }
}

// ---------------------------------------------------------------------------
// D3 container insertion union
// ---------------------------------------------------------------------------

const UNION: &str = "D3 container insertion union";

#[test]
fn entries_added_to_one_registry_dict_are_a_union() {
    let base = "HANDLERS = {\n    \"alpha\": run_alpha,\n}\n";
    let ours = "HANDLERS = {\n    \"alpha\": run_alpha,\n    \"zeta\": run_zeta,\n}\n";
    let theirs = "HANDLERS = {\n    \"alpha\": run_alpha,\n    \"beta\": run_beta,\n}\n";
    let (content, rules) = settled_by(base, ours, theirs, "registry.py");
    assert_eq!(
        content,
        "HANDLERS = {\n    \"alpha\": run_alpha,\n    \"beta\": run_beta,\n    \"zeta\": run_zeta,\n}\n"
    );
    assert_eq!(rules, vec![UNION]);
}

#[test]
fn an_object_literal_without_trailing_commas_gets_its_comma() {
    let base = "const handlers = {\n  alpha: runAlpha\n};\nregister(handlers);\n";
    let ours = "const handlers = {\n  alpha: runAlpha,\n  zeta: runZeta\n};\nregister(handlers);\n";
    let theirs =
        "const handlers = {\n  alpha: runAlpha,\n  beta: runBeta\n};\nregister(handlers);\n";
    let (content, _) = settled_by(base, ours, theirs, "handlers.js");
    assert_eq!(
        content,
        "const handlers = {\n  alpha: runAlpha,\n  beta: runBeta,\n  zeta: runZeta\n};\nregister(handlers);\n"
    );
}

#[test]
fn dunder_all_and_a_parenthesised_import_are_unions() {
    let base = "from .core import (\n    alpha,\n)\n\n__all__ = [\n    \"alpha\",\n]\n";
    let ours = "from .core import (\n    alpha,\n    zeta,\n)\n\n__all__ = [\n    \"alpha\",\n    \"zeta\",\n]\n";
    let theirs = "from .core import (\n    alpha,\n    beta,\n)\n\n__all__ = [\n    \"alpha\",\n    \"beta\",\n]\n";
    let (content, rules) = settled_by(base, ours, theirs, "pkg/__init__.py");
    assert_eq!(
        content,
        "from .core import (\n    alpha,\n    beta,\n    zeta,\n)\n\n__all__ = [\n    \"alpha\",\n    \"beta\",\n    \"zeta\",\n]\n"
    );
    assert_eq!(rules, vec![UNION]);
}

#[test]
fn keys_added_to_one_ini_section_are_a_union() {
    let base = "[flake8]\nmax-line-length = 100\n\n[mypy]\nstrict = true\n";
    let ours = "[flake8]\nmax-line-length = 100\nselect = E,W\n\n[mypy]\nstrict = true\n";
    let theirs = "[flake8]\nmax-line-length = 100\nexclude =\n    build\n    dist\n\n[mypy]\nstrict = true\n";
    let (content, rules) = settled_by(base, ours, theirs, "tox.ini");
    assert_eq!(
        content,
        "[flake8]\nmax-line-length = 100\nexclude =\n    build\n    dist\nselect = E,W\n\n[mypy]\nstrict = true\n"
    );
    assert_eq!(rules, vec![UNION]);
}

#[test]
fn a_commented_group_of_entries_stays_together() {
    let base = "EVENTS = {\n    \"open\": 1,\n}\n";
    let ours =
        "EVENTS = {\n    \"open\": 1,\n    # billing\n    \"charge\": 2,\n    \"refund\": 3,\n}\n";
    let theirs = "EVENTS = {\n    \"open\": 1,\n    \"close\": 4,\n}\n";
    let (content, _) = settled_by(base, ours, theirs, "events.py");
    assert_eq!(
        content,
        "EVENTS = {\n    \"open\": 1,\n    # billing\n    \"charge\": 2,\n    \"refund\": 3,\n    \"close\": 4,\n}\n"
    );
}

#[test]
fn a_registry_entry_given_two_values_stays_a_conflict() {
    let base = "HANDLERS = {\n    \"alpha\": run_alpha,\n}\n";
    let ours = "HANDLERS = {\n    \"alpha\": run_alpha,\n    \"beta\": run_beta,\n}\n";
    let theirs = "HANDLERS = {\n    \"alpha\": run_alpha,\n    \"beta\": run_other,\n}\n";
    assert!(!entity_merge(base, ours, theirs, "registry.py").is_clean());
}

#[test]
fn an_entry_edited_beside_the_insertions_stays_a_conflict() {
    let base = "HANDLERS = {\n    \"alpha\": run_alpha,\n}\n";
    let ours = "HANDLERS = {\n    \"alpha\": run_alpha_v2,\n    \"zeta\": run_zeta,\n}\n";
    let theirs = "HANDLERS = {\n    \"alpha\": run_alpha,\n    \"beta\": run_beta,\n}\n";
    assert!(!entity_merge(base, ours, theirs, "registry.py").is_clean());
    // deleted instead of edited
    let ours = "HANDLERS = {\n    \"zeta\": run_zeta,\n}\n";
    assert!(!entity_merge(base, ours, theirs, "registry.py").is_clean());
}

#[test]
fn an_entry_moved_to_the_insertion_point_stays_a_conflict() {
    let base = "H = {\n    \"alpha\": 1,\n    \"gamma\": 3,\n    \"omega\": 9,\n}\n";
    let ours = "H = {\n    \"gamma\": 3,\n    \"omega\": 9,\n    \"alpha\": 1,\n}\n";
    let theirs =
        "H = {\n    \"alpha\": 1,\n    \"gamma\": 3,\n    \"omega\": 9,\n    \"beta\": 2,\n}\n";
    assert!(!entity_merge(base, ours, theirs, "h.py").is_clean());
    assert!(!entity_merge(base, theirs, ours, "h.py").is_clean());
}

#[test]
fn keys_that_are_not_constants_stay_a_conflict() {
    let base = "H = {\n    \"alpha\": 1,\n}\n";
    let ours = "H = {\n    \"alpha\": 1,\n    KEY_A: 2,\n}\n";
    let theirs = "H = {\n    \"alpha\": 1,\n    KEY_B: 3,\n}\n";
    assert!(!entity_merge(base, ours, theirs, "h.py").is_clean());
    let ours = "H = {\n    \"alpha\": 1,\n    **extra,\n}\n";
    let theirs = "H = {\n    \"alpha\": 1,\n    \"beta\": 3,\n}\n";
    assert!(!entity_merge(base, ours, theirs, "h.py").is_clean());
}

// Order-sensitive containers: a conflict unless declared a set.

fn declare_all(_: &str) -> Option<String> {
    Some("set".to_string())
}

fn declare_build(_: &str) -> Option<String> {
    Some("build,console_scripts".to_string())
}

fn with(
    reader: Option<weave_core::host::AttributeReader>,
    base: &str,
    ours: &str,
    theirs: &str,
    path: &str,
) -> weave_core::MergeResult {
    let host = weave_core::host::Host {
        set_attribute: reader,
        ..Default::default()
    };
    weave_core::entity_merge_fmt(
        base,
        ours,
        theirs,
        path,
        &weave_core::MarkerFormat::default(),
        &host,
    )
}

#[test]
fn statements_inserted_at_one_point_are_a_union_only_by_declaration() {
    let base = "def build(sub):\n    sub.add_parser(\"alpha\")\n    return sub\n";
    let ours = "def build(sub):\n    sub.add_parser(\"alpha\")\n    sub.add_parser(\"zeta\")\n    return sub\n";
    let theirs = "def build(sub):\n    sub.add_parser(\"alpha\")\n    sub.add_parser(\"beta\")\n    return sub\n";
    assert!(!with(None, base, ours, theirs, "cli.py").is_clean());
    for reader in [declare_all, declare_build] {
        let r = with(Some(reader), base, ours, theirs, "cli.py");
        let s = with(Some(reader), base, theirs, ours, "cli.py");
        assert!(r.is_clean(), "{}", r.content);
        assert_eq!(r.content, s.content);
        assert_eq!(
            r.content,
            "def build(sub):\n    sub.add_parser(\"alpha\")\n    sub.add_parser(\"beta\")\n    sub.add_parser(\"zeta\")\n    return sub\n"
        );
    }
}

#[test]
fn a_declaration_names_its_container() {
    // `build` is declared; the list in `other` is not.
    let base = "def other():\n    return [\n        \"alpha\",\n    ]\n";
    let ours = "def other():\n    return [\n        \"alpha\",\n        \"zeta\",\n    ]\n";
    let theirs = "def other():\n    return [\n        \"alpha\",\n        \"beta\",\n    ]\n";
    assert!(!with(Some(declare_build), base, ours, theirs, "m.py").is_clean());
    assert!(with(Some(declare_all), base, ours, theirs, "m.py").is_clean());
}

#[test]
fn match_arms_parametrize_lists_and_value_lines_stay_conflicts_by_default() {
    let cases: [(&str, &str, &str, &str); 3] = [
        (
            "def f(x):\n    match x:\n        case 1:\n            return 1\n        case _:\n            return 0\n",
            "def f(x):\n    match x:\n        case 1:\n            return 1\n        case 2:\n            return 2\n        case _:\n            return 0\n",
            "def f(x):\n    match x:\n        case 1:\n            return 1\n        case 3:\n            return 3\n        case _:\n            return 0\n",
            "m.py",
        ),
        (
            "@pytest.mark.parametrize(\"x\", [\n    \"alpha\",\n])\ndef test_x(x):\n    assert x\n",
            "@pytest.mark.parametrize(\"x\", [\n    \"alpha\",\n    \"beta\",\n])\ndef test_x(x):\n    assert x\n",
            "@pytest.mark.parametrize(\"x\", [\n    \"alpha\",\n    \"gamma\",\n])\ndef test_x(x):\n    assert x\n",
            "test_m.py",
        ),
        (
            "[options.entry_points]\nconsole_scripts =\n    alpha = p.alpha:main\n",
            "[options.entry_points]\nconsole_scripts =\n    alpha = p.alpha:main\n    beta = p.beta:main\n",
            "[options.entry_points]\nconsole_scripts =\n    alpha = p.alpha:main\n    gamma = p.gamma:main\n",
            "setup.cfg",
        ),
    ];
    for (base, ours, theirs, path) in cases {
        assert!(!with(None, base, ours, theirs, path).is_clean(), "{path}");
        let r = with(Some(declare_all), base, ours, theirs, path);
        let s = with(Some(declare_all), base, theirs, ours, path);
        assert!(r.is_clean(), "{path}:\n{}", r.content);
        assert_eq!(r.content, s.content, "{path}");
    }
}

#[test]
fn a_yaml_sequence_is_a_union_only_by_declaration() {
    let base = "- id: alpha\n  entry: alpha\n";
    let ours = "- id: alpha\n  entry: alpha\n- id: zeta\n  entry: zeta\n";
    let theirs = "- id: alpha\n  entry: alpha\n- id: beta\n  entry: beta\n";
    assert!(!with(None, base, ours, theirs, "hooks.yaml").is_clean());
    let r = with(Some(declare_all), base, ours, theirs, "hooks.yaml");
    let s = with(Some(declare_all), base, theirs, ours, "hooks.yaml");
    assert!(r.is_clean(), "{}", r.content);
    assert_eq!(r.content, s.content);
    assert_eq!(
        r.content,
        "- id: alpha\n  entry: alpha\n- id: beta\n  entry: beta\n- id: zeta\n  entry: zeta\n"
    );
}
