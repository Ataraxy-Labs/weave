//! The findings wire format, v1.1.0 — serialisable form of
//! `crates/weave-mcp/schema/weave-findings.schema.json`.
//!
//! `weave-mcp/src/findings.rs` produces the **per-merge** document (one merge
//! of one file, `scope: "merge"`, declared version 1.0.0 because it uses no
//! 1.1 field). This module produces the **repo-scope** document (`weave check`,
//! `scope: "repo"`), which needs the same vocabulary from a crate the MCP
//! server can depend on. Both are the same schema; the difference is which
//! optional fields are populated.
//!
//! v1.1.0 is a *minor* bump over v1.0.0: it adds the optional top-level
//! `scope` and the optional `finding.side`. A v1.0.0 document is still valid
//! under it, which is exactly what minor is supposed to mean.

use serde::{Deserialize, Serialize};

use weave_core::conflict::{ConflictKind, EntityConflict};

/// The version a document that uses a 1.1 field must declare.
pub const SCHEMA_VERSION_1_1: &str = "1.1.0";

// ---------------------------------------------------------------------------
// Conflict repair lines — one owner for both producers
// ---------------------------------------------------------------------------

/// What a document calls the two sides of the comparison it describes.
///
/// This is the *only* thing that legitimately differs between the two producers
/// of a `class=CONFLICT` finding: `weave-mcp` describes a merge, whose sides are
/// ours and theirs; `weave patch` describes a patch landing on a target, whose
/// sides are the target and the patch. Everything else — which kinds exist,
/// what each one means, what to do about it — is one fact, and used to be
/// written down twice in prose that had already drifted apart in wording.
///
/// Passing the vocabulary in is what lets there be one owner: the audience is a
/// parameter, not a reason to fork the mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sides {
    /// Sentence-initial form — "Ours", "The target".
    pub ours: &'static str,
    pub theirs: &'static str,
    /// Mid-sentence form — "ours", "the target".
    pub ours_mid: &'static str,
    pub theirs_mid: &'static str,
}

impl Sides {
    /// A three-way merge of two branches.
    pub const MERGE: Sides = Sides {
        ours: "Ours",
        theirs: "Theirs",
        ours_mid: "ours",
        theirs_mid: "theirs",
    };
    /// A patch applied to a target file.
    pub(crate) const PATCH: Sides = Sides {
        ours: "The target",
        theirs: "The patch",
        ours_mid: "the target",
        theirs_mid: "the patch",
    };

    fn initial(self, is_ours: bool) -> &'static str {
        if is_ours {
            self.ours
        } else {
            self.theirs
        }
    }
    fn mid(self, is_ours: bool) -> &'static str {
        if is_ours {
            self.ours_mid
        } else {
            self.theirs_mid
        }
    }
}

/// The wire `kind` and the repair line for one conflict.
///
/// The kind comes from [`ConflictKind::wire_kind`] — weave-core owns the tag,
/// because the exhaustive match belongs next to the enum. The prose is owned
/// here, because it is schema-side and audience-dependent.
///
/// Every line below is *mechanical*: it restates the three-way fact weave
/// already established and names the step that follows from it, and it stops
/// there. None of them chooses a side, guesses intent, or recommends one
/// resolution over another — that is the same discipline the marker's
/// `refused_by:` line keeps, stated at the length a findings document can
/// afford. A finding whose suggestion only says "there is a conflict" has told
/// an agent nothing it could not already see; a finding that guesses has told
/// it something weave does not know.
pub fn conflict_advice(c: &EntityConflict, sides: Sides) -> (&'static str, String) {
    let name = &c.entity_name;
    let suggestion = match &c.kind {
        ConflictKind::RenameRename {
            base_name,
            ours_name,
            theirs_name,
        } => format!(
            "{} renamed `{base_name}` to `{ours_name}` while {} renamed it to `{theirs_name}`. \
             Pick one name, then rewrite every call site to it.",
            sides.ours, sides.theirs_mid
        ),
        ConflictKind::RenameModify {
            old_name,
            new_name,
            renamed_in_ours,
        } => format!(
            "{} renamed `{old_name}` to `{new_name}` while the other edited its body. \
             Keep the new name and replay the body edit onto it.",
            sides.initial(*renamed_in_ours)
        ),
        ConflictKind::ModifyDelete { modified_in_ours } => format!(
            "`{name}` was deleted on one side and modified on the other ({} kept it). \
             Decide whether it still has a reason to exist before resolving.",
            sides.mid(*modified_in_ours)
        ),
        ConflictKind::BothAdded => format!(
            "`{name}` was added independently by {} and {}. Reconcile them into one definition.",
            sides.ours_mid, sides.theirs_mid
        ),
        ConflictKind::BothModified => format!(
            "{} and {} both changed the body of `{name}` in ways weave will not guess between.",
            sides.ours, sides.theirs_mid
        ),
    };
    (c.kind.wire_kind(), suggestion)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Findings {
    pub schema_version: String,
    pub file: String,
    /// `merge` — this document describes one merge of one file.
    /// `repo`  — this document describes a repo-scope binding check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub clean: bool,
    pub confidence: String,
    pub findings: Vec<Finding>,
    pub ops: Vec<Op>,
    pub bindings: Bindings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    pub class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub entity: String,
    pub entity_type: String,
    pub source: String,
    /// Which side of the merge caused the finding. v1.1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ours: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theirs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RelatedRef>,
}

impl Finding {
    pub(crate) fn derived(class: &str, entity: &str, entity_type: &str) -> Self {
        Finding {
            class: class.to_string(),
            kind: None,
            entity: entity.to_string(),
            entity_type: entity_type.to_string(),
            source: "derived".to_string(),
            side: None,
            complexity: None,
            confidence: None,
            base: None,
            ours: None,
            theirs: None,
            suggestion: None,
            warning_kind: None,
            related: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelatedRef {
    pub entity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Op {
    pub entity: String,
    pub entity_type: String,
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub conflicted: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub fallback: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Bindings {
    pub renames: Vec<Rename>,
    pub references: Vec<Reference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rename {
    pub from: String,
    pub to: String,
    pub side: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reference {
    pub entity: String,
    pub defined_in_output: bool,
    pub referenced_in_output: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}
