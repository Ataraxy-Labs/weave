use base64::Engine;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::error::{GitHubError, Reason};

/// The status/body pair GitHub answered a declined call with.
async fn declined(call: &'static str, resp: reqwest::Response) -> GitHubError {
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    GitHubError::new(call, Reason::Status { status, body })
}

/// GitHub API client for a specific installation.
pub struct GitHubClient {
    client: reqwest::Client,
    token: String,
}

#[derive(Serialize)]
struct JwtClaims {
    iat: u64,
    exp: u64,
    iss: String,
}

#[derive(Deserialize)]
struct InstallationToken {
    token: String,
}

#[derive(Deserialize)]
pub struct CompareResponse {
    pub merge_base_commit: MergeBaseCommit,
    pub files: Option<Vec<CompareFile>>,
}

#[derive(Deserialize)]
pub struct MergeBaseCommit {
    pub sha: String,
}

#[derive(Deserialize)]
pub struct CompareFile {
    pub filename: String,
    pub status: String,
}

#[derive(Deserialize)]
struct ContentResponse {
    content: Option<String>,
    encoding: Option<String>,
}

/// The one field this crate reads off a pull request. GitHub sends dozens;
/// naming the one we use keeps `serde_json::Value` out of the call's return
/// path, where `pr["mergeable"]` would have answered `None` identically for
/// "not computed yet" and "we asked for the wrong key".
#[derive(Deserialize)]
struct PullRequest {
    mergeable: Option<bool>,
}

#[derive(Serialize)]
struct CheckRunRequest {
    name: String,
    head_sha: String,
    status: String,
    conclusion: Option<String>,
    output: Option<CheckRunOutput>,
}

#[derive(Serialize)]
struct CheckRunOutput {
    title: String,
    summary: String,
}

impl GitHubClient {
    /// Create a client authenticated as a GitHub App installation.
    pub async fn for_installation(
        config: &Config,
        installation_id: u64,
    ) -> Result<Self, GitHubError> {
        let jwt = create_jwt(config)?;
        let client = reqwest::Client::new();

        let resp = client
            .post(format!(
                "https://api.github.com/app/installations/{installation_id}/access_tokens"
            ))
            .header("Authorization", format!("Bearer {jwt}"))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "weave-github")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| GitHubError::new("for_installation", Reason::Transport(e)))?;

        if !resp.status().is_success() {
            return Err(declined("for_installation", resp).await);
        }

        let token: InstallationToken = resp
            .json()
            .await
            .map_err(|e| GitHubError::new("for_installation", Reason::Decode(e)))?;

        Ok(GitHubClient {
            client,
            token: token.token,
        })
    }

    fn api_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        headers.insert("Accept", "application/vnd.github+json".parse().unwrap());
        headers.insert("User-Agent", "weave-github".parse().unwrap());
        headers.insert("X-GitHub-Api-Version", "2022-11-28".parse().unwrap());
        headers
    }

    /// Compare two refs, returns merge base SHA and changed files.
    pub async fn compare(
        &self,
        owner: &str,
        repo: &str,
        base: &str,
        head: &str,
    ) -> Result<CompareResponse, GitHubError> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/compare/{base}...{head}");
        let resp = self
            .client
            .get(&url)
            .headers(self.api_headers())
            .send()
            .await
            .map_err(|e| GitHubError::new("compare", Reason::Transport(e)))?;

        if !resp.status().is_success() {
            return Err(declined("compare", resp).await);
        }

        resp.json()
            .await
            .map_err(|e| GitHubError::new("compare", Reason::Decode(e)))
    }

    /// Fetch file content at a specific ref.
    pub async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        ref_: &str,
    ) -> Result<Option<String>, GitHubError> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/contents/{path}?ref={ref_}");
        let resp = self
            .client
            .get(&url)
            .headers(self.api_headers())
            .send()
            .await
            .map_err(|e| GitHubError::new("get_file_content", Reason::Transport(e)))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !resp.status().is_success() {
            return Err(declined("get_file_content", resp).await);
        }

        let content: ContentResponse = resp
            .json()
            .await
            .map_err(|e| GitHubError::new("get_file_content", Reason::Decode(e)))?;

        match (content.content, content.encoding.as_deref()) {
            (Some(encoded), Some("base64")) => {
                let cleaned: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(&cleaned)
                    .map_err(|e| GitHubError::new("get_file_content", Reason::NotBase64(e)))?;
                String::from_utf8(decoded)
                    .map(Some)
                    .map_err(|e| GitHubError::new("get_file_content", Reason::NotUtf8(e)))
            }
            (Some(raw), _) => Ok(Some(raw)),
            (None, _) => Ok(None),
        }
    }

    /// Post a comment on a PR.
    pub async fn post_comment(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        body: &str,
    ) -> Result<(), GitHubError> {
        let url =
            format!("https://api.github.com/repos/{owner}/{repo}/issues/{pr_number}/comments");
        let resp = self
            .client
            .post(&url)
            .headers(self.api_headers())
            .json(&serde_json::json!({ "body": body }))
            .send()
            .await
            .map_err(|e| GitHubError::new("post_comment", Reason::Transport(e)))?;

        if !resp.status().is_success() {
            return Err(declined("post_comment", resp).await);
        }

        Ok(())
    }

    /// Create a check run on a commit.
    pub async fn create_check_run(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
        conclusion: &str,
        title: &str,
        summary: &str,
    ) -> Result<(), GitHubError> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/check-runs");
        let req = CheckRunRequest {
            name: "weave".to_string(),
            head_sha: head_sha.to_string(),
            status: "completed".to_string(),
            conclusion: Some(conclusion.to_string()),
            output: Some(CheckRunOutput {
                title: title.to_string(),
                summary: summary.to_string(),
            }),
        };

        let resp = self
            .client
            .post(&url)
            .headers(self.api_headers())
            .json(&req)
            .send()
            .await
            .map_err(|e| GitHubError::new("create_check_run", Reason::Transport(e)))?;

        if !resp.status().is_success() {
            return Err(declined("create_check_run", resp).await);
        }

        Ok(())
    }

    /// Check if a PR is mergeable (has conflicts).
    pub async fn get_pr_mergeable(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Option<bool>, GitHubError> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}");
        let resp = self
            .client
            .get(&url)
            .headers(self.api_headers())
            .send()
            .await
            .map_err(|e| GitHubError::new("get_pr_mergeable", Reason::Transport(e)))?;

        if !resp.status().is_success() {
            return Err(declined("get_pr_mergeable", resp).await);
        }

        let pr: PullRequest = resp
            .json()
            .await
            .map_err(|e| GitHubError::new("get_pr_mergeable", Reason::Decode(e)))?;

        Ok(pr.mergeable)
    }
}

fn create_jwt(config: &Config) -> Result<String, GitHubError> {
    let credentials =
        |what: String| GitHubError::new("for_installation", Reason::Credentials(what));
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| credentials(format!("the system clock is before the epoch: {e}")))?
        .as_secs();

    let claims = JwtClaims {
        iat: now - 60,
        exp: now + (10 * 60),
        iss: config.app_id.to_string(),
    };

    let key = EncodingKey::from_rsa_pem(config.private_key.as_bytes())
        .map_err(|e| credentials(format!("GITHUB_PRIVATE_KEY is not an RSA PEM key: {e}")))?;

    encode(&Header::new(Algorithm::RS256), &claims, &key)
        .map_err(|e| credentials(format!("the request token could not be signed: {e}")))
}
