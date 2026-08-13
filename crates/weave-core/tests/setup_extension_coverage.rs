//! Parity between the parser registry and what `weave setup` claims.
//!
//! `weave setup` writes `*.<ext> merge=weave` for the extensions the engine
//! entity-merges. That set is DERIVED from the parser registry via
//! [`weave_core::supported_merge_extensions`] minus
//! [`weave_core::DECLINED_EXTENSIONS`], so it cannot silently fall behind the
//! grammars `sem-core` compiles in. These tests are the guard that keeps the
//! derivation honest: every registered extension must be accounted for, and the
//! extensions this change exists to rescue (`.mts`/`.cts` + the registry's
//! newer languages) must actually appear.

use std::collections::BTreeSet;

use sem_core::parser::plugins::create_default_registry;
use weave_core::{supported_merge_extensions, DECLINED_EXTENSIONS};

/// Guard: no registered extension may fall through the crack. Every extension
/// the parser recognises is EITHER emitted by setup OR explicitly declined —
/// never silently dropped. A future grammar added to `sem-core` that setup
/// omits (and that nobody consciously declined) fails here, forcing a decision
/// instead of a quiet coverage hole.
#[test]
fn every_registered_extension_is_emitted_or_explicitly_declined() {
    let registry = create_default_registry();
    let emitted: BTreeSet<String> = supported_merge_extensions().into_iter().collect();
    let declined: BTreeSet<&str> = DECLINED_EXTENSIONS.iter().copied().collect();

    let mut unaccounted: Vec<String> = registry
        .registered_extensions()
        .into_iter()
        .filter(|ext| !emitted.contains(*ext) && !declined.contains(*ext))
        .map(str::to_string)
        .collect();
    unaccounted.sort();

    assert!(
        unaccounted.is_empty(),
        "these parser extensions are neither emitted by `weave setup` nor listed \
         in weave_core::DECLINED_EXTENSIONS — decide which and update the \
         declined set (they merge badly) or let the derivation claim them: {unaccounted:?}"
    );
}

/// Parity, the other direction: everything setup emits is really a registered,
/// non-declined extension — setup never invents coverage the engine can't back.
#[test]
fn every_emitted_extension_is_registered_and_not_declined() {
    let registry = create_default_registry();
    let registered: BTreeSet<&str> = registry.registered_extensions().into_iter().collect();
    let declined: BTreeSet<&str> = DECLINED_EXTENSIONS.iter().copied().collect();

    for ext in supported_merge_extensions() {
        assert!(
            registered.contains(ext.as_str()),
            "setup would claim `{ext}` but no parser plugin registers it"
        );
        assert!(
            !declined.contains(ext.as_str()),
            "setup would claim `{ext}` but it is declined for entity merge"
        );
    }
}

/// The concrete drift this work closes: `.mts`/`.cts` (and `.mjs`/`.cjs`) plus a
/// sample of the registry's newer languages must be claimed. These parse and
/// entity-merge; before the derivation, setup's hand-maintained list omitted
/// `.mts`/`.cts` entirely.
#[test]
fn mts_cts_and_registry_langs_are_merged() {
    let emitted: BTreeSet<String> = supported_merge_extensions().into_iter().collect();
    for ext in [
        ".mts", ".cts", ".mjs", ".cjs", ".kt", ".tf", ".hcl", ".ml", ".mli", ".zig", ".elm",
        ".clj", ".edn", ".d", ".lua", ".fish", ".nix", ".sql", ".tex", ".pl", ".csv",
    ] {
        assert!(
            emitted.contains(ext),
            "{ext} must be entity-merged by setup"
        );
    }
    for ext in [".hs", ".vue", ".svelte", ".erb"] {
        assert!(
            !emitted.contains(ext),
            "{ext} merges worse than git and must not be claimed"
        );
    }
}

/// The whole Svelte family merges via the Svelte plugin and is declined — not
/// just the bare `.svelte` component and the two suffixes an earlier declined
/// list happened to name. This guards the gap that once let `.svelte.spec.js`
/// slip into the emitted set: NO emitted extension may belong to a declined
/// language, however many compound suffixes that language registers.
#[test]
fn no_emitted_extension_belongs_to_a_declined_language() {
    let emitted: BTreeSet<String> = supported_merge_extensions().into_iter().collect();
    let offenders: Vec<&String> = emitted
        .iter()
        .filter(|ext| {
            ext.starts_with(".svelte") || *ext == ".vue" || *ext == ".erb" || *ext == ".hs"
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "these belong to a language that merges worse than git and must not be          claimed — extend weave_core::DECLINED_EXTENSIONS: {offenders:?}"
    );
}

/// `.lock` is genuinely not a parseable language — it must stay out, so git's
/// default line merge handles lockfiles.
#[test]
fn unparseable_lockfiles_are_not_claimed() {
    let emitted: BTreeSet<String> = supported_merge_extensions().into_iter().collect();
    assert!(
        !emitted.contains(".lock"),
        ".lock is not a parseable language and must not be claimed"
    );
}
