//! Batch driver: reads JSONL `{id, path, base, ours, theirs, merged}` (file
//! paths; base/ours/theirs may be null = absent) on stdin, writes one JSON
//! verdict per line on stdout. `--dump DIR` writes each both-changed region's
//! o/a/b (and the diff3 result, when clean) for an external cross-check;
//! `--emit-construct DIR` writes the selection-built merge (nest + subsume_ins).

use serde_json::{json, Value};
use std::io::{BufRead, Write};
use weave_certify::*;

fn read(v: &Value) -> Result<Option<String>, String> {
    match v.as_str() {
        None => Ok(None),
        Some(p) => {
            let bytes = std::fs::read(p).map_err(|e| format!("read {p}: {e}"))?;
            String::from_utf8(bytes)
                .map(|s| Some(normalize(&s)))
                .map_err(|_| "not utf-8".into())
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dump = args
        .iter()
        .position(|a| a == "--dump")
        .map(|i| args[i + 1].clone());
    let emit = args
        .iter()
        .position(|a| a == "--emit-construct")
        .map(|i| args[i + 1].clone());
    let reg = sem_core::parser::plugins::create_default_registry();
    let out = std::io::stdout();
    let mut out = out.lock();
    for line in std::io::stdin().lock().lines() {
        let job: Value = serde_json::from_str(&line.unwrap()).expect("bad job line");
        let id = job["id"].clone();
        let path = job["path"].as_str().unwrap_or("").to_string();
        let texts: Result<Vec<Option<String>>, String> = ["base", "ours", "theirs", "merged"]
            .iter()
            .map(|f| read(&job[*f]))
            .collect();
        let t = match texts {
            Ok(t) if t[3].is_some() => t,
            Ok(_) => {
                writeln!(out, "{}", json!({"id": id, "error": "no merged"})).unwrap();
                continue;
            }
            Err(e) => {
                writeln!(out, "{}", json!({"id": id, "error": e})).unwrap();
                continue;
            }
        };
        let v: Vec<Option<Version>> = t
            .iter()
            .map(|s| s.as_ref().map(|s| decompose(&reg, &path, s)))
            .collect();
        let (o, a, b, m) = (
            v[0].as_ref(),
            v[1].as_ref(),
            v[2].as_ref(),
            v[3].as_ref().unwrap(),
        );
        let r = check(&path, o, a, b, m);
        let merged = t[3].as_ref().unwrap();
        let cons = |allow: &[&str]| match construct(o, a, b, allow) {
            None => "none",
            Some(c) if &c == merged => "eq",
            Some(_) => "ne",
        };
        if let Some(dir) = &emit {
            if let Some(c) = construct(o, a, b, &["nest", "subsume_ins"]) {
                let safe: String = id
                    .as_str()
                    .unwrap_or("x")
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                std::fs::write(format!("{dir}/{safe}.construct"), c).unwrap();
            }
        }
        if let Some(dir) = &dump {
            let get = |x: Option<&Version>, k: &str| x.and_then(|x| x.text.get(k)).cloned();
            for (n, (k, v)) in r.both.iter().enumerate() {
                if v[0].1 == "decline" {
                    continue;
                }
                let safe: String = id
                    .as_str()
                    .unwrap_or("x")
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                let stem = format!("{dir}/{safe}.{n}");
                let (ot, at, bt) = (
                    get(o, k).unwrap_or_default(),
                    get(a, k).unwrap(),
                    get(b, k).unwrap(),
                );
                std::fs::write(format!("{stem}.o"), &ot).unwrap();
                std::fs::write(format!("{stem}.a"), &at).unwrap();
                std::fs::write(format!("{stem}.b"), &bt).unwrap();
                if let Some(d) = diff3(&ot, &at, &bt) {
                    std::fs::write(format!("{stem}.d3"), d).unwrap();
                }
            }
        }
        let both: Vec<Value> = r
            .both
            .iter()
            .map(|(k, v)| {
                let mut o = serde_json::Map::new();
                o.insert("key".into(), json!(k));
                for (n, s) in v {
                    o.insert((*n).into(), json!(s));
                }
                Value::Object(o)
            })
            .collect();
        writeln!(out, "{}", json!({
            "id": id, "regions": m.keys.len(), "n_hard": r.hard.len(),
            "hard": r.hard.iter().take(8).collect::<Vec<_>>(), "both": both,
            "construct": {"R0": cons(&[]), "diff3": cons(&["diff3"]), "nest": cons(&["nest"]), "nest_diff3": cons(&["nest", "diff3"]), "chosen": cons(&["nest", "subsume_ins"])},
        })).unwrap();
    }
}
