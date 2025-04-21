use log::info;
use owo_colors::OwoColorize;

pub(crate) fn print_sysinfo() {
    info!("{}", "🖥️ System Information:".yellow());

    // Detect if we're in a container
    let in_container = match fs_err::read_to_string("/etc/mtab") {
        Ok(content) => {
            if content.starts_with("overlay /") {
                info!("root fs is overlay, we're probably in a container");
                true
            } else {
                false
            }
        }
        Err(_) => false,
    };

    let mut sys_info = Vec::new();

    if in_container {
        sys_info.push(format!(
            "{} {}",
            "Environment".dimmed(),
            "Container".cyan().underline()
        ));

        // Check CPU quota
        if let Ok(cpu_max) = fs_err::read_to_string("/sys/fs/cgroup/cpu.max") {
            let parts: Vec<&str> = cpu_max.split_whitespace().collect();
            if parts.len() == 2 && parts[0] != "max" {
                if let (Ok(quota), Ok(period)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>())
                {
                    let cpu_limit = quota / period;
                    sys_info.push(format!(
                        "{} {}",
                        "CPU Quota".dimmed(),
                        format!("{:.2} CPUs", cpu_limit).cyan().underline()
                    ));
                }
            }
        }

        // Check Memory limit
        if let Ok(memory_max) = fs_err::read_to_string("/sys/fs/cgroup/memory.max") {
            let memory_max = memory_max.trim();
            if memory_max != "max" {
                if let Ok(memory_limit) = memory_max.parse::<u64>() {
                    sys_info.push(format!(
                        "{} {}",
                        "Memory Limit".dimmed(),
                        crate::format_bytes(memory_limit).cyan().underline()
                    ));
                }
            }
        }
    }

    // Always show host resources
    if let Ok(cpu_count) = sys_info::cpu_num() {
        sys_info.push(format!(
            "{} {}",
            "CPU Cores".dimmed(),
            cpu_count.to_string().cyan().underline()
        ));
    }

    // Hostname
    if let Ok(hostname) = hostname::get() {
        if let Some(hostname_str) = hostname.to_str() {
            sys_info.push(format!(
                "{} {}",
                "Hostname".dimmed(),
                hostname_str.cyan().underline()
            ));
        }
    }

    // Memory information
    if let Ok(mem_info) = sys_info::mem_info() {
        let used_memory = mem_info.total - mem_info.free;
        let usage_percentage = (used_memory as f64 / mem_info.total as f64) * 100.0;
        sys_info.push(format!(
            "{} {} out of {} total ({:.1}% used)",
            "Memory".dimmed(),
            crate::format_bytes(used_memory * 1024).cyan().underline(),
            crate::format_bytes(mem_info.total * 1024)
                .cyan()
                .underline(),
            usage_percentage
        ));
    }

    // Disk space
    if let Ok(disk_info) = sys_info::disk_info() {
        let used_space = disk_info.total - disk_info.free;
        let usage_percentage = (used_space as f64 / disk_info.total as f64) * 100.0;
        sys_info.push(format!(
            "{} {} out of {} total ({:.1}% used)",
            "Disk Space".dimmed(),
            crate::format_bytes(used_space * 1024).cyan().underline(),
            crate::format_bytes(disk_info.total * 1024)
                .cyan()
                .underline(),
            usage_percentage
        ));
    }

    // OS information
    let os_info = sys_info::os_type().unwrap_or_else(|_| "Unknown".to_string());
    let os_release = sys_info::os_release().unwrap_or_else(|_| "Unknown".to_string());
    sys_info.push(format!(
        "{} {} {}",
        "OS".dimmed(),
        os_info.cyan().underline(),
        os_release.cyan().underline()
    ));

    // Print system information
    info!("{}", sys_info.join(&" :: ".dimmed().to_string()));

    // Read and print /etc/os-release if it exists
    if let Ok(os_release_content) = fs_err::read_to_string("/etc/os-release") {
        let mut os_release_info = Vec::new();
        for line in os_release_content.lines() {
            if line.starts_with("NAME=") || line.starts_with("VERSION=") || line.starts_with("ID=")
            {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let value = parts[1].trim_matches('"');
                    os_release_info.push(format!(
                        "{} {}",
                        parts[0].dimmed(),
                        value.cyan().underline()
                    ));
                }
            }
        }
        if !os_release_info.is_empty() {
            info!(
                "{}: {}",
                "OS Release Details".dimmed(),
                os_release_info.join(&" :: ".dimmed().to_string())
            );
        }
    }
}
