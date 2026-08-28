# Algebraic Extraction Dossier — the entity-level 3-way merge engine
**Subsystem:** `weave-core::merge::entity_merge_with_registry` and the `v2` pipeline it fronts
**Checkout:** branch `docs/social-card` (HEAD `a60d62c`), 2026-08-29
**Witness suite:** `crates/weave-core/tests/laws_merge.rs` (`cargo test -p weave-core --test laws_merge`)

**Branch validity.** `crates/weave-core` is byte-identical between this checkout,
`proofs/failure-case-witnesses-r2`, and `fix/missing-file-empty-base` (verified by
`git diff`; the only delta vs the proofs branch is the `Cargo.toml` version string,
and the fix branch's change is confined to `crates/weave-mcp/src/server.rs`,
commit `ad70cdc`). Every law result below therefore holds verbatim on **both**
branches named in the mission. The one fix-branch-only behavior — *missing file =
empty base* in `weave_update_entity_content` — lives at the MCP layer and is
characterized in §5, not re-proven here.

---

## 0. MAP — the topology and the altitudes

Public boundary (everything witnessed here goes through it):

```
entity_merge(base, ours, theirs, file_path) -> MergeResult
    { content: String, conflicts: Vec<EntityConflict>, warnings, stats, audit }
```

Consumers: `weave-driver` (git merge driver: three blobs in, marker text +
exit code out), `weave-mcp` (`weave_merge_file`, `weave_update_entity_content`:
same function, conflicts surfaced as payload), `weave-cli`.

Internal route (read for RECOGNIZE only; no law reads internals):

```
entity_merge_with_registry            merge.rs
 ├─ refuse: inputs already carry markers        (whole-file conflict, content = the marked input)
 ├─ fast paths: o==t → o;  b==o → t;  b==t → o  (the cell axioms, at file granularity)
 ├─ subsumption: one side's hunks carried by the other's → the superset side
 ├─ binary / >1MB → line-level fallback
 └─ v2::merge_file                    v2/mod.rs
     parse → Match → Classify → Resolve → Bind → Plan → Render
       │       │        └ Triple → Cell (the 17-cell table, classify.rs)
       │       └ the ONLY place entities are paired (match_phase.rs)
       └ Err(Unsupported::{NoGrammar, NoEntities, BothCreated, AmbiguousIdentity})
           → line-level diff3 fallback
```

Altitudes: **L3** concurrent editing of a module by two agents · **L2** the
pipeline stages above · **L1** the cell algebra + the matching · **L0** gap
arithmetic, separator commas, marker rendering. Every finding below is an L1
structure whose imprecision is visible only at L0.

---

## 1. RECOGNIZE — what this engine is (and is not)

### The structure, named

**A file is denoted as a partial map from entity identity to content**
(plus an order component and an interstitial component), and the merge is the
**pointwise lift of the conservative three-way merge cell function**

```
cell(b, o, t) = o         if o = t
              = t         if o = b
              = o         if t = b
              = CONFLICT  otherwise
```

with CONFLICT **reified as data** (an `EntityConflict` + rendered markers), not
as failure. Precisely:

- This is the *maximal conservative merge* — the unique merge that applies
  every one-sided change and invents nothing — formalized for diff3 by
  **Khanna, Kunal & Pierce, “A Formal Investigation of Diff3”, FSTTCS 2007**
  (their *stability* and *locality* properties are our L1/L2 and L5). weave
  computes it at **entity** granularity instead of line granularity, which is
  exactly the move of **Lindholm, “A three-way merge for XML documents”,
  ACM DocEng 2004 (3DM)**: *match first, then merge pointwise over the
  matching*. The matching phase is therefore part of the trusted base — 3DM's
  central lesson, and the locus of every broken law found below.
- Categorically: for edits with disjoint support the result is the **pushout**
  of `ours ← base → theirs` in the category of keyed edits; when the pushout
  does not exist, the conflict objects are the formal colimits that exist only
  in the **free (finite) cocompletion** — the construction of
  **Mimram & Di Giusto, “A Categorical Theory of Patches”, ENTCS 298, 2013**.
  weave's `EntityConflict` is a concrete realization of that formal colimit:
  the diagram (b, o, t) kept as data because its colimit is missing.
- The `subsumption.rs` rule is the **absorption/comparability law of a partial
  join**: when the two edits are comparable in the “carried-by” order
  (`eo ⊑ et`), the join is the maximum (`o ∨ t = t`). It generalizes the unit
  law `merge(b,b,t) = t` from “changed nothing” to “changed nothing new” —
  the engine's own doc comment says exactly this, and it is correct.

**DENOTATION** line: `⟦file⟧ : Key ⇀ Body` × `⟦order⟧ : Key*` ×
`⟦interstitial⟧ : Position ⇀ Text`; `⟦merge⟧ = pointwise cell ⊗ order-merge ⊗
gap-merge`. Where the code *resists* this denotation is precisely where laws
break: the JSON comma (Body not a function of the entity alone, §4-F3), the
TS rename inference (Key not stable, §4-F2), the gap sum (gap-merge not a
join, §4-F4).

### NOT-THAT (mis-namings this dossier forecloses)

| Candidate | Verdict | Why not |
|---|---|---|
| **CRDT** (Shapiro et al. 2011) | **NOT-THAT** | No join-semilattice: the operation is *partial* (conflicts), *ancestor-mediated* (3 arguments, not 2), and not associative. There is no convergence theorem to inherit. The repo's `weave-crdt` crate and the `entity-crdt-v2` branch must not lend this word to `entity_merge` — a CRDT promises conflict-freedom by construction; this engine's entire value is the opposite: *reifying* the conflict. |
| **Operational Transformation** (Ellis & Gibbs 1989; Ressel et al. 1996) | **NOT-THAT** | No transformation functions, no TP1/TP2 obligations; edits are never rewritten against each other, only compared against base. |
| **Darcs patch theory** (Roundy 2005; Jacobson 2009 — inverse semigroups) | **NOT-THAT** | No commutation/residuals; patches are not first-class (states are). The categorical cousin is Mimram–Di Giusto, and even there weave keeps states, not morphisms. |
| **diff3 itself** (Khanna–Kunal–Pierce 2007) | **80%-THAT** | Same merge relation, different (coarser, semantic) chunking; and KKP's *no-stability* pathologies at line level are exactly what entity chunking removes. The line-level fallback route IS diff3, and inherits its weaker guarantees — that route is why several laws carry a “supported domain” condition. |

### μ vs ν

`entity_merge` is a single-shot function — no internal fixpoint. But its
consumers iterate it (drift re-checks, `weave-mcp`'s update loop, a driver run
twice): that is a **μ-iteration** of `merge(b, o, ·)` toward a hoped-for fixed
point. Finding F4 shows the iteration **has no fixpoint** today (gap widths
strictly increase), so any “merge until stable by content equality” loop in a
consumer is non-terminating by construction. The correct witness for the fix is
`merge(b, o, m) == m` byte-level — preserved RED as
`red_l2b_absorption_bytes_diverge` / `l2b_red_absorption_strict`.

---

## 2. The law ledger (every law: statement, domain, status, witness)

All witnesses are properties over generated arbitraries (synthetic Python /
TypeScript / JSON modules with per-writer 5-digit sentinel bodies; see §6) in
`crates/weave-core/tests/laws_merge.rs`. GREEN = passing property; RED =
violation preserved as an `#[ignore]`d minimized counterexample whose
un-ignoring is the fix's acceptance test.

| # | Law | Domain | Status | Witness |
|---|---|---|---|---|
| L1 | `merge(b,b,t) = t` and `merge(b,o,b) = o`, byte-identical, clean | marker-free inputs | **GREEN** | `l1_identity_unchanged_side_yields_other_side` |
| L2 | `merge(b,x,x) = x`, byte-identical, clean | marker-free inputs | **GREEN**, with one RED corner | `l2_idempotence_agreeing_sides_win`; RED `red_l2_idempotence_defeated_by_marker_bearing_base` |
| L2b | absorption `merge(b,o,merge(b,o,t)) ~ merge(b,o,t)` | clean disjoint merges | **GREEN on Python** for ~ = same surviving entity set; **RED** at byte level (all langs, gap drift) and **RED on TS** at every level (rename-steal) | `l2b_absorption_remerge_is_clean_and_preserves_the_entity_image`; RED `l2b_red_absorption_strict`, `red_l2b_absorption_bytes_diverge`, `red_l5_ts_delete_add_fabricates_rename_rename_conflict` |
| L3a | side-swap preserves cleanliness and the total surviving body set | all triples, all 3 langs | **GREEN** | `l3a_side_swap_preserves_cleanliness_and_surviving_content` |
| L3b | disjoint edits: `merge(b,o,t)` **byte-identical** to `merge(b,t,o)` | disjoint, Py/TS | **GREEN** — placement canonicalizes; no ours-first bias observed | `l3b_disjoint_commutativity_is_byte_identical_on_code` |
| L4 | **No silent loss**: every body a side wrote appears in content or conflict payloads | all triples, all 3 langs, incl. overlapping edits, delete-vs-edit, colliding adds | **GREEN** (96 cases/run; also holds across every RED below — the REDs are wrong *verdicts*, never lost *content*) | `l4_no_silent_loss` |
| L5 | conflict soundness: disjoint entity edits merge clean; writes survive; deletions honored | non-empty base, Py/TS, unique names | **GREEN**; **RED for JSON** (comma leak); boundary at empty base (issue #51) | `l5_disjoint_edits_merge_clean_and_complete`; RED `red_l5_json_disjoint_add_forces_comma_conflict`; boundary `boundary_both_created_conflicts_but_loses_nothing` |
| L5b | conflict completeness: divergent same-name additions conflict | no concurrent delete | **GREEN** (all 3 langs); **RED corner on TS** with concurrent delete | `l5b_divergent_same_name_additions_conflict`; RED `red_l5b_ts_divergent_add_conflict_suppressed` |
| L6 | determinism: identical `content`, `conflicts`, `audit` across repeated calls | all triples | **GREEN** (fresh `RandomState` per call means hash-order dependence would surface here) | `l6_determinism` |
| L7 | totality: returns on arbitrary strings (unicode, partial markers, corrupted modules); `is_clean ⇔ conflicts.is_empty` | any input | **GREEN** (the historical UTF-8 panic, PR #9, is fixed on all audited branches) | `l7a_totality_arbitrary_strings`, `l7b_totality_corrupted_modules` |
| L8 | linearity: a clean merge defines each name at most once (the pipeline's own claim, `v2/mod.rs`: “an entity cannot be emitted twice”) | clean merges | **GREEN on Python and JSON**; **RED on TypeScript** | `l8_linearity_clean_output_defines_each_name_once`; RED `red_l8_ts_rename_steal_duplicates_convergent_add` |

Non-vacuity: generators force ≥1 real change per side; coverage floors are
asserted by `control_generators_are_not_degenerate` (both-sides-write ≥ 30%,
overlapping-edit ≥ 10%, convergent-add ≥ 2.5% of samples); every observation
function is proven able to fail by the `control_*` tests; every RED was
executed and observed failing (7/7) at this commit.

---

## 3. Confirmations — what is already lawfully realized

This half is the audit, not the complaint list.

1. **The cell table is the KKP merge relation, symmetrically realized.**
   `classify.rs` sorts the action pair by rank before building the cell, so the
   same change is handled identically whichever side made it — and L3a/L3b
   prove the whole *pipeline* (not just classification) is side-symmetric, to
   the byte on the disjoint code domain. The internal symmetry unit test's
   promise survives at the public boundary.
2. **Conflict-as-data with one owner.** `is_clean()` reads typed dispositions,
   never scans text; L7's coherence check (`is_clean ⇔ conflicts.is_empty`)
   held over every generated and hostile input.
3. **The safety law holds everywhere we could reach** — including on every
   broken path found. All four RED bug-classes corrupt the *verdict* or the
   *shape* of the output; none silently drops a body. The product's central
   promise (L4) is the best-defended law in the engine.
4. **Determinism is real**, not an accident of one process: interstitial
   answers are ordered by `BTreeSet` deliberately (the `merge_interstitials`
   comment says why), and L6 could not distinguish repeated runs.
5. **The subsumption rule's carve-outs are correct**: modify/delete stays a
   conflict (a deletion is carried only by a deletion), version-bump textual
   extension is not subsumption, blank-line-only edits never fire it. These are
   exactly the absorption law's side conditions, and its unit suite states them
   as such.
6. **The empty-base gate (issue #51) is a boundary, not a bug**: it trades
   false conflicts for structural validity, and L4 holds across it
   (`boundary_both_created_conflicts_but_loses_nothing`). The MCP fix
   `ad70cdc` extends the same judgment to the missing-file case: never an
   error, never silent loss, refusal with both payloads when both sides create.

---

## 4. Findings ladder (ranked by leverage)

### F2 — BROKEN: rename inference outranks identity (TypeScript) — *the* finding

- **STRUCTURE**: entity matching as a stable-identity correspondence
  (Lindholm 2004 §matching; the pointwise denotation requires `Key` stable).
- **STATUS**: BROKEN on TS. Python 0/597, JSON 0/597, TS 12/597 clean merges
  with a duplicated definition under uniform sampling (10 of the 12 silently
  resolved a real contradiction).
- **CARRIER**: `match_phase.rs` rename-candidate generation (body-hash buckets,
  prefix-filtered token index). TS bodies share a long token prefix
  (`export function …(): number { return`), so a *deleted* base entity is
  paired with the *same side's addition* as a rename even when that addition
  has an exact-name (even exact-body) partner on the other side.
- **THREE SYMPTOMS, one root** (all first-order triples, no re-merge needed):
  1. `red_l8_ts_rename_steal_duplicates_convergent_add` — convergent both-side
     add + one-side delete ⇒ the name **defined twice in a clean merge**
     (audit: `e1: Renamed{to: ox9}` *and* `ox9: AddedOurs`).
  2. `red_l5b_ts_divergent_add_conflict_suppressed` — divergent both-side add
     (the engine's own `AddedBothDivergent` conflict cell) ⇒ **clean** merge
     containing both contradictory bodies; TS semantics silently makes the
     later one win. The silent-wrong-answer class the product promise rules out.
  3. `red_l5_ts_delete_add_fabricates_rename_rename_conflict` — deletes + an
     agreeing add ⇒ **fabricated** `RenameRename` conflict between two renames
     neither side made, plus the duplicate, plus an incoherent audit (two
     triples claim `ox0`).
- **LAW GAP → STRUCTURE**: the missing guarantee is *name-linearity of the
  matching* — the matching must be a maximum-weight matching in which
  cross-side name-equality **strictly dominates** body-shape similarity, and a
  both-side add of one name must form one triple before rename inference runs.
  With that, L8 holds by construction (linearity of claims becomes linearity
  of names) and symptoms 2–3 vanish; the richer structure is exactly the
  stable-keyed pointwise map merge that Python and JSON already realize.
- **HANDOFF**: miller-bug-hunter — the three `#[ignore]` tests are the RED
  suite; severity order 2 > 3 > 1.

### F3 — BROKEN: the JSON carrier fails to quotient the sequence encoding

- **STRUCTURE**: `⟦json object⟧ = key ⇀ value`; the separator comma belongs to
  the encoding, not to any key's value (Hoare 1972: compare images, not
  representations — the carrier compares representations).
- **STATUS**: BROKEN. A value-edit beside a key-add falsely conflicts
  (`EDIT×EDIT` on an entity only one side changed): the adder's comma rewrite
  of the previous last line is read as an edit of that entity.
- **WITNESS**: RED `red_l5_json_disjoint_add_forces_comma_conflict`; GREEN L5
  is therefore domain-restricted to Py/TS.
- **COLLAPSE**: normalize separators out of the compared entity text at parse
  and re-derive them at render (a carrier change, not a rule change). This also
  deletes the reason JSON sits in `skip_expansion` special cases.
- **NOTE**: false conflict, not loss — L4 holds; still, `package.json`
  value-bump + dependency-add is the most common concurrent pair this engine
  will ever see.

### F4 — BROKEN: the gap merge is a sum, not a join — iterated merge diverges

- **STRUCTURE**: blank-line gaps form the total order `(ℕ, ≤)`; `widest_gap`
  is *documented* as a join (`max`), which is idempotent. The realization
  compounds the two gaps flanking a deletion per pass.
- **STATUS**: BROKEN at byte level. Observed: `m1 = …\n\n\n…`,
  `m2 = …\n\n\n\n\n…`, `m3 = …\n\n\n\n\n\n\n…` — strictly increasing, no
  fixpoint; the μ-iteration “re-merge until stable” never terminates by
  content equality.
- **WITNESS**: RED `red_l2b_absorption_bytes_diverge` (and the standing RED
  property `l2b_red_absorption_strict`); GREEN L2b at entity-image level
  (Python) shows the drift is confined to whitespace.
- **COLLAPSE**: make the gap combination the actual join (idempotent);
  absorption then makes the merge output a **normal form** and
  `merge(b, o, ·)` idempotent on its image — which is what `subsumption.rs`
  already assumes it is.

### F5 — 80%-REALIZED: guard order inverts the agreement axiom

- **STRUCTURE**: `o = t ⇒ merge = o` is an *axiom* of the cell function; the
  marker-refusal guard is a domain restriction. Axioms precede restrictions.
- **STATUS**: the `has_conflict_markers` refusal runs before the `ours ==
  theirs` fast path, so a marker-bearing **base** under byte-agreeing sides
  yields a conflict whose proposed content is the stale ancestor.
- **WITNESS**: RED `red_l2_idempotence_defeated_by_marker_bearing_base`;
  boundary pin `boundary_pre_conflicted_inputs_are_refused_not_merged` (the
  refusal itself is correct whenever the sides disagree).
- **COLLAPSE**: move the `o == t` check above the guard — one reorder, no
  semantics lost on any disagreeing input.

### F1 — REALIZED (for completeness of the ladder): everything in §3.

---

## 5. Guarantee→structure loop (error classes → missing structure)

| Observed / likely error class | Missing guarantee | Structure that confers it | Does current ⊕ guarantee collapse? |
|---|---|---|---|
| Duplicate TS definitions after a clean merge; “rename” audit entries nobody made | name-linearity of matching | maximum-weight matching, name-equality-dominant (3DM) | **Yes** — matching becomes a stable key; TS joins Py/JSON under one uniform pointwise merge; the `AmbiguousIdentity` gate can likely narrow |
| False conflicts on `package.json` value+add | carrier quotients encoding | canonical-form carrier (parse-normalize / render-derive) | **Yes** — deletes JSON special cases; L5 domain extends to JSON |
| Blank lines accreting in re-merge loops; non-terminating “merge until stable” | idempotent gap combination | join in `(ℕ, max)` as documented | **Yes** — output becomes a normal form; `subsumption` becomes a theorem about the output, not a hope |
| Agreed resolutions “conflicted” because the ancestor quotes markers | agreement axiom precedes refusal | guard ordering | trivial reorder |
| Second creator of a new file refused (MCP) | — | none richer: this is the `BothCreated` boundary surfacing at the tool layer; commit `ad70cdc` already chose *coarse and safe* and pins it | no (documented boundary, issue #51) |

The four fixable rows share one telos: **make the realized carrier equal the
denoted carrier** (`Key` stable, `Body` canonical, gaps a lattice). After all
four, the engine *is* the KKP/3DM pointwise merge with no asterisks on the
supported domain, and L2b-strict + L8-all-langs become provable — un-ignoring
the seven RED tests is the complete acceptance suite for that campaign.

---

## 6. Property inventory (laws → arbitraries)

Generators (all in `laws_merge.rs`):

- `base_strategy(lang)` — 2..=5 uniquely-named entities, sentinels `10000+100i`;
  surfaces: Python (`def`), TypeScript (`export function`), JSON (object keys).
- `general_triple_strategy()` — independent per-side plans: Keep/Modify/Delete
  per entity; 0..=2 additions each, from side-private pools, shared-divergent
  (`sx…` with side sentinel), or shared-convergent (`sx…` with sentinel
  `60000+100j` on both sides). Reaches every `b=1` cell plus AddedOneSide /
  AddedBothConvergent / AddedBothDivergent, with and without racing deletes.
- `disjoint_triple_strategy(lang)` — owner-partitioned edits (entity 0 always
  untouched; each other entity owned by ≤1 side), disjoint add pools, ≥1
  change per side guaranteed.
- Hostile strings: `.{0,200}` triples over 5 extensions; corruption injector
  (brace/quote/marker/BOM/CRLF fragments at arbitrary char boundaries).

Observation functions (boundary-only, each with a RED-capable control):
`written_sentinels` (what the law must conserve), `visible` (content ∪ conflict
payloads), `sentinel_image` / `full_image` (output up to ordering and
duplication), `definition_count` (linearity), `key_lines` (modulo-whitespace).

Scenario tests this inventory retires (they are single points of these
properties): `public_properties.rs` — all five are instances of L1/L2/L4/L5;
`integration.rs::both_make_identical_changes` (L2),
`json_multiple_keys_added_at_end` (L5-JSON's surviving fragment),
`empty_base_json_both_add_different_keys` (kept — it is the boundary pin,
mirrored here as `boundary_both_created_conflicts_but_loses_nothing`).

---

## 7. COLLAPSE proposals (ranked; none executed — read-only extraction)

1. **Name-dominant matching** (F2). Target: `v2/match_phase.rs` candidate
   generation. Guarded by: un-ignoring the three TS REDs + L8/L5b properties
   extended to `any_lang()`. Honest leverage: no LoC win; the win is
   `γ↑` — the linearity invariant the module already claims becomes true, and
   the silent-wrong-answer class on the clean path closes. Handoff:
   miller-bug-hunter first (it is a bug), then effect-expert/refactor if the
   weight model wants restructuring.
2. **Canonical carrier for structured formats** (F3). Target: JSON entity
   text normalization at the parse/render seam. Guarded by: un-ignoring
   `red_l5_json_*`, extending L5's generator to `any_lang()`. Leverage:
   deletes special-casing; extends the flagship guarantee to the most-merged
   file kind in real repos.
3. **Gap join, not gap sum** (F4). Target: `widest_gap` call sites /
   interstitial rebuild around deletions. Guarded by: un-ignoring
   `red_l2b_absorption_bytes_diverge` and promoting `l2b_red_absorption_strict`
   to the green suite. Leverage: merge output becomes a normal form; consumer
   re-merge loops terminate.
4. **Guard reorder** (F5). Target: first ~40 lines of
   `entity_merge_with_registry`. Guarded by: un-ignoring
   `red_l2_idempotence_defeated_by_marker_bearing_base`; the boundary pin
   keeps the refusal lawful for disagreeing inputs. Leverage: one moved block.

---

## 8. Bibliography

- S. Khanna, K. Kunal, B. C. Pierce. *A Formal Investigation of Diff3.*
  FSTTCS 2007. (The merge relation; stability/locality; line-level pathologies.)
- T. Lindholm. *A three-way merge for XML documents.* ACM DocEng 2004. (Keyed
  match-then-merge; the matching as trusted base.)
- S. Mimram, C. Di Giusto. *A Categorical Theory of Patches.* ENTCS 298, 2013.
  (Merge as pushout; conflicts as objects of the free cocompletion.)
- C. A. R. Hoare. *Proof of Correctness of Data Representations.* Acta
  Informatica 1, 1972. (Compare images, not representations — the F3 carrier
  argument.)
- M. Shapiro, N. Preguiça, C. Baquero, M. Zawirski. *Conflict-free Replicated
  Data Types.* SSS 2011. (The NOT-THAT: what a semilattice join would promise.)
- C. A. Ellis, S. J. Gibbs. *Concurrency Control in Groupware Systems.*
  SIGMOD 1989; M. Ressel et al., CSCW 1996. (The OT NOT-THAT; TP1/TP2.)
- D. Roundy. *Darcs: distributed version management in Haskell.* Haskell
  Workshop 2005; J. Jacobson. *A formalization of Darcs patch theory using
  inverse semigroups.* UCLA CAM 09-83, 2009. (The patch-theory NOT-THAT.)
- G. Birkhoff. *Lattice Theory.* AMS Colloq. XXV. (Join, absorption,
  idempotence — the F4 argument.)
- weave repository: issue #51 (`empty_base_json_both_add_different_keys`),
  PR #9 (UTF-8 panic fix), commit `ad70cdc` (missing file = empty base,
  `weave-mcp`).
