//! A clean merge never leaves a use of a name that some input bound and the
//! merge no longer binds.
//!
//! One side deletes a binding — an import it no longer needs, a top-level
//! constant — and the other side's surviving code still reads it. Each side
//! is consistent on its own; the composition is a compile error with no
//! marker to point at it. The merge must either keep the binding or refuse.
//!
//! The use is read in every reference position, not only as a call: a type
//! argument (`List<Receipt>`), a bare argument (`open(WORDS_BOX)`).
//!
//! And the rule stays within the file: a name no version bound here, a name
//! the merge still imports, a name deleted together with all its uses, and a
//! name one side unbound while keeping its use (the definition moved to a
//! module the file imports whole) are all left alone.
//!
//! Stated at the public boundary (`entity_merge`), in both argument orders.

use weave_core::entity_merge;

/// Every word `name` in `content` outside import lines and comments.
fn mentions(content: &str, name: &str) -> bool {
    content
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("import ") && !t.starts_with("//")
        })
        .any(|l| {
            l.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|w| w == name)
        })
}

/// Is `name` bound in `content`: imported by name, or declared.
fn binds(content: &str, name: &str) -> bool {
    content.lines().any(|l| {
        let t = l.trim_start();
        (t.starts_with("import ") && t.contains(name))
            || [
                format!("const {name} "),
                format!("const String {name} "),
                format!("class {name} "),
            ]
            .iter()
            .any(|d| t.contains(d.as_str()))
    })
}

/// Both argument orders: a conflict, or a clean result that binds every name
/// it uses from `names`.
fn assert_never_dangles(base: &str, a: &str, b: &str, path: &str, names: &[&str]) {
    for (ours, theirs) in [(a, b), (b, a)] {
        let r = entity_merge(base, ours, theirs, path);
        if !r.is_clean() {
            continue;
        }
        for n in names {
            assert!(
                !mentions(&r.content, n) || binds(&r.content, n),
                "clean merge uses `{n}` and no longer binds it:\n{}",
                r.content
            );
        }
    }
}

/// Both argument orders merge clean.
fn assert_clean(base: &str, a: &str, b: &str, path: &str) {
    for (ours, theirs) in [(a, b), (b, a)] {
        let r = entity_merge(base, ours, theirs, path);
        assert!(r.is_clean(), "{:?}\n{}", r.conflicts, r.content);
    }
}

// ===========================================================================
// RED: an import deleted as unused, the other side starts using it as a type
// ===========================================================================

const JAVA_BASE: &str = "package org.example.billing;

import org.example.billing.model.Invoice;
import org.example.billing.model.Receipt;
import org.example.billing.port.LedgerPort;

import java.util.List;

public class LedgerService {

    private final LedgerPort ledgerPort;

    public LedgerService(LedgerPort ledgerPort) {
        this.ledgerPort = ledgerPort;
    }

    public void record(Invoice invoice) {
        ledgerPort.record(invoice);
    }

    public List<Invoice> listAll() {
        return ledgerPort.listAll();
    }
}
";

#[test]
fn a_type_argument_keeps_an_import_the_other_side_deleted_as_unused() {
    // Theirs: `Receipt` was never used, so its import goes.
    let theirs = JAVA_BASE.replace("import org.example.billing.model.Receipt;\n", "");
    // Ours: `listAll` now answers receipts — the import is used, as a type
    // argument only.
    let ours = JAVA_BASE
        .replace(
            "    public List<Invoice> listAll() {\n        return ledgerPort.listAll();\n    }\n",
            "    public List<Receipt> listAll() {\n        return ledgerPort.receipts();\n    }\n",
        )
        .replace(
            "    public void record(Invoice invoice) {\n        ledgerPort.record(invoice);\n    }\n\n",
            "",
        )
        .replace("import org.example.billing.model.Invoice;\n", "");
    assert_never_dangles(
        JAVA_BASE,
        &ours,
        &theirs,
        "LedgerService.java",
        &["Receipt"],
    );
}

// ===========================================================================
// RED: a top-level constant deleted with its uses, the other side adds a use
// as a bare argument
// ===========================================================================

const TS_BASE: &str = "import { openBox } from \"./boxes\"
import { WordStore } from \"./stores\"

const WORDS_BOX = WordStore.boxName

export async function openAll(): Promise<void> {
  await openBox(WORDS_BOX)
}

export function registerAll(): void {
  register(\"words\")
}
";

#[test]
fn a_bare_argument_keeps_a_constant_the_other_side_deleted() {
    // Ours: `openAll` goes, and with it the constant only it read.
    let ours = TS_BASE
        .replace("const WORDS_BOX = WordStore.boxName\n\n", "")
        .replace(
            "export async function openAll(): Promise<void> {\n  await openBox(WORDS_BOX)\n}\n\n",
            "",
        )
        .replace("import { WordStore } from \"./stores\"\n", "");
    // Theirs: box opening moves into registration, reading the constant as an
    // argument.
    let theirs = TS_BASE.replace(
        "  register(\"words\")\n",
        "  register(\"words\")\n  void openBox(WORDS_BOX)\n",
    );
    assert_never_dangles(TS_BASE, &ours, &theirs, "harness.ts", &["WORDS_BOX"]);
}

const DART_BASE: &str = "import 'package:app/boxes.dart' show openBox;
import 'package:app/stores.dart' show WordStore;

const wordsBox = WordStore.boxName;

Future<void> openAll() async {
  await openBox(wordsBox);
}

void registerAll() {
  register('words');
}
";

#[test]
fn a_bare_argument_keeps_a_dart_constant_the_other_side_deleted() {
    let ours = DART_BASE
        .replace("const wordsBox = WordStore.boxName;\n\n", "")
        .replace(
            "Future<void> openAll() async {\n  await openBox(wordsBox);\n}\n\n",
            "",
        )
        .replace("import 'package:app/stores.dart' show WordStore;\n", "");
    let theirs = DART_BASE.replace(
        "  register('words');\n",
        "  register('words');\n  openBox(wordsBox);\n",
    );
    assert_never_dangles(DART_BASE, &ours, &theirs, "harness.dart", &["wordsBox"]);
}

// ===========================================================================
// GREEN controls
// ===========================================================================

#[test]
fn a_name_no_version_binds_here_is_external() {
    // `settingsBox` comes from another file in every version.
    let base = TS_BASE.replace(
        "  register(\"words\")\n",
        "  register(\"words\")\n  void openBox(settingsBox)\n",
    );
    let ours = base.replace("register(\"words\")", "register(\"words\", 1)");
    let theirs = base.replace("await openBox(WORDS_BOX)", "await openBox(WORDS_BOX, true)");
    assert_clean(&base, &ours, &theirs, "harness.ts");
}

#[test]
fn a_name_the_merge_still_imports_is_bound() {
    // Ours moves the constant into the module it imports it from; theirs adds
    // a use. The merge imports it: nothing dangles.
    let ours = TS_BASE
        .replace("const WORDS_BOX = WordStore.boxName\n\n", "")
        .replace(
            "import { WordStore } from \"./stores\"\n",
            "import { WORDS_BOX } from \"./stores\"\n",
        );
    let theirs = TS_BASE.replace(
        "  register(\"words\")\n",
        "  register(\"words\")\n  void openBox(WORDS_BOX)\n",
    );
    assert_clean(TS_BASE, &ours, &theirs, "harness.ts");
}

#[test]
fn a_name_deleted_with_all_its_uses_is_gone() {
    let ours = TS_BASE
        .replace("const WORDS_BOX = WordStore.boxName\n\n", "")
        .replace(
            "export async function openAll(): Promise<void> {\n  await openBox(WORDS_BOX)\n}\n\n",
            "",
        )
        .replace("import { WordStore } from \"./stores\"\n", "");
    let theirs = TS_BASE.replace("register(\"words\")", "register(\"words\", 2)");
    assert_clean(TS_BASE, &ours, &theirs, "harness.ts");
}

#[test]
fn a_side_that_unbinds_a_name_and_keeps_its_use_says_it_lives_elsewhere() {
    // Ours moves the constant to a library the file imports whole (Dart's
    // plain `import` binds every name the library exports) and keeps reading
    // it; theirs edits elsewhere. The merge is ours' own file there, and the
    // use was not the merge's doing.
    let base = DART_BASE.replace(
        "import 'package:app/stores.dart' show WordStore;\n",
        "import 'package:app/stores.dart' show WordStore;\nimport 'package:app/constants.dart';\n",
    );
    let ours = base
        .replace("const wordsBox = WordStore.boxName;\n\n", "")
        .replace("import 'package:app/stores.dart' show WordStore;\n", "");
    let theirs = base.replace("register('words');", "register('words', 2);");
    assert_clean(&base, &ours, &theirs, "harness.dart");
}
