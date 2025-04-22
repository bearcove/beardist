use camino::Utf8PathBuf;
use color_eyre::eyre;
use convert_case::{Case, Casing};
use eyre::Context;
use log::*;
use owo_colors::OwoColorize;
use reqwest::blocking::Client;
use std::{path::PathBuf, sync::Arc};
use url::Url;

use crate::{Indented, command::get_trimmed_cmd_stdout, github::GitHubClient, run_command};

use serde::Deserialize;

#[cfg(test)]
mod tests;

#[derive(Deserialize, Debug, Clone)]
struct TapConfig {
    formulas: Vec<Formula>,
}

#[derive(Deserialize, Debug, Clone)]
struct Formula {
    repo: String,
    homepage: String,
    desc: String,
    license: String,
    bins: Vec<String>,

    #[serde(default)]
    deps: Vec<String>,
}

struct Binaries {
    mac: Binary,
    linux_x86_64: Binary,
    linux_aarch64: Binary,
}

impl Formula {
    fn org(&self) -> &str {
        self.repo.split('/').next().unwrap()
    }

    fn name(&self) -> &str {
        self.repo.split('/').nth(1).unwrap()
    }

    /// Where the formula is written on disk
    fn disk_path(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(format!("Formula/{}.rb", self.name()))
    }

    fn github_version(
        &self,
        _config: &TapConfig,
        github_token: &str,
    ) -> eyre::Result<Option<String>> {
        let github_client = GitHubClient::new(
            "https://api.github.com".to_string(),
            github_token.to_string(),
        );
        github_client.get_latest_release_version(self.org(), self.name())
    }

    fn formula_version(&self) -> Option<String> {
        let disk_path = self.disk_path();
        if !disk_path.exists() {
            return None;
        }
        let content = fs_err::read_to_string(&disk_path).unwrap();
        let version_line = content
            .lines()
            .find(|line| line.trim().starts_with("version"))?;
        let version = version_line.split('"').nth(1)?;
        Some(version.to_string())
    }
}

struct Binary {
    url: String,
    sha256: String,
}

#[derive(Clone)]
struct HomebrewContext {
    client: Arc<Client>,
    dry_run: bool,
    formula: Formula,
    new_version: String,
}

impl HomebrewContext {
    fn new(
        client: Arc<Client>,
        formula: Formula,
        github_version: String,
        dry_run: bool,
    ) -> eyre::Result<Option<Self>> {
        let formula_version = formula.formula_version();
        if let Some(formula_version) = formula_version {
            if formula_version == github_version {
                info!(
                    "Formula version {} is already up-to-date with GitHub version {}",
                    formula_version.bright_green(),
                    github_version.bright_green()
                );
                return Ok(None);
            }
        }

        Ok(Some(Self {
            client,
            dry_run,
            formula,
            new_version: github_version,
        }))
    }

    fn get_binary(&self, url: &str) -> eyre::Result<Binary> {
        Ok(Binary {
            url: url.to_string(),
            sha256: self.fetch_and_hash(url)?,
        })
    }

    fn package_artifact_url(&self, arch: &str) -> String {
        format!(
            "https://github.com/{}/{}/releases/download/v{}/{}.tar.xz",
            self.formula.org(),
            self.formula.name(),
            self.new_version,
            arch
        )
    }

    fn update_formula(&self) -> eyre::Result<()> {
        info!("Updating Homebrew {}...", "formula".bright_yellow());

        // Set up URLs for all architectures
        let mac_url = self.package_artifact_url("aarch64-apple-darwin");
        let linux_x86_64_url = self.package_artifact_url("x86_64-unknown-linux-gnu");
        let linux_aarch64_url = self.package_artifact_url("aarch64-unknown-linux-gnu");

        // Use threads to fetch binaries in parallel
        let self_clone1 = self.clone();
        let mac = std::thread::spawn(move || self_clone1.get_binary(&mac_url));

        let self_clone2 = self.clone();
        let linux_x86_64 = std::thread::spawn(move || self_clone2.get_binary(&linux_x86_64_url));

        let self_clone3 = self.clone();
        let linux_aarch64 = std::thread::spawn(move || self_clone3.get_binary(&linux_aarch64_url));

        let mac = mac.join().unwrap();
        let linux_x86_64 = linux_x86_64.join().unwrap();
        let linux_aarch64 = linux_aarch64.join().unwrap();

        let binaries = Binaries {
            mac: mac?,
            linux_x86_64: linux_x86_64?,
            linux_aarch64: linux_aarch64?,
        };

        let formula = self.generate_homebrew_formula(binaries)?;
        let formula_path = self.formula.disk_path();

        if self.dry_run {
            info!(
                "Dry run: Would write formula to {}",
                formula_path.to_string().cyan()
            );
            info!("Formula content:\n{}", formula);
        } else {
            if let Some(parent) = formula_path.parent() {
                fs_err::create_dir_all(parent)?;
            }
            fs_err::write(&formula_path, formula)?;
            info!(
                "Homebrew formula written to {}",
                formula_path.to_string().bright_green()
            );
        }

        Ok(())
    }

    fn generate_homebrew_formula(&self, binaries: Binaries) -> eyre::Result<String> {
        use std::fmt::Write;

        let mut w = String::new();

        writeln!(w, "# frozen_string_literal: true")?;
        writeln!(w)?;
        writeln!(w, "# {}", self.formula.desc)?;
        writeln!(
            w,
            "class {} < Formula",
            self.formula.name().to_case(Case::Pascal)
        )?;
        {
            let mut w = w.indented();
            writeln!(w, "desc \"{}\"", self.formula.desc)?;
            writeln!(w, "homepage \"{}\"", self.formula.homepage)?;
            writeln!(w, "version \"{}\"", self.new_version)?;
            writeln!(w, "license \"{}\"", self.formula.license)?;
            writeln!(w)?;
            for dep in &self.formula.deps {
                let parts: Vec<&str> = dep.split('#').collect();
                let (name, keyword) = match parts.as_slice() {
                    [name] => (name, None),
                    [name, keyword] => (name, Some(keyword.trim())),
                    _ => {
                        return Err(eyre::eyre!(
                            "Invalid dependency syntax. Use 'name' or 'name#keyword' where keyword is 'recommended' or 'optional'"
                        ));
                    }
                };

                match keyword {
                    None => writeln!(w, "depends_on \"{}\"", name)?,
                    Some("recommended") => writeln!(w, "depends_on \"{}\" => :recommended", name)?,
                    Some("optional") => writeln!(w, "depends_on \"{}\" => :optional", name)?,
                    Some(k) => {
                        return Err(eyre::eyre!(
                            "Unknown dependency keyword: '{}'. Use 'recommended' or 'optional'",
                            k
                        ));
                    }
                }
            }
            writeln!(w)?;
            writeln!(w, "if OS.mac?")?;
            {
                let mut w = w.indented();
                writeln!(w, "url \"{}\"", binaries.mac.url)?;
                writeln!(w, "sha256 \"{}\"", binaries.mac.sha256)?;
            }
            writeln!(w, "elsif OS.linux?")?;
            {
                let mut w = w.indented();
                writeln!(w, "on_intel do")?;
                {
                    let mut w = w.indented();
                    writeln!(w, "url \"{}\"", binaries.linux_x86_64.url)?;
                    writeln!(w, "sha256 \"{}\"", binaries.linux_x86_64.sha256)?;
                }
                writeln!(w, "end")?;
                writeln!(w, "on_arm do")?;
                {
                    let mut w = w.indented();
                    writeln!(w, "url \"{}\"", binaries.linux_aarch64.url)?;
                    writeln!(w, "sha256 \"{}\"", binaries.linux_aarch64.sha256)?;
                }
                writeln!(w, "end")?;
            }
            writeln!(w, "end")?;
            writeln!(w)?;
            writeln!(w, "def install")?;
            {
                let mut w = w.indented();
                for bin in &self.formula.bins {
                    writeln!(w, "bin.install \"{}\"", bin)?;
                }
                writeln!(w, "libexec.install Dir[\"lib*.dylib\"] if OS.mac?")?;
                writeln!(w, "libexec.install Dir[\"lib*.so\"] if OS.linux?")?;
            }
            writeln!(w, "end")?;
        }
        writeln!(w, "end")?;

        Ok(w)
    }

    fn fetch_and_hash(&self, url: &str) -> eyre::Result<String> {
        info!("Fetching binary from {}...", url.cyan());
        if self.dry_run {
            info!("Dry run: Would fetch {}", "binary".bright_yellow());
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(url);
            let sha256 = format!("{:x}", hasher.finalize());
            return Ok(sha256);
        }

        let response = self.client.get(url).send()?;
        let status = response.status();
        if status != 200 {
            let error_text = response.text()?;
            error!(
                "Failed to fetch binary: HTTP status {}, Response: {}",
                status.to_string().red(),
                error_text.red()
            );
            return Err(eyre::eyre!(
                "Failed to fetch binary: HTTP status {}",
                status
            ));
        }
        let bytes = response.bytes()?;
        let byte_count = bytes.len();
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let sha256 = format!("{:x}", hasher.finalize());
        info!(
            "Binary fetched ({} bytes) and SHA256 {}",
            byte_count.to_string().green(),
            "computed".green()
        );
        Ok(sha256)
    }
}

fn load_tap_config() -> eyre::Result<TapConfig> {
    let config_path = fs_err::canonicalize(PathBuf::from(".beardist-tap.json"))?;
    let config_str = fs_err::read_to_string(&config_path).wrap_err_with(|| {
        format!(
            "Failed to read tap config file at {}",
            config_path.display().to_string().cyan()
        )
    })?;
    let config: TapConfig = serde_json::from_str(&config_str).wrap_err_with(|| {
        format!(
            "Failed to parse config file at {}",
            config_path.display().to_string().cyan()
        )
    })?;
    Ok(config)
}

pub(crate) fn update_tap() -> eyre::Result<()> {
    let dry_run = std::env::var("DRY_RUN").is_ok();
    if dry_run {
        info!("Dry run {}", "enabled".bright_yellow());
    }
    let github_token =
        std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN environment variable not set");

    info!("Loading tap {}...", "configuration".cyan());
    let config = load_tap_config()?;
    info!("Tap configuration loaded {}", "successfully".green());

    let client = Arc::new(Client::new());

    info!("Processing {}...", "formulas".bright_yellow());
    let mut bumped_formulas = Vec::new();
    for (index, formula) in config.formulas.iter().enumerate() {
        info!(
            "Processing formula {} of {}: {}",
            (index + 1).to_string().cyan(),
            config.formulas.len().to_string().cyan(),
            formula.name().cyan()
        );

        info!("Fetching GitHub {}...", "version".cyan());
        let github_version = formula.github_version(&config, &github_token)?;
        let github_version = match github_version {
            Some(version) => version,
            None => {
                info!("No version found for {}, skipping", formula.name().cyan());
                continue;
            }
        };

        info!("GitHub version: {}", github_version.green());

        let context = HomebrewContext::new(
            client.clone(),
            formula.clone(),
            github_version.clone(),
            dry_run,
        )?;

        if let Some(context) = context {
            info!("Updating formula for {}...", formula.name().bright_yellow());
            context.update_formula()?;
            info!(
                "Formula update completed for {}",
                formula.name().bright_green()
            );
            bumped_formulas.push((formula.name().to_string(), github_version));
        } else {
            info!("No update needed for {}", formula.name().bright_blue());
        }
    }
    info!("All formulas {}", "processed".bright_green());

    if !bumped_formulas.is_empty() {
        let commit_message = bumped_formulas
            .iter()
            .map(|(name, version)| format!("{} to {}", name, version))
            .collect::<Vec<String>>()
            .join(", ");

        let full_commit_message = format!("Bump formulas: {}", commit_message);

        info!("Committing changes...");
        if !dry_run {
            run_command("git", &["add", "."], None)?;
            run_command(
                "git",
                &["commit", "-m", &full_commit_message],
                Some(indexmap::indexmap! {
                    "GIT_AUTHOR_NAME".to_string() => "beardist".to_string(),
                    "GIT_AUTHOR_EMAIL".to_string() => "amos@bearcove.eu".to_string(),
                    "GIT_COMMITTER_NAME".to_string() => "beardist".to_string(),
                    "GIT_COMMITTER_EMAIL".to_string() => "amos@bearcove.eu".to_string(),
                }),
            )?;
            info!("Changes committed successfully");
        } else {
            info!(
                "Dry run: Would commit changes with message: {}",
                full_commit_message.cyan()
            );
        }

        info!("Formulas bumped:");
        for (name, version) in bumped_formulas {
            info!("  {} to version {}", name.cyan(), version.green());
        }

        info!("Pushing changes...");
        let remote_output = get_trimmed_cmd_stdout("git", &["remote", "-v"], None)?;
        let remote_url = remote_output
            .lines()
            .find(|line| line.contains("(push)"))
            .and_then(|line| {
                let url = line.split_whitespace().nth(1)?;
                // Convert SSH URLs to HTTPS URLs
                if url.starts_with("git@github.com:") {
                    let repo_path = url.trim_start_matches("git@github.com:");
                    Some(format!("https://github.com/{}", repo_path))
                } else {
                    Some(url.to_string())
                }
            })
            .ok_or_else(|| eyre::eyre!("Failed to get remote URL"))?;

        let org_repo = remote_url
            .trim_start_matches("https://")
            .trim_end_matches(".git")
            .split('/')
            .skip(1)
            .take(2)
            .collect::<Vec<&str>>()
            .join("/");

        info!("Remote URL: {}", remote_url.cyan());
        info!("Organization/Repo: {}", org_repo.cyan());

        let mut push_url = Url::parse(&remote_url)?;
        push_url.set_username("token").unwrap();
        push_url.set_password(Some(&github_token)).unwrap();

        if !dry_run {
            run_command("git", &["push", push_url.as_str(), "HEAD:main"], None)?;
            info!("Changes pushed successfully");
        } else {
            info!("Dry run: Would push changes to remote repository");
            info!("Push command that would be executed:");
            let mut redacted_url = push_url.clone();
            redacted_url.set_password(Some("REDACTED")).unwrap();
            info!("git push {} HEAD:main", redacted_url.to_string().cyan());
        }
    } else {
        info!("No formulas were bumped");
    }
    Ok(())
}
