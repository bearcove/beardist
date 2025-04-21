use color_eyre::owo_colors::OwoColorize;

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["Bytes", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 Byte".to_string();
    }
    let i = (bytes as f64).log(1024.0).floor() as usize;
    format!(
        "{:.2} {}",
        (bytes as f64) / 1024f64.powi(i as i32),
        UNITS[i]
    )
}

pub fn format_secret(secret: &str) -> String {
    if secret.len() >= 4 {
        format!(
            "{}{}{}",
            &secret[..2],
            "...".dimmed(),
            &secret[secret.len() - 2..]
        )
    } else {
        "(too short)".dimmed().to_string()
    }
}
