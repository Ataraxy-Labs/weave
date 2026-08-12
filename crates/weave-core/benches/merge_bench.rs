//! Criterion benches for the merge core.
//!
//! Three shapes, chosen because they exercise three different costs:
//!
//! * `small_py` — a ~100-line Python module with a real divergence. Dominated
//!   by fixed per-merge cost (parse setup, region split, render), so it is the
//!   bench that moves when per-merge overhead moves.
//! * `stress_500fn` — 500 top-level functions, both sides editing a scattered
//!   subset. Dominated by the match phase and by per-entity allocation, so it
//!   is the bench that moves when the entity pipeline's asymptotics move.
//! * `member_heavy_class` — one class with 300 methods, both sides editing
//!   different methods. Exercises the container / inner-merge path, which is a
//!   different code path from the top-level one above.
//!
//! Every bench merges through `entity_merge`, i.e. the same entry point the
//! driver uses — the numbers therefore include whatever the hot path does,
//! including any process or filesystem work it reaches.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use weave_core::entity_merge;

/// A small real-world-shaped Python module, plus a divergence on each side.
fn small_py() -> (String, String, String) {
    let base = "\
import os
import sys


def load(path):
    with open(path) as f:
        return f.read()


def save(path, data):
    with open(path, 'w') as f:
        f.write(data)


class Store:
    def __init__(self, root):
        self.root = root

    def get(self, key):
        return load(os.path.join(self.root, key))

    def put(self, key, value):
        save(os.path.join(self.root, key), value)


def main(argv):
    store = Store(argv[1])
    print(store.get(argv[2]))
";
    // ours: adds a helper and touches `load`
    let ours = base
        .replace(
            "def load(path):\n    with open(path) as f:\n        return f.read()",
            "def load(path):\n    with open(path, encoding='utf-8') as f:\n        return f.read()",
        )
        .replace(
            "def main(argv):",
            "def exists(path):\n    return os.path.isfile(path)\n\n\ndef main(argv):",
        );
    // theirs: touches `save` and a method, disjoint from ours
    let theirs = base
        .replace(
            "def save(path, data):\n    with open(path, 'w') as f:\n        f.write(data)",
            "def save(path, data):\n    with open(path, 'w', encoding='utf-8') as f:\n        f.write(data)",
        )
        .replace(
            "    def put(self, key, value):\n        save(os.path.join(self.root, key), value)",
            "    def put(self, key, value):\n        save(os.path.join(self.root, key), str(value))",
        );
    (base.to_string(), ours, theirs)
}

/// 500 top-level Python functions; each side edits a disjoint scattered subset.
fn stress_500fn() -> (String, String, String) {
    let n = 500usize;
    let mut base = String::with_capacity(n * 64);
    for i in 0..n {
        base.push_str(&format!(
            "def fn_{i}(a, b):\n    total = a + b\n    return total * {i}\n\n\n"
        ));
    }
    let mut ours = String::with_capacity(base.len());
    let mut theirs = String::with_capacity(base.len());
    for i in 0..n {
        let ours_body = if i % 7 == 0 {
            format!("def fn_{i}(a, b):\n    total = a + b + 1\n    return total * {i}\n\n\n")
        } else {
            format!("def fn_{i}(a, b):\n    total = a + b\n    return total * {i}\n\n\n")
        };
        let theirs_body = if i % 11 == 0 {
            format!("def fn_{i}(a, b):\n    total = a + b\n    return total * {i} + 2\n\n\n")
        } else {
            format!("def fn_{i}(a, b):\n    total = a + b\n    return total * {i}\n\n\n")
        };
        ours.push_str(&ours_body);
        theirs.push_str(&theirs_body);
    }
    (base, ours, theirs)
}

/// One class with 300 methods; each side edits a disjoint scattered subset.
fn member_heavy_class() -> (String, String, String) {
    let n = 300usize;
    let header = "class Big:\n    def __init__(self):\n        self.state = {}\n\n";
    let mut base = String::from(header);
    for i in 0..n {
        base.push_str(&format!(
            "    def m_{i}(self, x):\n        self.state[{i}] = x\n        return x + {i}\n\n"
        ));
    }
    let mut ours = String::from(header);
    let mut theirs = String::from(header);
    for i in 0..n {
        if i % 5 == 0 {
            ours.push_str(&format!(
                "    def m_{i}(self, x):\n        self.state[{i}] = x * 2\n        return x + {i}\n\n"
            ));
        } else {
            ours.push_str(&format!(
                "    def m_{i}(self, x):\n        self.state[{i}] = x\n        return x + {i}\n\n"
            ));
        }
        if i % 9 == 0 {
            theirs.push_str(&format!(
                "    def m_{i}(self, x):\n        self.state[{i}] = x\n        return x + {i} + 1\n\n"
            ));
        } else {
            theirs.push_str(&format!(
                "    def m_{i}(self, x):\n        self.state[{i}] = x\n        return x + {i}\n\n"
            ));
        }
    }
    (base, ours, theirs)
}

fn bench_merges(c: &mut Criterion) {
    let cases: Vec<(&str, (String, String, String))> = vec![
        ("small_py", small_py()),
        ("stress_500fn", stress_500fn()),
        ("member_heavy_class", member_heavy_class()),
    ];

    let mut group = c.benchmark_group("entity_merge");
    for (name, (base, ours, theirs)) in &cases {
        group.bench_with_input(BenchmarkId::from_parameter(name), name, |b, _| {
            b.iter(|| {
                black_box(entity_merge(
                    black_box(base),
                    black_box(ours),
                    black_box(theirs),
                    "bench.py",
                ))
            })
        });
    }
    group.finish();
}

/// The line-level fallback, reached for files the entity model does not
/// describe. Benched separately because it is the path that historically
/// shelled out to `git merge-file`.
fn bench_fallback(c: &mut Criterion) {
    // A `.lock`-shaped file: `skip_expansion` is true for it, so this is the
    // straight-to-line-level path with no entity pipeline in front of it.
    let mut base = String::new();
    for i in 0..400 {
        base.push_str(&format!(
            "[[package]]\nname = \"pkg{i}\"\nversion = \"1.0.{i}\"\n\n"
        ));
    }
    let ours = base.replace("version = \"1.0.7\"", "version = \"1.1.7\"");
    let theirs = base.replace("version = \"1.0.11\"", "version = \"1.1.11\"");

    c.bench_function("line_level_fallback/uv.lock-shaped", |b| {
        b.iter(|| {
            black_box(entity_merge(
                black_box(&base),
                black_box(&ours),
                black_box(&theirs),
                "uv.lock",
            ))
        })
    });
}

criterion_group!(benches, bench_merges, bench_fallback);
criterion_main!(benches);
