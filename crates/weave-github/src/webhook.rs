use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::merge::handle_pull_request;
use crate::AppState;

type HmacSha256 = Hmac<Sha256>;

/// Verify the webhook signature and dispatch the event.
///
/// Returns 200 immediately for valid webhooks. Merge processing
/// happens in a background tokio task.
pub async fn handle_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    // Verify signature
    let signature = match headers.get("x-hub-signature-256") {
        Some(v) => v.to_str().unwrap_or(""),
        None => return StatusCode::UNAUTHORIZED,
    };

    if !verify_signature(&state.config.webhook_secret, &body, signature) {
        return StatusCode::UNAUTHORIZED;
    }

    // Parse event type
    let event = match headers.get("x-github-event") {
        Some(v) => v.to_str().unwrap_or(""),
        None => return StatusCode::BAD_REQUEST,
    };

    if event != "pull_request" {
        return StatusCode::OK;
    }

    // Parse payload. GitHub sends a large document; this decodes the parts
    // weave acts on and nothing else. It used to be a `serde_json::Value`
    // indexed by string, where a missing key and a hostile one were the same
    // answer — `payload["action"].as_str().unwrap_or("")` returned `""` for
    // both, fell through the match below, and returned 200 on a body that had
    // never been read.
    let payload: PullRequestPayload = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("webhook body did not decode as a pull_request event: {e}");
            return StatusCode::BAD_REQUEST;
        }
    };

    // Only handle opened, synchronize (new push), reopened
    if !matches!(
        payload.action.as_str(),
        "opened" | "synchronize" | "reopened"
    ) {
        return StatusCode::OK;
    }

    let pr = payload.into_event();

    // Spawn background task, return 200 immediately
    tokio::spawn(async move {
        if let Err(e) = handle_pull_request(&state, &pr).await {
            tracing::error!(
                repo = %pr.repo_full_name,
                pr = pr.pr_number,
                "merge processing failed: {e}"
            );
        }
    });

    StatusCode::OK
}

/// Parsed pull request event data.
pub struct PrEvent {
    pub installation_id: u64,
    pub repo_full_name: String,
    pub owner: String,
    pub repo: String,
    pub pr_number: u64,
    pub head_sha: String,
    pub base_sha: String,
}

/// The parts of GitHub's `pull_request` event weave acts on.
///
/// Not `deny_unknown_fields`: this document is GitHub's, it carries dozens of
/// fields weave has no use for, and it grows without asking. Denying here would
/// mean a new GitHub field breaks the webhook. The closed-schema rule applies to
/// documents whose shape weave publishes — the ops document and the tool
/// arguments — not to somebody else's payload.
#[derive(Debug, Deserialize)]
struct PullRequestPayload {
    action: String,
    installation: Installation,
    repository: Repository,
    pull_request: PullRequest,
}

#[derive(Debug, Deserialize)]
struct Installation {
    id: u64,
}

#[derive(Debug, Deserialize)]
struct Repository {
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct PullRequest {
    number: u64,
    head: Commit,
    base: Commit,
}

#[derive(Debug, Deserialize)]
struct Commit {
    sha: String,
}

impl PullRequestPayload {
    /// `owner/repo` is one field on the wire and two everywhere else. A name
    /// without a slash keeps the whole string as the owner and leaves the repo
    /// empty, which the API call will reject — the same outcome the old
    /// `split_once('/')?` produced, minus the silent 400 with no reason.
    fn into_event(self) -> PrEvent {
        let (owner, repo) = self
            .repository
            .full_name
            .split_once('/')
            .map(|(o, r)| (o.to_string(), r.to_string()))
            .unwrap_or_else(|| (self.repository.full_name.clone(), String::new()));

        PrEvent {
            installation_id: self.installation.id,
            repo_full_name: self.repository.full_name,
            owner,
            repo,
            pr_number: self.pull_request.number,
            head_sha: self.pull_request.head.sha,
            base_sha: self.pull_request.base.sha,
        }
    }
}

fn verify_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    let sig_hex = signature.strip_prefix("sha256=").unwrap_or(signature);

    let expected = match hex::decode(sig_hex) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);

    mac.verify_slice(&expected).is_ok()
}
