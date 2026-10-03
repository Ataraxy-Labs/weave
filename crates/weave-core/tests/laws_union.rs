//! Law witnesses for the two set-union rules on same-point insertions: the
//! changelog union (`D3 changelog union`) and the data key union (`D3 data
//! key union`).
//!
//! The structure under test: two sides that each insert a run of new items at
//! ONE point of an unordered collection — the entries of a changelog section,
//! the keys of a TOML table, JSON object or YAML mapping — made edits that commute under set
//! semantics. The merge is the union, with the base's items where the base
//! had them, and the two runs in a canonical order (the run whose text sorts
//! first goes first). Stated at the public boundary (`entity_merge`) only:
//!
//!   U1  Identity:          merge(b, b, t) = t   and   merge(b, o, b) = o
//!   U2  Idempotence:       merge(b, x, x) = x
//!   U3  Commutativity:     merge(b, o, t) is clean and byte-identical to
//!                          merge(b, t, o)
//!   U4  Union:             every base item, and every item either side
//!                          added, is in the merge exactly once; base items in
//!                          base order; each side's new items in that side's
//!                          order
//!   U5  Fail closed:       one key added by both with different values, a
//!                          heading inside one side's insertion, an existing
//!                          entry edited beside the insertion — a conflict
//!
//! The domain is synthetic: sentinel-tagged items, so which side wrote what is
//! decidable by substring. Ours writes 4xxxx, theirs 5xxxx, both 6xxxx.
//!
//! The changelog union is opt-in: a section is a set only where the file's
//! `weave-set` gitattribute declares it (two differently worded entries can
//! describe one change). The changelog surface is merged here under a host
//! that declares it; the data surfaces under the default host, where key
//! identity is structural and the union needs no declaration.

use proptest::prelude::*;
use weave_core::host::Host;
use weave_core::{entity_merge_fmt, MarkerFormat, MergeResult};

fn declare_all(_: &str) -> Option<String> {
    Some("set".to_string())
}

/// The public merge, with the changelog declared a set.
fn entity_merge(b: &str, o: &str, t: &str, path: &str) -> MergeResult {
    let host = Host {
        set_attribute: path
            .ends_with("CHANGELOG.md")
            .then_some(declare_all as fn(&str) -> Option<String>),
        ..Host::default()
    };
    entity_merge_fmt(b, o, t, path, &MarkerFormat::default(), &host)
}

// ===========================================================================
// The arbitrary domain
// ===========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Surface {
    Changelog,
    Toml,
    Json,
    Yaml,
}

impl Surface {
    fn path(self) -> &'static str {
        match self {
            Surface::Changelog => "CHANGELOG.md",
            Surface::Toml => "Cargo.toml",
            Surface::Json => "package.json",
            Surface::Yaml => "config.yaml",
        }
    }
}

/// An item: its sentinel, and (changelog only) whether it carries a
/// continuation line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Item {
    sentinel: u32,
    long: bool,
}

/// A base collection and two same-point insertions into it.
#[derive(Debug, Clone)]
struct Case {
    surface: Surface,
    base: Vec<Item>,
    at: usize,
    ours: Vec<Item>,
    theirs: Vec<Item>,
}

fn item_text(s: Surface, it: Item) -> String {
    match s {
        Surface::Changelog if it.long => format!(
            "- `cmd{0}`: new subcommand {0}.\n  It reads {0}.\n",
            it.sentinel
        ),
        Surface::Changelog => format!("- `cmd{0}`: new subcommand {0}.\n", it.sentinel),
        Surface::Toml => format!("dep{0} = \"{0}\"\n", it.sentinel),
        Surface::Json => format!("    \"dep{0}\": \"{0}\"", it.sentinel),
        Surface::Yaml => format!("dep{0}: \"{0}\"\n", it.sentinel),
    }
}

fn render(s: Surface, items: &[Item]) -> String {
    match s {
        Surface::Changelog => {
            let body: String = items.iter().map(|i| item_text(s, *i)).collect();
            format!(
                "# Changelog\n\n## Unreleased\n\n### Added\n{body}\n## 0.1.0\n\n- initial release\n"
            )
        }
        Surface::Toml => {
            let body: String = items.iter().map(|i| item_text(s, *i)).collect();
            format!(
                "[package]\nname = \"demo\"\n\n[dependencies]\n{body}\n[features]\ndefault = []\n"
            )
        }
        Surface::Json => {
            let body: Vec<String> = items.iter().map(|i| item_text(s, *i)).collect();
            let body = if body.is_empty() {
                String::new()
            } else {
                format!("{}\n", body.join(",\n"))
            };
            format!("{{\n  \"name\": \"demo\",\n  \"dependencies\": {{\n{body}  }},\n  \"private\": true\n}}\n")
        }
        Surface::Yaml => {
            let body: String = items.iter().map(|i| item_text(s, *i)).collect();
            format!("# settings\nname: demo\n{body}private: true\n")
        }
    }
}

fn side(case: &Case, run: &[Item]) -> Vec<Item> {
    let mut v = case.base.clone();
    for (k, it) in run.iter().enumerate() {
        v.insert(case.at + k, *it);
    }
    v
}

fn texts(case: &Case) -> (String, String, String) {
    let s = case.surface;
    (
        render(s, &case.base),
        render(s, &side(case, &case.ours)),
        render(s, &side(case, &case.theirs)),
    )
}

fn run_strategy(offset: u32) -> impl Strategy<Value = Vec<Item>> {
    prop::collection::btree_set(0u32..90, 1..4).prop_flat_map(move |ids| {
        let n = ids.len();
        (Just(ids), prop::collection::vec(any::<bool>(), n)).prop_map(move |(ids, long)| {
            ids.into_iter()
                .zip(long)
                .map(|(i, long)| Item {
                    sentinel: offset + 100 * i,
                    long,
                })
                .collect::<Vec<_>>()
        })
    })
}

/// Each side's run, with one item both wrote spliced into both at arbitrary
/// places when `shared` is set.
fn case_strategy(surface: Surface) -> impl Strategy<Value = Case> {
    (0usize..4)
        .prop_flat_map(move |k| {
            (
                Just(k),
                0..=k,
                run_strategy(40000),
                run_strategy(50000),
                any::<bool>(),
                any::<prop::sample::Index>(),
                any::<prop::sample::Index>(),
            )
        })
        .prop_map(move |(k, at, mut ours, mut theirs, shared, i, j)| {
            let base = (0..k as u32)
                .map(|n| Item {
                    sentinel: 10000 + 100 * n,
                    long: false,
                })
                .collect();
            // Only the changelog writes one entry once; a data file with one
            // key added twice beside distinct keys is refused (see the module
            // docs of `dataunion`).
            if shared && surface == Surface::Changelog {
                let both = Item {
                    sentinel: 60000,
                    long: false,
                };
                ours.insert(i.index(ours.len() + 1), both);
                theirs.insert(j.index(theirs.len() + 1), both);
            }
            Case {
                surface,
                base,
                at,
                ours,
                theirs,
            }
        })
}

fn any_case() -> impl Strategy<Value = Case> {
    prop_oneof![
        case_strategy(Surface::Changelog),
        case_strategy(Surface::Toml),
        case_strategy(Surface::Json),
        case_strategy(Surface::Yaml),
    ]
}

/// The sentinels of `text`, in order of first appearance.
fn sentinels(text: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i + 5 <= b.len() {
        let w = &b[i..i + 5];
        let bounded = (i == 0 || !b[i - 1].is_ascii_digit())
            && (i + 5 == b.len() || !b[i + 5].is_ascii_digit());
        if bounded && w.iter().all(u8::is_ascii_digit) {
            let n: u32 = std::str::from_utf8(w).unwrap().parse().unwrap();
            if n >= 10000 && !out.contains(&n) {
                out.push(n);
            }
            i += 5;
        } else {
            i += 1;
        }
    }
    out
}

/// How many times the item for `sentinel` is stated.
fn stated(surface: Surface, text: &str, sentinel: u32) -> usize {
    let needle = match surface {
        Surface::Changelog => format!("- `cmd{sentinel}`"),
        Surface::Toml => format!("dep{sentinel} ="),
        Surface::Json => format!("\"dep{sentinel}\":"),
        Surface::Yaml => format!("dep{sentinel}:"),
    };
    text.matches(&needle).count()
}

fn loads(surface: Surface, text: &str) -> bool {
    match surface {
        Surface::Changelog => true,
        Surface::Toml => text.parse::<toml::Table>().is_ok(),
        Surface::Json => serde_json::from_str::<serde_json::Value>(text).is_ok(),
        Surface::Yaml => serde_yaml::from_str::<serde_yaml::Value>(text).is_ok(),
    }
}

fn is_subsequence(needle: &[u32], hay: &[u32]) -> bool {
    let mut it = hay.iter();
    needle.iter().all(|n| it.any(|h| h == n))
}

// ===========================================================================
// U1 — Identity
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn u1_identity(case in any_case()) {
        let (b, o, t) = texts(&case);
        let path = case.surface.path();
        let left = entity_merge(&b, &b, &t, path);
        prop_assert!(left.is_clean());
        prop_assert_eq!(&left.content, &t);
        let right = entity_merge(&b, &o, &b, path);
        prop_assert!(right.is_clean());
        prop_assert_eq!(&right.content, &o);
    }
}

// ===========================================================================
// U2 — Idempotence
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn u2_idempotence(case in any_case()) {
        let (b, o, _) = texts(&case);
        let r = entity_merge(&b, &o, &o, case.surface.path());
        prop_assert!(r.is_clean());
        prop_assert_eq!(&r.content, &o);
    }
}

// ===========================================================================
// U3 + U4 — Commutativity and union
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    #[test]
    fn u3_u4_same_point_insertions_commute_to_the_union(case in any_case()) {
        let (b, o, t) = texts(&case);
        prop_assume!(o != t);
        let s = case.surface;
        let r = entity_merge(&b, &o, &t, s.path());
        prop_assert!(r.is_clean(), "same-point insertions of distinct items must settle:\n{}", r.content);
        let swapped = entity_merge(&b, &t, &o, s.path());
        prop_assert!(swapped.is_clean());
        prop_assert_eq!(&swapped.content, &r.content, "the answer must not depend on which side is ours");
        prop_assert!(loads(s, &r.content), "the merge must load:\n{}", r.content);

        let order = sentinels(&r.content);
        let mut every: Vec<u32> = case.base.iter().chain(&case.ours).chain(&case.theirs).map(|i| i.sentinel).collect();
        every.sort_unstable();
        every.dedup();
        for n in &every {
            prop_assert_eq!(stated(s, &r.content, *n), 1, "item {} must be stated once:\n{}", n, r.content);
        }
        let base: Vec<u32> = case.base.iter().map(|i| i.sentinel).collect();
        prop_assert!(is_subsequence(&base, &order), "base order");
        for run in [&case.ours, &case.theirs] {
            let own: Vec<u32> = run.iter().map(|i| i.sentinel).filter(|n| *n != 60000).collect();
            prop_assert!(is_subsequence(&own, &order), "a side's order:\n{}", r.content);
        }
    }
}

// ===========================================================================
// U5 — Fail closed
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// One key added by both sides with different values, beside the distinct
    /// keys: there is no union.
    #[test]
    fn u5_one_key_two_values_conflicts(
        case in prop_oneof![
            case_strategy(Surface::Toml),
            case_strategy(Surface::Json),
            case_strategy(Surface::Yaml),
        ]
    ) {
        let mut case = case;
        case.ours.push(Item { sentinel: 70000, long: false });
        case.theirs.push(Item { sentinel: 70000, long: false });
        let (b, o, t) = texts(&case);
        let t = t.replace("dep70000\": \"70000\"", "dep70000\": \"71111\"")
            .replace("dep70000 = \"70000\"", "dep70000 = \"71111\"")
            .replace("dep70000: \"70000\"", "dep70000: \"71111\"");
        prop_assert!(!entity_merge(&b, &o, &t, case.surface.path()).is_clean());
        prop_assert!(!entity_merge(&b, &t, &o, case.surface.path()).is_clean());
    }

    /// A heading inside one side's insertion: the other side's entries would
    /// land in a section their author did not put them in.
    #[test]
    fn u5_a_heading_inside_an_insertion_conflicts(case in case_strategy(Surface::Changelog)) {
        let (b, o, t) = texts(&case);
        let last = item_text(Surface::Changelog, *case.theirs.last().unwrap());
        let t = t.replacen(&last, &format!("{last}\n### Fixed\n- `fix`: a fix.\n"), 1);
        prop_assert!(!entity_merge(&b, &o, &t, case.surface.path()).is_clean());
        prop_assert!(!entity_merge(&b, &t, &o, case.surface.path()).is_clean());
    }

    /// An existing entry edited beside the insertion point.
    #[test]
    fn u5_an_edited_entry_beside_the_insertion_conflicts(case in case_strategy(Surface::Changelog)) {
        prop_assume!(case.at > 0);
        let (b, o, t) = texts(&case);
        let before = item_text(Surface::Changelog, case.base[case.at - 1]);
        let o = o.replacen(&before, &before.replace("new subcommand", "renamed subcommand"), 1);
        prop_assert!(!entity_merge(&b, &o, &t, case.surface.path()).is_clean());
        prop_assert!(!entity_merge(&b, &t, &o, case.surface.path()).is_clean());
    }
}

// ===========================================================================
// Controls: the observations can fail
// ===========================================================================

#[test]
fn control_stated_counts_a_duplicate() {
    let text = "- `cmd40000`: a.\n- `cmd40000`: a.\n";
    assert_eq!(stated(Surface::Changelog, text, 40000), 2);
    assert_eq!(sentinels("x 40000 y 50000 40000"), vec![40000, 50000]);
    assert!(!is_subsequence(&[2, 1], &[1, 2]));
}

#[test]
fn control_the_same_insertions_outside_a_changelog_conflict() {
    let case = Case {
        surface: Surface::Changelog,
        base: vec![],
        at: 0,
        ours: vec![Item {
            sentinel: 40000,
            long: false,
        }],
        theirs: vec![Item {
            sentinel: 50000,
            long: false,
        }],
    };
    let (b, o, t) = texts(&case);
    assert!(entity_merge(&b, &o, &t, "CHANGELOG.md").is_clean());
    assert!(!entity_merge(&b, &o, &t, "GUIDE.md").is_clean());
    // Undeclared, the changelog is a conflict too.
    assert!(!weave_core::entity_merge(&b, &o, &t, "CHANGELOG.md").is_clean());
}
