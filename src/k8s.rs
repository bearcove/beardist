use ignore::WalkBuilder;
use log::info;
use owo_colors::OwoColorize;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::github::{GitHubClient, PackageType};

#[derive(Debug, Clone)]
struct ImageOccurrence {
    start: usize,
    end: usize,
    current_version: String,
    context: String,
}

#[derive(Debug, Clone)]
struct Manifest {
    path: PathBuf,
    occurrences: Vec<ImageOccurrence>,
}

#[derive(Debug, Clone)]
struct Workspace {
    manifests: Vec<Manifest>,
}

fn collect_workspace(manifest_dir: &Path, image: &str) -> Result<Workspace, std::io::Error> {
    let search_regex =
        Regex::new(&format!(r"image:\s*code\.bearcove\.cloud/{}:(\S+)", image)).unwrap();
    let manifests = Arc::new(std::sync::Mutex::new(Vec::new()));

    WalkBuilder::new(manifest_dir)
        .types(
            ignore::types::TypesBuilder::new()
                .add_defaults()
                .build()
                .unwrap(),
        )
        .build_parallel()
        .run(|| {
            let search_regex = search_regex.clone();
            let manifests = Arc::clone(&manifests);
            Box::new(move |result| {
                if let Ok(entry) = result {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                        || path.extension().and_then(|s| s.to_str()) == Some("yml")
                    {
                        if let Ok(contents) = fs_err::read_to_string(path) {
                            let mut occurrences = Vec::new();
                            for captures in search_regex.captures_iter(&contents) {
                                let full_match = captures.get(0).unwrap();
                                let version = captures.get(1).unwrap();
                                let start = full_match.start();
                                let end = full_match.end();

                                let lines: Vec<&str> = contents.lines().collect();
                                let line_number = contents[..start].lines().count();
                                let context_start = line_number.saturating_sub(2);
                                let context_end = (line_number + 3).min(lines.len());
                                let context = lines[context_start..context_end].join("\n");

                                occurrences.push(ImageOccurrence {
                                    start,
                                    end,
                                    current_version: version.as_str().to_string(),
                                    context,
                                });
                            }
                            if !occurrences.is_empty() {
                                manifests.lock().unwrap().push(Manifest {
                                    path: path.to_path_buf(),
                                    occurrences,
                                });
                            }
                        }
                    }
                }
                ignore::WalkState::Continue
            })
        });

    Ok(Workspace {
        manifests: manifests.lock().unwrap().clone(),
    })
}

pub(crate) fn k8s(args: crate::DeployArgs) -> eyre::Result<()> {
    let manifest_dir = Path::new("manifests");
    info!(
        "Searching for manifests in: {}",
        manifest_dir.display().bright_cyan()
    );
    let workspace = collect_workspace(manifest_dir, &args.image)?;

    let (org, package_name) = match args.image.split_once('/') {
        Some((org, name)) if !org.is_empty() && !name.is_empty() => (org, name),
        _ => {
            return Err(eyre::eyre!("Invalid image format. Expected 'org/name'."));
        }
    };

    info!("YAML files containing '{}' are:", args.image.bright_cyan());
    for manifest in &workspace.manifests {
        info!("File: {}", manifest.path.display().bright_green());
        for occurrence in &manifest.occurrences {
            info!(
                "  Version {} at positions {} to {}",
                occurrence.current_version.bright_yellow(),
                occurrence.start,
                occurrence.end
            );
            info!("  Context:");
            for (i, line) in occurrence.context.lines().enumerate() {
                let prefix = ">>> ".bright_cyan();
                if i == 1 {
                    info!("    {}{}", prefix, line.bright_yellow());
                } else {
                    info!("    {}{}", prefix, line);
                }
            }
            info!("");
        }
    }

    info!("Initializing GitHub client...");
    let github_client = GitHubClient::from_env()?;

    info!("Checking for new versions...");
    let mut spinner = ['|', '/', '-', '\\'].iter().cycle();
    let mut last_check_time = std::time::Instant::now();
    let new_version = loop {
        let latest_version =
            github_client.get_latest_version(org, package_name, PackageType::Container)?;

        if let Some(version) = latest_version {
            // Skip versions that end with -amd64 or -arm64
            if version.ends_with("-amd64") || version.ends_with("-arm64") {
                info!("Skipping architecture-specific version: {}", version);
                std::thread::sleep(std::time::Duration::from_secs(1));
                last_check_time = std::time::Instant::now();
                continue;
            }

            let is_new_version = workspace.manifests.iter().any(|manifest| {
                manifest
                    .occurrences
                    .iter()
                    .any(|occurrence| occurrence.current_version != version)
            });

            if is_new_version {
                eprintln!("\r\x1B[KNew version detected: {}", version.bright_green());
                break version;
            }
        }

        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let elapsed = last_check_time.elapsed();
            eprint!(
                "\r\x1B[K{} Checking for new versions... Last checked: {}",
                spinner.next().unwrap().bright_cyan(),
                format!(
                    "{:02}:{:02} ago",
                    elapsed.as_secs() / 60,
                    elapsed.as_secs() % 60
                )
                .bright_yellow()
            );
            if elapsed >= std::time::Duration::from_secs(2) {
                eprint!(
                    "\r                                                                                      \r"
                );
                break;
            }
        }
        last_check_time = std::time::Instant::now(); // Update last_check_time after each check
    };

    info!("Updating manifests...");

    for manifest in &workspace.manifests {
        let mut contents = fs_err::read_to_string(&manifest.path)?;
        for occurrence in &manifest.occurrences {
            let before = &contents[..occurrence.start];
            let after = &contents[occurrence.end..];
            let new_image_line =
                format!("image: code.bearcove.cloud/{}:{}", args.image, new_version);
            contents = format!("{}{}{}", before, new_image_line, after);
        }
        fs_err::write(&manifest.path, contents)?;
        info!("Updated {}", manifest.path.display().bright_green());
    }

    info!("Deploying manifests...");
    let mut deploy_cmd = std::process::Command::new("./deploy");

    // Add all updated manifest paths as arguments
    for manifest in &workspace.manifests {
        deploy_cmd.arg(manifest.path.as_os_str());
    }

    deploy_cmd
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()?
        .wait()?;

    info!("Deployment process completed successfully.");
    Ok(())
}
