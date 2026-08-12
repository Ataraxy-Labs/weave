//! One sanity sweep per language weave tells git to route through it.
//!
//! `weave setup` writes a `.gitattributes` line for every extension in
//! `weave-cli`'s `SUPPORTED_EXTENSIONS`, which is a promise: files with that
//! extension get an entity-level merge rather than git's line-level one. A
//! language that parses to no entities still *works* — the merge falls back to
//! the line-level route — but the promise is then hollow, and nothing in the
//! tree noticed the difference.
//!
//! This file is what notices. Every language gets the same five synthetic
//! three-way merges, built from three interchangeable definitions:
//!
//!   1. **disjoint add** — each side adds a different new definition. Clean,
//!      and both additions survive.
//!   2. **add against modify** — ours rewrites a definition, theirs adds a new
//!      one. Clean, and neither edit is lost.
//!   3. **contradiction** — both sides rewrite the same definition to
//!      different values. Conflicts, rather than silently picking a side.
//!   4. **no loss** — theirs changed nothing, so ours is the answer, byte for
//!      byte.
//!   5. **same edit twice** — both sides made the identical edit. Clean, and
//!      the edit appears once, not twice.
//!
//! Scenarios 1, 2 and 4 are the "no loss" half: every definition that went in
//! is asserted to come out. Scenario 3 is the half that keeps weave from
//! passing this file by merging everything cleanly and losing edits quietly.
//!
//! A language that cannot pass all five does not belong in `SUPPORTED_EXTENSIONS`
//! and is not listed in the README. See the bottom of this file for the ones
//! that are deliberately out, and why.

use weave_core::entity_merge;

/// One language's fixtures. `a`, `b` and `c` are three independent top-level
/// definitions named `alpha`, `beta` and `gamma`; `a_ours` and `a_theirs` are
/// two incompatible rewrites of `alpha`, distinguishable by the values
/// `ours_value` and `theirs_value`.
struct Lang {
    /// The file name a merge is asked about — the extension is the whole point.
    file: &'static str,
    /// Text that has to sit before the definitions for the file to parse
    /// (a module header, an SFC `<script>` tag, a CSV header row).
    prefix: &'static str,
    /// Text that has to sit after them.
    suffix: &'static str,
    /// What separates two definitions in this language's normal formatting.
    sep: &'static str,
    a: &'static str,
    b: &'static str,
    c: &'static str,
    a_ours: &'static str,
    a_theirs: &'static str,
}

impl Lang {
    /// Assemble a file out of the definitions given, in order.
    fn file_of(&self, defs: &[&str]) -> String {
        format!("{}{}{}", self.prefix, defs.join(self.sep), self.suffix)
    }
}

/// Assert a clean merge that kept every one of `wanted`.
fn clean_and_keeps(lang: &Lang, base: &str, ours: &str, theirs: &str, wanted: &[&str]) {
    let result = entity_merge(base, ours, theirs, lang.file);
    assert!(
        result.is_clean(),
        "[{}] expected a clean merge, got conflicts: {:?}\n--- output ---\n{}",
        lang.file,
        result.conflicts,
        result.content
    );
    for token in wanted {
        assert!(
            result.content.contains(token),
            "[{}] merged output dropped `{}`\n--- output ---\n{}",
            lang.file,
            token,
            result.content
        );
    }
}

fn disjoint_add(lang: &Lang) {
    let base = lang.file_of(&[lang.a]);
    let ours = lang.file_of(&[lang.a, lang.b]);
    let theirs = lang.file_of(&[lang.a, lang.c]);
    // Both additions survive, and so does the definition neither side touched.
    clean_and_keeps(
        lang,
        &base,
        &ours,
        &theirs,
        &["alpha", "beta", "gamma", "base_value"],
    );
}

fn add_against_modify(lang: &Lang) {
    let base = lang.file_of(&[lang.a]);
    let ours = lang.file_of(&[lang.a_ours]);
    let theirs = lang.file_of(&[lang.a, lang.b]);
    // Ours' rewrite and theirs' addition are independent edits; both land.
    clean_and_keeps(lang, &base, &ours, &theirs, &["ours_value", "beta"]);
}

fn contradiction(lang: &Lang) {
    let base = lang.file_of(&[lang.a]);
    let ours = lang.file_of(&[lang.a_ours]);
    let theirs = lang.file_of(&[lang.a_theirs]);
    let result = entity_merge(&base, &ours, &theirs, lang.file);
    assert!(
        !result.is_clean(),
        "[{}] two sides rewrote `alpha` to different values; that is a \
         judgment call and has to be reported, not guessed at\n--- output ---\n{}",
        lang.file,
        result.content
    );
    // Whichever way it is rendered, neither side's claim may go missing.
    assert!(
        result.content.contains("ours_value") && result.content.contains("theirs_value"),
        "[{}] a conflict has to carry both claims\n--- output ---\n{}",
        lang.file,
        result.content
    );
}

fn no_loss_when_one_side_stood_still(lang: &Lang) {
    let base = lang.file_of(&[lang.a, lang.b]);
    let ours = lang.file_of(&[lang.a, lang.b, lang.c]);
    let result = entity_merge(&base, &ours, &base, lang.file);
    assert!(
        result.is_clean(),
        "[{}] theirs changed nothing, so this cannot conflict: {:?}",
        lang.file,
        result.conflicts
    );
    assert_eq!(
        result.content, ours,
        "[{}] theirs changed nothing, so ours is the answer byte for byte",
        lang.file
    );
}

fn the_same_edit_from_both_sides(lang: &Lang) {
    let base = lang.file_of(&[lang.a]);
    let ours = lang.file_of(&[lang.a_ours]);
    let result = entity_merge(&base, &ours, &ours, lang.file);
    assert!(
        result.is_clean(),
        "[{}] both sides made the identical edit: {:?}",
        lang.file,
        result.conflicts
    );
    assert_eq!(
        result.content, ours,
        "[{}] an edit made twice is still one edit",
        lang.file
    );
}

/// Every scenario, for one language.
fn sweep(lang: &Lang) {
    disjoint_add(lang);
    add_against_modify(lang);
    contradiction(lang);
    no_loss_when_one_side_stood_still(lang);
    the_same_edit_from_both_sides(lang);
}

// ---------------------------------------------------------------------------
// The languages added to SUPPORTED_EXTENSIONS.
// ---------------------------------------------------------------------------

#[test]
fn kotlin() {
    sweep(&Lang {
        file: "Sample.kt",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "fun alpha(): String {\n    return \"base_value\"\n}",
        b: "fun beta(): Int {\n    return 2\n}",
        c: "fun gamma(): Int {\n    return 3\n}",
        a_ours: "fun alpha(): String {\n    return \"ours_value\"\n}",
        a_theirs: "fun alpha(): String {\n    return \"theirs_value\"\n}",
    });
}

#[test]
fn terraform() {
    sweep(&Lang {
        file: "main.tf",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "resource \"local_file\" \"alpha\" {\n  content = \"base_value\"\n}",
        b: "resource \"local_file\" \"beta\" {\n  content = \"b\"\n}",
        c: "resource \"local_file\" \"gamma\" {\n  content = \"c\"\n}",
        a_ours: "resource \"local_file\" \"alpha\" {\n  content = \"ours_value\"\n}",
        a_theirs: "resource \"local_file\" \"alpha\" {\n  content = \"theirs_value\"\n}",
    });
}

#[test]
fn hcl() {
    sweep(&Lang {
        file: "config.hcl",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "alpha {\n  content = \"base_value\"\n}",
        b: "beta {\n  content = \"b\"\n}",
        c: "gamma {\n  content = \"c\"\n}",
        a_ours: "alpha {\n  content = \"ours_value\"\n}",
        a_theirs: "alpha {\n  content = \"theirs_value\"\n}",
    });
}

#[test]
fn ocaml() {
    sweep(&Lang {
        file: "sample.ml",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "let alpha () = \"base_value\"",
        b: "let beta () = \"b\"",
        c: "let gamma () = \"c\"",
        a_ours: "let alpha () = \"ours_value\"",
        a_theirs: "let alpha () = \"theirs_value\"",
    });
}

#[test]
fn ocaml_interface() {
    sweep(&Lang {
        file: "sample.mli",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "(* base_value *)\nval alpha : unit -> string",
        b: "val beta : unit -> int",
        c: "val gamma : unit -> int",
        a_ours: "(* ours_value *)\nval alpha : unit -> string",
        a_theirs: "(* theirs_value *)\nval alpha : unit -> string",
    });
}

#[test]
fn zig() {
    sweep(&Lang {
        file: "sample.zig",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "pub fn alpha() []const u8 {\n    return \"base_value\";\n}",
        b: "pub fn beta() u8 {\n    return 2;\n}",
        c: "pub fn gamma() u8 {\n    return 3;\n}",
        a_ours: "pub fn alpha() []const u8 {\n    return \"ours_value\";\n}",
        a_theirs: "pub fn alpha() []const u8 {\n    return \"theirs_value\";\n}",
    });
}

#[test]
fn elm() {
    sweep(&Lang {
        file: "Sample.elm",
        prefix: "module Sample exposing (..)\n\n",
        suffix: "\n",
        sep: "\n\n",
        a: "alpha : String\nalpha =\n    \"base_value\"",
        b: "beta : String\nbeta =\n    \"b\"",
        c: "gamma : String\ngamma =\n    \"c\"",
        a_ours: "alpha : String\nalpha =\n    \"ours_value\"",
        a_theirs: "alpha : String\nalpha =\n    \"theirs_value\"",
    });
}

#[test]
fn clojure() {
    sweep(&Lang {
        file: "sample.clj",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "(defn alpha []\n  \"base_value\")",
        b: "(defn beta []\n  \"b\")",
        c: "(defn gamma []\n  \"c\")",
        a_ours: "(defn alpha []\n  \"ours_value\")",
        a_theirs: "(defn alpha []\n  \"theirs_value\")",
    });
}

#[test]
fn dlang() {
    sweep(&Lang {
        file: "sample.d",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "string alpha() {\n    return \"base_value\";\n}",
        b: "int beta() {\n    return 2;\n}",
        c: "int gamma() {\n    return 3;\n}",
        a_ours: "string alpha() {\n    return \"ours_value\";\n}",
        a_theirs: "string alpha() {\n    return \"theirs_value\";\n}",
    });
}

#[test]
fn lua() {
    sweep(&Lang {
        file: "sample.lua",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "function alpha()\n    return \"base_value\"\nend",
        b: "function beta()\n    return \"b\"\nend",
        c: "function gamma()\n    return \"c\"\nend",
        a_ours: "function alpha()\n    return \"ours_value\"\nend",
        a_theirs: "function alpha()\n    return \"theirs_value\"\nend",
    });
}

#[test]
fn fish() {
    sweep(&Lang {
        file: "sample.fish",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "function alpha\n    echo \"base_value\"\nend",
        b: "function beta\n    echo \"b\"\nend",
        c: "function gamma\n    echo \"c\"\nend",
        a_ours: "function alpha\n    echo \"ours_value\"\nend",
        a_theirs: "function alpha\n    echo \"theirs_value\"\nend",
    });
}

#[test]
fn sql() {
    sweep(&Lang {
        file: "schema.sql",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "CREATE TABLE alpha (\n    id INT,\n    label VARCHAR(32) DEFAULT 'base_value'\n);",
        b: "CREATE TABLE beta (\n    id INT\n);",
        c: "CREATE TABLE gamma (\n    id INT\n);",
        a_ours: "CREATE TABLE alpha (\n    id INT,\n    label VARCHAR(32) DEFAULT 'ours_value'\n);",
        a_theirs:
            "CREATE TABLE alpha (\n    id INT,\n    label VARCHAR(32) DEFAULT 'theirs_value'\n);",
    });
}

#[test]
fn perl() {
    sweep(&Lang {
        file: "sample.pl",
        prefix: "",
        suffix: "",
        sep: "\n\n",
        a: "sub alpha {\n    return \"base_value\";\n}",
        b: "sub beta {\n    return \"b\";\n}",
        c: "sub gamma {\n    return \"c\";\n}",
        a_ours: "sub alpha {\n    return \"ours_value\";\n}",
        a_theirs: "sub alpha {\n    return \"theirs_value\";\n}",
    });
}

#[test]
fn nix() {
    sweep(&Lang {
        file: "default.nix",
        prefix: "{\n",
        suffix: "\n}\n",
        sep: "\n",
        a: "  alpha = \"base_value\";",
        b: "  beta = \"b\";",
        c: "  gamma = \"c\";",
        a_ours: "  alpha = \"ours_value\";",
        a_theirs: "  alpha = \"theirs_value\";",
    });
}

#[test]
fn csv() {
    sweep(&Lang {
        file: "rows.csv",
        prefix: "id,label\n",
        suffix: "\n",
        sep: "\n",
        a: "alpha,base_value",
        b: "beta,b",
        c: "gamma,c",
        a_ours: "alpha,ours_value",
        a_theirs: "alpha,theirs_value",
    });
}

#[test]
fn edn() {
    sweep(&Lang {
        file: "config.edn",
        prefix: "{\n",
        suffix: "\n}\n",
        sep: "\n",
        a: " :alpha \"base_value\"",
        b: " :beta \"b\"",
        c: " :gamma \"c\"",
        a_ours: " :alpha \"ours_value\"",
        a_theirs: " :alpha \"theirs_value\"",
    });
}

#[test]
fn latex() {
    sweep(&Lang {
        file: "paper.tex",
        prefix: "\\documentclass{article}\n\\begin{document}\n\n",
        suffix: "\n\n\\end{document}\n",
        sep: "\n\n",
        a: "\\section{alpha}\nbase_value",
        b: "\\section{beta}\nb",
        c: "\\section{gamma}\nc",
        a_ours: "\\section{alpha}\nours_value",
        a_theirs: "\\section{alpha}\ntheirs_value",
    });
}

// ---------------------------------------------------------------------------
// Deliberately NOT in SUPPORTED_EXTENSIONS.
// ---------------------------------------------------------------------------
//
// These four have a grammar and do parse, so it is tempting to list them. They
// are left out because the sweep above fails on them, in the way that matters
// most: two sides adding two different definitions — the single most common
// merge there is — does not come out clean, and the marker weave writes lands
// in the wrong place. Listing them in `.gitattributes` would route real merges
// into that, which is worse than the line-level merge git would otherwise do.
//
//   * `.hs` (Haskell). The parser takes the type signature (`alpha :: String`)
//     as the entity and leaves the binding line (`alpha = ...`) as
//     interstitial text. The two drift apart: on a disjoint add the output
//     paired `beta :: String` with `alpha = "base_value"`, duplicating one
//     binding and boxing the other. That is lost work, not a conflict.
//
//   * `.vue` and `.svelte`. The whole `<script>` block is one entity
//     (`sfc_block` / `svelte_instance_script`), so every edit anywhere in the
//     script contends with every other edit in it. Two sides adding two
//     different functions conflicts, and the box is cut mid-function — the
//     `<<<<<<<` lands between a signature and its body.
//
//   * `.erb`. The whole template is one entity, with the same result: two
//     disjoint `<% def %>` additions conflict, and the marker splits a block
//     from its `<% end %>`.
//
// The fix for all four is in the parser's entity model, not here. When one of
// them gains a real per-definition entity model, add its fixture back to the
// sweep above; if it passes, it earns its line in SUPPORTED_EXTENSIONS and in
// the README's language list.
