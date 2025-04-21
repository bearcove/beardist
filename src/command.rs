use color_eyre::owo_colors::OwoColorize;
use eyre::Context;
use indexmap::IndexMap;
use log::*;
use owo_colors::Style;
use std::process::{Command, Stdio};

pub(crate) fn run_command(
    command: &str,
    args: &[&str],
    env: Option<IndexMap<String, String>>,
) -> eyre::Result<()> {
    use std::process::{Command, Stdio};
    use std::time::Instant;

    debug!(
        "🚀 Running command: {} {}",
        command.cyan(),
        args.join(" ").cyan()
    );

    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(env_vars) = env {
        cmd.envs(env_vars);
    }

    let start_time = Instant::now();
    let status = cmd.status()?;
    let duration = start_time.elapsed();

    let status_icon = if status.success() { "✅" } else { "❌" };
    let status_message = if status.success() {
        "executed successfully"
    } else {
        "failed"
    };
    let status_style = if status.success() {
        Style::new().green()
    } else {
        Style::new().red()
    };

    let log_message = format!(
        "{} Command '{}' with args '{}' {} in {:.2?} with status code {}",
        status_icon,
        command.cyan(),
        args.join(" ").cyan(),
        status_style.style(status_message),
        duration.cyan(),
        status.code().unwrap_or(-1).to_string().yellow()
    );

    if status.success() {
        debug!("{}", log_message);
    } else {
        error!("{}", log_message);
    }
    if !status.success() {
        error!("We really needed that command to work, so we're going to bail out now. Buh-bye.",);
        std::process::exit(status.code().unwrap_or(-1));
    }

    Ok(())
}

pub(crate) fn get_cmd_stdout(
    command: &str,
    args: &[&str],
    env: Option<IndexMap<String, String>>,
) -> eyre::Result<String> {
    debug!(
        "🚀 Running command: {} {}",
        command.cyan(),
        args.join(" ").cyan()
    );

    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(env_vars) = env {
        cmd.envs(env_vars);
    }

    let output = cmd
        .output()
        .wrap_err_with(|| format!("while running {} {}", command.cyan(), args.join(" ").cyan()))?;

    if !output.status.success() {
        error!(
            "Command failed with exit code {}",
            output.status.code().unwrap_or(-1)
        );
        eprintln!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR: {}", String::from_utf8_lossy(&output.stderr));
        std::process::exit(output.status.code().unwrap_or(-1));
    }

    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout)
}

pub(crate) fn get_trimmed_cmd_stdout(
    command: &str,
    args: &[&str],
    env: Option<IndexMap<String, String>>,
) -> eyre::Result<String> {
    let stdout = get_cmd_stdout(command, args, env)?;
    Ok(stdout.trim().to_string())
}
