//! The element union and the iota gate, end to end through `entity_merge`.

use weave_core::{entity_merge, ResolutionStrategy};

const OPCODES: &str = "package vm\n\ntype Opcode byte\n\nconst (\n\tOpPush Opcode = iota\n\tOpPop\n\tOpEnd // must be last\n)\n";

#[test]
fn both_sides_inserting_ahead_of_an_iota_constant_is_a_conflict() {
    // OpEnd would be n+3 — a value neither side gave it.
    let ours = OPCODES.replace("\tOpEnd", "\tOpShiftLeft\n\tOpEnd");
    let theirs = OPCODES.replace("\tOpEnd", "\tOpBitNot\n\tOpEnd");
    let r = entity_merge(OPCODES, &ours, &theirs, "vm/opcodes.go");
    assert!(!r.is_clean(), "{}", r.content);
}

#[test]
fn appending_after_the_last_iota_constant_stays_clean() {
    let ours = OPCODES.replace(
        "\tOpEnd // must be last\n",
        "\tOpEnd // must be last\n\tOpA\n",
    );
    let theirs = OPCODES.replace(
        "\tOpEnd // must be last\n",
        "\tOpEnd // must be last\n\tOpB\n",
    );
    let r = entity_merge(OPCODES, &ours, &theirs, "vm/opcodes.go");
    assert!(r.is_clean(), "{}", r.content);
    assert!(r.content.contains("OpA") && r.content.contains("OpB"));
}

#[test]
fn one_side_renumbering_its_own_block_is_that_side_s_edit() {
    let ours = OPCODES.replace("\tOpEnd", "\tOpShiftLeft\n\tOpEnd");
    let theirs = OPCODES.replace(
        "package vm\n",
        "package vm\n\n// Package vm runs programs.\n",
    );
    let r = entity_merge(OPCODES, &ours, &theirs, "vm/opcodes.go");
    assert!(r.is_clean(), "{}", r.content);
}

#[test]
fn switch_cases_two_branches_added_to_one_method_merge_by_element_union() {
    let base = "package vm\n\nfunc (vm *VM) Run() {\n\tfor {\n\t\tswitch op {\n\t\tcase OpPush:\n\t\t\tvm.push()\n\n\t\tcase OpEnd:\n\t\t\treturn\n\t\t}\n\t}\n}\n";
    let ours = base.replace(
        "\t\tcase OpEnd:",
        "\t\tcase OpShiftLeft:\n\t\t\tvm.shl()\n\n\t\tcase OpEnd:",
    );
    let theirs = base.replace(
        "\t\tcase OpEnd:",
        "\t\tcase OpBitNot:\n\t\t\tvm.not()\n\n\t\tcase OpEnd:",
    );
    let r = entity_merge(base, &ours, &theirs, "vm/vm.go");
    assert!(r.is_clean(), "{}", r.content);
    assert!(r.content.contains("case OpShiftLeft:") && r.content.contains("case OpBitNot:"));
    assert!(r
        .audit
        .iter()
        .any(|a| matches!(a.resolution, ResolutionStrategy::ElementUnion)));
    let swapped = entity_merge(base, &theirs, &ours, "vm/vm.go");
    assert_eq!(r.content, swapped.content);
}
