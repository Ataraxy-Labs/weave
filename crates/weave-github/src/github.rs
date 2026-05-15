use base64::Engine;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;

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
    pub async fn for_installation(config: &Config, installation_id: u64) -> Result<Self, String> {
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
            .map_err(|e| format!("token request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("token exchange failed ({status}): {body}"));
        }

        let token: InstallationToken = resp
            .json()
            .await
            .map_err(|e| format!("token parse failed: {e}"))?;

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
    ) -> Result<CompareResponse, String> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/compare/{base}...{head}");
        let resp = self
            .client
            .get(&url)
            .headers(self.api_headers())
            .send()
            .await
            .map_err(|e| format!("compare failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("compare failed ({status}): {body}"));
        }

        resp.json()
            .await
            .map_err(|e| format!("compare parse failed: {e}"))
    }

    /// Fetch file content at a specific ref.
    pub async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        ref_: &str,
    ) -> Result<Option<String>, String> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/contents/{path}?ref={ref_}");
        let resp = self
            .client
            .get(&url)
            .headers(self.api_headers())
            .send()
            .await
            .map_err(|e| format!("contents fetch failed: {e}"))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("contents fetch failed ({status}): {body}"));
        }

        let content: ContentResponse = resp
            .json()
            .await
            .map_err(|e| format!("contents parse failed: {e}"))?;

        match (content.content, content.encoding.as_deref()) {
            (Some(encoded), Some("base64")) => {
                let cleaned: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(&cleaned)
                    .map_err(|e| format!("base64 decode failed: {e}"))?;
                String::from_utf8(decoded)
                    .map(Some)
                    .map_err(|e| format!("utf8 decode failed: {e}"))
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
    ) -> Result<(), String> {
        let url =
            format!("https://api.github.com/repos/{owner}/{repo}/issues/{pr_number}/comments");
        let resp = self
            .client
            .post(&url)
            .headers(self.api_headers())
            .json(&serde_json::json!({ "body": body }))
            .send()
            .await
            .map_err(|e| format!("comment post failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("comment post failed ({status}): {body}"));
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
    ) -> Result<(), String> {
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
            .map_err(|e| format!("check run failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("check run failed ({status}): {body}"));
        }

        Ok(())
    }

    /// Check if a PR is mergeable (has conflicts).
    pub async fn get_pr_mergeable(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Option<bool>, String> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}");
        let resp = self
            .client
            .get(&url)
            .headers(self.api_headers())
            .send()
            .await
            .map_err(|e| format!("PR fetch failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("PR fetch failed ({status}): {body}"));
        }

        let pr: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("PR parse failed: {e}"))?;

        Ok(pr["mergeable"].as_bool())
    }
}

fn create_jwt(config: &Config) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("time error: {e}"))?
        .as_secs();

    let claims = JwtClaims {
        iat: now - 60,
        exp: now + (10 * 60),
        iss: config.app_id.to_string(),
    };

    let key = EncodingKey::from_rsa_pem(config.private_key.as_bytes())
        .map_err(|e| format!("invalid private key: {e}"))?;

    encode(&Header::new(Algorithm::RS256), &claims, &key)
        .map_err(|e| format!("JWT encode failed: {e}"))
}
