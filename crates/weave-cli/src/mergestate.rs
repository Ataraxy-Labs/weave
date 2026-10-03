//! What state a merge is really in, for `land --onto` / `--queue`.
//!
//! Two facts land must never guess at:
//!
//! * **`index.lock`.** git takes it to write the index and removes it when
//!   done. A git process that is killed leaves it behind, and every later
//!   index write (a merge, a commit) fails on it. A lock is stale only when
//!   no running process has the file open — checked, not assumed, on Linux
//!   through `/proc/*/fd` and elsewhere through `lsof`; when neither can
//!   tell, the lock counts as held.
//! * **Whose merge is in progress.** `MERGE_HEAD` alone does not say that a
//!   merge was applied: git 2.39 writes `MERGE_HEAD` even when it could not
//!   write the merge into the index (`index.lock` held), leaving the index
//!   and the working tree at HEAD. Taken as "the person's resolution", that
//!   is a merge whose tree is ours — every change only theirs made, gone.
//!   So land records each merge it starts in `<git-dir>/weave-land-merge`
//!   (`in-flight` until it is committed; `refused` once land has handed it
//!   to the person to resolve). A merge in progress that land started and
//!   never finished is aborted and merged again, never judged as a
//!   resolution.

use std::path::{Path, PathBuf};
use std::time::Duration;

type R<T> = Result<T, Box<dyn std::error::Error>>;

/// Who, if anyone, holds `index.lock`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lock {
    Absent,
    /// A running process has it open (pids, where known).
    Held(Vec<u32>),
    /// The file exists and no running process has it open.
    Stale,
    /// The file exists and whether it is held could not be told.
    Unknown(String),
}

/// The `index.lock` of the git dir `gd`.
pub fn index_lock(gd: &Path) -> PathBuf {
    gd.join("index.lock")
}

/// Look at `index.lock` in `gd`: three looks 150 ms apart, all of them with
/// no holder and the same file, before it counts as stale.
pub fn lock_state(gd: &Path) -> Lock {
    let path = index_lock(gd);
    let id = |p: &Path| {
        std::fs::symlink_metadata(p)
            .ok()
            .map(|m| (m.len(), m.modified().ok()))
    };
    let Some(first) = id(&path) else {
        return Lock::Absent;
    };
    for look in 0..3 {
        if look > 0 {
            std::thread::sleep(Duration::from_millis(150));
            match id(&path) {
                None => return Lock::Absent,
                Some(now) if now != first => return Lock::Held(Vec::new()),
                Some(_) => {}
            }
        }
        match holders(&path) {
            Ok(pids) if pids.is_empty() => {}
            Ok(pids) => return Lock::Held(pids),
            Err(why) => return Lock::Unknown(why),
        }
    }
    Lock::Stale
}

/// The pids that have `path` open.
fn holders(path: &Path) -> Result<Vec<u32>, String> {
    let want = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if Path::new("/proc/self/fd").is_dir() {
        let mut pids = Vec::new();
        let procs = std::fs::read_dir("/proc").map_err(|e| format!("/proc: {e}"))?;
        for p in procs.flatten() {
            let Some(pid) = p.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            // A process whose fds cannot be read is not ours to judge; one
            // that has exited (a zombie) has none.
            let Ok(fds) = std::fs::read_dir(p.path().join("fd")) else {
                continue;
            };
            for fd in fds.flatten() {
                if let Ok(target) = std::fs::read_link(fd.path()) {
                    if target == want || target == path {
                        pids.push(pid);
                        break;
                    }
                }
            }
        }
        return Ok(pids);
    }
    let out = std::process::Command::new("lsof")
        .arg("-t")
        .arg("--")
        .arg(&want)
        .output()
        .map_err(|e| format!("could not run lsof to see who holds it: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let pids: Vec<u32> = text
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    match (out.status.code(), pids.is_empty()) {
        (_, false) => Ok(pids),
        (Some(1), true) => Ok(Vec::new()),
        (Some(0), true) => Ok(Vec::new()),
        _ => Err(format!(
            "lsof failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// Make sure no stale `index.lock` stands in the way. `Ok(None)`: there is
/// none now (a stale one was removed, and that said in `out`). `Ok(Some(why))`:
/// a lock is held, or its holder could not be told — the caller refuses.
pub fn clear_stale_lock(gd: &Path, out: &mut dyn std::io::Write) -> R<Option<String>> {
    let path = index_lock(gd);
    match lock_state(gd) {
        Lock::Absent => Ok(None),
        Lock::Stale => {
            std::fs::remove_file(&path).or_else(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(e),
            })?;
            writeln!(
                out,
                "land: removed a stale {} (no running process has it open; a git process that \
                 was killed left it behind).",
                path.display()
            )?;
            Ok(None)
        }
        Lock::Held(pids) => Ok(Some(format!(
            "{} is held by a running process{}; another git command is using this repository",
            path.display(),
            if pids.is_empty() {
                String::new()
            } else {
                format!(
                    " (pid {})",
                    pids.iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        ))),
        Lock::Unknown(why) => Ok(Some(format!(
            "{} exists and weave cannot tell whether a running process holds it ({why})",
            path.display()
        ))),
    }
}

/// land's record of the merge it started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// `in-flight` (started, not yet committed or handed over) or `refused`
    /// (handed to the person to resolve in place).
    pub state: String,
    /// HEAD before the merge.
    pub pre: String,
    /// The commit merged in.
    pub tip: String,
}

fn record_path(gd: &Path) -> PathBuf {
    gd.join("weave-land-merge")
}

pub fn read_record(gd: &Path) -> Option<Record> {
    let text = std::fs::read_to_string(record_path(gd)).ok()?;
    let f: Vec<&str> = text.split_whitespace().collect();
    match f.as_slice() {
        [state, pre, tip] => Some(Record {
            state: state.to_string(),
            pre: pre.to_string(),
            tip: tip.to_string(),
        }),
        _ => None,
    }
}

pub fn write_record(gd: &Path, state: &str, pre: &str, tip: &str) -> R<()> {
    let tmp = gd.join("weave-land-merge.tmp");
    std::fs::write(&tmp, format!("{state} {pre} {tip}\n"))?;
    std::fs::rename(&tmp, record_path(gd))?;
    Ok(())
}

pub fn clear_record(gd: &Path) {
    let _ = std::fs::remove_file(record_path(gd));
}

/// Whether git's output from `git merge` says it could not write the merge
/// (the index, the working tree): such a merge was not applied, whatever
/// `MERGE_HEAD` says.
pub fn merge_not_written(stdout: &str, stderr: &str) -> bool {
    [stdout, stderr].iter().any(|t| {
        t.contains("index.lock")
            || t.contains("Unable to write index")
            || t.contains("unable to write")
            || t.lines().any(|l| l.starts_with("fatal:"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_nobody_has_open_is_stale_and_one_held_open_is_not() {
        let gd = std::env::temp_dir().join(format!("weave-lock-{}", std::process::id()));
        std::fs::create_dir_all(&gd).unwrap();
        assert_eq!(lock_state(&gd), Lock::Absent);
        std::fs::write(index_lock(&gd), "").unwrap();
        let held = std::fs::File::open(index_lock(&gd)).unwrap();
        match lock_state(&gd) {
            Lock::Held(pids) => assert!(pids.contains(&std::process::id()), "{pids:?}"),
            Lock::Unknown(_) => {} // no lsof on this machine
            other => panic!("an open lock is not stale: {other:?}"),
        }
        drop(held);
        match lock_state(&gd) {
            Lock::Stale | Lock::Unknown(_) => {}
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&gd);
    }

    #[test]
    fn git_saying_it_could_not_write_the_index_is_not_a_merge() {
        assert!(merge_not_written(
            "Automatic merge failed; fix conflicts and then commit the result.\n",
            "error: Unable to write index.\n"
        ));
        assert!(!merge_not_written(
            "Auto-merging a.go\nCONFLICT (content): Merge conflict in a.go\n",
            ""
        ));
    }
}
