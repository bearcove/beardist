use camino::Utf8PathBuf;
use indexmap::IndexMap;
use log::{debug, error, info, warn};
use owo_colors::{OwoColorize, Style};
use serde::{Deserialize, Serialize};

use crate::{BuildContext, PackagedFile, PackagedFileKind, TargetSpec, command};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CargoConfig {
    /// Name of binaries we should pack
    pub(crate) bins: Vec<String>,
}

/// builds values for RUSTUP_HOME, CARGO_HOME, etc.
struct BuildEnv {
    /// a cache dir we can use, that persists between builds.
    /// we can store rustup and cargo toolchains in there, the timelord cache, etc.
    cache_dir: Utf8PathBuf,
}

impl BuildEnv {
    fn cargo_home(&self) -> Utf8PathBuf {
        self.cache_dir.join("cargo")
    }

    fn rustup_home(&self) -> Utf8PathBuf {
        self.cache_dir.join("rustup")
    }

    fn pnpm_cache_folder(&self) -> Utf8PathBuf {
        self.cache_dir.join("pnpm")
    }

    fn get_env(&self) -> IndexMap<String, String> {
        let mut env = IndexMap::new();
        env.insert("CARGO_HOME".to_string(), self.cargo_home().to_string());
        env.insert("RUSTUP_HOME".to_string(), self.rustup_home().to_string());
        env.insert(
            "PNPM_CACHE_FOLDER".to_string(),
            self.pnpm_cache_folder().to_string(),
        );
        env.insert("CLICOLOR_FORCE".to_string(), "1".to_string());
        env.insert("FORCE_COLOR".to_string(), "1".to_string());
        env.insert("RUSTC_BOOTSTRAP".to_string(), "1".to_string());
        env.insert("RUSTFLAGS".to_string(), "-Z remap-cwd-prefix=.".to_string());
        env
    }
}

pub(crate) struct CargoBuildContext<'a> {
    /// the build context we're operating in
    parent: &'a BuildContext,

    /// the environment we're building in
    build_env: BuildEnv,

    /// the target we're building for
    target_spec: TargetSpec,

    /// the configuration for this build
    config: CargoConfig,
}

impl<'a> CargoBuildContext<'a> {
    pub(crate) fn new(parent: &'a BuildContext, config: CargoConfig) -> eyre::Result<Self> {
        let build_env = BuildEnv {
            cache_dir: parent.cache_dir.clone(),
        };

        info!("{}", "🌍 Environment:".yellow());
        let env = build_env.get_env();
        let max_key_len = env.keys().map(|k| k.len()).max().unwrap_or(0);
        for (key, value) in env.iter() {
            let formatted_value =
                if let Some(relative_path) = value.strip_prefix(build_env.cache_dir.as_str()) {
                    format!("{}{}", "$CACHE".dimmed(), relative_path)
                } else {
                    value.to_string()
                };

            info!(
                "  {:>width$}: {}",
                key.blue(),
                formatted_value,
                width = max_key_len + 2
            );
        }

        let rustc_version =
            command::get_trimmed_cmd_stdout("rustc", &["--version"], Some(env.clone()))?;
        let cargo_version =
            command::get_trimmed_cmd_stdout("cargo", &["--version"], Some(env.clone()))?;
        let cargo_sweep_version =
            command::get_trimmed_cmd_stdout("cargo", &["sweep", "--version"], Some(env))?;
        info!(
            "🔍 Toolchain: {} | {} | {}",
            rustc_version.red(),
            cargo_version.green(),
            cargo_sweep_version.blue()
        );

        info!("🔍 Rustup environment:");
        let rustup_show_output =
            command::get_trimmed_cmd_stdout("rustup", &["show"], Some(build_env.get_env()))?;
        for line in rustup_show_output.lines() {
            info!("  {line}");
        }

        let json_output = command::get_trimmed_cmd_stdout(
            "rustc",
            &["-Z", "unstable-options", "--print", "target-spec-json"],
            Some(build_env.get_env()),
        )?;
        let target_spec = TargetSpec::from_json(&json_output)?;
        target_spec.print_info();

        Ok(Self {
            parent,
            config,
            build_env,
            target_spec,
        })
    }

    pub(crate) fn build(&self, files_to_package: &mut Vec<PackagedFile>) -> eyre::Result<()> {
        self.run_timelord()?;
        self.build_project()?;

        for bin in &self.config.bins {
            let binary_path = self.cargo_out_dir().join(bin);
            if binary_path.exists() {
                let binary_size = fs_err::metadata(&binary_path)?.len();
                info!(
                    "✅ Produced {} binary at {}",
                    crate::format_bytes(binary_size).green(),
                    binary_path.to_string().cyan()
                );
                files_to_package.push(PackagedFile {
                    kind: PackagedFileKind::Bin,
                    path: binary_path,
                })
            } else {
                error!(
                    "❌ Binary file does not exist at path: {}",
                    binary_path.to_string().red()
                );
                panic!();
            }
        }

        let mut highlight_patterns = Vec::new();
        highlight_patterns.push((
            regex::Regex::new(r"(?i)(\.dylib|\.so|LC_RPATH|@rpath|@executable_path|\$ORIGIN)")
                .unwrap(),
            Style::new().blue(),
        ));
        for bin in &self.config.bins {
            highlight_patterns.push((
                regex::Regex::new(&format!(r"(?i)({})", regex::escape(bin))).unwrap(),
                Style::new().green(),
            ));
        }
        highlight_patterns.push((
            regex::Regex::new(&format!(
                r"(?i)({})",
                regex::escape(&self.target_spec.full_name())
            ))
            .unwrap(),
            Style::new().yellow(),
        ));

        debug!("📊 Running {} on rustc...", "target-libdir".dimmed());
        let target_libdir = command::get_trimmed_cmd_stdout(
            "rustc",
            &["--print", "target-libdir"],
            Some(self.get_env()),
        )?;
        debug!("📊 Target libdir: {}", target_libdir.cyan());

        let libstd_pattern =
            glob::Pattern::new(&format!("libstd-*{}", self.target_spec.dll_suffix))?;

        let libstd_path = fs_err::read_dir(target_libdir)?
            .filter_map(|entry| entry.ok())
            .find(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| libstd_pattern.matches(name))
            })
            .ok_or_else(|| eyre::eyre!("libstd not found in target libdir"))?;

        let libstd_size = libstd_path.metadata()?.len();
        debug!(
            "📊 Found libstd: {} ({})",
            libstd_path.file_name().to_str().unwrap().cyan(),
            crate::format_bytes(libstd_size).green()
        );

        let libstd_path: Utf8PathBuf = libstd_path.path().try_into().unwrap();

        let cargo_out_dir = self.cargo_out_dir();

        // Copy libstd next to the binary
        let libstd_copy_path = cargo_out_dir.join(libstd_path.file_name().unwrap());
        if self.target_spec.os == "linux" || self.target_spec.os == "macos" {
            // Remove any pre-existing symlinks or files, ignoring any errors
            let _ = fs_err::remove_file(&libstd_copy_path);

            // Copy the file
            fs_err::copy(&libstd_path, &libstd_copy_path)?;
            info!("📄 Copied libstd: {}", libstd_copy_path.to_string().cyan());
        } else {
            warn!(
                "Skipping libstd copy for unsupported OS: {}",
                self.target_spec.os
            );
        }

        // Add other libraries
        let dll_suffix = self.target_spec.dll_suffix.as_str();
        for entry in fs_err::read_dir(&cargo_out_dir)? {
            let entry = entry?;
            let file_name = entry.file_name().into_string().unwrap();
            debug!("Examining file: {file_name} (our dll_suffix is {dll_suffix})");
            if file_name.starts_with("lib") && file_name.ends_with(dll_suffix) {
                let file_path = entry.path();
                files_to_package.push(PackagedFile {
                    kind: PackagedFileKind::Lib,
                    path: file_path.try_into().unwrap(),
                });
            }
        }

        self.fix_install_names()?;

        if self.target_spec.os == "linux" {
            for file in files_to_package
                .iter()
                .filter(|f| matches!(f.kind, PackagedFileKind::Bin | PackagedFileKind::Lib))
            {
                show_fyi(
                    "ldd",
                    &[file.path.as_str()],
                    Some(self.get_env()),
                    &highlight_patterns,
                )?;

                show_fyi(
                    "readelf",
                    &["-d", file.path.as_str()],
                    Some(self.get_env()),
                    &highlight_patterns,
                )?;
            }
        } else if self.target_spec.os == "macos" {
            for file in files_to_package
                .iter()
                .filter(|f| matches!(f.kind, PackagedFileKind::Bin | PackagedFileKind::Lib))
            {
                show_fyi(
                    "otool",
                    &["-L", file.path.as_str()],
                    Some(self.get_env()),
                    &highlight_patterns,
                )?;
            }

            show_fyi(
                "bash",
                &[
                    "-c",
                    &format!(
                        "otool -l {}",
                        self.cargo_out_dir().join(&self.config.bins[0])
                    ),
                ],
                Some(self.get_env()),
                &highlight_patterns,
            )?;
        } else {
            warn!(
                "Skipping binary dependency check for unsupported OS: {}",
                self.target_spec.os
            );
        }

        info!(
            "📊 Running {} on {}...",
            "--version".dimmed(),
            self.cargo_out_dir()
                .join(&self.config.bins[0])
                .to_string()
                .cyan()
        );
        crate::run_command(
            self.cargo_out_dir().join(&self.config.bins[0]).as_str(),
            &["--version"],
            Some(self.get_env()),
        )?;

        Ok(())
    }

    fn run_timelord(&self) -> eyre::Result<()> {
        // Detect if we're in CI
        if std::env::var("CI").is_ok() {
            if std::env::var("SKIP_TIMELORD").is_ok() {
                info!("Skipping timelord ($SKIP_TIMELORD is set)");
            } else {
                // this manipulates timestamps on files to ensure that incremental builds work correctly
                timelord::sync(self.parent.source_dir.clone(), self.cargo_target_dir());
                info!("🕰️ Timelord sync completed in CI environment");
            }
        } else {
            info!("🏠 Not in CI environment, skipping Timelord sync");
        }
        Ok(())
    }

    fn get_env(&self) -> IndexMap<String, String> {
        let mut env = self.build_env.get_env();
        let mut additional_rustflags = Vec::new();

        if !additional_rustflags.is_empty() {
            env.entry("RUSTFLAGS".to_string()).and_modify(|e| {
                for flag in &additional_rustflags {
                    e.push(' ');
                    e.push_str(flag);
                }
            });
        }
        env.insert(
            "CARGO_TARGET_DIR".to_string(),
            self.cargo_target_dir().to_string(),
        );
        env
    }

    fn cargo_target_dir(&self) -> Utf8PathBuf {
        self.build_env
            .cache_dir
            .join("target")
            .join(&self.parent.config.org)
            .join(&self.parent.config.name)
            .join(self.target_spec.full_name())
    }

    /// ${TARGET}/${PROFILE}
    fn cargo_out_dir(&self) -> Utf8PathBuf {
        self.cargo_target_dir().join("release")
    }

    fn build_project(&self) -> eyre::Result<()> {
        info!("{}", "🔨 Building the project...".yellow());
        let env = self.get_env();
        crate::run_command("cargo", &["build", "--verbose", "--release"], Some(env))?;
        Ok(())
    }

    fn fix_install_names(&self) -> eyre::Result<()> {
        if self.target_spec.os != "macos" {
            return Ok(());
        }

        let deps_dir = self.cargo_out_dir().join("deps");

        if !deps_dir.exists() {
            warn!("deps directory not found: {}", deps_dir);
            return Ok(());
        }

        // Collect all dylib files we want to fix
        let mut dylibs_to_fix = Vec::new();

        // Build a hash set of all libraries we own
        let mut our_libraries = std::collections::HashSet::new();

        // Check target directory
        for entry in fs_err::read_dir(self.cargo_out_dir())? {
            let entry = entry?;
            let path = entry.path();
            let file_name = path.file_name().unwrap().to_string_lossy();

            if file_name.starts_with("lib") && file_name.ends_with(".dylib") {
                let is_symlink = path.is_symlink();
                our_libraries.insert(file_name.to_string());

                if is_symlink {
                    info!(
                        "Detected symlink: {}. Adding to our libraries but not fixing.",
                        path.display().to_string().cyan()
                    );
                } else {
                    match path.canonicalize() {
                        Ok(canonicalized_path) => {
                            info!(
                                "Adding {} to dylibs_to_fix",
                                canonicalized_path.display().to_string().cyan()
                            );
                            dylibs_to_fix.push(canonicalized_path);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to canonicalize path: {}. Error: {}",
                                path.display().to_string().red(),
                                e.to_string().red()
                            );
                            info!(
                                "Adding original path {} to dylibs_to_fix",
                                path.display().to_string().cyan()
                            );
                            dylibs_to_fix.push(path);
                        }
                    }
                }
            }
        }

        // Add binaries
        for binary_name in &self.config.bins {
            let binary_path = self.cargo_out_dir().join(binary_name);
            dylibs_to_fix.push(binary_path.canonicalize()?);
        }

        debug!("Our libraries: {:?}", our_libraries);

        // Fix each dylib
        for dylib_path in dylibs_to_fix {
            let file_name = dylib_path.file_name().unwrap().to_string_lossy();
            let dylib_path_str = dylib_path.to_string_lossy();
            debug!("🔍 Inspecting dependencies for: {}", file_name.cyan());

            let dependencies = get_dependencies(dylib_path_str.as_ref())?;

            // Formulate a plan: change only our libraries to use @rpath
            for dep in dependencies {
                let dep_name = dep.split('/').next_back().unwrap();
                debug!("Examining dependency: {}", dep.cyan());
                if our_libraries.contains(dep_name) {
                    let new_path = format!("@rpath/{}", dep_name);
                    debug!(
                        "🛠️ This is our library. Changing {} to {}",
                        dep.cyan(),
                        new_path.blue()
                    );
                    change_dep(dylib_path_str.as_ref(), &dep, &new_path)?;
                } else {
                    debug!("⏭️ This is not our library. Keeping as is: {}", dep.cyan());
                }
            }

            // Set the id of the dylib itself (skip for the main binary)
            if file_name.starts_with("lib") && file_name.ends_with(".dylib") {
                let id = format!("@rpath/{}", file_name);
                debug!("🛠️ Setting id for dylib: {}", id.blue());
                command::run_command(
                    "install_name_tool",
                    &["-id", &id, dylib_path_str.as_ref()],
                    None,
                )?;
            }

            // Verify changes
            let verify_deps = get_dependencies(dylib_path_str.as_ref())?;
            debug!(
                "✅ Verification output for {}:\n{}",
                file_name.cyan(),
                verify_deps.join("\n")
            );
        }
        Ok(())
    }

    pub(crate) fn sweep(&self) -> eyre::Result<()> {
        debug!("🧹 Running cargo sweep...");
        let env = self.get_env();
        crate::run_command("cargo", &["sweep", "--time", "30"], Some(env))?;
        Ok(())
    }
}

// Helper function to run otool -l and collect all dependencies
fn get_dependencies(path: &str) -> eyre::Result<Vec<String>> {
    let output = command::get_cmd_stdout("otool", &["-l", path], None)?;
    let mut dependencies = Vec::new();
    let mut in_load_dylib = false;
    let mut current_dependency;

    for line in output.lines() {
        if line.trim().starts_with("cmd LC_LOAD_DYLIB") {
            in_load_dylib = true;
        } else if in_load_dylib && line.trim().starts_with("name") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                current_dependency = parts[1].to_string();
                dependencies.push(current_dependency.clone());
            }
            in_load_dylib = false;
        }
    }

    Ok(dependencies)
}

// Helper function to run install_name_tool and verify the change
fn change_dep(path: &str, old_dep: &str, new_dep: &str) -> eyre::Result<()> {
    command::run_command(
        "install_name_tool",
        &["-change", old_dep, new_dep, path],
        None,
    )?;
    Ok(())
}

fn show_fyi(
    command: &str,
    args: &[&str],
    env: Option<IndexMap<String, String>>,
    highlight_patterns: &[(regex::Regex, Style)],
) -> eyre::Result<()> {
    let cmd_str = format!("{} {}", command, args.join(" "));
    info!("💅 FYI, {}", cmd_str.magenta());
    let output = command::get_cmd_stdout(command, args, env)?;
    for line in output.lines() {
        let mut highlighted_line = line.to_string();
        for (pattern, style) in highlight_patterns {
            highlighted_line = pattern
                .replace_all(&highlighted_line, |caps: &regex::Captures| {
                    caps[0].to_string().style(*style).to_string()
                })
                .to_string();
        }
        info!("    {}", highlighted_line);
    }
    Ok(())
}
