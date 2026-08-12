//! One error in every signature here, and the union of causes inside it.
//!
//! Eight public calls in this crate used to answer failure with `String`, so a
//! 401, a 404, a socket timeout and a body that did not match the expected
//! shape were the same type: no caller could retry one and give up on another
//! without matching on prose. Naming all of them in a single enum instead
//! would put a growing union in eight signatures and force every caller to
//! read past the six causes it does not care about.
//!
//! So the union lives one level down. [`GitHubError`] is the only thing in the
//! error position; it says which call failed, and carries the cause as
//! [`Reason`]. A caller that only needs "did this fail" reads the wrapper; a
//! caller writing a backoff policy asks [`GitHubError::is_transient`]; one
//! that needs the exact status matches `err.reason`.

/// A GitHub API call that did not produce an answer.
#[derive(Debug, thiserror::Error)]
#[error("{call} failed: {reason}")]
pub struct GitHubError {
    /// The API call, named as this crate names it (`compare`, `post_comment`).
    pub call: &'static str,
    /// What actually went wrong.
    pub reason: Reason,
}

impl GitHubError {
    /// Attach a call name to a cause.
    pub fn new(call: &'static str, reason: Reason) -> Self {
        GitHubError { call, reason }
    }

    /// Whether trying the same call again could plausibly succeed.
    ///
    /// Transport failures and GitHub's own 5xx are worth another attempt; a
    /// rejected credential or a body we could not read are not.
    pub fn is_transient(&self) -> bool {
        match &self.reason {
            Reason::Transport(_) => true,
            Reason::Status { status, .. } => *status >= 500 || *status == 429,
            _ => false,
        }
    }
}

/// Why a GitHub API call failed.
#[derive(Debug, thiserror::Error)]
pub enum Reason {
    /// The request never reached GitHub, or its answer never arrived.
    #[error("the request could not be completed: {0}")]
    Transport(#[source] reqwest::Error),

    /// GitHub answered, and declined. Status and body both travel: the status
    /// is what a policy branches on, the body is what a human reads.
    #[error("GitHub answered {status}: {body}")]
    Status { status: u16, body: String },

    /// The response arrived but did not match the shape this crate expects.
    #[error("the response did not have the expected shape: {0}")]
    Decode(#[source] reqwest::Error),

    /// File content GitHub declared as base64 was not base64.
    #[error("file content was not valid base64: {0}")]
    NotBase64(#[source] base64::DecodeError),

    /// File content decoded, but was not text.
    #[error("file content was not valid UTF-8: {0}")]
    NotUtf8(#[source] std::string::FromUtf8Error),

    /// The app's own credentials could not be turned into a request token —
    /// a configuration fault, not a transient one.
    #[error("the app's credentials were not usable: {0}")]
    Credentials(String),

    /// A merge ran on a worker thread and that thread did not come back.
    #[error("the merge task did not complete: {0}")]
    MergeTaskLost(#[source] tokio::task::JoinError),
}

/// Why the server could not start.
///
/// Separate from [`Reason`] on purpose: these are answered once, at startup,
/// by an operator editing the environment — never by a retry.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// A required variable is absent.
    #[error("{name} is not set")]
    Missing { name: &'static str },

    /// A variable is present and does not parse.
    #[error("{name} must be {expected}, got {found:?}")]
    Malformed {
        name: &'static str,
        expected: &'static str,
        found: String,
    },
}
