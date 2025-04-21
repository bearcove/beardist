use log::{debug, info};
use reqwest::blocking::Client;
use semver::Version;
use serde_json::Value;

use std::fmt;

#[derive(Debug, Clone, Copy)]
pub enum PackageType {
    Generic,
    Container,
}

impl fmt::Display for PackageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageType::Generic => write!(f, "generic"),
            PackageType::Container => write!(f, "container"),
        }
    }
}

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

    pub fn get_latest_version(
        &self,
        org: &str,
        package_name: &str,
        package_type: PackageType,
    ) -> eyre::Result<Option<String>> {
        let url = format!("{}/orgs/{}/packages", self.server_url, org);
        info!(
            "Fetching latest version for package '{}' from '{}'",
            package_name, url
        );

        let package_type_str = match package_type {
            PackageType::Generic => "maven",
            PackageType::Container => "container",
        };

        let start_time = std::time::Instant::now();
        let response = self
            .client
            .get(&url)
            .query(&[("package_type", package_type_str)])
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

        let body = response.text()?;
        debug!("Response body size: {} bytes", body.len());

        if status != 200 {
            return Err(eyre::eyre!(
                "Failed to get latest version: HTTP status {status}"
            ));
        }

        let packages: Vec<Value> = serde_json::from_str(&body)?;
        info!("Received {} packages in response", packages.len());

        let filtered_packages: Vec<_> = packages
            .into_iter()
            .filter(|package| package["name"].as_str() == Some(package_name))
            .collect();

        info!("Filtered to {} matching packages", filtered_packages.len());

        if filtered_packages.is_empty() {
            info!("No matching packages found");
            return Ok(None);
        }

        // For the found package, we need to get its versions
        if let Some(package) = filtered_packages.first() {
            if let Some(package_name_value) = package["name"].as_str() {
                let versions_url = format!(
                    "{}/orgs/{}/packages/{}/{}/versions",
                    self.server_url, org, package_type_str, package_name_value
                );

                let versions_response = self
                    .client
                    .get(&versions_url)
                    .header("Authorization", format!("token {}", self.token))
                    .header("Accept", "application/vnd.github+json")
                    .header("X-GitHub-Api-Version", "2022-11-28")
                    .send()?;

                if versions_response.status() != 200 {
                    return Err(eyre::eyre!(
                        "Failed to get package versions: HTTP status {}",
                        versions_response.status()
                    ));
                }

                let versions_body = versions_response.text()?;
                let versions: Vec<Value> = serde_json::from_str(&versions_body)?;

                let valid_versions: Vec<Version> = versions
                    .iter()
                    .filter_map(|version| {
                        version["name"]
                            .as_str()
                            .and_then(|v| Version::parse(v.trim_start_matches('v')).ok())
                    })
                    .collect();

                info!("Found {} valid versions", valid_versions.len());

                if valid_versions.is_empty() {
                    info!("No valid versions found");
                    return Ok(None);
                }

                let latest_version = valid_versions.iter().max().unwrap();
                info!("Latest version found: {}", latest_version);
                return Ok(Some(latest_version.to_string()));
            }
        }

        Ok(None)
    }
}
