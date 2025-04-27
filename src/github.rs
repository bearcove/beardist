use log::{debug, info};
use owo_colors::OwoColorize;
use reqwest::blocking::Client;
use semver::Version;
use serde_json::Value;

use crate::USER_AGENT;

pub struct GitHubClient {
    client: Client,
    server_url: String,
    token: String,
}

impl GitHubClient {
    pub fn new(server_url: String, token: String) -> Self {
        Self {
            client: Client::new(),
            server_url,
            token,
        }
    }

    pub fn from_env() -> eyre::Result<Self> {
        let server_url = std::env::var("GITHUB_SERVER_URL")
            .unwrap_or_else(|_| "https://api.github.com".to_string());
        let token = std::env::var("GITHUB_TOKEN")
            .map_err(|_| eyre::eyre!("GITHUB_TOKEN environment variable not set"))?;
        Ok(Self::new(server_url, token))
    }

    /// Get the latest version tag from a GitHub Container Registry (ghcr.io) package
    pub fn get_latest_container_version(
        &self,
        org: &str,
        package_name: &str,
    ) -> eyre::Result<Option<String>> {
        let url = format!(
            "{}/orgs/{}/packages/container/{}/versions",
            self.server_url, org, package_name
        );

        info!(
            "Fetching latest container version for '{}' from '{}'",
            package_name, url
        );

        let start_time = std::time::Instant::now();
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("token {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", USER_AGENT)
            .send()?;

        let status = response.status();
        let elapsed = start_time.elapsed();
        info!(
            "Request completed in {}ms with status {}",
            elapsed.as_millis(),
            status
        );

        if status != 200 {
            let body = response.text()?;
            debug!("Error response: {}", body);
            return Err(eyre::eyre!(
                "Failed to get container versions: HTTP status {status}"
            ));
        }

        let body = response.text()?;
        debug!("Response body size: {} bytes", body.len());

        let versions: Vec<Value> = serde_json::from_str(&body)?;
        info!("Received {} versions in response", versions.len());

        let valid_versions: Vec<Version> = versions
            .iter()
            .filter_map(|version| {
                // Look for metadata tags with semver format
                version["metadata"]["container"]["tags"]
                    .as_array()
                    .and_then(|tags| {
                        tags.iter()
                            .filter_map(|tag| tag.as_str())
                            .filter_map(|tag| Version::parse(tag.trim_start_matches('v')).ok())
                            .max()
                    })
            })
            .collect();

        info!("Found {} valid semver tags", valid_versions.len());

        if valid_versions.is_empty() {
            info!("No valid versioned tags found for container");
            return Ok(None);
        }

        let latest_version = valid_versions.into_iter().max().unwrap();
        info!("Latest container version found: {}", latest_version);
        Ok(Some(latest_version.to_string()))
    }

    /// Get the latest release version from a GitHub repository
    pub fn get_latest_release_version(
        &self,
        owner: &str,
        repo: &str,
    ) -> eyre::Result<Option<String>> {
        let url = format!(
            "{}/repos/{}/{}/releases/latest",
            self.server_url, owner, repo
        );

        info!(
            "Fetching latest release for repository '{}/{}' from '{}'",
            owner, repo, url
        );

        let start_time = std::time::Instant::now();
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("token {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", USER_AGENT)
            .send()?;

        let status = response.status();
        let elapsed = start_time.elapsed();
        info!(
            "Request completed in {}ms with status {}",
            elapsed.as_millis(),
            status
        );

        // 404 means no releases yet
        if status == 404 {
            info!("No releases found for repository '{}/{}'", owner, repo);
            return Ok(None);
        }

        if status != 200 {
            let body = response.text()?;
            debug!("Error response: {}", body);
            return Err(eyre::eyre!(
                "Failed to get latest release: HTTP status {status}"
            ));
        }

        let body = response.text()?;
        debug!("Response body size: {} bytes", body.len());

        let release: Value = serde_json::from_str(&body)?;

        if let Some(tag_name) = release["tag_name"].as_str() {
            let version_str = tag_name.trim_start_matches('v');
            info!("Latest release tag: {}", tag_name);

            // Try to parse as semver, but return the original tag if not valid
            match Version::parse(version_str) {
                Ok(version) => Ok(Some(version.to_string())),
                Err(_) => Ok(Some(version_str.to_string())),
            }
        } else {
            info!("Release found but no tag_name present");
            Ok(None)
        }
    }

    /// Create a release if it doesn't exist, and return the release ID
    pub fn create_release(&self, org: &str, name: &str, tag: &str) -> eyre::Result<u64> {
        let github_api_url = format!(
            "{}/repos/{}/{}/releases/tags/{}",
            self.server_url.replace("github.com", "api.github.com"),
            org,
            name,
            tag
        );

        info!("Checking if release exists at {}...", github_api_url);

        let release_response = self
            .client
            .get(&github_api_url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("token {}", self.token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", USER_AGENT)
            .send()?;

        let release_id = if !release_response.status().is_success() {
            info!("Release doesn't exist, creating one...");

            let release_create_url = format!(
                "{}/repos/{}/{}/releases",
                self.server_url.replace("github.com", "api.github.com"),
                org,
                name
            );

            let release_create_body = serde_json::json!({
                "tag_name": tag,
                "name": tag,
                "draft": false,
                "prerelease": false
            });

            let create_response = self
                .client
                .post(&release_create_url)
                .header("Accept", "application/vnd.github+json")
                .header("Authorization", format!("token {}", self.token))
                .header("X-GitHub-Api-Version", "2022-11-28")
                .header("User-Agent", USER_AGENT)
                .json(&release_create_body)
                .send()?;

            if !create_response.status().is_success() {
                return Err(eyre::eyre!(
                    "Failed to create release: {}",
                    create_response.text()?
                ));
            }

            let release_data: serde_json::Value = create_response.json()?;
            release_data["id"]
                .as_u64()
                .ok_or_else(|| eyre::eyre!("Invalid release ID"))?
        } else {
            let release_data: serde_json::Value = release_response.json()?;
            release_data["id"]
                .as_u64()
                .ok_or_else(|| eyre::eyre!("Invalid release ID"))?
        };

        Ok(release_id)
    }

    /// Upload an artifact to a GitHub release as a release asset
    pub fn upload_artifact(
        &self,
        org: &str,
        name: &str,
        release_id: u64,
        package_file_name: &str,
        file_content: &[u8],
    ) -> eyre::Result<()> {
        use log::warn;

        // Assemble the correct uploads.github.com asset endpoint
        let upload_url = format!(
            "{}/repos/{}/{}/releases/{}/assets?name={}",
            self.server_url.replace("github.com", "uploads.github.com"),
            org,
            name,
            release_id,
            package_file_name
        );

        info!(
            "📤 Uploading package to {} ({})...",
            "GitHub".yellow(),
            upload_url.cyan()
        );
        let upload_start = std::time::Instant::now();

        // Retry logic for upload attempts
        const MAX_RETRIES: usize = 3;
        const BASE_RETRY_DELAY_MS: u64 = 2000; // 2 seconds

        let mut attempt = 0;
        let mut last_error = None;

        while attempt < MAX_RETRIES {
            attempt += 1;

            if attempt > 1 {
                info!("🔄 Retry attempt {} of {}...", attempt, MAX_RETRIES);
                let jitter = rand::random::<u64>() % 1000; // Random jitter between 0-999ms
                let delay = BASE_RETRY_DELAY_MS + jitter;
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }

            match self
                .client
                .post(&upload_url)
                .header("Accept", "application/vnd.github+json")
                .header("Authorization", format!("token {}", self.token))
                .header("X-GitHub-Api-Version", "2022-11-28")
                .header("User-Agent", USER_AGENT)
                .header("Content-Type", "application/octet-stream")
                .body(file_content.to_vec())
                .send()
            {
                Ok(response) => {
                    info!(
                        "🔢 Response status code: {}",
                        format!("{}", response.status()).blue()
                    );

                    let status = response.status();
                    let response_text = response.text()?;
                    info!("{}", "----------------------------------------".yellow());
                    info!("📄 {}", "Response Data:".yellow());
                    info!("{}", "----------------------------------------".yellow());
                    info!("{}", response_text);
                    info!("{}", "----------------------------------------".yellow());

                    // If successful or not a 5xx error, break out of retry loop
                    if status.is_success() || !status.is_server_error() {
                        if !status.is_success() {
                            return Err(eyre::eyre!(
                                "❌ Upload failed with status code: {}",
                                status
                            ));
                        }

                        let upload_time = upload_start.elapsed().as_millis() as u64;
                        info!(
                            "✅ Package upload completed ({})",
                            format!("{}ms", upload_time).green()
                        );
                        return Ok(());
                    }

                    // If we get here, it's a 5xx error and we'll retry
                    last_error = Some(eyre::eyre!("Server error with status code: {}", status));
                }
                Err(e) => {
                    last_error = Some(eyre::eyre!("Request error: {}", e));
                }
            }

            warn!("📶 Upload attempt {} failed, retrying...", attempt);
        }

        // If we get here, all retries failed
        Err(last_error
            .unwrap_or_else(|| eyre::eyre!("Upload failed after {} attempts", MAX_RETRIES)))
    }
}
