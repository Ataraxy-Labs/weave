//! `weave land --queue` — a landing queue kept in the shared bare repository.
//! Experimental: the protocol and its refs may change.
//!
//! Many agents, one origin, one branch: every candidate is landed by exactly
//! one process at a time, in submission order, so no push can race another.
//!
//! # Why refs and not a daemon
//!
//! The bare origin is the one thing every agent container already shares, and
//! `git push --atomic` with `--force-with-lease` on every ref is a
//! compare-and-swap over several refs at once. A daemon would need an
//! address every container can reach, a process someone keeps alive, and its
//! own crash story; the refs need none of that and survive any process dying.
//!
//! # Protocol
//!
//! Three append-only commit chains under `refs/weave/<branch>/` (each commit's
//! tree is empty, its first parent the previous head, its message JSON). Only
//! ref creations and fast-forwards are ever pushed, so an origin with
//! `receive.denyNonFastForwards` and `receive.denyDeletes` works unchanged
//! (its pre-receive hook must admit `refs/weave/*`).
//!
//! - `queue`: one commit per submission. Its second parent is the candidate
//!   (so the candidate travels with it and stays reachable); the ticket
//!   number is the chain depth (0, 1, 2, …). Enqueue = fast-forward with a
//!   lease on the old head; losing the race means retrying on the new head.
//! - `done`: one commit per processed ticket, in ticket order. Its message is
//!   the result (landed / refused, reason, files, conflict regions, report);
//!   a landed result's second parent is the commit published.
//! - `lock`: who lands. A commit whose `holder` is null means free. Acquire
//!   = append a commit naming yourself with a lease on the free head; the
//!   holder appends a renewal every `ttl/3`. A waiter that sees the same lock
//!   head unchanged for `ttl` by its own clock (skew-free) takes it over with
//!   a lease on that head.
//!
//! Landing ticket `n` (the next after `done`'s head) is ONE atomic push:
//! `main` (lease: the tip that was merged and checked), `done` (lease: its
//! head) and a lock renewal (lease: the holder's last lock head). A lander
//! whose lock was taken over has a stale lock lease, so its push fails as a
//! whole: main never moves except by the current holder, and only to a tree
//! that [`crate::onto::prepare`] gated and verified against that exact tip.
//!
//! A submitter enqueues, then waits for its ticket's result in `done`. While
//! waiting it takes the lock when it is free (or stale) and lands tickets in
//! order up to its own, so the queue needs no process of its own and keeps
//! moving while any submitter is alive. A ticket outlives its submitter.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use weave_core::host::Host;

use crate::gitscan;
use crate::onto::{self, Options, Prep};

type R<T> = Result<T, Box<dyn std::error::Error>>;

const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// What `--queue` was asked to do.
pub struct QueueOptions {
    pub onto: Options,
    /// A lock head unchanged this long belongs to a dead lander.
    pub lease_ttl: Duration,
    /// Give up waiting for the result after this long (the ticket stays).
    pub wait_timeout: Duration,
    /// How often a waiter looks at origin.
    pub poll: Duration,
}

/// The result of one ticket, as recorded in `done`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketResult {
    pub ticket: u64,
    pub landed: Option<String>,
    pub reason: String,
    pub files: Vec<String>,
    pub regions: Vec<String>,
    pub report: String,
    pub tip: String,
}

/// How a `--queue` run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueOutcome {
    Landed(String),
    Refused(String),
}

/// Plumbing over one clone and its remote.
struct Ctx {
    dir: PathBuf,
    remote: String,
    branch: String,
    me: String,
}

fn cmd_git(dir: &Path, args: &[&str]) -> Command {
    let mut c = Command::new("git");
    c.args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "weave-queue")
        .env("GIT_AUTHOR_EMAIL", "weave-queue@localhost")
        .env("GIT_COMMITTER_NAME", "weave-queue")
        .env("GIT_COMMITTER_EMAIL", "weave-queue@localhost");
    c
}

/// `(success, stdout, stderr)`.
fn run(dir: &Path, args: &[&str], limit: Duration) -> R<(bool, String, String)> {
    let (status, out, err) =
        gitscan::run_bounded(cmd_git(dir, args), &format!("git {}", args[0]), None, limit)?;
    Ok((
        status.success(),
        String::from_utf8_lossy(&out).to_string(),
        String::from_utf8_lossy(&err).to_string(),
    ))
}

fn git(dir: &Path, args: &[&str]) -> R<String> {
    match run(dir, args, Duration::from_secs(300))? {
        (true, out, _) => Ok(out.trim().to_string()),
        (false, _, err) => Err(format!("git {} failed: {}", args.join(" "), err.trim()).into()),
    }
}

/// A push lost to another writer (a lease or fast-forward refusal), as
/// opposed to origin being unreachable.
fn lost_race(err: &str) -> bool {
    // a hook or a receive.deny* setting refusing the ref is not a race:
    // retrying would spin forever
    if ["hook declined", "denied", "deny", "not allowed"]
        .iter()
        .any(|w| err.contains(w))
    {
        return false;
    }
    [
        "incorrect old value",
        "stale info",
        "rejected",
        "fetch first",
        "non-fast-forward",
        "atomic push failed",
        "failed to update ref",
        "cannot lock ref",
        "already exists",
    ]
    .iter()
    .any(|w| err.contains(w))
}

fn jitter(base: Duration) -> Duration {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0) as u64;
    let pid = std::process::id() as u64;
    let ms = base.as_millis() as u64;
    Duration::from_millis(ms + (n ^ pid.wrapping_mul(2654435761)) % (ms.max(1)))
}

impl Ctx {
    fn r(&self, name: &str) -> String {
        format!("refs/weave/{}/{name}", self.branch)
    }
    fn local(&self, name: &str) -> String {
        format!("refs/weave-seen/{}/{}/{name}", self.remote, self.branch)
    }

    /// Heads of `queue`, `done`, `lock` and the branch on origin.
    fn heads(&self) -> R<HashMap<String, String>> {
        let main = format!("refs/heads/{}", self.branch);
        let pat = format!("refs/weave/{}/*", self.branch);
        let (ok, out, err) = run(
            &self.dir,
            &["ls-remote", &self.remote, &pat, &main],
            Duration::from_secs(120),
        )?;
        if !ok {
            return Err(format!("git ls-remote {} failed: {}", self.remote, err.trim()).into());
        }
        let mut m = HashMap::new();
        for line in out.lines() {
            if let Some((sha, name)) = line.split_once('\t') {
                let key = if name == main {
                    "tip".to_string()
                } else {
                    name.rsplit('/').next().unwrap_or(name).to_string()
                };
                m.insert(key, sha.to_string());
            }
        }
        Ok(m)
    }

    /// Bring the queue refs (and the commits they reach) and the branch tip
    /// into this clone, under `refs/weave-seen/`.
    fn fetch(&self) -> R<()> {
        let specs = [
            format!(
                "+refs/weave/{}/*:refs/weave-seen/{}/{}/*",
                self.branch, self.remote, self.branch
            ),
            format!("+refs/heads/{}:{}", self.branch, self.local("tip")),
        ];
        let mut args = vec![
            "fetch",
            "-q",
            "--no-tags",
            "--no-write-fetch-head",
            &self.remote,
        ];
        args.extend(specs.iter().map(String::as_str));
        let (ok, _, err) = run(&self.dir, &args, Duration::from_secs(300))?;
        if !ok {
            return Err(format!("git fetch {} failed: {}", self.remote, err.trim()).into());
        }
        Ok(())
    }

    fn message(&self, sha: &str) -> R<Value> {
        let body = git(&self.dir, &["show", "-s", "--format=%B", sha])?;
        Ok(serde_json::from_str(body.trim()).unwrap_or(Value::Null))
    }

    /// A chain commit: empty tree, `parents` in order, `msg` as JSON.
    fn chain_commit(&self, parents: &[&str], msg: &Value) -> R<String> {
        let _ = git(&self.dir, &["mktree"]); // the empty tree must exist locally
        let text = msg.to_string();
        let mut args = vec!["commit-tree", EMPTY_TREE];
        for p in parents {
            args.push("-p");
            args.push(p);
        }
        args.push("-m");
        args.push(&text);
        git(&self.dir, &args)
    }

    /// The parentless first commit of a chain, so that every real entry has
    /// the chain as its first parent and its payload as its second.
    fn genesis(&self, chain: &str) -> R<String> {
        self.chain_commit(&[], &json!({"kind": "genesis", "chain": chain}))
    }

    /// One atomic push of `(new, ref, expected old)`; `Ok(false)` when another
    /// writer got there first.
    fn cas(&self, updates: &[(&str, String, Option<&str>)]) -> R<bool> {
        let mut args: Vec<String> = vec![
            "push".into(),
            "-q".into(),
            "--atomic".into(),
            self.remote.clone(),
        ];
        for (new, refname, old) in updates {
            args.push(format!(
                "--force-with-lease={refname}:{}",
                old.unwrap_or("")
            ));
            args.push(format!("{new}:{refname}"));
        }
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let (ok, _, err) = run(&self.dir, &argv, Duration::from_secs(300))?;
        if ok {
            return Ok(true);
        }
        if lost_race(&err) {
            return Ok(false);
        }
        Err(format!("git push to {} failed: {}", self.remote, err.trim()).into())
    }

    fn ticket_of(&self, sha: &str) -> R<u64> {
        self.message(sha)?
            .get("ticket")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{sha} is not a weave queue entry").into())
    }

    /// Append `candidate` to the queue; its ticket number.
    fn enqueue(&self, candidate: &str, meta: &Value) -> R<u64> {
        loop {
            let heads = self.heads()?;
            let old = heads.get("queue").cloned();
            let n = match &old {
                Some(q) => {
                    self.fetch()?;
                    self.ticket_of(q)? + 1
                }
                None => 0,
            };
            let mut msg = meta.clone();
            msg["kind"] = json!("ticket");
            msg["ticket"] = json!(n);
            msg["candidate"] = json!(candidate);
            msg["submitter"] = json!(self.me);
            let first = match &old {
                Some(q) => q.clone(),
                None => self.genesis("queue")?,
            };
            let entry = self.chain_commit(&[&first, candidate], &msg)?;
            if self.cas(&[(&entry, self.r("queue"), old.as_deref())])? {
                return Ok(n);
            }
            std::thread::sleep(jitter(Duration::from_millis(50)));
        }
    }

    /// The chain entry for ticket `n`, walking back from `head` (ticket
    /// `head_n`) along first parents.
    fn entry(&self, head: &str, head_n: u64, n: u64) -> R<String> {
        if n > head_n {
            return Err(format!("ticket {n} is not in the chain (head is {head_n})").into());
        }
        let back = head_n - n;
        let e = git(&self.dir, &["rev-parse", &format!("{head}~{back}")])?;
        let got = self.ticket_of(&e)?;
        if got != n {
            return Err(format!("chain entry {e} is ticket {got}, expected {n}").into());
        }
        Ok(e)
    }

    /// The recorded result of ticket `n`, if `done` has it.
    fn result(&self, n: u64, heads: &HashMap<String, String>) -> R<Option<TicketResult>> {
        let Some(done) = heads.get("done") else {
            return Ok(None);
        };
        if git(&self.dir, &["cat-file", "-e", done]).is_err() {
            self.fetch()?;
        }
        let dn = self.ticket_of(done)?;
        if dn < n {
            return Ok(None);
        }
        let e = self.entry(done, dn, n)?;
        let m = self.message(&e)?;
        let strs = |k: &str| -> Vec<String> {
            m.get(k)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        };
        let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Ok(Some(TicketResult {
            ticket: n,
            landed: m.get("landed").and_then(Value::as_str).map(String::from),
            reason: s("reason"),
            files: strs("files"),
            regions: strs("regions"),
            report: s("report"),
            tip: s("tip"),
        }))
    }
}

/// The lock while this process holds it: the head it last wrote, renewed by
/// a heartbeat thread. Every push that must only happen while holding the
/// lock goes through [`Held::with`], which serializes it with the heartbeat.
struct Held {
    head: Mutex<String>,
    lost: AtomicBool,
    stop: AtomicBool,
}

impl Held {
    fn is_lost(&self) -> bool {
        self.lost.load(Ordering::SeqCst)
    }
}

fn lock_msg(ctx: &Ctx, holder: Option<&str>) -> Value {
    json!({"kind": "lock", "holder": holder, "by": ctx.me,
           "at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                 .map(|d| d.as_millis() as u64).unwrap_or(0)})
}

fn lock_is_free(ctx: &Ctx, head: Option<&String>) -> R<bool> {
    match head {
        None => Ok(true),
        Some(h) => {
            if git(&ctx.dir, &["cat-file", "-e", h]).is_err() {
                ctx.fetch()?;
            }
            Ok(ctx.message(h)?.get("holder").is_none_or(Value::is_null))
        }
    }
}

/// Try to take the lock over `expected` (None: it does not exist yet).
fn acquire(ctx: &Ctx, expected: Option<&str>) -> R<Option<Arc<Held>>> {
    let parents: Vec<&str> = expected.into_iter().collect();
    let new = ctx.chain_commit(&parents, &lock_msg(ctx, Some(&ctx.me)))?;
    if !ctx.cas(&[(&new, ctx.r("lock"), expected)])? {
        return Ok(None);
    }
    Ok(Some(Arc::new(Held {
        head: Mutex::new(new),
        lost: AtomicBool::new(false),
        stop: AtomicBool::new(false),
    })))
}

/// Renew the lock every `ttl/3` until stopped or lost.
fn heartbeat(
    dir: PathBuf,
    remote: String,
    branch: String,
    me: String,
    held: Arc<Held>,
    ttl: Duration,
) {
    let ctx = Ctx {
        dir,
        remote,
        branch,
        me,
    };
    let every = ttl / 3;
    loop {
        let mut slept = Duration::ZERO;
        while slept < every {
            if held.stop.load(Ordering::SeqCst) || held.is_lost() {
                return;
            }
            let step = Duration::from_millis(100).min(every - slept);
            std::thread::sleep(step);
            slept += step;
        }
        let mut head = held.head.lock().unwrap();
        if held.stop.load(Ordering::SeqCst) || held.is_lost() {
            return;
        }
        let renewed = ctx
            .chain_commit(&[&head], &lock_msg(&ctx, Some(&ctx.me)))
            .and_then(|new| Ok((ctx.cas(&[(&new, ctx.r("lock"), Some(&head))])?, new)));
        match renewed {
            Ok((true, new)) => *head = new,
            Ok((false, _)) => held.lost.store(true, Ordering::SeqCst),
            Err(_) => {} // origin unreachable for a moment: try again next beat
        }
    }
}

fn release(ctx: &Ctx, held: &Held) {
    held.stop.store(true, Ordering::SeqCst);
    let head = held.head.lock().unwrap();
    if held.is_lost() {
        return;
    }
    if let Ok(new) = ctx.chain_commit(&[&head], &lock_msg(ctx, None)) {
        let _ = ctx.cas(&[(&new, ctx.r("lock"), Some(&head))]);
    }
}

/// The persistent detached worktree the lander merges and verifies in,
/// reset to `candidate`.
fn lander_tree(ctx: &Ctx, candidate: &str) -> R<PathBuf> {
    let common = PathBuf::from(git(
        &ctx.dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    let safe: String = ctx
        .branch
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let wt = common
        .join("weave-queue")
        .join(format!("{}-{safe}", ctx.remote));
    if !wt.join(".git").exists() {
        let _ = git(&ctx.dir, &["worktree", "prune"]);
        std::fs::create_dir_all(wt.parent().unwrap())?;
        git(
            &ctx.dir,
            &[
                "worktree",
                "add",
                "-q",
                "-f",
                "--detach",
                wt.to_str().unwrap(),
                candidate,
            ],
        )?;
        return Ok(wt);
    }
    // We hold the queue lock, so no other lander of this clone is in here —
    // but a killed lander's git child may still be running. A lock file is
    // removed only when no running process has it open.
    let wgd = PathBuf::from(git(&wt, &["rev-parse", "--absolute-git-dir"])?);
    let mut said = Vec::new();
    if let Some(why) = crate::mergestate::clear_stale_lock(&wgd, &mut said)? {
        return Err(format!("the lander's worktree {}: {why}", wt.display()).into());
    }
    if wgd.join("MERGE_HEAD").exists() {
        let _ = git(&wt, &["merge", "--abort"]);
    }
    git(&wt, &["reset", "-q", "--hard"])?;
    crate::mergestate::clear_record(&wgd);
    git(&wt, &["checkout", "-q", "-f", "--detach", candidate])?;
    git(&wt, &["clean", "-q", "-f", "-d"])?;
    Ok(wt)
}

/// Land tickets in order, from the one after `done`'s head, until `mine` is
/// done, the queue is empty, or the lock is lost. Each landing is one
/// atomic push fenced by the lock.
fn land_tickets(
    ctx: &Ctx,
    q: &QueueOptions,
    host: &Host,
    held: &Held,
    mine: u64,
    log: &mut dyn Write,
) -> R<()> {
    loop {
        if held.is_lost() {
            return Ok(());
        }
        ctx.fetch()?;
        let seen =
            |name: &str| git(&ctx.dir, &["rev-parse", "-q", "--verify", &ctx.local(name)]).ok();
        let (Some(queue), Some(tip)) = (seen("queue"), seen("tip")) else {
            return Ok(());
        };
        let done = git(
            &ctx.dir,
            &["rev-parse", "-q", "--verify", &ctx.local("done")],
        )
        .ok();
        let qn = ctx.ticket_of(&queue)?;
        let next = match &done {
            Some(d) => ctx.ticket_of(d)? + 1,
            None => 0,
        };
        if next > qn {
            return Ok(());
        }
        let entry = ctx.entry(&queue, qn, next)?;
        let meta = ctx.message(&entry)?;
        let candidate = git(&ctx.dir, &["rev-parse", &format!("{entry}^2")])?;
        let submitter = meta
            .get("submitter")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();

        let opts = Options {
            remote: ctx.remote.clone(),
            branch: ctx.branch.clone(),
            resolver: None,
            verify_cmd: meta
                .get("verify_cmd")
                .and_then(Value::as_str)
                .map(String::from),
            verify_timeout: Duration::from_secs(
                meta.get("verify_timeout")
                    .and_then(Value::as_u64)
                    .unwrap_or(1800),
            ),
            attempts: 1,
            certificate_dir: q.onto.certificate_dir.clone(),
        };
        let mut report: Vec<u8> = Vec::new();
        let prepared = lander_tree(ctx, &candidate).and_then(|wt| {
            let n = next.to_string();
            let env = [
                ("WEAVE_QUEUE_TICKET", n.as_str()),
                ("WEAVE_QUEUE_CANDIDATE", candidate.as_str()),
                ("WEAVE_LAND_REPO", ctx.dir.to_str().unwrap_or(".")),
            ];
            onto::prepare(&wt, &opts, host, &mut report, &tip, &env)
        });
        let report_text = String::from_utf8_lossy(&report).to_string();
        let tail: String = {
            let lines: Vec<&str> = report_text.lines().collect();
            lines[lines.len().saturating_sub(200)..].join("\n")
        };
        let mut result = json!({
            "kind": "result", "ticket": next, "candidate": candidate, "submitter": submitter,
            "lander": ctx.me, "tip": tip, "report": tail,
        });
        let landed = match prepared {
            Ok(Prep::Ready(head)) => {
                result["landed"] = json!(head);
                result["reason"] = json!("landed");
                Some(head)
            }
            Ok(Prep::Refused(r)) => {
                result["landed"] = Value::Null;
                result["reason"] = json!(r.reason);
                result["files"] = json!(r.files);
                result["regions"] = json!(r.regions);
                None
            }
            // A failure inside one ticket's checks must not wedge the queue:
            // record it as that ticket's refusal and move on.
            Err(e) => {
                result["landed"] = Value::Null;
                result["reason"] = json!("land failed");
                result["report"] = json!(format!("{tail}\nland: the checks could not run: {e}"));
                None
            }
        };
        let first = match &done {
            Some(d) => d.clone(),
            None => ctx.genesis("done")?,
        };
        let mut parents = vec![first.as_str()];
        if let Some(h) = &landed {
            parents.push(h.as_str());
        }
        let done_new = ctx.chain_commit(&parents, &result)?;

        let pushed = {
            let mut head = held.head.lock().unwrap();
            if held.is_lost() {
                return Ok(());
            }
            let lock_new = ctx.chain_commit(&[&head], &lock_msg(ctx, Some(&ctx.me)))?;
            let mut ups: Vec<(&str, String, Option<&str>)> = vec![
                (&lock_new, ctx.r("lock"), Some(head.as_str())),
                (&done_new, ctx.r("done"), done.as_deref()),
            ];
            let main_ref = format!("refs/heads/{}", ctx.branch);
            if let Some(h) = &landed {
                ups.push((h, main_ref, Some(&tip)));
            }
            let ok = ctx.cas(&ups)?;
            if ok {
                *head = lock_new;
            }
            ok
        };
        if !pushed {
            // Either the lock was taken over (stop: the new holder redoes this
            // ticket) or the branch moved outside the queue (redo it).
            let h = ctx.heads()?;
            if h.get("lock") != Some(&held.head.lock().unwrap().clone()) {
                held.lost.store(true, Ordering::SeqCst);
                return Ok(());
            }
            continue;
        }
        let _ = writeln!(
            log,
            "queue: ticket {next} ({submitter}): {}",
            match &landed {
                Some(h) => format!("landed {}", &h[..h.len().min(12)]),
                None => format!("refused ({})", result["reason"].as_str().unwrap_or("?")),
            }
        );
        if next >= mine {
            return Ok(());
        }
    }
}

/// Submit HEAD to the queue and wait until it is landed or refused, landing
/// other tickets ahead of it while holding the lock.
pub fn submit(dir: &Path, q: &QueueOptions, host: &Host, out: &mut dyn Write) -> R<QueueOutcome> {
    let top = PathBuf::from(gitscan::git(dir, &["rev-parse", "--show-toplevel"])?.trim());
    let gd = PathBuf::from(gitscan::git(&top, &["rev-parse", "--absolute-git-dir"])?.trim());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let host_name = std::env::var("HOSTNAME").unwrap_or_else(|_| "host".into());
    let ctx = Ctx {
        dir: top.clone(),
        remote: q.onto.remote.clone(),
        branch: q.onto.branch.clone(),
        me: format!("{host_name}-{}-{nanos}", std::process::id()),
    };
    let target = format!("{}/{}", ctx.remote, ctx.branch);

    if gd.join("rebase-merge").exists() || gd.join("rebase-apply").exists() {
        writeln!(
            out,
            "land: a rebase is in progress; finish it first. land merges; it does not rebase."
        )?;
        return Ok(QueueOutcome::Refused("rebase in progress".into()));
    }
    // A merge in progress is the person's resolution of an earlier refusal:
    // judge it here, commit it, and submit the result.
    if gd.join("MERGE_HEAD").exists() {
        writeln!(
            out,
            "land: checking your resolution of the merge in progress …"
        )?;
        match onto::check_resolution(&top, &q.onto, host, out)? {
            true => {}
            false => return Ok(QueueOutcome::Refused("resolution refused".into())),
        }
    }
    if !git(&top, &["status", "--porcelain", "--untracked-files=no"])?.is_empty() {
        writeln!(
            out,
            "land: you have uncommitted changes to tracked files; commit them first."
        )?;
        return Ok(QueueOutcome::Refused("dirty".into()));
    }

    for attempt in 1..=q.onto.attempts.max(1) {
        let candidate = git(&top, &["rev-parse", "HEAD"])?;
        let (ok, _, err) = run(
            &top,
            &["fetch", "-q", &ctx.remote, &ctx.branch],
            Duration::from_secs(300),
        )?;
        if !ok {
            return Err(format!("git fetch {target} failed: {}", err.trim()).into());
        }
        if git(&top, &["rev-list", "FETCH_HEAD..HEAD"])?.is_empty() {
            writeln!(out, "land: nothing to land: your branch has no commits that are not already on {target}.")?;
            return Ok(QueueOutcome::Refused("nothing to land".into()));
        }
        let meta = json!({
            "verify_cmd": q.onto.verify_cmd,
            "verify_timeout": q.onto.verify_timeout.as_secs(),
        });
        let ticket = ctx.enqueue(&candidate, &meta)?;
        writeln!(
            out,
            "land: queued {} for {target} as ticket {ticket}; waiting for it to land …",
            &candidate[..12]
        )?;
        let res = wait(&ctx, q, host, ticket, out)?;
        let Some(res) = res else {
            writeln!(
                out,
                "land: ticket {ticket} is still queued after {}s; it will still be landed or \
                 refused. Run `weave land --queue` again later to see the result.",
                q.wait_timeout.as_secs()
            )?;
            return Ok(QueueOutcome::Refused("still queued".into()));
        };
        if let Some(h) = &res.landed {
            if !res.report.is_empty() {
                writeln!(out, "{}", res.report)?;
            }
            writeln!(
                out,
                "land: LANDED {} on {target} (ticket {ticket}).",
                &h[..h.len().min(12)]
            )?;
            return Ok(QueueOutcome::Landed(h.clone()));
        }
        writeln!(out, "{}", res.report)?;
        writeln!(
            out,
            "land: ticket {ticket} REFUSED ({}); nothing was published.",
            res.reason
        )?;
        if !res.files.is_empty() {
            writeln!(out, "land: refused files:\n{}", indent(&res.files))?;
        }
        if !res.regions.is_empty() {
            writeln!(
                out,
                "land: conflict regions (merged onto {}):\n{}",
                &res.tip[..res.tip.len().min(12)],
                indent(&res.regions)
            )?;
        }
        if res.reason != "merge refused" {
            return Ok(QueueOutcome::Refused(res.reason));
        }
        // Reproduce the merge here, against the branch as it is now, so the
        // conflicts are in the person's own tree to resolve in place.
        let (ok, _, err) = run(
            &top,
            &["fetch", "-q", &ctx.remote, &ctx.branch],
            Duration::from_secs(300),
        )?;
        if !ok {
            return Err(format!("git fetch {target} failed: {}", err.trim()).into());
        }
        let now = git(&top, &["rev-parse", "FETCH_HEAD"])?;
        let mut local_opts = clone_opts(&q.onto);
        local_opts.verify_cmd = None; // the lander verifies; here only the merge
        match onto::prepare(&top, &local_opts, host, out, &now, &[])? {
            Prep::Refused(r) => {
                if r.reason == "merge refused" {
                    writeln!(
                        out,
                        "\nland: the merge with {target} is left in progress in your tree. Resolve \
                         the files above in place (keep both sides' changes; remove every conflict \
                         marker), then run `weave land --queue` again — it checks your resolution, \
                         commits the merge and queues it."
                    )?;
                }
                return Ok(QueueOutcome::Refused(r.reason.into()));
            }
            // The branch has moved on and the merge now passes: queue again.
            Prep::Ready(_) => {
                writeln!(out, "land: the merge with the newer {target} passes; queueing again (attempt {attempt}).")?;
            }
        }
    }
    writeln!(
        out,
        "land: still refused after {} attempts.",
        q.onto.attempts.max(1)
    )?;
    Ok(QueueOutcome::Refused("attempts exhausted".into()))
}

fn clone_opts(o: &Options) -> Options {
    Options {
        remote: o.remote.clone(),
        branch: o.branch.clone(),
        resolver: o.resolver.clone(),
        verify_cmd: o.verify_cmd.clone(),
        verify_timeout: o.verify_timeout,
        attempts: o.attempts,
        certificate_dir: o.certificate_dir.clone(),
    }
}

fn indent(lines: &[String]) -> String {
    lines
        .iter()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wait for ticket `n`'s result, landing tickets whenever the lock is free or
/// its holder is dead. `None` on timeout.
fn wait(
    ctx: &Ctx,
    q: &QueueOptions,
    host: &Host,
    n: u64,
    out: &mut dyn Write,
) -> R<Option<TicketResult>> {
    let start = Instant::now();
    let mut seen: Option<(String, Instant)> = None;
    loop {
        let heads = ctx.heads()?;
        if let Some(r) = ctx.result(n, &heads)? {
            return Ok(Some(r));
        }
        if start.elapsed() > q.wait_timeout {
            return Ok(None);
        }
        let lock = heads.get("lock").cloned();
        let take = if lock_is_free(ctx, lock.as_ref())? {
            true
        } else {
            let l = lock.clone().unwrap_or_default();
            match &seen {
                Some((v, t)) if *v == l => t.elapsed() >= q.lease_ttl,
                _ => {
                    seen = Some((l, Instant::now()));
                    false
                }
            }
        };
        if take {
            let stale = !lock_is_free(ctx, lock.as_ref())?;
            if let Some(held) = acquire(ctx, lock.as_deref())? {
                if stale {
                    writeln!(
                        out,
                        "land: the lander holding the queue went quiet for {}s; taking over.",
                        q.lease_ttl.as_secs()
                    )?;
                }
                let hb = {
                    let (d, r, b, m, h, t) = (
                        ctx.dir.clone(),
                        ctx.remote.clone(),
                        ctx.branch.clone(),
                        ctx.me.clone(),
                        held.clone(),
                        q.lease_ttl,
                    );
                    std::thread::spawn(move || heartbeat(d, r, b, m, h, t))
                };
                let res = land_tickets(ctx, q, host, &held, n, out);
                release(ctx, &held);
                let _ = hb.join();
                res?;
                seen = None;
                continue;
            }
        }
        std::thread::sleep(jitter(q.poll));
    }
}
