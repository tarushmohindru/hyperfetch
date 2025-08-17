pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

pub fn format_speed(bytes_per_sec: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec as u64))
}

pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

pub fn extract_filename_from_url(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|segments| segments.last())
                .filter(|name| !name.is_empty())
                .map(|name| name.to_string())
        })
        .unwrap_or_else(|| "downloaded_file".to_string())
}

pub fn parse_size(size_str: &str) -> Result<u64, anyhow::Error> {
    let size_str = size_str.trim().to_uppercase();

    let (number_part, suffix) = if size_str.ends_with('K') {
        (&size_str[..size_str.len() - 1], 1024u64)
    } else if size_str.ends_with('M') {
        (&size_str[..size_str.len() - 1], 1024u64.pow(2))
    } else if size_str.ends_with('G') {
        (&size_str[..size_str.len() - 1], 1024u64.pow(3))
    } else if size_str.ends_with('T') {
        (&size_str[..size_str.len() - 1], 1024u64.pow(4))
    } else {
        (size_str.as_str(), 1u64)
    };

    let number: f64 = number_part.parse()?;
    Ok((number * suffix as f64) as u64)
}
