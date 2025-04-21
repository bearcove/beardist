#![deny(clippy::disallowed_methods)]

use camino::Utf8PathBuf;
use cargo::{CargoBuildContext, CargoConfig};
use clap::{Parser, Subcommand};
use command::run_command;
use eyre::{self, Context, Result};
use homebrew::update_tap;
use log::*;
use owo_colors::OwoColorize;
use rand::seq::IndexedRandom;
use semver::{BuildMetadata, Prerelease, Version};
use serde::{Deserialize, Serialize};
use std::{env, os::unix::fs::PermissionsExt, path::PathBuf};
use target_spec::TargetSpec;
use tempfile::TempDir;

pub(crate) mod github;

mod cargo;
pub(crate) mod command;
mod homebrew;
mod system;
pub(crate) mod target_spec;

mod utils;
pub use utils::*;

mod k8s;

mod indented_writer;
pub(crate) use indented_writer::*;

/// CLI interface for beardist
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The subcommand to execute
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands for beardist
#[derive(Subcommand)]
enum Commands {
    /// Build the project, create a package, and upload it to github
    Build,
    /// Bump the version number and create a new git tag
    Bump(BumpArgs),
    /// Bump k8s manifests and run `./deploy-manifests`
    K8s(DeployArgs),
    /// Update a Homebrew tap containing a `.beardist-tap.json`
    UpdateTap,
}

/// Arguments for the Bump command
#[derive(Parser)]
struct BumpArgs {
    /// Type of version bump (major, minor, or patch)
    #[arg(value_enum)]
    bump_type: Option<BumpType>,
}

#[derive(clap::ValueEnum, Clone)]
enum BumpType {
    Major,
    Minor,
    Patch,
}

/// Arguments for the Deploy command
#[derive(Parser)]
struct DeployArgs {
    /// The name of the image to deploy, e.g. "bearcove/home" (`ghcr.io` is implied)
    image: String,
}

pub const CONFIG_VERSION: u64 = 3;
pub const USER_AGENT: &str = "github.com/bearcove/beardist@1.0";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    /// Version of beardist required
    version: u64,

    /// Organization or user name
    org: String,

    /// Project name
    name: String,

    cargo: Option<CargoConfig>,
    custom: Option<CustomConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomConfig {
    /// Any custom build steps to run (`bun build` etc.)
    #[serde(default)]
    steps: Vec<Vec<String>>,

    /// Any other files to include in the archive. You don't need to specify cargo binaries
    /// here. This is for data files.
    #[serde(default)]
    files: Vec<String>,
}

/// Context for `build` subcommand
struct BuildContext {
    /// Configuration for the project (read from .beardist.json)
    config: Config,

    /// URL of the github server
    github_server_url: String,

    /// github read-write API token
    github_rw_token: String,

    /// The git tag we're reacting to (in CI)
    tag: String,

    /// Flag indicating whether this is a dry run
    is_dry_run: bool,

    /// `$BEARDIST_CACHE_DIR`
    cache_dir: Utf8PathBuf,

    /// the source directory we're building (initially, the current directory)
    source_dir: Utf8PathBuf,

    /// Temporary directory for package archive
    temp_dir: TempDir,

    /// `$BEARDIST_ARTIFACT_NAME`
    artifact_name: String,
}

#[derive(Debug)]
enum PackagedFileKind {
    /// Mach-O/PE/ELF, etc.
    Bin,
    /// .dylib, .so, etc.
    Lib,
    /// anything else, really
    Misc,
}

struct PackagedFile {
    kind: PackagedFileKind,

    /// absolute path on disk — for now the archives are all flat.
    path: Utf8PathBuf,
}

impl BuildContext {
    fn new(config: Config) -> Result<Self> {
        let source_dir =
            camino::Utf8PathBuf::from_path_buf(env::current_dir()?.canonicalize()?).unwrap();
        info!(
            "🏗️ Building project from: {}",
            source_dir.to_string().cyan()
        );

        info!("");

        // BEARDIST_CACHE_DIR must be set to point to persistent storage
        // This tool is meant to be run in CI, and we want the cache
        // to use a persistent location for faster builds
        // We'll place rustup home, cargo home, target directory, etc. in this cache
        let cache_dir = env::var("BEARDIST_CACHE_DIR")
            .map(Utf8PathBuf::from)
            .map_err(|_| {
                eyre::eyre!(
                    "{} is not set. It should point to persistent storage for CI builds. This is where we'll store rustup home, cargo home, target directory, etc.",
                    "BEARDIST_CACHE_DIR".cyan()
                )
            })?;

        if !cache_dir.try_exists().unwrap_or(false) {
            fs_err::create_dir_all(&cache_dir)?;
        }

        fs_err::set_permissions(&cache_dir, std::fs::Permissions::from_mode(0o755))?;

        let cache_messages = [
            "🍭 that's where we hide the goodies",
            "🕵️ our secret stash of bits and bytes",
            "💎 the treasure trove of cached wonders",
            "✨ where the magic happens behind the scenes",
            "🎡 our digital playground",
            "🏞️ the land of cached opportunities",
            "💭 where code dreams come true",
            "🏰 the fortress of solitude for our builds",
            "🏠 our cozy little corner of the disk",
            "🐾 where we keep our digital pets",
            "🎭 the VIP lounge for our data",
            "☕ our code's favorite hangout spot",
            "🌼 the secret garden of compilation",
            "🏖️ where bits go for vacation",
            "🍽️ the all-you-can-cache buffet",
            "🕹️ our digital gaming den",
            "🚪 the magical wardrobe of our build process",
            "📚 where we store our binary collection",
            "🏨 the Hotel California of data (you can checkout anytime you like...)",
            "⏰ our code's time machine",
            "🦇 the Batcave of our build system",
            "🍯 where we keep the good stuff",
            "🏦 our digital Swiss bank account",
            "🏫 the School of Caching",
            "🍰 our little slice of binary heaven",
        ];
        let cache_message = cache_messages.choose(&mut rand::rng()).unwrap();

        info!("🔍 Cache {} — {}", cache_dir.cyan(), cache_message.yellow());

        let mut is_dry_run = false;

        let github_rw_token = match env::var("GH_READWRITE_TOKEN") {
            Ok(token) => {
                info!(
                    "{} is set: {}",
                    "GH_READWRITE_TOKEN".cyan(),
                    format_secret(&token)
                );
                token
            }
            Err(_) => {
                is_dry_run = true;
                "placeholder_token".to_string()
            }
        };

        let maybe_tag = env::var("GITHUB_REF").ok().and_then(|ref_str| {
            info!("{} is set: {}", "GITHUB_REF".cyan(), ref_str);
            ref_str.strip_prefix("refs/tags/").map(String::from)
        });
        let tag = match maybe_tag {
            Some(t) => t,
            None => {
                is_dry_run = true;
                warn!(
                    "{} is not set or invalid, falling back to placeholder",
                    "GITHUB_REF".cyan()
                );
                "vX.Y.Z".to_string()
            }
        };

        let github_server_url = match env::var("GITHUB_SERVER_URL") {
            Ok(url) => url,
            Err(_) => {
                warn!(
                    "{} is not set, falling back to default",
                    "GITHUB_SERVER_URL".cyan()
                );
                "https://github.com".to_string()
            }
        };

        let temp_dir = TempDir::new()?;

        let artifact_name_var = "BEARDIST_ARTIFACT_NAME";
        let artifact_name = std::env::var(artifact_name_var).unwrap_or_else(|_| {
            if std::env::var("CI").is_ok() {
                error!(
                    "beardist expects ${} to be set to determine the custom package name to upload",
                    artifact_name_var
                );
                error!(
                    "example values include 'x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', etc."
                );
                error!(
                    "
                    you can run the following command to find our triplet easily:
                "
                );
                error!(
                    r#"
                    rustc +stable --print target-libdir | sed -E 's/.*stable-([^/]+).*/\1/'
                "#
                );
                panic!("${artifact_name_var} must be set in CI environment")
            } else {
                let output = command::get_trimmed_cmd_stdout(
                    "rustc",
                    &["+stable", "--print", "target-libdir"],
                    None,
                )
                .expect("Failed to execute rustc command");
                let triplet = output
                    .split('/')
                    .find(|s| s.contains("stable-"))
                    .and_then(|s| s.strip_prefix("stable-"))
                    .expect("Failed to extract triplet from rustc output");
                info!(
                    "Automatically determined artifact name: {}",
                    triplet.cyan()
                );
                triplet.to_string()
            }
        });

        let cx = Self {
            artifact_name,
            config,
            cache_dir,
            github_server_url,
            github_rw_token,
            tag,
            is_dry_run,
            source_dir,
            temp_dir,
        };
        Ok(cx)
    }

    fn create_package_archive(
        &self,
        files_to_package: &[PackagedFile],
    ) -> Result<camino::Utf8PathBuf> {
        let artifact_name = &self.artifact_name;
        let package_file = camino::Utf8PathBuf::from_path_buf(
            self.temp_dir.path().join(format!("{artifact_name}.tar.xz")),
        )
        .unwrap();

        info!(
            "📦 Packaging {} with {} files:",
            package_file.cyan(),
            files_to_package.len().to_string().yellow()
        );
        for file in files_to_package {
            let file_size = fs_err::metadata(&file.path)?.len();
            info!(
                "  - {} {}",
                file.path.file_name().unwrap().to_string().blue(),
                format!("({})", format_bytes(file_size)).green()
            );
        }

        let tar_args = files_to_package
            .iter()
            .flat_map(|f| {
                vec![
                    "-C".to_string(),
                    f.path.parent().unwrap().to_string(),
                    f.path.file_name().unwrap().to_string(),
                ]
            })
            .collect::<Vec<_>>()
            .join(" ");

        let archive_command = format!(
            "tar --create --verbose --file=- {} | xz -2 --threads=0 --stdout > {}",
            tar_args, package_file
        );
        run_command("bash", &["-euo", "pipefail", "-c", &archive_command], None)?;

        Ok(package_file)
    }

    fn upload_package(
        &self,
        package_file: &camino::Utf8Path,
        file_content: &[u8],
        files_to_package: &[PackagedFile],
    ) -> Result<()> {
        let org = &self.config.org;
        let name = &self.config.name;
        let tag = &self.tag;
        let package_file_name = package_file.file_name().unwrap();
        assert!(!package_file_name.contains('/'));

        const INSPECT_OUTPUT_DIR: &str = "/tmp/beardist-output";
        let _ = fs_err::remove_dir_all(INSPECT_OUTPUT_DIR);
        fs_err::create_dir_all(INSPECT_OUTPUT_DIR)?;
        for file in files_to_package {
            let dest_path = format!("{}/{}", INSPECT_OUTPUT_DIR, file.path.file_name().unwrap());
            fs_err::copy(&file.path, &dest_path)?;
            info!(
                "📄 Copied {} to {}",
                file.path.to_string().cyan(),
                dest_path.bold().underline()
            );
        }
        info!(
            "📁 All files copied to: {}",
            INSPECT_OUTPUT_DIR.bold().underline()
        );

        const INSPECT_OUTPUT_PATH: &str = "/tmp/beardist-output.tar.xz";
        fs_err::write(INSPECT_OUTPUT_PATH, file_content)?;
        info!(
            "📦 {} package written to: {}",
            format_bytes(file_content.len() as _).blue(),
            INSPECT_OUTPUT_PATH.bold().underline()
        );
        if file_content.len() < 10 * 1024 {
            return Err(eyre::eyre!(
                "Suspiciously small package size ({}). Aborting.",
                format_bytes(file_content.len() as _)
            ));
        }

        if self.is_dry_run {
            warn!("Not uploading (dry run)");
            return Ok(());
        }

        // Create a release if it doesn't exist
        let client = reqwest::blocking::Client::new();
        let github_api_url = format!(
            "{}/repos/{}/{}/releases/tags/{}",
            self.github_server_url
                .replace("github.com", "api.github.com"),
            org,
            name,
            tag
        );

        info!(
            "🔍 Checking if release exists at {}...",
            github_api_url.cyan()
        );

        let release_response = client
            .get(&github_api_url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {}", self.github_rw_token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", USER_AGENT)
            .send()?;

        let release_id = if !release_response.status().is_success() {
            info!("📝 Release doesn't exist, creating one...");

            let release_create_url = format!(
                "{}/repos/{}/{}/releases",
                self.github_server_url
                    .replace("github.com", "api.github.com"),
                org,
                name
            );

            let release_create_body = serde_json::json!({
                "tag_name": tag,
                "name": tag,
                "draft": false,
                "prerelease": false
            });

            let create_response = client
                .post(&release_create_url)
                .header("Accept", "application/vnd.github+json")
                .header("Authorization", format!("Bearer {}", self.github_rw_token))
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

        // Upload the asset to the release
        let upload_url = format!(
            "{}/repos/{}/{}/releases/{}/assets?name={}",
            self.github_server_url
                .replace("github.com", "uploads.github.com"),
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

            match client
                .post(&upload_url)
                .header("Accept", "application/vnd.github+json")
                .header("Authorization", format!("Bearer {}", self.github_rw_token))
                .header("X-GitHub-Api-Version", "2022-11-28")
                .header("User-Agent", USER_AGENT)
                .header("Content-Type", "application/octet-stream")
                .body(file_content.to_vec())
                .send()
            {
                Ok(response) => {
                    info!("🔢 Response status code: {}", response.status().blue());

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

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") }
    }
    env_logger::builder()
        .format_timestamp(None)
        .format_target(false)
        .format_level(false) // would be nice for non-info, but shrug
        .init();
    color_eyre::install()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Build => build()?,
        Commands::Bump(args) => bump(args)?,
        Commands::UpdateTap => update_tap()?,
        Commands::K8s(args) => k8s::k8s(args)?,
    }

    Ok(())
}

fn bump(args: BumpArgs) -> Result<()> {
    // Check for unstaged changes
    let status = command::get_trimmed_cmd_stdout("git", &["status", "--porcelain"], None)?;
    if !status.is_empty() {
        info!("There are unstaged changes:");
        for line in status.lines() {
            info!("  {}", line);
        }
        info!("Do you want to stage these changes? (y/n)");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() == "y" {
            run_command("git", &["add", "."], None)?;
            info!("Changes staged.");
        }
    }

    // Check for uncommitted changes
    let status = command::get_trimmed_cmd_stdout("git", &["status", "--short"], None)?;
    if !status.is_empty() {
        info!("There are uncommitted changes:");
        for line in status.lines() {
            info!("  {}", line);
        }
        info!("Do you want to commit these changes? (y/n)");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() == "y" {
            info!("Enter commit message:");
            let mut message = String::new();
            std::io::stdin().read_line(&mut message)?;
            run_command("git", &["commit", "-m", message.trim()], None)?;
            info!("Changes committed.");
        }
    }

    // Check for unpushed commits
    let unpushed = command::get_trimmed_cmd_stdout("git", &["log", "@{u}..", "--oneline"], None)?;
    if !unpushed.is_empty() {
        info!("There are unpushed commits:");
        for line in unpushed.lines() {
            info!("  {}", line);
        }
        info!("Do you want to push these commits? (y/n)");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() == "y" {
            run_command("git", &["push"], None)?;
            info!("Commits pushed.");
        }
    }

    // Fetch all tags
    run_command("git", &["fetch", "--tags"], None)?;
    info!("Fetched all tags from remote.");

    // Get all tags sorted by version (newest to oldest)
    let output = command::get_trimmed_cmd_stdout("git", &["tag", "--sort=-version:refname"], None)?;
    let tags: Vec<String> = output.lines().map(String::from).collect();

    if tags.is_empty() {
        return Err(eyre::eyre!("No tags found"));
    }

    let latest_tag = &tags[0];
    info!("Latest tag: {}", latest_tag);

    // Parse the latest tag
    let latest_version = semver::Version::parse(latest_tag.trim_start_matches('v'))?;

    let patch_bump = Version {
        major: latest_version.major,
        minor: latest_version.minor,
        patch: latest_version.patch + 1,
        pre: Prerelease::EMPTY,
        build: BuildMetadata::EMPTY,
    };
    let minor_bump = Version {
        major: latest_version.major,
        minor: latest_version.minor + 1,
        patch: 0,
        pre: Prerelease::EMPTY,
        build: BuildMetadata::EMPTY,
    };
    let major_bump = Version {
        major: latest_version.major + 1,
        minor: 0,
        patch: 0,
        pre: Prerelease::EMPTY,
        build: BuildMetadata::EMPTY,
    };

    let new_version = if let Some(bt) = args.bump_type {
        match bt {
            BumpType::Patch => patch_bump,
            BumpType::Minor => minor_bump,
            BumpType::Major => major_bump,
        }
    } else {
        // Ask user for bump type
        info!("Choose version bump type:");
        info!(
            "1. Patch ({}.{}.{})",
            patch_bump.major,
            patch_bump.minor,
            patch_bump.patch.to_string().green()
        );
        info!(
            "2. Minor ({}.{}.{})",
            minor_bump.major,
            minor_bump.minor.to_string().green(),
            minor_bump.patch.to_string().green()
        );
        info!(
            "3. Major ({}.{}.{})",
            major_bump.major.to_string().green(),
            major_bump.minor.to_string().green(),
            major_bump.patch.to_string().green()
        );

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        match input.trim() {
            "1" => patch_bump,
            "2" => minor_bump,
            "3" => major_bump,
            _ => return Err(eyre::eyre!("Invalid choice")),
        }
    };

    let new_tag = format!("v{}", new_version);
    info!("Creating new tag: {}", new_tag);

    // Create and push the new tag
    run_command("git", &["tag", &new_tag], None)?;
    run_command("git", &["push", "origin", &new_tag], None)?;

    info!("Tag {} created and pushed successfully", new_tag);

    Ok(())
}

fn print_banner() {
    let art = r#"    __         __
   /  \.-"""-./  \
   \    -   -    /   ============
    |   o   o   |      beardist
    \  .-'''-.  /    ============
     '-\__Y__/-'
        `---`"#;
    for line in art.lines() {
        info!("{}", line.dimmed());
    }
}

fn build() -> Result<()> {
    print_banner();
    let start_time = std::time::Instant::now();
    let config = load_config()?;
    let mut cx = BuildContext::new(config)?;

    info!(
        "📦 Building {}/{}",
        cx.config.org.blue(),
        cx.config.name.green(),
    );

    system::print_sysinfo();

    let cargo = cx
        .config
        .cargo
        .take()
        .map(|cc| CargoBuildContext::new(&cx, cc))
        .transpose()?;

    let mut files_to_package: Vec<PackagedFile> = Vec::new();

    let build_start = std::time::Instant::now();
    if let Some(cargo) = cargo.as_ref() {
        cargo.build(&mut files_to_package)?;
    }

    if let Some(custom) = cx.config.custom.as_ref() {
        info!("📋 Executing custom build steps");
        for (index, step) in custom.steps.iter().enumerate() {
            let step = step.iter().map(|s| s.as_str()).collect::<Vec<_>>();
            info!(
                "🔧 Running custom step {}: {}",
                index + 1,
                step.join(" ").cyan()
            );
            run_command(step[0], &step[1..], None)?;
        }

        info!("📁 Adding custom files to package");
        for file in &custom.files {
            let path = cx.source_dir.join(file);
            info!("➕ Adding file: {}", path.to_string().cyan());
            files_to_package.push(PackagedFile {
                kind: PackagedFileKind::Misc,
                path,
            });
        }
    }
    let build_time = build_start.elapsed().as_millis() as u64;
    info!("🔨 Built in {}", format!("{}ms", build_time).green());

    info!("{}", "----------------------------------------".dimmed());

    let package_file = cx.create_package_archive(&files_to_package)?;
    let archive_time = std::time::Instant::now().elapsed().as_millis() as u64;
    let file_content = fs_err::read(&package_file)?;
    let upload_start = std::time::Instant::now();
    cx.upload_package(&package_file, &file_content, &files_to_package)?;
    let upload_time = upload_start.elapsed().as_millis() as u64;

    if let Some(cargo) = cargo.as_ref() {
        cargo.sweep()?;
    }

    let total_time = start_time.elapsed().as_millis();
    info!(
        "📊 Summary: 🔨 Build: {}ms | 📦 Archive: {}ms{} | ⏱️ Total: {}ms",
        build_time.to_string().cyan(),
        archive_time.to_string().cyan(),
        if !cx.is_dry_run {
            format!(" | 📤 Upload: {}ms", upload_time.to_string().cyan())
        } else {
            String::new()
        },
        total_time.to_string().green()
    );

    Ok(())
}

fn load_config() -> Result<Config> {
    let config_path = fs_err::canonicalize(PathBuf::from(".beardist.json"))?;
    let config_str = fs_err::read_to_string(&config_path).wrap_err_with(|| {
        format!(
            "Failed to read config file at {}",
            config_path.display().to_string().cyan()
        )
    })?;
    let config: Config = serde_json::from_str(&config_str).wrap_err_with(|| {
        format!(
            "Failed to parse config file at {}",
            config_path.display().to_string().cyan()
        )
    })?;
    if config.version != CONFIG_VERSION {
        return Err(eyre::eyre!(
            "Invalid beardist config version: {}. Expected: {} (in file {})",
            config.version,
            CONFIG_VERSION,
            config_path.display().to_string().cyan()
        ));
    }
    Ok(config)
}
