//! Maps a [`Triple`] to a [`Cell`] with a match on each side's action.
//!
//! Each side's action is read first, then the pair is turned into a case,
//! using the name/body agreement flags only where they change the answer.
//! Because `Cell` is an enum and this is a `match`, adding an action or a case
//! forces every consumer to handle it before the crate compiles.
//!
//! The two sides are sorted by *action rank* before the cell is built, and the
//! orientation tag names the higher-ranked side. Swapping ours and theirs
//! therefore changes only which side the tag names, not which arm runs — so the
//! same change is handled the same way whichever side made it.

use super::types::*;

/// What one side did to the entity in this triple.
pub(crate) fn action(arena: &Arena, triple: &Triple, side: Side) -> Action {
    let branch = triple.side_idx(side);
    match (triple.base_idx(), branch) {
        (None, Some(_)) => Action::Added,
        (None, None) => Action::Absent,
        (Some(_), None) => Action::Deleted,
        (Some(b), Some(x)) => {
            let base = arena.get(b);
            let side_e = arena.get(x);
            let name_eq = base.key.name == side_e.key.name;
            let body_eq = base.body_hash == side_e.body_hash;
            match (name_eq, body_eq) {
                (true, true) => Action::Unchanged,
                (true, false) => Action::Edited,
                (false, true) => Action::Renamed,
                (false, false) => Action::RenameEdited,
            }
        }
    }
}

/// Sort order used to pick the triple's representative side. Purely a
/// canonicalization device: it decides which of the two sides the orientation
/// tag names, never what the cell means.
fn rank(a: Action) -> u8 {
    match a {
        Action::Absent => 0,
        Action::Unchanged => 1,
        Action::Edited => 2,
        Action::Renamed => 3,
        Action::RenameEdited => 4,
        Action::Deleted => 5,
        Action::Added => 6,
    }
}

pub(crate) fn classify(arena: &Arena, triple: &Triple) -> Cell {
    let a_ours = action(arena, triple, Side::Ours);
    let a_theirs = action(arena, triple, Side::Theirs);

    // The unordered pair, plus the tag naming which side the first came from.
    let ((first, tag), (second, _)) = if rank(a_ours) >= rank(a_theirs) {
        ((a_ours, Side::Ours), (a_theirs, Side::Theirs))
    } else {
        ((a_theirs, Side::Theirs), (a_ours, Side::Ours))
    };

    // n′ and c′ — read only where they are free to vary.
    let names_converge = || match (triple.ours_idx(), triple.theirs_idx()) {
        (Some(o), Some(t)) => arena.get(o).key.name == arena.get(t).key.name,
        _ => false,
    };
    let bodies_converge = || match (triple.ours_idx(), triple.theirs_idx()) {
        (Some(o), Some(t)) => arena.get(o).body_hash == arena.get(t).body_hash,
        _ => false,
    };

    use Action as A;
    match (first, second) {
        // -- b = 0 -------------------------------------------------------
        (A::Added, A::Absent) => Cell::AddedOneSide { adder: tag },
        (A::Added, A::Added) => {
            if bodies_converge() {
                Cell::AddedBothConvergent
            } else {
                Cell::AddedBothDivergent
            }
        }

        // -- b = 1, at least one delete ----------------------------------
        (A::Deleted, A::Deleted) => Cell::DeletedBoth,
        (A::Deleted, A::Unchanged) => Cell::DeleteVsUnchanged { deleter: tag },
        (A::Deleted, A::Edited) => Cell::DeleteVsEdit { deleter: tag },
        (A::Deleted, A::Renamed) => Cell::DeleteVsRename { deleter: tag },
        (A::Deleted, A::RenameEdited) => Cell::DeleteVsRenameEdit { deleter: tag },

        // -- b = 1, both present, no rename ------------------------------
        (A::Unchanged, A::Unchanged) => Cell::UnchangedBoth,
        (A::Edited, A::Unchanged) => Cell::EditOneSide { editor: tag },
        (A::Edited, A::Edited) => {
            if bodies_converge() {
                Cell::EditBothConvergent
            } else {
                Cell::EditBothDivergent
            }
        }

        // -- b = 1, exactly one rename -----------------------------------
        (A::Renamed, A::Unchanged) => Cell::RenameVsUnchanged { renamer: tag },
        (A::RenameEdited, A::Unchanged) => Cell::RenameEditVsUnchanged { renamer: tag },
        (A::Renamed, A::Edited) => Cell::RenameVsEdit { renamer: tag },
        (A::RenameEdited, A::Edited) => Cell::RenameEditVsEdit {
            renamer: tag,
            bodies_converge: bodies_converge(),
        },

        // -- b = 1, two renames ------------------------------------------
        (A::Renamed, A::Renamed) => Cell::RenameBoth {
            names_converge: names_converge(),
        },
        (A::RenameEdited, A::Renamed) => Cell::RenameEditVsRename {
            rename_editor: tag,
            names_converge: names_converge(),
        },
        (A::RenameEdited, A::RenameEdited) => Cell::RenameEditBoth {
            names_converge: names_converge(),
            bodies_converge: bodies_converge(),
        },

        // -- unreachable: no such triple exists --------------------------
        // (ABSENT, ABSENT) is a vacuous corner: no claims, so no
        // triple. Every other absent pair is excluded by the rank order, and
        // ADD can only pair with ADD or ABSENT because a triple with no base
        // has no other actions available.
        (A::Absent, _) => unreachable!("a triple with no entity on either side cannot exist"),
        (A::Added, other) => {
            unreachable!("a base-absent triple cannot have action {other:?} on a side")
        }
        (first, second) => unreachable!("rank order excludes ({first:?}, {second:?})"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::match_phase::{build_arena, match_phase, RawEntity};
    use super::*;
    use std::collections::HashSet;

    fn raw(name: &str, body: &str) -> RawEntity {
        RawEntity {
            name: name.to_string(),
            entity_type: "function".to_string(),
            parent: None,
            content: format!("def {name}():\n{body}"),
            src_id: format!("f::{name}"),
        }
    }

    /// (label, base, ours, theirs)
    type Scenario = (&'static str, Vec<RawEntity>, Vec<RawEntity>, Vec<RawEntity>);

    fn cells(b: Vec<RawEntity>, o: Vec<RawEntity>, t: Vec<RawEntity>) -> Vec<Cell> {
        let (arena, claims) = build_arena(b, o, t);
        let m = match_phase(arena, claims);
        m.triples.iter().map(|t| classify(&m.arena, t)).collect()
    }

    const BODY: &str = "    for item in fetch():\n        record(item)\n    return done()\n";
    const EDIT_A: &str =
        "    touched_a()\n    for item in fetch():\n        record(item)\n    return done()\n";
    const EDIT_B: &str =
        "    touched_b()\n    for item in fetch():\n        record(item)\n    return done()\n";

    #[test]
    fn every_cell_the_universe_can_reach_is_reached_symmetrically() {
        // A hand-built scenario per cell family, checked in both orientations.
        let scenarios: Vec<Scenario> = vec![
            (
                "unchanged",
                vec![raw("a", BODY)],
                vec![raw("a", BODY)],
                vec![raw("a", BODY)],
            ),
            (
                "edit one side",
                vec![raw("a", BODY)],
                vec![raw("a", EDIT_A)],
                vec![raw("a", BODY)],
            ),
            (
                "edit both divergent",
                vec![raw("a", BODY)],
                vec![raw("a", EDIT_A)],
                vec![raw("a", EDIT_B)],
            ),
            (
                "edit both convergent",
                vec![raw("a", BODY)],
                vec![raw("a", EDIT_A)],
                vec![raw("a", EDIT_A)],
            ),
            (
                "delete vs edit",
                vec![raw("a", BODY)],
                vec![],
                vec![raw("a", EDIT_A)],
            ),
            ("delete both", vec![raw("a", BODY)], vec![], vec![]),
            (
                "rename vs unchanged",
                vec![raw("a", BODY)],
                vec![raw("a_v2", BODY)],
                vec![raw("a", BODY)],
            ),
            (
                "rename both divergent names",
                vec![raw("a", BODY)],
                vec![raw("a_v2", BODY)],
                vec![raw("a_alt", BODY)],
            ),
            (
                "added one side",
                vec![raw("a", BODY)],
                vec![raw("a", BODY), raw("z", BODY)],
                vec![raw("a", BODY)],
            ),
            (
                "added both divergent",
                vec![raw("a", BODY)],
                vec![raw("a", BODY), raw("z", EDIT_A)],
                vec![raw("a", BODY), raw("z", EDIT_B)],
            ),
        ];
        for (label, b, o, t) in scenarios {
            let fwd = cells(clone_raws(&b), clone_raws(&o), clone_raws(&t));
            let rev = cells(clone_raws(&b), clone_raws(&t), clone_raws(&o));
            let fwd_flipped: HashSet<String> =
                fwd.iter().map(|c| format!("{:?}", c.flip())).collect();
            let rev_set: HashSet<String> = rev.iter().map(|c| format!("{c:?}")).collect();
            assert_eq!(
                fwd_flipped, rev_set,
                "classification is not symmetric for {label}: {fwd:?} vs {rev:?}"
            );
        }
    }

    fn clone_raws(v: &[RawEntity]) -> Vec<RawEntity> {
        v.iter()
            .map(|r| RawEntity {
                name: r.name.clone(),
                entity_type: r.entity_type.clone(),
                parent: r.parent.clone(),
                content: r.content.clone(),
                src_id: r.src_id.clone(),
            })
            .collect()
    }

    #[test]
    fn the_edit_pair_is_discriminated_by_convergence() {
        assert_eq!(
            cells(
                vec![raw("a", BODY)],
                vec![raw("a", EDIT_A)],
                vec![raw("a", EDIT_A)]
            ),
            vec![Cell::EditBothConvergent]
        );
        assert_eq!(
            cells(
                vec![raw("a", BODY)],
                vec![raw("a", EDIT_A)],
                vec![raw("a", EDIT_B)]
            ),
            vec![Cell::EditBothDivergent]
        );
    }

    #[test]
    fn convergent_renames_are_not_a_rename_rename_conflict() {
        // Both sides renaming to the same name is a rename, not a conflict.
        assert_eq!(
            cells(
                vec![raw("a", BODY)],
                vec![raw("a_v2", BODY)],
                vec![raw("a_v2", BODY)]
            ),
            vec![Cell::RenameBoth {
                names_converge: true
            }]
        );
    }
}
