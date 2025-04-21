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

pub struct ForgejoClient {
    client: Client,
    server_url: String,
    token: String,
}

impl ForgejoClient {
    pub fn new(server_url: String, token: String) -> Self {
        Self {
            client: Client::new(),
            server_url,
            token,
        }
    }

    pub fn from_env() -> eyre::Result<Self> {
        let server_url = std::env::var("FORGEJO_SERVER_URL")
            .unwrap_or_else(|_| "https://code.bearcove.cloud".to_string());
        let token = std::env::var("FORGEJO_TOKEN")
            .map_err(|_| eyre::eyre!("FORGEJO_TOKEN environment variable not set"))?;
        Ok(Self::new(server_url, token))
    }

    pub fn get_latest_version(
        &self,
        org: &str,
        package_name: &str,
        package_type: PackageType,
    ) -> eyre::Result<Option<String>> {
        let url = format!("{}/api/v1/packages/{}", self.server_url, org);
        info!(
            "Fetching latest version for package '{}' from '{}'",
            package_name, url
        );

        let start_time = std::time::Instant::now();
        let response = self
            .client
            .get(&url)
            .query(&[
                ("q", package_name),
                ("limit", "100"),
                ("type", &package_type.to_string()),
            ])
            .header("Authorization", format!("token {}", self.token))
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

        let valid_versions: Vec<Version> = filtered_packages
            .iter()
            .filter_map(|package| {
                package["version"]
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
        Ok(Some(latest_version.to_string()))
    }
}
