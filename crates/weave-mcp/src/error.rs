//! What the server can fail at, and how each failure reaches the client.
//!
//! Every one of the 22 tools used to answer failure with
//! `ErrorData::internal_error(msg.to_string())`. That signature took
//! `impl ToString`, so the one properly typed error in the workspace
//! (`weave_crdt::WeaveError`) arrived at an agent as prose, indistinguishable
//! from a missing repository or a typo in an entity name — and `internal_error`
//! told the agent "this is our fault, do not retry", which for a mistyped
//! entity name is simply untrue.
//!
//! [`ToolError`] is one type in the error position with the union of causes in
//! its `reason` field: a caller that only needs "did this fail" reads the
//! wrapper, and the mapping from cause to JSON-RPC code is written once, here,
//! rather than guessed at 60 call sites.

use std::path::Path;

/// A tool call that did not produce an answer.
#[derive(Debug, thiserror::Error)]
#[error("{reason}")]
pub struct ToolError {
    /// What actually went wrong.
    pub reason: Reason,
}

impl ToolError {
    /// Wrap a cause.
    pub fn new(reason: Reason) -> Self {
        ToolError { reason }
    }

    /// Whether the fault is in what the caller asked for, rather than in the
    /// server or the machine it runs on.
    ///
    /// This is the whole reason the type exists: a caller's own mistake is
    /// worth restating and retrying with different arguments, and everything
    /// else is not.
    pub fn is_callers_fault(&self) -> bool {
        match &self.reason {
            Reason::NoRepository | Reason::EntityNotFound { .. } | Reason::Unreadable { .. } => {
                true
            }
            // A git query in this server always runs over refs and paths the
            // caller named, so git declining to resolve one is a mistake the
            // caller can fix by asking differently (a real ref, a fetched
            // branch) — invalid_params, not "our fault, stop". The one
            // exception is git itself being unrunnable, which no re-phrasing
            // helps. (A damaged object is also a `Refused`; reporting it as
            // caller-fixable is imperfect, but the alternative is matching
            // git's stderr prose, which this module rejects — and the caller
            // still receives git's own message either way.)
            Reason::Git(e) => !matches!(**e, weave_core::git::GitError::NotRunnable { .. }),
            Reason::State { .. } => false,
        }
    }
}

/// Why a tool call failed.
#[derive(Debug, thiserror::Error)]
pub enum Reason {
    /// No repository could be found by any of the three strategies.
    #[error(
        "cannot find a git repository. Either:\n\
         - pass an absolute file path (e.g. /Users/you/project/src/lib.ts)\n\
         - set WEAVE_REPO to the repo root\n\
         - run weave-mcp from inside a git repo"
    )]
    NoRepository,

    /// A `git` query failed for a reason git itself named.
    ///
    /// Boxed, like `State` below: these two carry another crate's error, and
    /// an unboxed union of them makes every `Result` in the server as wide as
    /// its rarest failure.
    #[error("git could not answer: {0}")]
    Git(Box<weave_core::git::GitError>),

    /// The shared edit state could not be opened or written.
    #[error("the shared edit state at {path} could not be opened: {source}")]
    State {
        path: String,
        #[source]
        source: Box<weave_crdt::WeaveError>,
    },

    /// A file the caller named could not be read.
    #[error("failed to read {path}: {source}")]
    Unreadable {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// The named entity is not in the named file.
    #[error("entity '{entity}' not found in '{file}'")]
    EntityNotFound { entity: String, file: String },
}

impl Reason {
    /// The state-open failure, with the path it was opening.
    pub fn state(path: &Path, source: weave_crdt::WeaveError) -> Self {
        Reason::State {
            path: path.display().to_string(),
            source: Box::new(source),
        }
    }
}

impl From<Reason> for ToolError {
    fn from(reason: Reason) -> Self {
        ToolError::new(reason)
    }
}

impl From<weave_core::git::GitError> for ToolError {
    fn from(e: weave_core::git::GitError) -> Self {
        ToolError::new(Reason::Git(Box::new(e)))
    }
}

/// The one place a cause becomes a JSON-RPC code.
///
/// `invalid_params` for the caller's own mistakes, `internal_error` for ours.
/// Getting this wrong in the safe direction (calling everything internal) is
/// what the old `internal_err` did, and it is why an agent could not tell
/// "retry with a different entity name" from "stop asking".
impl From<ToolError> for rmcp::ErrorData {
    fn from(e: ToolError) -> Self {
        let message = e.to_string();
        if e.is_callers_fault() {
            rmcp::ErrorData::invalid_params(message, None)
        } else {
            rmcp::ErrorData::internal_error(message, None)
        }
    }
}
