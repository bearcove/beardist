use log::{debug, info};
use reqwest::blocking::Client;
use semver::Version;
use serde_json::Value;

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
}
