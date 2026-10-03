//! Fail closed: when weave cannot justify a composition, the driver reports a
//! conflict instead of a clean merge.
//!
//! Every fixture below is synthetic and recreates one SHAPE of wrong-but-clean
//! merge: both sides edited the same import, the same function, the same line,
//! the same singular declaration, or rewrote the same document. Each one used
//! to come back with exit 0. The driver must now exit 1 and write markers.
//!
//! The controls at the bottom are merges that ARE provably disjoint, and must
//! stay clean: failing closed is only worth anything if it does not fail on
//! everything.

use std::fs;
use std::process::Command;

/// Run the driver as git would, on a file named `name`. Returns the exit code
/// and the merged bytes.
fn merge(name: &str, base: &str, ours: &str, theirs: &str) -> (i32, String) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let base_path = dir.join("base");
    let current = dir.join("current");
    let theirs_path = dir.join("theirs");
    fs::write(&base_path, base).unwrap();
    fs::write(&current, ours).unwrap();
    fs::write(&theirs_path, theirs).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_weave-driver"))
        .args([
            base_path.to_str().unwrap(),
            current.to_str().unwrap(),
            theirs_path.to_str().unwrap(),
            "7",
            name,
        ])
        .output()
        .expect("failed to run weave-driver");
    let code = out.status.code().expect("driver exited by signal");
    assert_ne!(
        code,
        2,
        "driver errored: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    (code, fs::read_to_string(&current).unwrap())
}

#[track_caller]
fn assert_conflict(name: &str, base: &str, ours: &str, theirs: &str) {
    let (code, merged) = merge(name, base, ours, theirs);
    assert_eq!(code, 1, "expected a conflict, got a clean merge:\n{merged}");
    assert!(
        merged.lines().any(|l| l.starts_with("<<<<<<<")),
        "a conflict must be written with markers:\n{merged}"
    );
}

#[track_caller]
fn assert_clean(name: &str, base: &str, ours: &str, theirs: &str) -> String {
    let (code, merged) = merge(name, base, ours, theirs);
    assert_eq!(code, 0, "expected a clean merge, got a conflict:\n{merged}");
    assert!(
        !merged.contains("<<<<<<<"),
        "clean merge carries markers:\n{merged}"
    );
    merged
}

// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

/// Both sides moved the same import to a different module. Keeping both binds
/// one name twice.
#[test]
fn same_import_repointed_by_both_sides() {
    let base = "import { loadWidgets } from \"./widgets\"\nimport { log } from \"./log\"\n\nexport function run() {\n  log(loadWidgets())\n}\n";
    let ours = "import { loadWidgets } from \"./widgets/v2\"\nimport { log } from \"./log\"\n\nexport function run() {\n  log(loadWidgets())\n}\n";
    let theirs = "import { loadWidgets } from \"./legacy/widgets\"\nimport { log } from \"./log\"\n\nexport function run() {\n  log(loadWidgets())\n}\n";
    assert_conflict("app.ts", base, ours, theirs);
}

/// One side turned an import into a `const … = require(…)`, the other edited
/// the import. Keeping both redeclares the name.
#[test]
fn import_replaced_by_const_on_one_side_edited_on_the_other() {
    let base = "import { menuItems } from \"./menu\"\n\nexport function render() {\n  return menuItems.length\n}\n";
    let ours = "const menuItems = require(\"./menu.generated\")\n\nexport function render() {\n  return menuItems.length\n}\n";
    let theirs = "import { menuItems, menuTitle } from \"./menu\"\n\nexport function render() {\n  return menuItems.length + menuTitle.length\n}\n";
    assert_conflict("render.js", base, ours, theirs);
}

/// Python: both sides edited the same `from … import` line differently.
/// Keeping both lines imports `parse` twice.
#[test]
fn same_python_import_line_edited_by_both_sides() {
    let base = "from shapes import parse\n\n\ndef area(s):\n    return parse(s).area()\n";
    let ours = "from shapes import parse, render\n\n\ndef area(s):\n    return parse(s).area()\n";
    let theirs =
        "from shapes import parse, validate\n\n\ndef area(s):\n    return parse(s).area()\n";
    let (code, merged) = merge("geo.py", base, ours, theirs);
    // A single composed line (`parse, render, validate`) would be a sound
    // union of distinct specifiers; two lines that both bind `parse` is not.
    if code == 0 {
        let binds_parse = merged
            .lines()
            .filter(|l| l.starts_with("from shapes import") && l.contains("parse"))
            .count();
        assert_eq!(
            binds_parse, 1,
            "`parse` imported twice in a clean merge:\n{merged}"
        );
    }
}

/// An import one side deleted must not come back because the other side
/// touched the lines around it.
#[test]
fn import_deleted_by_one_side_is_not_resurrected() {
    let base = "import { alpha } from \"./alpha\"\nimport { beta } from \"./beta\"\n\nexport function go() {\n  return alpha() + beta()\n}\n";
    let ours = "import { beta } from \"./beta\"\n\nexport function go() {\n  return beta()\n}\n";
    let theirs = "import { alpha } from \"./alpha\"\nimport { beta } from \"./beta\"\nimport { gamma } from \"./gamma\"\n\nexport function go() {\n  return alpha() + beta()\n}\n\nexport function more() {\n  return gamma()\n}\n";
    let (code, merged) = merge("go.ts", base, ours, theirs);
    if code == 0 {
        assert!(
            !merged.contains("import { alpha }"),
            "ours deleted the `alpha` import and nothing new uses it:\n{merged}"
        );
    }
}

/// Two Java files that each moved the class to a different package. A file
/// has one package declaration.
#[test]
fn both_sides_changed_the_java_package() {
    let base = "package org.sample.app;\n\nimport java.util.List;\n\npublic class Tool {\n    public int size(List<String> xs) { return xs.size(); }\n}\n";
    let ours = "package org.sample.core;\n\nimport java.util.List;\nimport java.util.Map;\n\npublic class Tool {\n    public int size(List<String> xs) { return xs.size(); }\n}\n";
    let theirs = "package org.sample.api;\n\nimport java.util.List;\nimport java.util.Set;\n\npublic class Tool {\n    public int size(List<String> xs) { return xs.size(); }\n}\n";
    assert_conflict("Tool.java", base, ours, theirs);
}

/// Kotlin: both sides imported the same simple name from different packages,
/// and both added the same enum entry.
#[test]
fn kotlin_same_name_imported_from_two_packages() {
    let base = "package org.sample\n\nimport org.sample.model.Quota\n\nenum class Kind {\n    SMALL,\n    LARGE,\n}\n\nfun limit(q: Quota): Int = q.max\n";
    let ours = "package org.sample\n\nimport org.sample.model.Quota\nimport org.sample.model.v2.Window\n\nenum class Kind {\n    SMALL,\n    LARGE,\n    WINDOW,\n}\n\nfun limit(q: Quota): Int = q.max\n";
    let theirs = "package org.sample\n\nimport org.sample.model.Quota\nimport org.sample.legacy.Window\n\nenum class Kind {\n    SMALL,\n    LARGE,\n    WINDOW,\n}\n\nfun limit(q: Quota): Int = q.max\n";
    assert_conflict("Limits.kt", base, ours, theirs);
}

/// Go: both sides edited the same import block. A clean result must at least
/// parse and carry no markers.
#[test]
fn go_import_block_edited_by_both_sides() {
    let base = "package store\n\nimport (\n\t\"fmt\"\n\t\"strings\"\n)\n\nfunc Key(a string) string {\n\treturn fmt.Sprintf(\"%s\", strings.ToLower(a))\n}\n";
    let ours = "package store\n\nimport (\n\t\"fmt\"\n\t\"strconv\"\n\t\"strings\"\n)\n\nfunc Key(a string) string {\n\treturn fmt.Sprintf(\"%s\", strings.ToLower(a))\n}\n\nfunc Num(n int) string {\n\treturn strconv.Itoa(n)\n}\n";
    let theirs = "package store\n\nimport (\n\t\"errors\"\n\t\"fmt\"\n)\n\nfunc Key(a string) string {\n\treturn fmt.Sprintf(\"%s\", a)\n}\n\nfunc Check(a string) error {\n\treturn errors.New(a)\n}\n";
    let (code, merged) = merge("keys.go", base, ours, theirs);
    assert!(
        !merged.lines().any(|l| l.starts_with("<<<<<<<")) || code == 1,
        "markers written with exit 0:\n{merged}"
    );
    if code == 0 {
        assert_eq!(merged.matches("import (").count(), 1, "{merged}");
    }
}

// ---------------------------------------------------------------------------
// The same body, rewritten on both sides
// ---------------------------------------------------------------------------

/// Both sides rewrote the same component. Fusing the two rewrites leaves two
/// `return`s and a use of a variable one side removed.
#[test]
fn both_sides_rewrote_the_same_function() {
    let base = "export function App() {\n  const mode = useMode()\n  const user = useUser()\n  return render(mode, user)\n}\n\nexport function Other() {\n  return 1\n}\n";
    let ours = "export function App() {\n  const user = useUser()\n  const theme = useTheme()\n  return renderThemed(theme, user)\n}\n\nexport function Other() {\n  return 1\n}\n";
    let theirs = "export function App() {\n  const mode = useMode()\n  const user = useUser()\n  const flags = useFlags()\n  if (!flags.ready) {\n    return null\n  }\n  return renderWithFlags(mode, user, flags)\n}\n\nexport function Other() {\n  return 1\n}\n";
    assert_conflict("App.tsx", base, ours, theirs);
}

/// Both sides replaced the same statement with a different one of the same
/// name.
#[test]
fn both_sides_redefined_the_same_local() {
    let base = "export async function open(url: string) {\n  const client = makeClient(url)\n  await client.ready()\n  return client\n}\n";
    let ours = "export async function open(url: string) {\n  const connect = () => makeClient(url, { retries: 3 })\n  const client = connect()\n  await client.ready()\n  return client\n}\n";
    let theirs = "export async function open(url: string) {\n  const connect = async () => makeSecureClient(url)\n  const client = await connect()\n  await client.ready()\n  return client\n}\n";
    assert_conflict("net.ts", base, ours, theirs);
}

/// Both sides edited the same line. Never fuse the two edits token by token.
#[test]
fn same_line_edited_by_both_sides_is_not_token_fused() {
    let base = "export async function signIn(email: string, password: string) {\n  const res = await login(email, password)\n  return res.token\n}\n";
    let ours = "export async function signIn(email: string, password: string) {\n  const res = await apiLogin(email, password)\n  return res.token\n}\n";
    let theirs = "export async function signIn(email: string, password: string) {\n  const res = await login(email, password, { remember: true })\n  return res.token\n}\n";
    assert_conflict("auth.ts", base, ours, theirs);
}

/// Python: the same statement edited on both sides, in different tokens.
#[test]
fn same_python_statement_edited_by_both_sides() {
    let base = "def total(items):\n    return sum(i.price for i in items)\n";
    let ours = "def total(items):\n    return sum(i.price * i.qty for i in items)\n";
    let theirs = "def total(items):\n    return sum(i.price for i in items if i.active)\n";
    assert_conflict("cart.py", base, ours, theirs);
}

/// One side deleted a helper (and its only caller), the other added a new
/// caller in a different function. Each entity merges on its own; the file
/// calls a function it no longer has.
#[test]
fn a_deleted_helper_with_a_new_caller_is_a_conflict() {
    let base = "def scale(x):\n    return x * 2\n\n\ndef area(w, h):\n    return scale(w) * h\n\n\ndef keep():\n    return 'unchanged'\n";
    let ours = "def area(w, h):\n    return w * 2 * h\n\n\ndef keep():\n    return 'unchanged'\n";
    let theirs = "def scale(x):\n    return x * 2\n\n\ndef area(w, h):\n    return scale(w) * h\n\n\ndef keep():\n    return 'unchanged'\n\n\ndef volume(w, h, d):\n    return scale(w) * h * d\n";
    assert_conflict("shapes.py", base, ours, theirs);
}

// ---------------------------------------------------------------------------
// Header / region placement
// ---------------------------------------------------------------------------

/// One side prepended a license header, the other edited the imports. The
/// header must not land between imports or after them.
#[test]
fn license_header_does_not_land_mid_file() {
    let base = "import { a } from \"./a\"\nimport { b } from \"./b\"\n\nexport const f = () => a() + b()\n";
    let ours = "import { a } from \"./a\"\nimport { b } from \"./b\"\nimport { c } from \"./c\"\n\nexport const f = () => a() + b() + c()\n";
    let theirs = "/*\n * Copyright (c) Example Authors.\n * SPDX-License-Identifier: MIT\n */\nimport { a } from \"./a\"\nimport { b } from \"./b\"\n\nexport const f = () => a() + b()\n";
    let (code, merged) = merge("f.ts", base, ours, theirs);
    if code == 0 {
        assert!(
            merged.starts_with("/*\n * Copyright"),
            "header moved away from the top of the file:\n{merged}"
        );
    }
}

// ---------------------------------------------------------------------------
// Prose
// ---------------------------------------------------------------------------

/// Two unrelated rewrites of one README must not be interleaved section by
/// section.
#[test]
fn two_readme_rewrites_are_not_interleaved() {
    // Both sides replaced the same two sections with two different ones each.
    // Section by section nothing collides, and the old answer was all four
    // new sections, one rewrite's after the other's.
    let base =
        "# Tool\n\nA tool.\n\n## Install\n\nRun make.\n\n## Use\n\nRun it.\n\n## License\n\nMIT.\n";
    let ours = "# Tool\n\nA tool.\n\n## Installation\n\nMake ausfuehren.\n\n## Benutzung\n\nStarten.\n\n## License\n\nMIT.\n";
    let theirs = "# Tool\n\nA tool.\n\n## Getting started\n\nDownload a release.\n\n## Configuration\n\nEdit the config file.\n\n## License\n\nMIT.\n";
    assert_conflict("README.md", base, ours, theirs);
}

/// Two sections added at the same place are not a rewrite of each other: they
/// still merge.
#[test]
fn control_two_sections_appended_to_a_document() {
    let base = "# Doc\n\nIntro.\n\n## A\n\nText a.\n";
    let ours = "# Doc\n\nIntro.\n\n## A\n\nText a.\n\n## B\n\nText b.\n";
    let theirs = "# Doc\n\nIntro.\n\n## A\n\nText a.\n\n## C\n\nText c.\n";
    let merged = assert_clean("DOC.md", base, ours, theirs);
    assert!(
        merged.contains("## B") && merged.contains("## C"),
        "{merged}"
    );
}

// ---------------------------------------------------------------------------
// Data files: a key stated twice at one table path
// ---------------------------------------------------------------------------

const MANIFEST: &str = "[package]\nname = \"gizmo\"\nversion = \"0.1.0\"\n\n[dependencies]\nalpha = \"1\"\nbravo = \"1\"\ncharlie = \"1\"\ndelta = \"1\"\necho = \"1\"\nfoxtrot = \"1\"\ngolf = \"1\"\n\n[dev-dependencies]\nhotel = \"1\"\n";

/// Both sides add the same dependency, with different settings, at lines far
/// enough apart that a line merge takes both. No line is stated twice; the
/// KEY is, and the manifest no longer loads.
#[test]
fn toml_both_sides_add_the_same_key_to_one_table() {
    let ours = MANIFEST.replace(
        "alpha = \"1\"\n",
        "alpha = \"1\"\nwidget-kit = { version = \"0.6\", features = [\"cors\"] }\n",
    );
    let theirs = MANIFEST.replace(
        "golf = \"1\"\n",
        "golf = \"1\"\nwidget-kit = { version = \"0.6\", features = [\"trace\"], optional = true }\n",
    );
    assert_conflict("Cargo.toml", MANIFEST, &ours, &theirs);
}

const SETTINGS: &str = "{\n  \"name\": \"gizmo\",\n  \"server\": {\n    \"host\": \"localhost\",\n    \"port\": 8080,\n    \"workers\": 4,\n    \"log\": \"info\",\n    \"cache\": true,\n    \"tls\": false\n  },\n  \"client\": {\n    \"retries\": 3\n  }\n}\n";

/// JSON parsers accept a duplicate key and keep the last, so the file still
/// parses; it just silently drops one side's value.
#[test]
fn json_both_sides_add_the_same_key_to_one_object() {
    let ours = SETTINGS.replace(
        "\"host\": \"localhost\",\n",
        "\"host\": \"localhost\",\n    \"timeout\": 30,\n",
    );
    let theirs = SETTINGS.replace(
        "\"cache\": true,\n",
        "\"cache\": true,\n    \"timeout\": 90,\n",
    );
    assert_conflict("settings.json", SETTINGS, &ours, &theirs);
}

const PIPELINE: &str = "name: build\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: make test\n  lint:\n    runs-on: ubuntu-latest\n    image: rust\n    workdir: src\n    shell: bash\n    steps:\n      - run: make lint\n";

#[test]
fn yaml_both_sides_add_the_same_key_to_one_mapping() {
    let ours = PIPELINE.replace(
        "  lint:\n    runs-on: ubuntu-latest\n",
        "  lint:\n    runs-on: ubuntu-latest\n    timeout-minutes: 10\n",
    );
    let theirs = PIPELINE.replace(
        "    shell: bash\n",
        "    shell: bash\n    timeout-minutes: 30\n",
    );
    assert_conflict("ci.yaml", PIPELINE, &ours, &theirs);
}

/// A merge that breaks a data file one side still loads is refused, even
/// when the other side was already broken.
#[test]
fn json_a_merge_that_no_longer_loads_is_refused() {
    let ours = SETTINGS.replace("\"retries\": 3\n", "\"retries\": 3,\n    \"backoff\" 2\n");
    let theirs = SETTINGS.replace("\"port\": 8080", "\"port\": 9090");
    assert_conflict("settings.json", SETTINGS, &ours, &theirs);
}

/// One side's file already carries the other side's edit — and is broken.
/// Taking it whole is not "the file a developer wrote" when it no longer
/// loads and the other side's still does.
#[test]
fn json_a_superset_side_that_no_longer_loads_is_refused() {
    let theirs = SETTINGS.replace("\"port\": 8080", "\"port\": 9090");
    let ours = theirs.replace("\"retries\": 3\n", "\"retries\": 3\n    \"backoff\": 2\n");
    assert_conflict("settings.json", SETTINGS, &ours, &theirs);
}

// ---------------------------------------------------------------------------
// The changelog and data unions: where the insertions do not commute
// ---------------------------------------------------------------------------

const NOTES: &str = "# Changelog\n\n## Unreleased\n\n### Added\n- `scan`: new subcommand.\n\n## 1.0.0\n\n- first release\n";

/// A conflict by default AND where the notes file is declared a set: the
/// guard, not the missing declaration, is what refuses.
fn assert_changelog_conflict(name: &str, base: &str, ours: &str, theirs: &str) {
    assert_conflict(name, base, ours, theirs);
    let (code, merged) = merge_in_repo(&format!("{name} weave-set\n"), name, base, ours, theirs);
    assert_eq!(code, 1, "declared:\n{merged}");
}

/// One side rewrote an existing entry while adding its own; the other added
/// an entry next to it. The union would have to choose a text for the entry.
#[test]
fn changelog_an_existing_entry_edited_beside_an_addition() {
    let ours = NOTES.replace(
        "- `scan`: new subcommand.\n",
        "- `scan`: new subcommand, recursive.\n- `walk`: new subcommand.\n",
    );
    let theirs = NOTES.replace(
        "- `scan`: new subcommand.\n",
        "- `scan`: new subcommand.\n- `count`: new subcommand.\n",
    );
    assert_changelog_conflict("CHANGELOG.md", NOTES, &ours, &theirs);
}

/// One side added an entry to the section; the other added one and then
/// opened a new section, at the same point. Any order of the two runs puts
/// one side's entry under the other side's heading or splits a run across
/// it: which section an entry belongs to is intent.
#[test]
fn changelog_insertions_across_a_heading() {
    let ours = NOTES.replace(
        "- `scan`: new subcommand.\n",
        "- `scan`: new subcommand.\n- `walk`: new subcommand.\n",
    );
    let theirs = NOTES.replace(
        "- `scan`: new subcommand.\n",
        "- `scan`: new subcommand.\n- `count`: new subcommand.\n\n### Fixed\n- `scan`: no longer follows links.\n",
    );
    assert_changelog_conflict("CHANGELOG.md", NOTES, &ours, &theirs);
    assert_changelog_conflict("CHANGELOG.md", NOTES, &theirs, &ours);
}

/// A numbered list says its order out loud.
#[test]
fn changelog_numbered_entries_are_not_a_set() {
    let base = "# Upgrading\n\n## 2.0\n\n1. Back up the data.\n";
    let ours = base.replace("data.\n", "data.\n2. Stop the service.\n");
    let theirs = base.replace("data.\n", "data.\n2. Run the migration.\n");
    assert_changelog_conflict("RELEASE_NOTES.md", base, &ours, &theirs);
}

/// A continuation line added under an existing entry is an edit of that entry.
#[test]
fn changelog_a_continuation_of_an_existing_entry() {
    let ours = NOTES.replace(
        "- `scan`: new subcommand.\n",
        "- `scan`: new subcommand.\n  It reads stdin.\n",
    );
    let theirs = NOTES.replace(
        "- `scan`: new subcommand.\n",
        "- `scan`: new subcommand.\n- `walk`: new subcommand.\n",
    );
    assert_changelog_conflict("CHANGELOG.md", NOTES, &ours, &theirs);
}

/// Distinct keys added side by side, plus one key both added with different
/// values: the key union is not a function of the sides.
#[test]
fn data_one_key_two_values_beside_distinct_keys() {
    let ours = MANIFEST.replace(
        "golf = \"1\"\n",
        "golf = \"1\"\nhotel-kit = \"1\"\nwidget-kit = \"0.6\"\n",
    );
    let theirs = MANIFEST.replace(
        "golf = \"1\"\n",
        "golf = \"1\"\nindia = \"1\"\nwidget-kit = \"0.7\"\n",
    );
    assert_conflict("Cargo.toml", MANIFEST, &ours, &theirs);
}

/// A side edited the base line its neighbour's key was added after.
#[test]
fn data_an_existing_key_edited_beside_an_addition() {
    let ours = SETTINGS.replace(
        "    \"retries\": 3\n",
        "    \"retries\": 5,\n    \"backoff\": 2\n",
    );
    let theirs = SETTINGS.replace(
        "    \"retries\": 3\n",
        "    \"retries\": 3,\n    \"jitter\": true\n",
    );
    assert_conflict("settings.json", SETTINGS, &ours, &theirs);
}

/// A side that does not load certifies nothing.
#[test]
fn data_a_malformed_side_is_not_united() {
    let ours = SETTINGS.replace(
        "    \"retries\": 3\n",
        "    \"retries\": 3,\n    \"backoff\" 2\n",
    );
    let theirs = SETTINGS.replace(
        "    \"retries\": 3\n",
        "    \"retries\": 3,\n    \"jitter\": true\n",
    );
    assert_conflict("settings.json", SETTINGS, &ours, &theirs);
}

/// A list that is not a dependency-style list keeps its order: both sides
/// appending to it is a conflict.
#[test]
fn data_an_ordered_array_appended_by_both() {
    let base = "steps:\n  - checkout\n  - build\nname: ci\n";
    let ours = base.replace("  - build\n", "  - build\n  - test\n");
    let theirs = base.replace("  - build\n", "  - build\n  - publish\n");
    assert_conflict("pipeline.yaml", base, &ours, &theirs);
}

// ---------------------------------------------------------------------------
// Controls: provably disjoint merges stay clean
// ---------------------------------------------------------------------------

/// Different keys added to one table, and one key edited: still clean.
#[test]
fn control_data_files_with_disjoint_keys_stay_clean() {
    let ours = MANIFEST.replace("alpha = \"1\"\n", "alpha = \"1\"\nwidget-kit = \"0.6\"\n");
    let theirs = MANIFEST.replace("golf = \"1\"\n", "golf = \"1\"\nzulu = \"2\"\n");
    let merged = assert_clean("Cargo.toml", MANIFEST, &ours, &theirs);
    assert!(merged.contains("widget-kit") && merged.contains("zulu"));

    let ours = SETTINGS.replace("\"port\": 8080", "\"port\": 9090");
    let theirs = SETTINGS.replace(
        "\"cache\": true,\n",
        "\"cache\": true,\n    \"timeout\": 90,\n",
    );
    let merged = assert_clean("settings.json", SETTINGS, &ours, &theirs);
    assert!(merged.contains("9090") && merged.contains("\"timeout\": 90"));
}

#[test]
fn control_edits_to_different_functions_merge_clean() {
    let base = "def a():\n    return 1\n\n\ndef b():\n    return 2\n";
    let ours = "def a():\n    return 10\n\n\ndef b():\n    return 2\n";
    let theirs = "def a():\n    return 1\n\n\ndef b():\n    return 20\n";
    let merged = assert_clean("m.py", base, ours, theirs);
    assert!(merged.contains("return 10") && merged.contains("return 20"));
}

#[test]
fn control_both_sides_add_different_functions_at_the_end() {
    let base = "export function a() {\n  return 1\n}\n";
    let ours = "export function a() {\n  return 1\n}\n\nexport function b() {\n  return 2\n}\n";
    let theirs = "export function a() {\n  return 1\n}\n\nexport function c() {\n  return 3\n}\n";
    let merged = assert_clean("m.ts", base, ours, theirs);
    assert!(merged.contains("function b()") && merged.contains("function c()"));
}

#[test]
fn control_distinct_imports_added_by_both_sides_union() {
    let base =
        "import { a } from \"./a\"\n\nexport const f = () => a()\n\nexport const g = () => 1\n";
    let ours = "import { a } from \"./a\"\nimport { b } from \"./b\"\n\nexport const f = () => a() + b()\n\nexport const g = () => 1\n";
    let theirs = "import { a } from \"./a\"\nimport { c } from \"./c\"\n\nexport const f = () => a()\n\nexport const g = () => c()\n";
    let merged = assert_clean("m.ts", base, ours, theirs);
    assert!(
        merged.contains("import { b }") && merged.contains("import { c }"),
        "{merged}"
    );
    assert_eq!(merged.matches("import { a }").count(), 1, "{merged}");
}

#[test]
fn control_disjoint_lines_in_one_function_merge_clean() {
    let base =
        "def f(x):\n    a = x + 1\n    b = a * 2\n    c = b - 3\n    d = c / 4\n    return d\n";
    let ours =
        "def f(x):\n    a = x + 100\n    b = a * 2\n    c = b - 3\n    d = c / 4\n    return d\n";
    let theirs =
        "def f(x):\n    a = x + 1\n    b = a * 2\n    c = b - 3\n    d = c / 400\n    return d\n";
    let merged = assert_clean("f.py", base, ours, theirs);
    assert!(merged.contains("x + 100") && merged.contains("c / 400"));
}

#[test]
fn control_sections_edited_in_different_places_of_a_document() {
    let base = "# Doc\n\nIntro.\n\n## A\n\nText a.\n\n## B\n\nText b.\n\n## C\n\nText c.\n";
    let ours =
        "# Doc\n\nIntro.\n\n## A\n\nText a, revised.\n\n## B\n\nText b.\n\n## C\n\nText c.\n";
    let theirs =
        "# Doc\n\nIntro.\n\n## A\n\nText a.\n\n## B\n\nText b.\n\n## C\n\nText c, revised.\n";
    let merged = assert_clean("DOC.md", base, ours, theirs);
    assert!(merged.contains("Text a, revised.") && merged.contains("Text c, revised."));
}

/// Names imported through a multi-line block are bound: two sides editing
/// different functions that use them still merge clean.
#[test]
fn control_multiline_import_block_uses_stay_bound() {
    let base = "import {\n  Widget,\n  render,\n} from \"./ui\"\n\nexport function a() {\n  return Widget.make(1)\n}\n\nexport function b() {\n  return render(2)\n}\n";
    let ours = base.replace("Widget.make(1)", "Widget.make(10)");
    let theirs = base.replace("render(2)", "render(20)");
    let merged = assert_clean("ui.ts", base, &ours, &theirs);
    assert!(merged.contains("make(10)") && merged.contains("render(20)"));

    let base = "from shapes import (\n    Circle,\n    draw,\n)\n\n\ndef a():\n    return Circle(1)\n\n\ndef b():\n    return draw(2)\n";
    let ours = base.replace("Circle(1)", "Circle(10)");
    let theirs = base.replace("draw(2)", "draw(20)");
    assert_clean("pics.py", base, &ours, &theirs);
}

/// Entries added at one point of a changelog section, through the driver's
/// own line merge: a conflict unless the file's `weave-set` declares the
/// section a set (two differently worded entries can describe one change);
/// declared, the union, whichever side is ours.
#[test]
fn control_changelog_entries_at_one_point_union() {
    let ours = NOTES.replace(
        "- `scan`: new subcommand.\n",
        "- `scan`: new subcommand.\n- `walk`: new subcommand.\n",
    );
    let theirs = NOTES.replace(
        "- `scan`: new subcommand.\n",
        "- `scan`: new subcommand.\n- `count`: new subcommand.\n",
    );
    assert_conflict("CHANGELOG.md", NOTES, &ours, &theirs);
    for attributes in ["", "CHANGELOG.md weave-set=Fixed\n"] {
        let (code, merged) = merge_in_repo(attributes, "CHANGELOG.md", NOTES, &ours, &theirs);
        assert_eq!(code, 1, "{attributes:?}:\n{merged}");
    }
    for attributes in [
        "CHANGELOG.md weave-set\n",
        "CHANGELOG.md weave-set=Unreleased\n",
    ] {
        let (code, merged) = merge_in_repo(attributes, "CHANGELOG.md", NOTES, &ours, &theirs);
        assert_eq!(code, 0, "{attributes:?}:\n{merged}");
        let (_, swapped) = merge_in_repo(attributes, "CHANGELOG.md", NOTES, &theirs, &ours);
        assert_eq!(merged, swapped);
        assert!(merged.contains("- `count`: new subcommand.\n- `walk`: new subcommand.\n"));
    }
}

/// Keys added after one line of a table, through the driver's own line merge.
#[test]
fn control_data_keys_at_one_point_union() {
    let ours = MANIFEST.replace("golf = \"1\"\n", "golf = \"1\"\nwidget-kit = \"0.6\"\n");
    let theirs = MANIFEST.replace("golf = \"1\"\n", "golf = \"1\"\nhotel-kit = \"2\"\n");
    let merged = assert_clean("Cargo.toml", MANIFEST, &ours, &theirs);
    assert_eq!(merged, assert_clean("Cargo.toml", MANIFEST, &theirs, &ours));
    assert!(merged.contains("golf = \"1\"\nhotel-kit = \"2\"\nwidget-kit = \"0.6\"\n"));
}

// ---------------------------------------------------------------------------
// Container insertion union, through the driver's own line merge
// ---------------------------------------------------------------------------

const REGISTRY: &str = "from .handlers import run_alpha\n\nHANDLERS = {\n    \"alpha\": run_alpha,\n}\n\n\ndef lookup(name):\n    return HANDLERS[name]\n";

/// A fleet: the file already holds two branches' entries, a third adds one
/// at the same point.
#[test]
fn control_registry_entries_at_one_point_union() {
    let add = |text: &str, key: &str| {
        text.replace(
            "    \"alpha\": run_alpha,\n",
            &format!("    \"alpha\": run_alpha,\n    \"{key}\": run_{key},\n"),
        )
    };
    let ours = add(&add(REGISTRY, "gamma"), "beta");
    let theirs = add(REGISTRY, "delta");
    let merged = assert_clean("registry.py", REGISTRY, &ours, &theirs);
    assert_eq!(
        merged,
        assert_clean("registry.py", REGISTRY, &theirs, &ours)
    );
    assert!(merged.contains(
        "    \"alpha\": run_alpha,\n    \"beta\": run_beta,\n    \"delta\": run_delta,\n    \"gamma\": run_gamma,\n"
    ));
}

#[test]
fn control_export_names_and_ini_keys_at_one_point_union() {
    let base = "export {\n  alpha,\n} from \"./names\";\n";
    let ours = "export {\n  alpha,\n  zeta,\n} from \"./names\";\n";
    let theirs = "export {\n  alpha,\n  beta,\n} from \"./names\";\n";
    let merged = assert_clean("index.ts", base, ours, theirs);
    assert_eq!(
        merged,
        "export {\n  alpha,\n  beta,\n  zeta,\n} from \"./names\";\n"
    );
    let base = "[tool]\nalpha = 1\n";
    let merged = assert_clean(
        "tox.ini",
        base,
        "[tool]\nalpha = 1\nzeta = 2\n",
        "[tool]\nalpha = 1\nbeta = 3\n",
    );
    assert_eq!(merged, "[tool]\nalpha = 1\nbeta = 3\nzeta = 2\n");
}

#[test]
fn registry_one_key_two_values() {
    let ours = REGISTRY.replace(
        "    \"alpha\": run_alpha,\n",
        "    \"alpha\": run_alpha,\n    \"beta\": run_beta,\n",
    );
    let theirs = REGISTRY.replace(
        "    \"alpha\": run_alpha,\n",
        "    \"alpha\": run_alpha,\n    \"beta\": run_other,\n",
    );
    assert_conflict("registry.py", REGISTRY, &ours, &theirs);
}

#[test]
fn registry_an_existing_entry_edited_beside_an_addition() {
    let ours = REGISTRY.replace(
        "    \"alpha\": run_alpha,\n",
        "    \"alpha\": run_alpha_v2,\n    \"beta\": run_beta,\n",
    );
    let theirs = REGISTRY.replace(
        "    \"alpha\": run_alpha,\n",
        "    \"alpha\": run_alpha,\n    \"gamma\": run_gamma,\n",
    );
    assert_conflict("registry.py", REGISTRY, &ours, &theirs);
}

#[test]
fn registry_an_existing_entry_moved_to_the_insertion_point() {
    let base = "H = {\n    \"alpha\": 1,\n    \"gamma\": 3,\n    \"omega\": 9,\n}\n";
    let ours = "H = {\n    \"gamma\": 3,\n    \"omega\": 9,\n    \"alpha\": 1,\n}\n";
    let theirs =
        "H = {\n    \"alpha\": 1,\n    \"gamma\": 3,\n    \"omega\": 9,\n    \"beta\": 2,\n}\n";
    assert_conflict("h.py", base, ours, theirs);
}

/// The union settles its region, but the file also carries a declaration
/// one side deleted and the other changed: the gate every settled file
/// passes refuses the whole answer.
#[test]
fn registry_union_beside_a_modify_delete() {
    let base = format!(
        "{REGISTRY}

def helper():\n    return 1\n"
    );
    let ours = base
        .replace(
            "    \"alpha\": run_alpha,\n",
            "    \"alpha\": run_alpha,\n    \"beta\": run_beta,\n",
        )
        .replace("\n\ndef helper():\n    return 1\n", "");
    let theirs = base
        .replace(
            "    \"alpha\": run_alpha,\n",
            "    \"alpha\": run_alpha,\n    \"gamma\": run_gamma,\n",
        )
        .replace("    return 1\n", "    return 2\n");
    assert_conflict("registry.py", &base, &ours, &theirs);
}

// Order-sensitive containers and the `weave-set` declaration.

/// Run the driver the way git does, inside a repository whose
/// `.gitattributes` is `attributes`.
fn merge_in_repo(
    attributes: &str,
    name: &str,
    base: &str,
    ours: &str,
    theirs: &str,
) -> (i32, String) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let init = Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .status()
        .expect("git");
    assert!(init.success());
    fs::write(dir.join(".gitattributes"), attributes).unwrap();
    let (b, o, t) = (dir.join(".base"), dir.join(".ours"), dir.join(".theirs"));
    fs::write(&b, base).unwrap();
    fs::write(&o, ours).unwrap();
    fs::write(&t, theirs).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_weave-driver"))
        .args([
            b.to_str().unwrap(),
            o.to_str().unwrap(),
            t.to_str().unwrap(),
            "7",
            name,
        ])
        .current_dir(dir)
        .output()
        .expect("failed to run weave-driver");
    (
        out.status.code().expect("exit code"),
        fs::read_to_string(&o).unwrap(),
    )
}

const ENTRY_POINTS: &str = "[metadata]\nname = demo\n\n[options.entry_points]\nconsole_scripts =\n    alpha = demo.alpha:main\n";

#[test]
fn value_lines_are_a_union_only_where_declared() {
    let ours = format!("{ENTRY_POINTS}    zeta = demo.zeta:main\n");
    let theirs = format!("{ENTRY_POINTS}    beta = demo.beta:main\n");
    // No declaration, or one naming another container: a conflict.
    for attributes in [
        "",
        "setup.cfg weave-set=install_requires\n",
        "setup.cfg -weave-set\n",
    ] {
        let (code, merged) = merge_in_repo(attributes, "setup.cfg", ENTRY_POINTS, &ours, &theirs);
        assert_eq!(code, 1, "{attributes:?}:\n{merged}");
    }
    for attributes in ["setup.cfg weave-set=console_scripts\n", "*.cfg weave-set\n"] {
        let (code, merged) = merge_in_repo(attributes, "setup.cfg", ENTRY_POINTS, &ours, &theirs);
        assert_eq!(code, 0, "{attributes:?}:\n{merged}");
        assert_eq!(
            merged,
            format!("{ENTRY_POINTS}    beta = demo.beta:main\n    zeta = demo.zeta:main\n")
        );
        let (_, swapped) = merge_in_repo(attributes, "setup.cfg", ENTRY_POINTS, &theirs, &ours);
        assert_eq!(swapped, merged);
    }
}

#[test]
fn a_parametrize_list_is_a_union_only_where_declared() {
    let base = "import pytest\n\n\n@pytest.mark.parametrize(\"x\", [\n    \"alpha\",\n])\ndef test_x(x):\n    assert x\n";
    let ours = base.replace("    \"alpha\",\n", "    \"alpha\",\n    \"zeta\",\n");
    let theirs = base.replace("    \"alpha\",\n", "    \"alpha\",\n    \"beta\",\n");
    let (code, _) = merge_in_repo("", "tests/test_x.py", base, &ours, &theirs);
    assert_eq!(code, 1);
    let (code, merged) = merge_in_repo(
        "tests/test_*.py weave-set=parametrize\n",
        "tests/test_x.py",
        base,
        &ours,
        &theirs,
    );
    assert_eq!(code, 0, "{merged}");
    assert!(merged.contains("    \"alpha\",\n    \"beta\",\n    \"zeta\",\n"));
}
