//! The vocabulary of the v2 pipeline: arena-indexed entities, claims, triples,
//! the resolution cases, dispositions, plans.
//!
//! Two design rules run through this module.
//!
//! **Every fact has exactly one owner.** An entity's text lives in the arena
//! and nowhere else; which side it came from lives in `Entity::version`; which
//! triple it belongs to lives in the triple's claim slot. v1 kept the same
//! facts in six overlapping maps (`ours_entity_map`, `resolved_entities` keyed
//! by three aliases, two rename maps, `audit_index`) and spent guards keeping
//! them consistent.
//!
//! **Authority is the least each stage needs.** Matching may move claims but
//! cannot read a cell; classification may read but cannot move; rendering may
//! concatenate but cannot decide.

// ---------------------------------------------------------------------------
// Arena
// ---------------------------------------------------------------------------

/// An index into the [`Arena`]. Identity is a position in an arena, never a
/// string, and never a line number — so nothing an edit does to line numbers
/// can change what an entity *is*. (v1's ids embedded `@L<line>`, which is how
/// an unrelated edit above a function could fabricate a rename of it.)
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub(crate) struct Idx(pub u32);

/// One of the two *branch* sides.
///
/// There used to be a `Version` enum beside this one — `Base | Ours | Theirs` —
/// and an `Entity::version` field recording which of the three an entity was
/// read from. Nothing ever read either. Which version an entity belongs to is
/// owned by the claim slot it sits in (`Triple::base`/`ours`/`theirs`), decided
/// at compile time by the claim's *type*; the field was a second copy of that
/// fact, kept in sync by the one function that could not get it wrong. The
/// single-owner rule says the copy goes, and with it the enum, `Side::version`
/// (which converted between the two and was called nowhere), and one `match`
/// in `plan` that turned a version back into the side it came from.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Side {
    Ours,
    Theirs,
}

impl Side {
    #[must_use]
    pub(crate) fn flip(self) -> Side {
        match self {
            Side::Ours => Side::Theirs,
            Side::Theirs => Side::Ours,
        }
    }
}

/// The identity key of an entity: everything about *where it sits in the
/// program* and nothing about where it sits in the file.
///
/// `ordinal` disambiguates same-named siblings (overloads, repeated `impl`
/// blocks) by their order of occurrence rather than by line number.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub(crate) struct Key {
    pub parent: Option<String>,
    pub entity_type: String,
    pub name: String,
    pub ordinal: u32,
}

/// An entity as v2 sees it: text, identity, and the two precomputed indexes
/// the matcher needs. Nothing here is optional-and-recomputed-later.
#[derive(Debug, Clone)]
pub(crate) struct Entity {
    pub key: Key,
    /// Region content — the file's own bytes for this entity, including
    /// leading doc comments and modifiers.
    pub content: String,
    /// Hash of the content with the entity's own name replaced by a sentinel.
    /// Equal body hashes ⇒ the two entities differ at most in their name, so
    /// this is what lets the matcher tell a pure rename from a body edit
    /// without a separate name-diffing pass.
    pub body_hash: u64,
    /// Position in this version's ordered entity list. A merge that silently
    /// reorders entities is a user-visible change, so position is tracked
    /// explicitly rather than left to whatever order a map happens to yield.
    pub position: u32,
    /// The sem-core id this entity was read from, and a live key — not a
    /// carried-along label. `render` looks leading trivia up by it
    /// (`render::lead_by_entity`), reports which entities reached the output by
    /// it (`render::emitted_src_ids`, read at `v2::mod`), and `resolve` reads
    /// it to find an entity's children. Removing it breaks the renderer.
    pub src_id: String,
}

impl Entity {
    pub(crate) fn name(&self) -> &str {
        &self.key.name
    }
    pub(crate) fn entity_type(&self) -> &str {
        &self.key.entity_type
    }
}

/// The one place entities live.
#[derive(Debug, Default)]
pub(crate) struct Arena {
    entities: Vec<Entity>,
}

impl Arena {
    pub(crate) fn from_entities(entities: Vec<Entity>) -> Self {
        Self { entities }
    }
    pub(crate) fn get(&self, idx: Idx) -> &Entity {
        &self.entities[idx.0 as usize]
    }
    pub(crate) fn len(&self) -> usize {
        self.entities.len()
    }
}

// ---------------------------------------------------------------------------
// Claims: the right to place one entity in one triple
// ---------------------------------------------------------------------------

/// A `Claim` is a move-once handle: it names one arena entity and can be
/// spent exactly once, because it is neither `Copy` nor `Clone`. Placing the
/// same entity into two triples — the shape of the duplication bugs v1's
/// alias-keyed `resolved_entities` map was prone to — is not an error to be
/// caught, it is a program that does not compile.
///
/// One type per version, so a base claim cannot be dropped into the ours slot.
/// The *type* is the whole statement of which version a claim belongs to — the
/// slot it fits is decided at compile time, so there is nothing left for a
/// runtime `VERSION` constant to tell anyone. One was carried here anyway, `pub`
/// and never read by any caller in any crate; it is gone.
macro_rules! claim_type {
    ($name:ident) => {
        #[derive(Debug, PartialEq, Eq)]
        pub(crate) struct $name(Idx);
        impl $name {
            pub(crate) fn new(idx: Idx) -> Self {
                Self(idx)
            }
            pub(crate) fn idx(&self) -> Idx {
                self.0
            }
        }
    };
}

claim_type!(BaseClaim);
claim_type!(OursClaim);
claim_type!(TheirsClaim);

/// Every claim in the universe, produced once by the arena builder. There is
/// no other constructor, so the claim multiset is exactly the entity multiset.
#[derive(Debug)]
pub(crate) struct Claims {
    pub(crate) base: Vec<BaseClaim>,
    pub(crate) ours: Vec<OursClaim>,
    pub(crate) theirs: Vec<TheirsClaim>,
}

// ---------------------------------------------------------------------------
// Evidence and triples
// ---------------------------------------------------------------------------

/// Why the matcher believes a branch entity is the same thing as a base entity.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Link {
    /// The side has no entity for this triple.
    Absent,
    /// Same key: same name, type, parent and ordinal.
    Identity,
    /// Different name, identical body.
    BodyHash,
    /// Different name, similar body. Carries the Jaccard similarity.
    Signature { similarity: f64 },
    /// Different name, body edited too — the pairing rests on third-party call
    /// sites that moved from the old name to the new one.
    CallSiteCorroborated { similarity: f64 },
    /// No base counterpart: this side added the entity.
    Added,
}

impl Link {
    /// Test-only: no production path branches on this — the resolver reads
    /// the links directly — but the test suite asserts on the evidence the
    /// matcher recorded, so the accessor is gated behind `cfg(test)` rather
    /// than deleted.
    #[cfg(test)]
    pub(crate) fn is_rename(&self) -> bool {
        matches!(
            self,
            Link::BodyHash | Link::Signature { .. } | Link::CallSiteCorroborated { .. }
        )
    }
}

/// Relational facts a per-key table cannot express — a base entity's body
/// scattered across several new ones, for instance, which no single-key
/// lookup can represent. The matcher records them; `bind` is the stage with
/// the authority to act on them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Relational {
    /// A base entity whose key vanished on `side` and whose body reappeared
    /// inside entities that side added (split, extract, absorb). `coverage` is
    /// the fraction of the base body's signature lines found across `homes`.
    RehomedInto {
        deleted: Idx,
        side: Side,
        homes: Vec<Idx>,
        coverage: f64,
    },
    /// SPLIT with a surviving source — the extract-method refactoring. The base
    /// entity `source` is still there on `side` as `host`, but part of its body
    /// left: `moved` are base body lines that are gone from `host` and turned up
    /// in `extracted`, entities that side added.
    ///
    /// [`RehomedInto`] cannot express this: it fires only when the base key
    /// *vanishes*, so the commonest refactoring — pull a few statements out
    /// into a helper and call it — was invisible. The fact it carries is what
    /// lets the other side's edit follow its lines into the helper instead of
    /// colliding with the call that replaced them.
    ///
    /// [`RehomedInto`]: Relational::RehomedInto
    ExtractedInto {
        source: Idx,
        side: Side,
        host: Idx,
        extracted: Vec<Idx>,
        moved: Vec<String>,
    },
}

/// One entity's worth of the merge: its base, ours and theirs incarnations.
/// A triple owns its claims, so an entity appears in exactly one triple.
#[derive(Debug)]
pub struct Triple {
    pub(crate) base: Option<BaseClaim>,
    pub(crate) ours: Option<OursClaim>,
    pub(crate) theirs: Option<TheirsClaim>,
    /// Why the matcher paired these entities. Recorded on every triple and read
    /// by the test suite; no production stage branches on it, because the
    /// resolver's case is computed from the links rather than from this summary. Kept
    /// because a pairing that cannot say why it happened is the thing this
    /// pipeline exists not to do.
    #[allow(dead_code)]
    pub(crate) evidence: Evidence,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Evidence {
    pub ours_link: Link,
    pub theirs_link: Link,
}

impl Triple {
    pub(crate) fn base_idx(&self) -> Option<Idx> {
        self.base.as_ref().map(|c| c.idx())
    }
    pub(crate) fn ours_idx(&self) -> Option<Idx> {
        self.ours.as_ref().map(|c| c.idx())
    }
    pub(crate) fn theirs_idx(&self) -> Option<Idx> {
        self.theirs.as_ref().map(|c| c.idx())
    }
    pub(crate) fn side_idx(&self, side: Side) -> Option<Idx> {
        match side {
            Side::Ours => self.ours_idx(),
            Side::Theirs => self.theirs_idx(),
        }
    }
    /// The entity that names this triple, preferring base (the shared name).
    pub(crate) fn representative(&self) -> Idx {
        self.base_idx()
            .or_else(|| self.ours_idx())
            .or_else(|| self.theirs_idx())
            .expect("a triple with no claims cannot be constructed")
    }
}

/// The output of the match phase: every arena entity placed in exactly one
/// triple, plus the relational evidence that does not fit inside any one triple.
#[derive(Debug)]
pub struct Matching {
    pub(crate) arena: Arena,
    pub(crate) triples: Vec<Triple>,
    pub(crate) relational: Vec<Relational>,
}

// ---------------------------------------------------------------------------
// The resolution cases
// ---------------------------------------------------------------------------

/// The per-side actions, minus ABSENT-with-no-base (which cannot occur in a
/// constructed triple).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    Added,
    /// The key occurs nowhere on this side and the base did not have it either
    /// — reachable only as the partner of a one-sided ADD.
    Absent,
    Deleted,
    Unchanged,
    Edited,
    Renamed,
    RenameEdited,
}

/// The per-key resolution cases the resolver handles, one variant family per
/// kind of change pair — see [`Cell::ALL`].
///
/// A case that treats the two sides differently names the side it favours in an
/// orientation tag, and [`Cell::flip`] swaps ours and theirs. Keeping the side
/// in a tag rather than baked into the variant is what lets a single arm handle
/// both orientations, so the same change is not handled one way when ours made
/// it and another when theirs did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    // -- base absent: one or both sides added ----------------------------
    /// One side added it, the other never had it.
    AddedOneSide {
        adder: Side,
    },
    /// Both added it with equal bodies.
    AddedBothConvergent,
    /// Both added it with different bodies.
    AddedBothDivergent,

    // -- base present, at least one side deleted -------------------------
    DeletedBoth,
    DeleteVsUnchanged {
        deleter: Side,
    },
    DeleteVsEdit {
        deleter: Side,
    },
    DeleteVsRename {
        deleter: Side,
    },
    DeleteVsRenameEdit {
        deleter: Side,
    },

    // -- base present, both kept, no rename ------------------------------
    UnchangedBoth,
    EditOneSide {
        editor: Side,
    },
    EditBothConvergent,
    EditBothDivergent,

    // -- base present, exactly one side renamed --------------------------
    RenameVsUnchanged {
        renamer: Side,
    },
    RenameEditVsUnchanged {
        renamer: Side,
    },
    RenameVsEdit {
        renamer: Side,
    },
    RenameEditVsEdit {
        renamer: Side,
        bodies_converge: bool,
    },

    // -- base present, both sides renamed --------------------------------
    RenameBoth {
        names_converge: bool,
    },
    RenameEditVsRename {
        rename_editor: Side,
        names_converge: bool,
    },
    RenameEditBoth {
        names_converge: bool,
        bodies_converge: bool,
    },
}

impl Cell {
    /// Every case above, listed once in a fixed order. `all_cases_are_distinct`
    /// keeps this list in step with the enum.
    pub const ALL: [Cell; 38] = {
        use Side::{Ours, Theirs};
        [
            Cell::AddedOneSide { adder: Ours },
            Cell::AddedOneSide { adder: Theirs },
            Cell::AddedBothConvergent,
            Cell::AddedBothDivergent,
            Cell::DeletedBoth,
            Cell::DeleteVsUnchanged { deleter: Ours },
            Cell::DeleteVsUnchanged { deleter: Theirs },
            Cell::DeleteVsEdit { deleter: Ours },
            Cell::DeleteVsEdit { deleter: Theirs },
            Cell::DeleteVsRename { deleter: Ours },
            Cell::DeleteVsRename { deleter: Theirs },
            Cell::DeleteVsRenameEdit { deleter: Ours },
            Cell::DeleteVsRenameEdit { deleter: Theirs },
            Cell::UnchangedBoth,
            Cell::EditOneSide { editor: Ours },
            Cell::EditOneSide { editor: Theirs },
            Cell::EditBothConvergent,
            Cell::EditBothDivergent,
            Cell::RenameVsUnchanged { renamer: Ours },
            Cell::RenameVsUnchanged { renamer: Theirs },
            Cell::RenameEditVsUnchanged { renamer: Ours },
            Cell::RenameEditVsUnchanged { renamer: Theirs },
            Cell::RenameVsEdit { renamer: Ours },
            Cell::RenameVsEdit { renamer: Theirs },
            Cell::RenameEditVsEdit {
                renamer: Ours,
                bodies_converge: true,
            },
            Cell::RenameEditVsEdit {
                renamer: Ours,
                bodies_converge: false,
            },
            Cell::RenameEditVsEdit {
                renamer: Theirs,
                bodies_converge: true,
            },
            Cell::RenameEditVsEdit {
                renamer: Theirs,
                bodies_converge: false,
            },
            Cell::RenameBoth {
                names_converge: true,
            },
            Cell::RenameBoth {
                names_converge: false,
            },
            Cell::RenameEditVsRename {
                rename_editor: Ours,
                names_converge: true,
            },
            Cell::RenameEditVsRename {
                rename_editor: Ours,
                names_converge: false,
            },
            Cell::RenameEditVsRename {
                rename_editor: Theirs,
                names_converge: true,
            },
            Cell::RenameEditVsRename {
                rename_editor: Theirs,
                names_converge: false,
            },
            Cell::RenameEditBoth {
                names_converge: true,
                bodies_converge: true,
            },
            Cell::RenameEditBoth {
                names_converge: true,
                bodies_converge: false,
            },
            Cell::RenameEditBoth {
                names_converge: false,
                bodies_converge: true,
            },
            Cell::RenameEditBoth {
                names_converge: false,
                bodies_converge: false,
            },
        ]
    };

    /// The case you would get by swapping ours and theirs.
    ///
    /// Test-only: used by the swap tests here and in `classify` to check that
    /// swapping the two sides maps each case to its mirror. Not reached from the
    /// shipped path.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn flip(self) -> Cell {
        match self {
            Cell::AddedOneSide { adder } => Cell::AddedOneSide {
                adder: adder.flip(),
            },
            Cell::DeleteVsUnchanged { deleter } => Cell::DeleteVsUnchanged {
                deleter: deleter.flip(),
            },
            Cell::DeleteVsEdit { deleter } => Cell::DeleteVsEdit {
                deleter: deleter.flip(),
            },
            Cell::DeleteVsRename { deleter } => Cell::DeleteVsRename {
                deleter: deleter.flip(),
            },
            Cell::DeleteVsRenameEdit { deleter } => Cell::DeleteVsRenameEdit {
                deleter: deleter.flip(),
            },
            Cell::EditOneSide { editor } => Cell::EditOneSide {
                editor: editor.flip(),
            },
            Cell::RenameVsUnchanged { renamer } => Cell::RenameVsUnchanged {
                renamer: renamer.flip(),
            },
            Cell::RenameEditVsUnchanged { renamer } => Cell::RenameEditVsUnchanged {
                renamer: renamer.flip(),
            },
            Cell::RenameVsEdit { renamer } => Cell::RenameVsEdit {
                renamer: renamer.flip(),
            },
            Cell::RenameEditVsEdit {
                renamer,
                bodies_converge,
            } => Cell::RenameEditVsEdit {
                renamer: renamer.flip(),
                bodies_converge,
            },
            Cell::RenameEditVsRename {
                rename_editor,
                names_converge,
            } => Cell::RenameEditVsRename {
                rename_editor: rename_editor.flip(),
                names_converge,
            },
            // Symmetric cells are their own mirror image.
            other => other,
        }
    }

    /// The per-side actions this cell encodes, as the table's alphabet.
    pub fn actions(self) -> (Action, Action) {
        use Action as A;
        let oriented =
            |favoured: Side, favoured_action: Action, other_action: Action| match favoured {
                Side::Ours => (favoured_action, other_action),
                Side::Theirs => (other_action, favoured_action),
            };
        match self {
            Cell::AddedOneSide { adder } => oriented(adder, A::Added, A::Absent),
            Cell::AddedBothConvergent | Cell::AddedBothDivergent => (A::Added, A::Added),
            Cell::DeletedBoth => (A::Deleted, A::Deleted),
            Cell::DeleteVsUnchanged { deleter } => oriented(deleter, A::Deleted, A::Unchanged),
            Cell::DeleteVsEdit { deleter } => oriented(deleter, A::Deleted, A::Edited),
            Cell::DeleteVsRename { deleter } => oriented(deleter, A::Deleted, A::Renamed),
            Cell::DeleteVsRenameEdit { deleter } => oriented(deleter, A::Deleted, A::RenameEdited),
            Cell::UnchangedBoth => (A::Unchanged, A::Unchanged),
            Cell::EditOneSide { editor } => oriented(editor, A::Edited, A::Unchanged),
            Cell::EditBothConvergent | Cell::EditBothDivergent => (A::Edited, A::Edited),
            Cell::RenameVsUnchanged { renamer } => oriented(renamer, A::Renamed, A::Unchanged),
            Cell::RenameEditVsUnchanged { renamer } => {
                oriented(renamer, A::RenameEdited, A::Unchanged)
            }
            Cell::RenameVsEdit { renamer } => oriented(renamer, A::Renamed, A::Edited),
            Cell::RenameEditVsEdit { renamer, .. } => oriented(renamer, A::RenameEdited, A::Edited),
            Cell::RenameBoth { .. } => (A::Renamed, A::Renamed),
            Cell::RenameEditVsRename { rename_editor, .. } => {
                oriented(rename_editor, A::RenameEdited, A::Renamed)
            }
            Cell::RenameEditBoth { .. } => (A::RenameEdited, A::RenameEdited),
        }
    }

    /// Cases where one side changed nothing that the other side's change could
    /// contradict, so no conflict should be reported for them.
    ///
    /// `pub` with no caller inside this crate: the test suite uses it to check
    /// the resolver's output on these cases. It earns its visibility there
    /// rather than by being called here.
    pub fn is_trivially_clean(self) -> bool {
        matches!(
            self,
            Cell::UnchangedBoth
                | Cell::EditOneSide { .. }
                | Cell::AddedOneSide { .. }
                | Cell::AddedBothConvergent
                | Cell::EditBothConvergent
                | Cell::DeletedBoth
                | Cell::DeleteVsUnchanged { .. }
                | Cell::RenameVsUnchanged { .. }
                | Cell::RenameEditVsUnchanged { .. }
                | Cell::RenameBoth {
                    names_converge: true
                }
        )
    }
}

// ---------------------------------------------------------------------------
// Dispositions and plans
// ---------------------------------------------------------------------------

/// What the merge decided to *do* with one triple. `Resolve` produces these
/// from cells and nothing else; `bind` may revise one into a conflict; `plan`
/// may only order them; `render` may only concatenate them.
///
/// **Not `Clone`, deliberately.** That sentence above is the whole authority
/// argument for the back half of the pipeline, and it is only true if there is
/// one decision to revise. A clone is a second copy of the decision downstream
/// of the only stage allowed to make it: `plan` or `render` could hold a
/// disposition that `bind` had already revised, and "plan may only order" would
/// become a description of what the code happens to do rather than a fact about
/// what it can do. An audit of this file's derives found this one unused —
/// nothing needed it.
#[derive(Debug)]
pub(crate) enum Disposition {
    /// Emit this text.
    Emit { text: String, name: String },
    /// Emit nothing: the entity is gone from the merge.
    Drop,
    /// Emit a conflict. `text` is the rendered marker block; `conflict` is the
    /// typed record that makes `is_clean` a question about types, not markers.
    Conflict {
        text: String,
        conflict: crate::conflict::EntityConflict,
    },
}

impl Disposition {
    pub(crate) fn is_conflict(&self) -> bool {
        matches!(self, Disposition::Conflict { .. })
    }
}

/// Which line of the merge summary this triple lands on — exactly one, always.
///
/// `MergeStats` used to be a shared `&mut` counter that every stage added to,
/// and `bind` — which is allowed to revise a disposition — had to *repair* the
/// count `resolve` had already made, with `saturating_sub`. That is an
/// ownership violation wearing a runtime guard: `saturating_sub` clamps at zero
/// rather than being impossible, so a missed decrement is silent, and a stage
/// that revised a disposition without knowing which bucket the previous stage
/// had chosen simply added a second count (extract-landing did exactly that).
///
/// Here the bucket is a *field of the decision*. Revising a disposition
/// replaces its tally in the same move, so the two cannot drift, and the
/// summary is folded once from the finished dispositions — read-only, at the
/// end, by the stage that owns the result.
///
/// **Not `Clone` and not `Copy`, deliberately** — the one place in this file
/// where a fieldless enum is denied the derive it would normally get. The
/// paragraph above is the reason: a tally is not a value describing a decision,
/// it *is* that decision's line of the summary, and the whole point of moving
/// it into the decision was that revising one revises the other in the same
/// move. A copy is a count that can outlive the revision it belonged to, which
/// is precisely the `saturating_sub` bug this type was introduced to make
/// impossible — reintroduced by a derive. The same audit that caught the
/// unused derive above caught this one too; nothing needed it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Tally {
    Unchanged,
    OursOnly,
    TheirsOnly,
    BothMerged,
    AddedOurs,
    AddedTheirs,
    Deleted,
    Conflicted,
}

impl Tally {
    /// The tally of a one-sided survivor, from the side that acted.
    pub(crate) fn only(side: Side) -> Tally {
        match side {
            Side::Ours => Tally::OursOnly,
            Side::Theirs => Tally::TheirsOnly,
        }
    }
    /// The tally of a one-sided addition, from the side that added.
    pub(crate) fn added(side: Side) -> Tally {
        match side {
            Side::Ours => Tally::AddedOurs,
            Side::Theirs => Tally::AddedTheirs,
        }
    }
}

/// Where an entity goes, expressed relative to the shared history rather than
/// to one peer's file. Ordered: this is the output order.
///
/// Deterministic and independent of which side is "ours", so two replicas
/// folding in different orders emit the same bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Anchor {
    /// Position of the nearest preceding base entity (the shared anchor), or 0
    /// for "before everything". Base positions are shared by both replicas.
    pub after_base: u32,
    /// Tie-break among entities anchored to the same base position: additions
    /// sort after survivors, then by identity key. Never by peer-relative
    /// position, which is what made ours-skeleton placement diverge.
    pub tier: u8,
    /// Rank inside the gap, for additions that share one anchor.
    ///
    /// The identity key alone used to decide this, and an identity key is
    /// `(parent, entity_type, name)` — so a `const` and the function written
    /// under it came back sorted by *type*, an order that exists in no version
    /// of the file. Where two co-anchored additions appear together in some
    /// input version, that version has already answered the question, and the
    /// answer is read off it; where no version has both, nothing has been
    /// written down and the key still decides.
    ///
    /// Read off a version, never off a side, so it survives the n-way fold:
    /// a pair that some input orders is ordered the same way in every fold
    /// order, and a pair no input orders never becomes ordered by one.
    pub within: u32,
    pub key: Key,
}

// ---------------------------------------------------------------------------
// Entity order as a language property
// ---------------------------------------------------------------------------

/// Is the ORDER of top-level definitions part of the language's semantics?
///
/// Modeling a file as an unordered entity set plus an ordered list of
/// effectful items works for most languages, but not all — OCaml and F#
/// evaluate top-level definitions in order, C without prototypes cannot call
/// a function declared below, and a shell script's functions must exist
/// before the line that calls them. In those languages an entity's position
/// is as effectful as an import's, so the ORDER rule applies to the entity
/// sequence itself.
///
/// The flag changes exactly one decision, and only in the cell where both sides
/// asserted an order (see [`super::plan::plan`]). Everywhere else the placement
/// rule is the same, because the anchor rule already honours a one-sided
/// reorder in every language: a side that moved something said something, and
/// discarding it would be loss whatever the grammar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EntityOrder {
    /// Definition order carries no meaning: `plan` may choose any deterministic
    /// coordinate system (Python defs, Rust items, TypeScript functions).
    Unordered,
    /// Definition order is semantics: it must be preserved or conflicted on.
    Effectful,
}

impl EntityOrder {
    /// The per-language table. Membership is a claim about the *language*, so
    /// each entry names why: definitions are evaluated in file order, or a use
    /// may not precede its declaration.
    pub(crate) fn of_path(file_path: &str) -> EntityOrder {
        let p = file_path.to_ascii_lowercase();
        const EFFECTFUL: [&str; 20] = [
            // C / C++: a call above the declaration is an error (or, worse,
            // an implicit declaration), so moving a definition is semantics.
            ".c", ".h", ".cpp", ".cc", ".cxx", ".hpp", ".hh", ".hxx",
            // ML family: `let` bindings are evaluated top to bottom and a
            // definition is not in scope above itself.
            ".ml", ".mli", ".sml", ".sig", ".fs", ".fsi", ".fsx",
            // Shell: a function must be defined before the line that calls it.
            ".sh", ".bash", ".zsh", // SQL: statements are executed in order.
            ".sql", ".psql",
        ];
        if EFFECTFUL.iter().any(|e| p.ends_with(e)) {
            EntityOrder::Effectful
        } else {
            EntityOrder::Unordered
        }
    }
}

/// The text a language writes BETWEEN two entities to make them a sequence.
///
/// It belongs to the ENCODING, not to any entity: `"a": 1,` and `"a": 1` are
/// one member of an object written at two positions in it. Carrying it inside
/// the entity's text made the carrier compare representations rather than
/// images (Hoare 1972), and the sequence then leaked into the merge: appending
/// a key rewrites the previous member's line to add a comma, so a side that
/// added a key was classified as having EDITED the member above it, and a value
/// bump beside a key addition — the commonest concurrent pair a `package.json`
/// will ever see — came back as a both-modified conflict on a member only one
/// side had touched.
///
/// The separator is therefore stripped where the arena is built and re-derived
/// where the file is rendered, so no stage in between can see it. Nothing but
/// JSON is listed: YAML and TOML separate members by line, and a language whose
/// separator is a line break has nothing to quotient.
pub(crate) fn entity_separator(file_path: &str) -> Option<&'static str> {
    let p = file_path.to_ascii_lowercase();
    const COMMA_SEPARATED: [&str; 3] = [".json", ".jsonc", ".json5"];
    COMMA_SEPARATED
        .iter()
        .any(|e| p.ends_with(e))
        .then_some(",")
}

/// The two sides gave entities they SHARE contradictory relative order, in a
/// file where order is semantics. No sequence satisfies both, so there is no
/// clean answer — this is the ORDER class at the entity layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrderConflict {
    pub base: Vec<String>,
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_cases_are_distinct() {
        // No two entries in ALL are the same case, and the list length matches
        // the count the array type declares.
        let unique: HashSet<String> = Cell::ALL.iter().map(|c| format!("{c:?}")).collect();
        assert_eq!(unique.len(), 38, "Cell::ALL has duplicates");
        assert_eq!(Cell::ALL.len(), 38);
    }

    #[test]
    fn swapping_sides_twice_is_identity() {
        for cell in Cell::ALL {
            assert_eq!(
                cell.flip().flip(),
                cell,
                "swapping sides twice changed the case at {cell:?}"
            );
        }
    }

    #[test]
    fn swapping_sides_maps_cases_onto_cases() {
        let all: HashSet<String> = Cell::ALL.iter().map(|c| format!("{c:?}")).collect();
        let flipped: HashSet<String> = Cell::ALL
            .iter()
            .map(|c| format!("{:?}", c.flip()))
            .collect();
        assert_eq!(all, flipped, "swap produced a case outside the set");
    }

    #[test]
    fn swapping_sides_swaps_the_action_pair() {
        for cell in Cell::ALL {
            let (o, t) = cell.actions();
            let (fo, ft) = cell.flip().actions();
            assert_eq!((o, t), (ft, fo), "actions disagree with swap at {cell:?}");
        }
    }

    #[test]
    fn every_action_pair_has_a_case() {
        use Action::*;
        // Each of the five change kinds can pair with each of the five on the
        // other side; both-added and the two one-sided adds round out what a
        // constructed triple can show. Check every pairing maps to some case.
        let mut seen: HashSet<(Action, Action)> = HashSet::new();
        for cell in Cell::ALL {
            seen.insert(cell.actions());
        }
        for a in [Deleted, Unchanged, Edited, Renamed, RenameEdited] {
            for b in [Deleted, Unchanged, Edited, Renamed, RenameEdited] {
                assert!(seen.contains(&(a, b)), "no case for ({a:?}, {b:?})");
            }
        }
        assert!(seen.contains(&(Added, Added)));
    }
}
