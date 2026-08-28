//! The single matching pass.
//!
//! v1 matched entities four times: sem-core's `match_entities`, then
//! `build_rename_map`, then `augment_rename_map_with_rename_plus_edit`, then
//! again inside `reconstruct` (via `theirs_rename_base_ids`) — four opinions
//! about the same question, kept consistent by hand. Every duplication bug
//! lived in the gaps between them.
//!
//! Here there is one pass and one output: a [`Matching`] in which every entity
//! lands in exactly one triple. Entities are moved in as [`Claim`]s, which
//! are neither `Copy` nor `Clone`, so an entity cannot be placed twice; the
//! constructor drains every claim, so none can be forgotten.
//!
//! Cost: no pairwise scan, no bucket cap, no timeout.
//!   * identity — one hash lookup per entity;
//!   * equal bodies — bucketed by body hash and paired in key order, so the
//!     "2000 identically-shaped handlers" file that used to go quadratic is
//!     `O(k log k)` in the bucket rather than `O(k²)`, and needs no cap: when
//!     every candidate is equally supported the *only* honest tiebreak is a
//!     deterministic one;
//!   * edited bodies — a prefix-filtered inverted index over token signatures,
//!     so candidate generation is driven by rare tokens instead of by every
//!     pair;
//!   * rename+edit — corroborated by call sites that moved, computed once from
//!     the triples identity already settled, and looked up by name.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};

use super::types::*;
use crate::binding::{called_names, replace_at_word_boundaries};

/// Body similarity at or above which a name change is a rename on its own.
const RENAME_MIN_SIMILARITY: f64 = 0.7;
/// Body similarity floor when third-party call sites corroborate the rename.
/// Far below the solo bar because the whole point is that the body changed;
/// the corroboration carries the confidence.
const RENAME_EDIT_MIN_SIMILARITY: f64 = 0.3;
/// A body with fewer distinct tokens than this has no resolution left in a
/// Jaccard score: one added line swings it by more than the whole threshold
/// band, so similarity stops being evidence and the corroborating call sites
/// become the entire case. This is the recall debt an earlier version left
/// behind — it applied a 0.3 similarity floor uniformly and lost every
/// rename+edit of a one-line function to it.
const SHORT_BODY_TOKENS: usize = 12;
/// Containment (|A∩B| / min(|A|,|B|)) at or above which two *short* bodies are
/// the same body with a line added. Containment, not Jaccard, because an added
/// line costs a short body a fifth of its Jaccard score and none of its
/// containment — the base body is still entirely present.
const SHORT_BODY_MIN_CONTAINMENT: f64 = 0.7;
/// A token shared by more than this many candidates carries no information
/// about which of them is the rename. Indexing only rarer tokens is what makes
/// the short-body stage both discriminating and flood-resistant: in a file of 2000
/// identically-shaped one-liners every token is common, so the stage generates
/// no candidates at all and costs nothing — the case `MAX_RENAME_BUCKET` used
/// to bail out of, now handled by the evidence being absent rather than by a
/// cap on how much of it we look at.
const RARE_DF: usize = 4;
/// Fraction of a deleted body's signature lines that must reappear inside one
/// added entity before we call it a re-home rather than a coincidence.
const REHOME_MIN_OVERLAP: f64 = 0.6;

// ---------------------------------------------------------------------------
// Arena construction
// ---------------------------------------------------------------------------

/// An entity as the parser handed it over, before v2 gives it an identity.
pub(crate) struct RawEntity {
    pub name: String,
    pub entity_type: String,
    pub parent: Option<String>,
    pub content: String,
    pub src_id: String,
}

/// Drop lifetime arguments from an entity's name.
///
/// A Rust `impl` block's name, as the grammar hands it over, is the header it
/// was spelled with: `InferenceContext<'a, 'db>`. Elide a lifetime and the name
/// changes — `InferenceContext<'db>` — and the matcher, which has nothing else
/// to go on, reads a RENAME. Rename-vs-edit is an honest conflict, so a
/// lifetime elision landing on top of somebody's body edit came out as a
/// conflict about a name nobody meant to change — seen across several real
/// merges, every one of them a lifetime.
///
/// Lifetimes are not part of the identity of the type being implemented. They
/// are erased here, at the one place identity is minted, rather than special-
/// cased later by every stage that compares names. An empty argument list left
/// behind (`Foo<>`) is closed up too, so the two spellings converge on one.
fn erase_lifetimes(name: &str) -> String {
    if !name.contains('\'') {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\'' {
            out.push(c);
            continue;
        }
        // `'a`, `'_`, `'static` — the lifetime and the separator behind it.
        while chars
            .peek()
            .is_some_and(|n| n.is_alphanumeric() || *n == '_')
        {
            chars.next();
        }
        while chars.peek().is_some_and(|n| *n == ',' || *n == ' ') {
            chars.next();
        }
        while out.ends_with(", ") || out.ends_with(',') || out.ends_with(' ') {
            out.pop();
        }
    }
    let collapsed = out.replace("<>", "");
    collapsed.trim().to_string()
}

/// Rust's anonymous item — `const _: () = assert!(…);` — has the name `_`.
///
/// That is not an identity. It cannot be referenced, two of them in one file
/// are two different items, and the grammar hands over the same string for
/// both. The matcher, keying on the name, read them as one item that both
/// sides had edited, and two INDEPENDENT additions of two different static
/// assertions came out as a conflict.
///
/// For an item with no name, the text IS the identity: two `_` items are the
/// same item exactly when they say the same thing. The trade is deliberate:
/// an *edit* of an anonymous const is now a delete plus an add. That composes
/// to the edited text, which is the right
/// answer; what it gives up is the ability to call a two-sided edit of one
/// anonymous const a conflict, and a two-sided edit of an item neither side
/// can name is not a claim anyone can check anyway.
fn name_the_unnameable(name: &str, content: &str) -> String {
    if name != "_" {
        return name.to_string();
    }
    format!("_#{:016x}", hash64(content.trim()))
}

fn hash64(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Build the arena and mint exactly one claim per entity. This is the only
/// constructor of `Claim`s anywhere, so an entity cannot be handed two claims.
pub(crate) fn build_arena(
    base: Vec<RawEntity>,
    ours: Vec<RawEntity>,
    theirs: Vec<RawEntity>,
) -> (Arena, Claims) {
    let mut entities: Vec<Entity> = Vec::with_capacity(base.len() + ours.len() + theirs.len());
    let mut push_version = |mut raws: Vec<RawEntity>| -> Vec<Idx> {
        for raw in raws.iter_mut() {
            raw.name = erase_lifetimes(&raw.name);
            raw.name = name_the_unnameable(&raw.name, &raw.content);
        }

        // Ordinals disambiguate same-named siblings. Counting them used to
        // clone `parent`, `entity_type` and `name` into the map key on *every*
        // entity — three allocations per entity per version, thrown away
        // immediately. Borrowing the names for the count and consuming the
        // entities afterwards costs nothing and says the same thing.
        let ordinals_per_entity: Vec<u32> = {
            let mut counts: HashMap<(Option<&str>, &str, &str), u32> = HashMap::new();
            raws.iter()
                .map(|raw| {
                    let slot = counts
                        .entry((
                            raw.parent.as_deref(),
                            raw.entity_type.as_str(),
                            raw.name.as_str(),
                        ))
                        .or_insert(0);
                    let ordinal = *slot;
                    *slot += 1;
                    ordinal
                })
                .collect()
        };

        let mut out = Vec::with_capacity(raws.len());
        for (position, raw) in raws.into_iter().enumerate() {
            let ordinal = ordinals_per_entity[position];
            let normalized = replace_at_word_boundaries(&raw.content, &raw.name, "__ENTITY__");
            let entity = Entity {
                key: Key {
                    parent: raw.parent,
                    entity_type: raw.entity_type,
                    name: raw.name,
                    ordinal,
                },
                body_hash: hash64(&normalized),
                content: raw.content,
                position: position as u32,
                src_id: raw.src_id,
            };
            out.push(Idx(entities.len() as u32));
            entities.push(entity);
        }
        out
    };

    let base_idx = push_version(base);
    let ours_idx = push_version(ours);
    let theirs_idx = push_version(theirs);

    let claims = Claims {
        base: base_idx.into_iter().map(BaseClaim::new).collect(),
        ours: ours_idx.into_iter().map(OursClaim::new).collect(),
        theirs: theirs_idx.into_iter().map(TheirsClaim::new).collect(),
    };
    (Arena::from_entities(entities), claims)
}

// ---------------------------------------------------------------------------
// Claim pools
// ---------------------------------------------------------------------------

/// A pool of not-yet-placed claims. `take` moves a claim out; a second `take`
/// of the same slot yields `None`, so a claim cannot be spent twice even by a
/// buggy matcher.
struct Pool<C> {
    slots: Vec<Option<C>>,
    by_idx: HashMap<Idx, usize>,
}

impl<C> Pool<C> {
    fn take(&mut self, idx: Idx) -> Option<C> {
        let slot = *self.by_idx.get(&idx)?;
        self.slots[slot].take()
    }
    fn remaining(&self) -> Vec<Idx> {
        let mut out: Vec<Idx> = self
            .by_idx
            .iter()
            .filter(|(_, &slot)| self.slots[slot].is_some())
            .map(|(&idx, _)| idx)
            .collect();
        out.sort_unstable();
        out
    }
    fn drain(self) -> Vec<C> {
        self.slots.into_iter().flatten().collect()
    }
}

macro_rules! pool_from {
    ($claims:expr) => {{
        let claims = $claims;
        let by_idx = claims
            .iter()
            .enumerate()
            .map(|(slot, c)| (c.idx(), slot))
            .collect();
        Pool {
            slots: claims.into_iter().map(Some).collect(),
            by_idx,
        }
    }};
}

// ---------------------------------------------------------------------------
// The pass
// ---------------------------------------------------------------------------

/// Token signatures, borrowed from the arena and built only for the entities a
/// similarity stage actually looks at.
///
/// Signatures are NOT stored on the entity. Tokenizing every entity of every
/// version up front costs an allocation per token on files where identity
/// settles everything — which is most files, and is exactly the 2000-entity
/// case the perf test pins.
type Tokens<'a> = HashMap<Idx, BTreeSet<&'a str>>;

fn tokens_for<'a>(arena: &'a Arena, idxs: &[Idx]) -> Tokens<'a> {
    idxs.iter()
        .map(|&i| (i, arena.get(i).content.split_whitespace().collect()))
        .collect()
}

/// A proposed base↔branch pairing, with the evidence that supports it.
struct Candidate {
    base: Idx,
    branch: Idx,
    confidence: f64,
    link: Link,
}

pub(crate) fn match_phase(arena: Arena, claims: Claims) -> Matching {
    let mut base_pool: Pool<BaseClaim> = pool_from!(claims.base);
    let mut ours_pool: Pool<OursClaim> = pool_from!(claims.ours);
    let mut theirs_pool: Pool<TheirsClaim> = pool_from!(claims.theirs);

    // -- 1. Identity ------------------------------------------------------
    // Same key on both sides. One hash lookup per entity, no scoring.
    let ours_by_key = key_index(&arena, &ours_pool.remaining());
    let theirs_by_key = key_index(&arena, &theirs_pool.remaining());

    let base_names: HashSet<&str> = base_pool
        .remaining()
        .into_iter()
        .map(|i| arena.get(i).name())
        .collect();

    let mut pairing_ours: HashMap<Idx, (Idx, Link)> = HashMap::new();
    let mut pairing_theirs: HashMap<Idx, (Idx, Link)> = HashMap::new();
    for base_idx in base_pool.remaining() {
        let key = &arena.get(base_idx).key;
        if let Some(&o) = ours_by_key.get(key) {
            pairing_ours.insert(base_idx, (o, Link::Identity));
        }
        if let Some(&t) = theirs_by_key.get(key) {
            pairing_theirs.insert(base_idx, (t, Link::Identity));
        }
    }

    // -- 2. Relational evidence, read off the identity pairing ------------
    // Computed here, before rename pairing, and deliberately independent of it:
    // whether a body moved house is a fact about the text, not a consequence of
    // which pairing the matcher went on to prefer. (A split whose larger half
    // also passes the rename bar is *both* a rename and a re-home; suppressing
    // the second fact because the first fired is how REHOME-DUP stayed
    // invisible.)
    let ours_unspent = ours_pool.remaining();
    let theirs_unspent = theirs_pool.remaining();
    let base_unspent = base_pool.remaining();
    let mut relational = detect_rehomes(
        &arena,
        &pairing_ours,
        &pairing_theirs,
        &ours_unspent,
        &theirs_unspent,
        &base_unspent,
    );
    relational.extend(detect_extractions(
        &arena,
        &pairing_ours,
        &pairing_theirs,
        &ours_unspent,
        &theirs_unspent,
    ));

    // -- 3. Renames, per side --------------------------------------------
    // Run per side from the identity pairing, so a re-run below starts from the
    // same place this one did.
    let identity_ours = pairing_ours;
    let identity_theirs = pairing_theirs;
    let infer = |reserved_ours: &HashSet<Idx>, reserved_theirs: &HashSet<Idx>| {
        let mut ours = identity_ours.clone();
        let mut theirs = identity_theirs.clone();
        for (branch_unspent, reserved, pairing) in [
            (&ours_unspent, reserved_ours, &mut ours),
            (&theirs_unspent, reserved_theirs, &mut theirs),
        ] {
            infer_renames(
                &arena,
                &base_unspent,
                branch_unspent,
                reserved,
                &base_names,
                pairing,
            );
        }
        (ours, theirs)
    };
    let nothing_reserved: HashSet<Idx> = HashSet::new();
    let (mut pairing_ours, mut pairing_theirs) = infer(&nothing_reserved, &nothing_reserved);

    // -- 3b. Name equality dominates body shape ---------------------------
    // An addition whose key also appears as an addition on the *other* side is
    // one entity both sides created, and that agreement — or that contradiction
    // — is the fact about it. Rename inference is entitled to consume such an
    // addition only if the *other* side inferred the same rename: then the two
    // sides agree that one base entity acquired that name, and the pair is a
    // convergent rename rather than two independent creations.
    //
    // Otherwise the inference is a steal. TypeScript bodies share a long
    // `export function …(): number { return` prefix, so the similarity bar is
    // trivially cleared and a base entity deleted on one side captured that
    // side's copy of a both-added name; the other side's copy was then emitted
    // beside it as a lone addition. That is the same name defined twice in a
    // merge reported clean and — when the two bodies differed — a real
    // `AddedBothDivergent` contradiction resolved silently. Forbid those
    // additions and infer again: name equality outranks body shape.
    //
    // The reservation is on the key alone, never on the key *and* body: the
    // divergent case is exactly the one that must survive to become a conflict.
    let mut reserved_ours: HashSet<Idx> = HashSet::new();
    let mut reserved_theirs: HashSet<Idx> = HashSet::new();
    for (ours_idx, theirs_idx) in convergent_additions(
        &arena,
        &identity_ours,
        &identity_theirs,
        &ours_unspent,
        &theirs_unspent,
    ) {
        if renamed_from(&pairing_ours, ours_idx) != renamed_from(&pairing_theirs, theirs_idx) {
            reserved_ours.insert(ours_idx);
            reserved_theirs.insert(theirs_idx);
        }
    }
    if !reserved_ours.is_empty() {
        let (ours, theirs) = infer(&reserved_ours, &reserved_theirs);
        pairing_ours = ours;
        pairing_theirs = theirs;
    }

    // -- 4. Build the base-anchored triples -------------------------------
    let mut triples: Vec<Triple> = Vec::new();
    for base_idx in base_pool.remaining() {
        let base_claim = base_pool.take(base_idx).expect("base claim is unspent");
        let (ours_claim, ours_link) = match pairing_ours.get(&base_idx) {
            Some((idx, link)) => (ours_pool.take(*idx), link.clone()),
            None => (None, Link::Absent),
        };
        let (theirs_claim, theirs_link) = match pairing_theirs.get(&base_idx) {
            Some((idx, link)) => (theirs_pool.take(*idx), link.clone()),
            None => (None, Link::Absent),
        };
        triples.push(Triple {
            base: Some(base_claim),
            ours: ours_claim,
            theirs: theirs_claim,
            evidence: Evidence {
                ours_link,
                theirs_link,
            },
        });
    }

    // -- 5. Both-added --------------------------------------------------
    // An added entity's key is derived from its name, so two additions
    // collide exactly when they agree on the key.
    let theirs_added_by_key = key_index(&arena, &theirs_pool.remaining());
    for ours_idx in ours_pool.remaining() {
        let key = arena.get(ours_idx).key.clone();
        let Some(&theirs_idx) = theirs_added_by_key.get(&key) else {
            continue;
        };
        let (Some(o), Some(t)) = (ours_pool.take(ours_idx), theirs_pool.take(theirs_idx)) else {
            continue;
        };
        triples.push(Triple {
            base: None,
            ours: Some(o),
            theirs: Some(t),
            evidence: Evidence {
                ours_link: Link::Added,
                theirs_link: Link::Added,
            },
        });
    }

    // -- 6. Singletons: everything still unspent -------------------------
    for claim in ours_pool.drain() {
        triples.push(Triple {
            base: None,
            ours: Some(claim),
            theirs: None,
            evidence: Evidence {
                ours_link: Link::Added,
                theirs_link: Link::Absent,
            },
        });
    }
    for claim in theirs_pool.drain() {
        triples.push(Triple {
            base: None,
            ours: None,
            theirs: Some(claim),
            evidence: Evidence {
                ours_link: Link::Absent,
                theirs_link: Link::Added,
            },
        });
    }

    let matching = Matching {
        arena,
        triples,
        relational,
    };
    debug_assert!(
        matching.is_total_partition(),
        "match phase lost or duplicated an entity"
    );
    matching
}

impl Matching {
    /// True when every arena entity appears in exactly one triple slot. Claims
    /// cannot be copied, so this checks that none was dropped either.
    pub fn is_total_partition(&self) -> bool {
        let mut seen: HashSet<Idx> = HashSet::new();
        for t in &self.triples {
            for idx in [t.base_idx(), t.ours_idx(), t.theirs_idx()]
                .into_iter()
                .flatten()
            {
                if !seen.insert(idx) {
                    return false;
                }
            }
        }
        seen.len() == self.arena.len()
    }
}

// ---------------------------------------------------------------------------
// Candidate generation
// ---------------------------------------------------------------------------

fn key_index(arena: &Arena, idxs: &[Idx]) -> HashMap<Key, Idx> {
    idxs.iter()
        .map(|&i| (arena.get(i).key.clone(), i))
        .collect()
}

/// One side's rename pairing, greedily assigned from the candidate stages.
///
/// `pairing` comes in carrying the identity pairs and goes out carrying those
/// plus the renames. `reserved` names branch entities this pass may not
/// consume — see the name-equality dominance rule in [`match_phase`].
fn infer_renames(
    arena: &Arena,
    base_all: &[Idx],
    branch_all: &[Idx],
    reserved: &HashSet<Idx>,
    base_names: &HashSet<&str>,
    pairing: &mut HashMap<Idx, (Idx, Link)>,
) {
    let matched_branch: HashSet<Idx> = pairing.values().map(|(i, _)| *i).collect();
    let base_left: Vec<Idx> = base_all
        .iter()
        .copied()
        .filter(|b| !pairing.contains_key(b))
        .collect();
    let branch_left: Vec<Idx> = branch_all
        .iter()
        .copied()
        .filter(|b| !matched_branch.contains(b) && !reserved.contains(b))
        .collect();
    if base_left.is_empty() || branch_left.is_empty() {
        return;
    }

    // The corroborating evidence, computed once per side from the pairs
    // identity already settled: a third party that called `old` in base and
    // calls `new` in the branch is a call site the developer updated.
    let moves = moved_call_sites(arena, pairing);

    let mut candidates = Vec::new();
    candidates.extend(equal_body_candidates(arena, &base_left, &branch_left));
    let already: HashSet<Idx> = candidates.iter().map(|c| c.branch).collect();
    let base_still: Vec<Idx> = base_left
        .iter()
        .copied()
        .filter(|b| !candidates.iter().any(|c| c.base == *b))
        .collect();
    let branch_still: Vec<Idx> = branch_left
        .iter()
        .copied()
        .filter(|b| !already.contains(b))
        .collect();
    if !base_still.is_empty() && !branch_still.is_empty() {
        let mut all = base_still.clone();
        all.extend(branch_still.iter().copied());
        let tokens = tokens_for(arena, &all);
        candidates.extend(signature_candidates(
            arena,
            &tokens,
            &base_still,
            &branch_still,
        ));
        candidates.extend(short_body_candidates(
            arena,
            &tokens,
            &base_still,
            &branch_still,
            base_names,
        ));
        candidates.extend(corroborated_candidates(
            arena,
            &tokens,
            &base_still,
            &branch_still,
            &moves,
            base_names,
        ));
    }

    // Greedy by confidence, ties broken by identity key: deterministic and
    // independent of file order.
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| arena.get(a.base).key.cmp(&arena.get(b.base).key))
            .then_with(|| arena.get(a.branch).key.cmp(&arena.get(b.branch).key))
    });
    let mut used_base: HashSet<Idx> = pairing.keys().copied().collect();
    let mut used_branch: HashSet<Idx> = pairing.values().map(|(i, _)| *i).collect();
    for c in candidates {
        if used_base.contains(&c.base) || used_branch.contains(&c.branch) {
            continue;
        }
        used_base.insert(c.base);
        used_branch.insert(c.branch);
        pairing.insert(c.base, (c.branch, c.link));
    }
}

/// The base entity a side's pairing says `branch` came from, if any.
///
/// Two sides "inferred the same rename" exactly when this answers the same base
/// entity for their two copies of one name — including answering `None` for
/// both, which is the case where neither side inferred a rename at all.
fn renamed_from(pairing: &HashMap<Idx, (Idx, Link)>, branch: Idx) -> Option<Idx> {
    pairing
        .iter()
        .find(|(_, (idx, _))| *idx == branch)
        .map(|(&base, _)| base)
}

/// The (ours, theirs) pairs of additions that both sides made under one key.
///
/// "Addition" here means exactly what the both-added step means by it: the
/// entity did not pair with any base entity by identity. Ordered by ours-side
/// index so the reservation set does not depend on hash iteration order.
fn convergent_additions(
    arena: &Arena,
    pairing_ours: &HashMap<Idx, (Idx, Link)>,
    pairing_theirs: &HashMap<Idx, (Idx, Link)>,
    ours_all: &[Idx],
    theirs_all: &[Idx],
) -> Vec<(Idx, Idx)> {
    let paired_ours: HashSet<Idx> = pairing_ours.values().map(|(i, _)| *i).collect();
    let paired_theirs: HashSet<Idx> = pairing_theirs.values().map(|(i, _)| *i).collect();
    let theirs_added: Vec<Idx> = theirs_all
        .iter()
        .copied()
        .filter(|i| !paired_theirs.contains(i))
        .collect();
    let theirs_by_key = key_index(arena, &theirs_added);

    ours_all
        .iter()
        .copied()
        .filter(|i| !paired_ours.contains(i))
        .filter_map(|ours_idx| {
            theirs_by_key
                .get(&arena.get(ours_idx).key)
                .map(|&theirs_idx| (ours_idx, theirs_idx))
        })
        .collect()
}

/// Identical bodies under different names. Buckets are paired in key order:
/// when `k` base and `k` branch entities all share one normalized body there is
/// no evidence distinguishing any pairing from any other, so the honest choice
/// is the deterministic one — and it costs `O(k log k)`, not `O(k²)`, which is
/// why `MAX_RENAME_BUCKET` is gone.
fn equal_body_candidates(arena: &Arena, base: &[Idx], branch: &[Idx]) -> Vec<Candidate> {
    let mut base_by_body: BTreeMap<u64, Vec<Idx>> = BTreeMap::new();
    for &b in base {
        base_by_body
            .entry(arena.get(b).body_hash)
            .or_default()
            .push(b);
    }
    let mut branch_by_body: BTreeMap<u64, Vec<Idx>> = BTreeMap::new();
    for &b in branch {
        branch_by_body
            .entry(arena.get(b).body_hash)
            .or_default()
            .push(b);
    }
    let mut out = Vec::new();
    for (hash, mut bases) in base_by_body {
        let Some(mut branches) = branch_by_body.remove(&hash) else {
            continue;
        };
        bases.sort_by(|a, b| arena.get(*a).key.cmp(&arena.get(*b).key));
        branches.sort_by(|a, b| arena.get(*a).key.cmp(&arena.get(*b).key));
        for (&b, &n) in bases.iter().zip(branches.iter()) {
            // Same type only: a function is not a rename of a struct.
            if arena.get(b).key.entity_type != arena.get(n).key.entity_type {
                continue;
            }
            out.push(Candidate {
                base: b,
                branch: n,
                confidence: 0.95,
                link: Link::BodyHash,
            });
        }
    }
    out
}

/// Similar-but-not-equal bodies, found through a prefix-filtered inverted
/// index rather than by comparing every pair.
fn signature_candidates<'a>(
    arena: &Arena,
    tokens_of: &Tokens<'a>,
    base: &[Idx],
    branch: &[Idx],
) -> Vec<Candidate> {
    if base.is_empty() || branch.is_empty() {
        return Vec::new();
    }
    // Global token frequency over the branch side, so the index is keyed on the
    // rarest tokens — the ones that actually discriminate.
    let mut df: HashMap<&str, usize> = HashMap::new();
    for &b in branch {
        for tok in &tokens_of[&b] {
            *df.entry(*tok).or_insert(0) += 1;
        }
    }
    let ordered = |set: &BTreeSet<&'a str>| -> Vec<&'a str> {
        let mut v: Vec<&'a str> = set.iter().copied().collect();
        v.sort_by(|a, b| {
            df.get(*a)
                .copied()
                .unwrap_or(0)
                .cmp(&df.get(*b).copied().unwrap_or(0))
                .then_with(|| a.cmp(b))
        });
        v
    };
    let prefix_len = |n: usize| -> usize {
        if n == 0 {
            return 0;
        }
        // How many leading tokens to index: enough that any two bodies above the
        // similarity threshold are guaranteed to share at least one of them.
        let keep = (RENAME_MIN_SIMILARITY * n as f64).ceil() as usize;
        n.saturating_sub(keep) + 1
    };

    let mut index: HashMap<&'a str, Vec<Idx>> = HashMap::new();
    let branch_tokens: HashMap<Idx, Vec<&'a str>> = branch
        .iter()
        .map(|&b| (b, ordered(&tokens_of[&b])))
        .collect();
    for &b in branch {
        let toks = &branch_tokens[&b];
        for tok in toks.iter().take(prefix_len(toks.len())) {
            index.entry(tok).or_default().push(b);
        }
    }

    let mut out = Vec::new();
    for &b in base {
        let base_e = arena.get(b);
        let toks = ordered(&tokens_of[&b]);
        let mut seen: BTreeSet<Idx> = BTreeSet::new();
        for tok in toks.iter().take(prefix_len(toks.len())) {
            if let Some(cands) = index.get(tok) {
                seen.extend(cands.iter().copied());
            }
        }
        for cand in seen {
            let cand_e = arena.get(cand);
            if cand_e.key.entity_type != base_e.key.entity_type
                || cand_e.key.parent != base_e.key.parent
            {
                continue;
            }
            let sim = jaccard(&tokens_of[&b], &tokens_of[&cand]);
            if sim < RENAME_MIN_SIMILARITY {
                continue;
            }
            out.push(Candidate {
                base: b,
                branch: cand,
                confidence: 0.6 + sim * 0.3,
                link: Link::Signature { similarity: sim },
            });
        }
    }
    out
}

/// Renames of bodies too small for a symmetric similarity score to mean
/// anything. Candidates come from *rare* shared tokens, and the acceptance
/// test is containment rather than Jaccard.
fn short_body_candidates(
    arena: &Arena,
    tokens_of: &Tokens<'_>,
    base: &[Idx],
    branch: &[Idx],
    base_names: &HashSet<&str>,
) -> Vec<Candidate> {
    let short = |i: &Idx| tokens_of[i].len() <= SHORT_BODY_TOKENS;
    let base_short: Vec<Idx> = base.iter().copied().filter(short).collect();
    let branch_short: Vec<Idx> = branch.iter().copied().filter(short).collect();
    if base_short.is_empty() || branch_short.is_empty() {
        return Vec::new();
    }
    let mut df: HashMap<&str, usize> = HashMap::new();
    for &b in &branch_short {
        for tok in &tokens_of[&b] {
            *df.entry(*tok).or_insert(0) += 1;
        }
    }
    let mut index: HashMap<&str, Vec<Idx>> = HashMap::new();
    for &b in &branch_short {
        for tok in &tokens_of[&b] {
            if df.get(*tok).copied().unwrap_or(0) <= RARE_DF {
                index.entry(*tok).or_default().push(b);
            }
        }
    }
    let mut out = Vec::new();
    for &b in &base_short {
        let base_e = arena.get(b);
        let mut seen: BTreeSet<Idx> = BTreeSet::new();
        for tok in &tokens_of[&b] {
            if let Some(cands) = index.get(*tok) {
                seen.extend(cands.iter().copied());
            }
        }
        for cand in seen {
            let cand_e = arena.get(cand);
            if cand_e.key.entity_type != base_e.key.entity_type
                || cand_e.key.parent != base_e.key.parent
            {
                continue;
            }
            // A name that already existed in base is a different entity.
            if base_names.contains(cand_e.name()) {
                continue;
            }
            let inter = tokens_of[&b].intersection(&tokens_of[&cand]).count();
            let min = tokens_of[&b].len().min(tokens_of[&cand].len()).max(1);
            let containment = inter as f64 / min as f64;
            if containment < SHORT_BODY_MIN_CONTAINMENT {
                continue;
            }
            out.push(Candidate {
                base: b,
                branch: cand,
                // Below the corroborated band: a call site that moved is
                // stronger evidence than a body that mostly survived.
                confidence: 0.45 + containment * 0.1,
                link: Link::Signature {
                    similarity: containment,
                },
            });
        }
    }
    out
}

/// Rename *and* edit in one commit: the body moved too far for similarity to
/// carry the pairing alone, so it is carried by call sites that moved with it.
/// Driven by the (small) move set, so this costs a lookup per move, not a scan.
fn corroborated_candidates(
    arena: &Arena,
    tokens_of: &Tokens<'_>,
    base: &[Idx],
    branch: &[Idx],
    moves: &HashSet<(String, String)>,
    base_names: &HashSet<&str>,
) -> Vec<Candidate> {
    if moves.is_empty() {
        return Vec::new();
    }
    let mut base_by_name: HashMap<&str, Vec<Idx>> = HashMap::new();
    for &b in base {
        base_by_name.entry(arena.get(b).name()).or_default().push(b);
    }
    let mut branch_by_name: HashMap<&str, Vec<Idx>> = HashMap::new();
    for &b in branch {
        branch_by_name
            .entry(arena.get(b).name())
            .or_default()
            .push(b);
    }
    let mut out = Vec::new();
    let mut sorted: Vec<&(String, String)> = moves.iter().collect();
    sorted.sort();
    for (old, new) in sorted {
        let (Some(olds), Some(news)) = (
            base_by_name.get(old.as_str()),
            branch_by_name.get(new.as_str()),
        ) else {
            continue;
        };
        for &b in olds {
            for &n in news {
                let base_e = arena.get(b);
                let cand_e = arena.get(n);
                if base_e.key.entity_type != cand_e.key.entity_type
                    || base_e.key.parent != cand_e.key.parent
                {
                    continue;
                }
                // The target name must be genuinely new: a name that already
                // existed in base is a different entity, not a rename target.
                if base_names.contains(cand_e.name()) {
                    continue;
                }
                let sim = jaccard(&tokens_of[&b], &tokens_of[&n]);
                let floor = if tokens_of[&b].len().max(tokens_of[&n].len()) <= SHORT_BODY_TOKENS {
                    0.0
                } else {
                    RENAME_EDIT_MIN_SIMILARITY
                };
                if sim < floor {
                    continue;
                }
                out.push(Candidate {
                    base: b,
                    branch: n,
                    confidence: 0.55 + sim * 0.1,
                    link: Link::CallSiteCorroborated { similarity: sim },
                });
            }
        }
    }
    out
}

/// (old → new) call-site moves visible in entities that identity already
/// paired: they called `old` in base and call `new` — and no longer `old` — in
/// the branch.
fn moved_call_sites(
    arena: &Arena,
    pairing: &HashMap<Idx, (Idx, Link)>,
) -> HashSet<(String, String)> {
    let mut moves = HashSet::new();
    for (&base_idx, (branch_idx, link)) in pairing {
        if !matches!(link, Link::Identity) {
            continue;
        }
        let before = called_names(&arena.get(base_idx).content);
        let after = called_names(&arena.get(*branch_idx).content);
        for old in before.difference(&after) {
            for new in after.difference(&before) {
                moves.insert((old.to_string(), new.to_string()));
            }
        }
    }
    moves
}

fn jaccard(a: &BTreeSet<&str>, b: &BTreeSet<&str>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = a.intersection(b).count();
    let union = a.len() + b.len() - inter;
    if union == 0 {
        1.0
    } else {
        inter as f64 / union as f64
    }
}

// ---------------------------------------------------------------------------
// Relational evidence: bodies that moved house
// ---------------------------------------------------------------------------

/// A signature line: non-blank, non-trivial content, whitespace-normalized.
/// Short lines (`}`, `end`, `return`) are structural noise shared by every
/// entity and would make any two functions look like re-homes of each other.
pub(super) fn signature_lines(content: &str) -> BTreeSet<String> {
    content
        .lines()
        // A definition line names the entity, so it can never re-home; keeping
        // it would put a floor under every coverage score.
        .filter(|l| !DEFINERS.iter().any(|kw| l.trim_start().starts_with(kw)))
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| l.len() > 8)
        .collect()
}

const DEFINERS: [&str; 14] = [
    "def ",
    "function ",
    "fn ",
    "class ",
    "func ",
    "interface ",
    "struct ",
    "trait ",
    "impl ",
    "export ",
    "public ",
    "private ",
    "async ",
    "pub ",
];

/// Which vanished entities had their bodies re-homed into new ones, per side.
///
/// This is a fact a key-by-key comparison cannot see: both sides deleted `bravo`,
/// so the key-wise verdict is "convergent deletion", while in
/// truth each side moved the body into *different* new entities and unioning
/// the additions emits every base line twice (REHOME-DUP). The matcher can see
/// it because it can see all three versions at once; `bind` is the stage with
/// the authority to act on it.
///
/// Coverage is measured against ALL of a side's new entities together, because
/// a split distributes one body over several of them — no single new home
/// contains enough of it to look suspicious on its own.
fn detect_rehomes(
    arena: &Arena,
    pairing_ours: &HashMap<Idx, (Idx, Link)>,
    pairing_theirs: &HashMap<Idx, (Idx, Link)>,
    ours_all: &[Idx],
    theirs_all: &[Idx],
    base_all: &[Idx],
) -> Vec<Relational> {
    let mut out = Vec::new();
    for (side, pairing, side_all) in [
        (Side::Ours, pairing_ours, ours_all),
        (Side::Theirs, pairing_theirs, theirs_all),
    ] {
        let matched: HashSet<Idx> = pairing.values().map(|(i, _)| *i).collect();
        let new_entities: Vec<Idx> = side_all
            .iter()
            .copied()
            .filter(|i| !matched.contains(i))
            .collect();
        let vanished: Vec<Idx> = base_all
            .iter()
            .copied()
            .filter(|b| !pairing.contains_key(b))
            .collect();
        if new_entities.is_empty() || vanished.is_empty() {
            continue;
        }
        let mut line_index: HashMap<String, Vec<Idx>> = HashMap::new();
        for &idx in &new_entities {
            for line in signature_lines(&arena.get(idx).content) {
                line_index.entry(line).or_default().push(idx);
            }
        }
        for &del in &vanished {
            let lines = signature_lines(&arena.get(del).content);
            if lines.is_empty() {
                continue;
            }
            let mut homes: BTreeSet<Idx> = BTreeSet::new();
            let mut covered = 0usize;
            for line in &lines {
                if let Some(hs) = line_index.get(line) {
                    covered += 1;
                    homes.extend(hs.iter().copied());
                }
            }
            let coverage = covered as f64 / lines.len() as f64;
            if coverage >= REHOME_MIN_OVERLAP {
                out.push(Relational::RehomedInto {
                    deleted: del,
                    side,
                    homes: homes.into_iter().collect(),
                    coverage,
                });
            }
        }
    }
    out
}

/// Which surviving entities had part of their body extracted into new ones.
///
/// The extract-method refactoring keeps the source key alive, so
/// [`detect_rehomes`] — which quantifies over base keys that *vanished* — cannot
/// see it. The evidence here is the same kind, one predicate weaker: a base
/// signature line that is gone from the side's own version of the entity and
/// present in an entity that side added has *moved*, and the merge is entitled
/// to follow it.
///
/// Deliberately narrow. Only lines the host actually lost count, only entities
/// with no base counterpart can be a destination, and `signature_lines` already
/// drops definition lines and anything under 9 characters — so a coincidence of
/// `return value` between two unrelated functions cannot manufacture one.
fn detect_extractions(
    arena: &Arena,
    pairing_ours: &HashMap<Idx, (Idx, Link)>,
    pairing_theirs: &HashMap<Idx, (Idx, Link)>,
    ours_all: &[Idx],
    theirs_all: &[Idx],
) -> Vec<Relational> {
    let mut out = Vec::new();
    for (side, pairing, side_all) in [
        (Side::Ours, pairing_ours, ours_all),
        (Side::Theirs, pairing_theirs, theirs_all),
    ] {
        let matched: HashSet<Idx> = pairing.values().map(|(i, _)| *i).collect();
        let new_entities: Vec<Idx> = side_all
            .iter()
            .copied()
            .filter(|i| !matched.contains(i))
            .collect();
        if new_entities.is_empty() {
            continue;
        }
        let mut line_index: HashMap<String, Vec<Idx>> = HashMap::new();
        for &idx in &new_entities {
            for line in signature_lines(&arena.get(idx).content) {
                line_index.entry(line).or_default().push(idx);
            }
        }
        for (&source, &(host, _)) in pairing {
            let base_lines = signature_lines(&arena.get(source).content);
            if base_lines.is_empty() {
                continue;
            }
            let host_lines = signature_lines(&arena.get(host).content);
            let mut homes: BTreeSet<Idx> = BTreeSet::new();
            let mut moved: Vec<String> = Vec::new();
            for line in base_lines.difference(&host_lines) {
                if let Some(hs) = line_index.get(line) {
                    moved.push(line.clone());
                    homes.extend(hs.iter().copied());
                }
            }
            if moved.is_empty() {
                continue;
            }
            moved.sort();
            out.push(Relational::ExtractedInto {
                source,
                side,
                host,
                extracted: homes.into_iter().collect(),
                moved,
            });
        }
    }
    // `pairing` is a HashMap, so the discovery order is not stable. Nothing
    // downstream may depend on hash iteration order.
    out.sort_by_key(|r| match r {
        Relational::ExtractedInto { source, side, .. } => (*source, *side),
        Relational::RehomedInto { deleted, side, .. } => (*deleted, *side),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(name: &str, content: &str) -> RawEntity {
        RawEntity {
            name: name.to_string(),
            entity_type: "function".to_string(),
            parent: None,
            content: content.to_string(),
            src_id: format!("f::{name}"),
        }
    }

    fn run(b: Vec<RawEntity>, o: Vec<RawEntity>, t: Vec<RawEntity>) -> Matching {
        let (arena, claims) = build_arena(b, o, t);
        match_phase(arena, claims)
    }

    #[test]
    fn identity_pairs_and_partitions_totally() {
        let m = run(
            vec![raw("a", "def a():\n    return 1\n")],
            vec![raw("a", "def a():\n    return 2\n")],
            vec![raw("a", "def a():\n    return 1\n")],
        );
        assert!(m.is_total_partition());
        assert_eq!(m.triples.len(), 1);
    }

    #[test]
    fn equal_body_under_a_new_name_is_a_rename() {
        let body = "    for x in items():\n        handle(x)\n    return done()\n";
        let m = run(
            vec![raw("alpha", &format!("def alpha():\n{body}"))],
            vec![raw("alpha_v2", &format!("def alpha_v2():\n{body}"))],
            vec![raw("alpha", &format!("def alpha():\n{body}"))],
        );
        assert_eq!(m.triples.len(), 1);
        assert!(m.triples[0].evidence.ours_link.is_rename());
    }

    #[test]
    fn a_flood_of_identical_bodies_pairs_deterministically_without_a_cap() {
        // 400 identically-shaped entities renamed en masse: the case the
        // MAX_RENAME_BUCKET cap existed to bail out of.
        let body = "    return handle(payload)\n";
        let base: Vec<RawEntity> = (0..400)
            .map(|i| raw(&format!("h{i:03}"), &format!("def h{i:03}():\n{body}")))
            .collect();
        let ours: Vec<RawEntity> = (0..400)
            .map(|i| raw(&format!("g{i:03}"), &format!("def g{i:03}():\n{body}")))
            .collect();
        let theirs: Vec<RawEntity> = (0..400)
            .map(|i| raw(&format!("h{i:03}"), &format!("def h{i:03}():\n{body}")))
            .collect();
        let start = std::time::Instant::now();
        let m = run(base, ours, theirs);
        assert!(
            start.elapsed().as_millis() < 2000,
            "matching went quadratic: {:?}",
            start.elapsed()
        );
        assert!(m.is_total_partition());
        assert_eq!(
            m.triples.len(),
            400,
            "every base entity paired exactly once"
        );
    }

    #[test]
    fn rename_plus_edit_is_carried_by_moved_call_sites() {
        // One-line body: far too small for any similarity bar.
        let m = run(
            vec![
                raw("tiny", "def tiny():\n    return 1\n"),
                raw("caller", "def caller():\n    return tiny()\n"),
            ],
            vec![
                raw("tiny_v2", "def tiny_v2():\n    validated()\n    return 2\n"),
                raw("caller", "def caller():\n    return tiny_v2()\n"),
            ],
            vec![
                raw("tiny", "def tiny():\n    return 1\n"),
                raw("caller", "def caller():\n    return tiny()\n"),
            ],
        );
        let renamed = m
            .triples
            .iter()
            .find(|t| t.base.is_some() && m.arena.get(t.base_idx().unwrap()).name() == "tiny")
            .expect("tiny triple");
        assert!(
            renamed.evidence.ours_link.is_rename(),
            "rename+edit on a tiny body was not recovered: {:?}",
            renamed.evidence
        );
    }

    #[test]
    fn split_and_absorb_are_recorded_as_rehomes() {
        let base = vec![
            raw(
                "bravo",
                "def bravo():\n    prepare_the_payload()\n    value = compute_total()\n",
            ),
            raw(
                "charlie",
                "def charlie():\n    return settings_for_user()\n",
            ),
        ];
        // Ours splits bravo in two; theirs absorbs bravo+charlie into one.
        let ours = vec![
            raw("bravo_a", "def bravo_a():\n    prepare_the_payload()\n"),
            raw("bravo_b", "def bravo_b():\n    value = compute_total()\n"),
            raw(
                "charlie",
                "def charlie():\n    return settings_for_user()\n",
            ),
        ];
        let theirs = vec![raw(
            "bravo_charlie",
            "def bravo_charlie():\n    prepare_the_payload()\n    value = compute_total()\n    return settings_for_user()\n",
        )];
        let m = run(base, ours, theirs);
        assert!(
            !m.relational.is_empty(),
            "no re-home evidence recorded for a split+absorb"
        );
    }

    #[test]
    fn an_extraction_that_leaves_the_source_alive_is_recorded() {
        // The commonest refactoring there is: pull two statements into a helper
        // and call it. `bravo` keeps its key, so `detect_rehomes` — which
        // quantifies over keys that vanished — is silent, and the fact would be
        // lost without `ExtractedInto`.
        let base = vec![raw(
            "bravo",
            "def bravo():\n    prepare_the_payload()\n    value = compute_total()\n",
        )];
        let ours = vec![
            raw("helper", "def helper():\n    prepare_the_payload()\n"),
            raw(
                "bravo",
                "def bravo():\n    helper()\n    value = compute_total()\n",
            ),
        ];
        let theirs = base
            .iter()
            .map(|r| raw(&r.name, &r.content))
            .collect::<Vec<_>>();
        let m = run(base, ours, theirs);
        let found = m.relational.iter().find_map(|r| match r {
            Relational::ExtractedInto {
                side,
                moved,
                extracted,
                ..
            } => Some((*side, moved.clone(), extracted.len())),
            _ => None,
        });
        let (side, moved, homes) = found.expect("no extraction evidence recorded");
        assert_eq!(side, Side::Ours);
        assert_eq!(moved, vec!["prepare_the_payload()".to_string()]);
        assert_eq!(homes, 1);
    }

    #[test]
    fn an_unrelated_addition_is_not_an_extraction() {
        // The gate that keeps the evidence honest: nothing left `bravo`, so a
        // new function alongside it is just a new function.
        let base = vec![raw(
            "bravo",
            "def bravo():\n    prepare_the_payload()\n    value = compute_total()\n",
        )];
        let ours = vec![
            raw(
                "bravo",
                "def bravo():\n    prepare_the_payload()\n    value = compute_total()\n",
            ),
            raw("newcomer", "def newcomer():\n    return something_else()\n"),
        ];
        let theirs = base
            .iter()
            .map(|r| raw(&r.name, &r.content))
            .collect::<Vec<_>>();
        let m = run(base, ours, theirs);
        assert!(
            !m.relational
                .iter()
                .any(|r| matches!(r, Relational::ExtractedInto { .. })),
            "fabricated an extraction: {:?}",
            m.relational
        );
    }
}
