//! [`Cell`] → [`Disposition`]. Pure with respect to the merge: it reads text
//! and produces text, and it decides nothing about *where* anything goes.
//!
//! Every arm is a `Cell`, so this is the one place a resolution case turns into
//! policy — and the compiler will not let a case go unanswered. Content-level
//! merging bottoms out in diff3 (`diffy`) when both sides changed the same body.
//!
//! Authority: resolve may read the arena and call leaf mergers. It may not move
//! claims, reorder anything, or look at another triple.

use sem_core::model::entity::SemanticEntity;

use super::types::*;
use crate::binding::replace_at_word_boundaries;
use crate::conflict::{
    classify_conflict, ConflictComplexity, ConflictKind, EntityConflict, MarkerFormat,
};
use crate::merge::ResolutionStrategy;

/// Everything resolve is allowed to know beyond the triple itself: language
/// dispositions and the parsed children a container merge needs.
pub(crate) struct ResolveCtx<'a> {
    pub marker_format: &'a MarkerFormat,
    /// Re-indentation is semantics here, so the whitespace-only shortcut is
    /// unsound.
    pub indent_sensitive: bool,
    /// Decorators compose (Python/TS) rather than annotate (Java/C#), so two
    /// sides adding one is a fabricated composition order.
    pub decorators_compose: bool,
    pub base_all: &'a [SemanticEntity],
    pub ours_all: &'a [SemanticEntity],
    pub theirs_all: &'a [SemanticEntity],
    /// What this merge may reach outside itself. `container_merge` asks it for
    /// a second-opinion line merge when the in-process one refuses.
    pub host: &'a crate::host::Host,
}

/// What resolve decided, plus the facts `bind` needs from it.
pub(crate) struct Resolved {
    pub disposition: Disposition,
    pub strategy: ResolutionStrategy,
    /// A rename this triple carries, as (old name, new name). `bind` repairs
    /// call sites; resolve merely reports what it saw.
    ///
    /// Written by this module only. `bind` revises decisions through
    /// [`Resolved::revised`] rather than field by field, which is what keeps
    /// that true — see the note there.
    rename: Option<(String, String)>,
    /// Which line of the summary this decision lands on. Travels WITH the
    /// disposition, so a later stage that revises one revises the other.
    pub tally: Tally,
}

impl Resolved {
    /// The rename this decision carries, for the pass that repairs its call
    /// sites. Read-only outside this module: the field has one writer.
    pub(crate) fn rename(&self) -> Option<&(String, String)> {
        self.rename.as_ref()
    }

    /// The decision a later stage reached **instead of** this one — the whole
    /// decision, in one move.
    ///
    /// `bind` is allowed to revise what `resolve` decided, and did so by
    /// assigning the four fields one at a time at three sites. That gave
    /// `rename` two writing modules: `resolve` decided a rename, `bind`
    /// cleared it, and nothing
    /// in either signature said the two shared a field. Revision is
    /// construction here instead, so `rename` has exactly one author.
    ///
    /// A revision carries no rename, and that is an invariant rather than a
    /// convenience: [`bind`](super::bind::bind) runs `rename_repair` **first**,
    /// before any pass that can revise a decision, so every rename `resolve`
    /// reported has already been consumed by the time this constructor can be
    /// called. Stating it once here replaces remembering it at each site.
    pub(crate) fn revised(
        disposition: Disposition,
        strategy: ResolutionStrategy,
        tally: Tally,
    ) -> Self {
        Self {
            disposition,
            strategy,
            rename: None,
            tally,
        }
    }
}

pub(crate) fn resolve(
    arena: &Arena,
    triple: &Triple,
    cell: Cell,
    ctx: &ResolveCtx<'_>,
) -> Resolved {
    let text = |idx: Option<Idx>| idx.map(|i| arena.get(i).content.clone());
    let name = |idx: Option<Idx>| {
        idx.map(|i| arena.get(i).name().to_string())
            .unwrap_or_default()
    };
    let etype = |idx: Option<Idx>| {
        idx.map(|i| arena.get(i).entity_type().to_string())
            .unwrap_or_default()
    };
    let b = triple.base_idx();
    let o = triple.ours_idx();
    let t = triple.theirs_idx();
    let side_idx = |s: Side| triple.side_idx(s);

    // The rename this triple carries, if any: base name → surviving name.
    let rename_of = |renamer: Side| -> Option<(String, String)> {
        let old = name(b);
        let new = name(side_idx(renamer));
        (!old.is_empty() && !new.is_empty() && old != new).then_some((old, new))
    };

    let emit = |idx: Option<Idx>, strategy: ResolutionStrategy, tally: Tally| Resolved {
        disposition: Disposition::Emit {
            text: text(idx).unwrap_or_default(),
            name: name(idx),
        },
        strategy,
        rename: None,
        tally,
    };

    match cell {
        // -- nothing to arbitrate ---------------------------------------
        Cell::UnchangedBoth => emit(o.or(t), ResolutionStrategy::Unchanged, Tally::Unchanged),
        Cell::EditOneSide { editor } => emit(
            side_idx(editor),
            match editor {
                Side::Ours => ResolutionStrategy::OursOnly,
                Side::Theirs => ResolutionStrategy::TheirsOnly,
            },
            Tally::only(editor),
        ),
        Cell::EditBothConvergent => emit(o, ResolutionStrategy::ContentEqual, Tally::BothMerged),
        Cell::AddedOneSide { adder } => emit(
            side_idx(adder),
            match adder {
                Side::Ours => ResolutionStrategy::AddedOurs,
                Side::Theirs => ResolutionStrategy::AddedTheirs,
            },
            Tally::added(adder),
        ),
        Cell::AddedBothConvergent => emit(o, ResolutionStrategy::ContentEqual, Tally::AddedOurs),
        Cell::DeletedBoth | Cell::DeleteVsUnchanged { .. } => Resolved {
            disposition: Disposition::Drop,
            strategy: ResolutionStrategy::Deleted,
            rename: None,
            tally: Tally::Deleted,
        },

        // -- a rename one side did and the other did not contradict ------
        Cell::RenameVsUnchanged { renamer } | Cell::RenameEditVsUnchanged { renamer } => {
            let (from, to) = rename_of(renamer).unwrap_or_default();
            Resolved {
                disposition: Disposition::Emit {
                    text: text(side_idx(renamer)).unwrap_or_default(),
                    name: to.clone(),
                },
                strategy: ResolutionStrategy::Renamed {
                    from: from.clone(),
                    to: to.clone(),
                },
                rename: (!from.is_empty()).then_some((from, to)),
                tally: Tally::only(renamer),
            }
        }
        Cell::RenameBoth {
            names_converge: true,
        } => {
            let (from, to) = rename_of(Side::Ours).unwrap_or_default();
            Resolved {
                disposition: Disposition::Emit {
                    text: text(o).unwrap_or_default(),
                    name: to.clone(),
                },
                strategy: ResolutionStrategy::Renamed {
                    from: from.clone(),
                    to: to.clone(),
                },
                rename: (!from.is_empty()).then_some((from, to)),
                tally: Tally::BothMerged,
            }
        }

        // -- both edited the body ----------------------------------------
        Cell::EditBothDivergent => {
            let (base_rc, ours_rc, theirs_rc) = (
                text(b).unwrap_or_default(),
                text(o).unwrap_or_default(),
                text(t).unwrap_or_default(),
            );
            intra_entity(arena, triple, ctx, &base_rc, &ours_rc, &theirs_rc, o, None)
        }

        // -- one side renamed, the other edited --------------------------
        // The renaming developer and the editing developer each acted without
        // seeing the other, and the merge cannot tell whether the edit was
        // meant for the entity under its old contract or its new one. diff3
        // *would* usually merge this — which is exactly the danger: a clean
        // result here is a decision nobody made. Conflict is the honest
        // verdict, and it is this repo's recorded policy for the cell
        // (`test_rename_plus_modify_auto_resolves`).
        Cell::RenameVsEdit { renamer } | Cell::RenameEditVsEdit { renamer, .. } => {
            let (from, to) = rename_of(renamer).unwrap_or_default();
            let (base_rc, ours_rc, theirs_rc) = (text(b), text(o), text(t));
            let complexity =
                classify_conflict(base_rc.as_deref(), ours_rc.as_deref(), theirs_rc.as_deref());
            let conflict = EntityConflict {
                entity_name: name(b),
                entity_type: etype(b),
                kind: ConflictKind::RenameModify {
                    old_name: from,
                    new_name: to,
                    renamed_in_ours: renamer == Side::Ours,
                },
                complexity: match complexity {
                    ConflictComplexity::Syntax => ConflictComplexity::SyntaxFunctional,
                    other => other,
                },
                ours_content: ours_rc,
                theirs_content: theirs_rc,
                base_content: base_rc,
            };
            conflict_disposition(conflict, ctx, ResolutionStrategy::ConflictRenameModify)
        }

        // -- the honest conflicts ----------------------------------------
        Cell::DeleteVsEdit { deleter }
        | Cell::DeleteVsRename { deleter }
        | Cell::DeleteVsRenameEdit { deleter } => {
            let survivor = deleter.flip();
            let survivor_rc = text(side_idx(survivor));
            let base_rc = text(b);
            let modified_in_ours = survivor == Side::Ours;
            let (ours_content, theirs_content) = match survivor {
                Side::Ours => (survivor_rc.clone(), None),
                Side::Theirs => (None, survivor_rc.clone()),
            };
            let complexity = classify_conflict(
                base_rc.as_deref(),
                ours_content.as_deref(),
                theirs_content.as_deref(),
            );
            let conflict = EntityConflict {
                entity_name: name(b),
                entity_type: etype(b),
                kind: ConflictKind::ModifyDelete { modified_in_ours },
                complexity,
                ours_content,
                theirs_content,
                base_content: base_rc,
            };
            conflict_disposition(conflict, ctx, ResolutionStrategy::ConflictModifyDelete)
        }
        Cell::AddedBothDivergent => {
            let (ours_rc, theirs_rc) = (text(o), text(t));
            let complexity = classify_conflict(None, ours_rc.as_deref(), theirs_rc.as_deref());
            let conflict = EntityConflict {
                entity_name: name(o.or(t)),
                entity_type: etype(o.or(t)),
                kind: ConflictKind::BothAdded,
                complexity,
                ours_content: ours_rc,
                theirs_content: theirs_rc,
                base_content: None,
            };
            conflict_disposition(conflict, ctx, ResolutionStrategy::ConflictBothAdded)
        }
        Cell::RenameBoth {
            names_converge: false,
        }
        | Cell::RenameEditVsRename {
            names_converge: false,
            ..
        }
        | Cell::RenameEditBoth {
            names_converge: false,
            ..
        } => {
            let (base_rc, ours_rc, theirs_rc) = (text(b), text(o), text(t));
            let conflict = EntityConflict {
                entity_name: name(b),
                entity_type: etype(b),
                kind: ConflictKind::RenameRename {
                    base_name: name(b),
                    ours_name: name(o),
                    theirs_name: name(t),
                },
                complexity: ConflictComplexity::Syntax,
                ours_content: ours_rc,
                theirs_content: theirs_rc,
                base_content: base_rc,
            };
            conflict_disposition(conflict, ctx, ResolutionStrategy::ConflictRenameRename)
        }

        // -- both renamed to the SAME name, and at least one edited ------
        // The name is settled; what is left is an ordinary body merge.
        Cell::RenameEditVsRename {
            names_converge: true,
            ..
        }
        | Cell::RenameEditBoth {
            names_converge: true,
            bodies_converge: false,
        } => {
            let (from, to) = rename_of(Side::Ours).unwrap_or_default();
            let base_rc = replace_at_word_boundaries(&text(b).unwrap_or_default(), &from, &to);
            let mut r = intra_entity(
                arena,
                triple,
                ctx,
                &base_rc,
                &text(o).unwrap_or_default(),
                &text(t).unwrap_or_default(),
                o,
                None,
            );
            if !r.disposition.is_conflict() && !from.is_empty() {
                r.rename = Some((from, to));
            }
            r
        }
        Cell::RenameEditBoth {
            names_converge: true,
            bodies_converge: true,
        } => {
            let (from, to) = rename_of(Side::Ours).unwrap_or_default();
            Resolved {
                disposition: Disposition::Emit {
                    text: text(o).unwrap_or_default(),
                    name: to.clone(),
                },
                strategy: ResolutionStrategy::ContentEqual,
                rename: (!from.is_empty()).then_some((from, to)),
                tally: Tally::BothMerged,
            }
        }
    }
}

/// The intra-entity merge ladder: diff3, then the two structure-aware leaves,
/// then an honest conflict. Identical to v1's ladder — the change is that it is
/// reached only from the cells that mean "both sides changed this body", rather
/// than from a nest of booleans.
#[allow(clippy::too_many_arguments)]
fn intra_entity(
    arena: &Arena,
    triple: &Triple,
    ctx: &ResolveCtx<'_>,
    base_rc: &str,
    ours_rc: &str,
    theirs_rc: &str,
    output: Option<Idx>,
    rename: Option<(Side, String, String)>,
) -> Resolved {
    let name_of = |idx: Option<Idx>| {
        idx.map(|i| arena.get(i).name().to_string())
            .unwrap_or_default()
    };
    let etype_of = |idx: Option<Idx>| {
        idx.map(|i| arena.get(i).entity_type().to_string())
            .unwrap_or_default()
    };
    let out_name = name_of(output);
    let out_type = etype_of(output);

    if let Some(merged) = crate::merge::diffy_merge(base_rc, ours_rc, theirs_rc) {
        return Resolved {
            disposition: Disposition::Emit {
                text: merged,
                name: out_name,
            },
            strategy: ResolutionStrategy::DiffyMerged,
            rename: None,
            tally: Tally::BothMerged,
        };
    }

    // Whitespace-only shortcut, never for indentation-sensitive languages where
    // a re-indent moves a statement between blocks.
    //
    // It sits BELOW diff3, not above it. Above it, this rung
    // answered the whole body with one side's text, which discards the other
    // side's edit whenever that edit happens to be whitespace — a blank line a
    // developer deleted, a guard clause a developer re-indented: the two edits
    // are nowhere near each other and diff3 would compose them without
    // complaint, but the shortcut had already answered. Below diff3 it answers
    // only what diff3 refuses, which is the case it was written for: both
    // sides moving the same lines.
    if !ctx.indent_sensitive && rename.is_none() {
        if crate::merge::is_whitespace_only_diff(base_rc, ours_rc) {
            return Resolved {
                disposition: Disposition::Emit {
                    text: theirs_rc.to_string(),
                    name: out_name,
                },
                strategy: ResolutionStrategy::TheirsOnly,
                rename: None,
                tally: Tally::TheirsOnly,
            };
        }
        if crate::merge::is_whitespace_only_diff(base_rc, theirs_rc) {
            return Resolved {
                disposition: Disposition::Emit {
                    text: ours_rc.to_string(),
                    name: out_name,
                },
                strategy: ResolutionStrategy::OursOnly,
                rename: None,
                tally: Tally::OursOnly,
            };
        }
    }

    if let Some(merged) =
        crate::merge::try_decorator_aware_merge(base_rc, ours_rc, theirs_rc, ctx.decorators_compose)
    {
        return Resolved {
            disposition: Disposition::Emit {
                text: merged,
                name: out_name,
            },
            strategy: ResolutionStrategy::DecoratorMerged,
            rename: None,
            tally: Tally::BothMerged,
        };
    }

    // Containers merge member-wise: class members are unordered children, so a
    // conflict belongs to the member, not to the class.
    if crate::merge::is_container_entity_type(&out_type) {
        if let Some(inner) = inner_merge(
            arena,
            triple,
            ctx,
            base_rc,
            ours_rc,
            theirs_rc,
            crate::statement::License::Refused,
            &mut Vec::new(),
        ) {
            let complexity = classify_conflict(Some(base_rc), Some(ours_rc), Some(theirs_rc));
            if inner.has_conflicts {
                return Resolved {
                    disposition: Disposition::Conflict {
                        text: inner.content,
                        conflict: EntityConflict {
                            entity_name: out_name,
                            entity_type: out_type,
                            kind: ConflictKind::BothModified,
                            complexity,
                            ours_content: Some(ours_rc.to_string()),
                            theirs_content: Some(theirs_rc.to_string()),
                            base_content: Some(base_rc.to_string()),
                        },
                    },
                    strategy: ResolutionStrategy::InnerMerged,
                    rename: None,
                    tally: Tally::Conflicted,
                };
            }
            return Resolved {
                disposition: Disposition::Emit {
                    text: inner.content,
                    name: out_name,
                },
                strategy: ResolutionStrategy::InnerMerged,
                rename: None,
                tally: Tally::BothMerged,
            };
        }
    }

    // The last refinement before an honest conflict: the body is a sequence of
    // statements, decided the same way. Reached only from here —
    // after diff3, after the decorator merge, after the container merge — so
    // every verdict it can move was already CONFLICT, and no clean merge
    // routes through it.
    //
    // Both of the fold's answers are taken now. Its CLEAN answer is a merge
    // this stage would otherwise have refused; its CONFLICTED answer is the
    // same verdict as the whole-entity conflict below, rendered over three
    // lines instead of a whole method. The conflicted answer used
    // to be discarded because `scoped_conflict_marker` did not emit the
    // enhanced marker's `refused_by:` line and the boundary contract requires one
    // after every `<<<<<<< ours`; it emits one now, in the file's own comment
    // syntax, naming the entity the scope sits inside.
    //
    // This is still just a fact about control flow: this stage is reached only
    // after diff3, the decorator merge and the container merge have all refused,
    // so every verdict it can move was already CONFLICT. Taking the conflicted
    // fold changes the BYTES of a conflicted merge and never its verdict.
    let scope = crate::merge::ScopeMarkers::inside(
        ctx.marker_format,
        &out_type,
        &out_name,
        "statement_fold",
    );
    if let Some(inner) = crate::statement::statement_merge(base_rc, ours_rc, theirs_rc, &scope) {
        if !inner.has_conflicts {
            return Resolved {
                disposition: Disposition::Emit {
                    text: inner.content,
                    name: out_name,
                },
                strategy: ResolutionStrategy::StatementMerged,
                rename: None,
                tally: Tally::BothMerged,
            };
        }
        if rename.is_none() {
            let complexity = classify_conflict(Some(base_rc), Some(ours_rc), Some(theirs_rc));
            return Resolved {
                disposition: Disposition::Conflict {
                    text: inner.content,
                    conflict: EntityConflict {
                        entity_name: out_name,
                        entity_type: out_type,
                        kind: ConflictKind::BothModified,
                        complexity,
                        ours_content: Some(ours_rc.to_string()),
                        theirs_content: Some(theirs_rc.to_string()),
                        base_content: Some(base_rc.to_string()),
                    },
                },
                strategy: ResolutionStrategy::ConflictStatementScoped,
                rename: None,
                tally: Tally::Conflicted,
            };
        }
    }

    let complexity = classify_conflict(Some(base_rc), Some(ours_rc), Some(theirs_rc));
    let (kind, strategy) = match &rename {
        Some((renamer, old, new)) => (
            ConflictKind::RenameModify {
                old_name: old.clone(),
                new_name: new.clone(),
                renamed_in_ours: *renamer == Side::Ours,
            },
            ResolutionStrategy::ConflictRenameModify,
        ),
        None => (
            ConflictKind::BothModified,
            ResolutionStrategy::ConflictBothModified,
        ),
    };
    let conflict = EntityConflict {
        entity_name: out_name,
        entity_type: out_type,
        kind,
        complexity,
        ours_content: Some(ours_rc.to_string()),
        theirs_content: Some(theirs_rc.to_string()),
        base_content: Some(base_rc.to_string()),
    };
    conflict_disposition(conflict, ctx, strategy)
}

pub(crate) fn inner_merge(
    arena: &Arena,
    triple: &Triple,
    ctx: &ResolveCtx<'_>,
    base_rc: &str,
    ours_rc: &str,
    theirs_rc: &str,
    license: crate::statement::License,
    evidence: &mut Vec<crate::statement::LicensedGap>,
) -> Option<crate::container::InnerMergeResult> {
    let find = |idx: Option<Idx>, all: &[SemanticEntity]| -> Option<SemanticEntity> {
        let src = &arena.get(idx?).src_id;
        all.iter().find(|e| &e.id == src).cloned()
    };
    let base_e = find(triple.base_idx(), ctx.base_all);
    let ours_e = find(triple.ours_idx(), ctx.ours_all)?;
    let theirs_e = find(triple.theirs_idx(), ctx.theirs_all);
    let base_children = base_e
        .as_ref()
        .map(|b| crate::merge::get_child_entities(b, ctx.base_all))
        .unwrap_or_default();
    let ours_children = crate::merge::get_child_entities(&ours_e, ctx.ours_all);
    let theirs_children = theirs_e
        .as_ref()
        .map(|t| crate::merge::get_child_entities(t, ctx.theirs_all))
        .unwrap_or_default();
    crate::container::container_merge(
        base_rc,
        ours_rc,
        theirs_rc,
        &base_children,
        &ours_children,
        &theirs_children,
        base_e
            .as_ref()
            .map(|b| region_start_line(b, base_rc))
            .unwrap_or(1),
        region_start_line(&ours_e, ours_rc),
        theirs_e
            .as_ref()
            .map(|t| region_start_line(t, theirs_rc))
            .unwrap_or(1),
        &crate::merge::ScopeMarkers::inside(
            ctx.marker_format,
            ours_e.entity_type.as_str(),
            ours_e.name.as_str(),
            "container_member",
        ),
        ctx.decorators_compose,
        license,
        evidence,
        ctx.host,
    )
}

/// The 1-based line number of `region`'s FIRST line in the original file.
///
/// The region is the entity's own bytes *plus* whatever leading doc comment
/// `extract_regions` bundled onto it, so it starts before `start_line` by
/// exactly the length of that comment. The container merge slices child spans
/// out of this text, so handing it `start_line` shifted every span up by the
/// comment's height — tails truncated, header text duplicated into the first
/// member. `end_line` is the fixed point of the two, so the offset is derived
/// from it rather than guessed.
fn region_start_line(entity: &SemanticEntity, region: &str) -> usize {
    let height = region.lines().count();
    entity
        .end_line
        .saturating_add(1)
        .saturating_sub(height)
        .max(1)
}

/// Render a conflict once, here, so no later stage has to know how.
pub(crate) fn conflict_disposition(
    conflict: EntityConflict,
    ctx: &ResolveCtx<'_>,
    strategy: ResolutionStrategy,
) -> Resolved {
    Resolved {
        disposition: Disposition::Conflict {
            text: conflict.to_conflict_markers(ctx.marker_format, strategy.guard_or_ladder()),
            conflict,
        },
        strategy,
        rename: None,
        tally: Tally::Conflicted,
    }
}
