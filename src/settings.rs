use std::path::PathBuf;

/// Path to the small config file we use to remember user preferences between runs.
/// Lives under the platform's standard config directory, e.g.:
/// - Linux:   ~/.config/rusty_viewer/config.txt
/// - macOS:   ~/Library/Application Support/rusty_viewer/config.txt
fn config_file_path() -> Option<PathBuf> {
    let mut dir = dirs::config_dir()?;
    dir.push("rusty_viewer");
    std::fs::create_dir_all(&dir).ok()?;
    dir.push("config.txt");
    Some(dir)
}

/// Load the last batch output directory the user picked, if we have one saved
/// and it still exists on disk.
pub fn load_last_batch_output_dir() -> Option<PathBuf> {
    let path = config_file_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if path.is_dir() { Some(path) } else { None }
}

/// Persist the given directory as the last-used batch output directory.
/// Best-effort: failures (e.g. read-only config dir) are silently ignored,
/// since this is a convenience feature and shouldn't block batch conversion.
pub fn save_last_batch_output_dir(dir: &str) {
    if dir.is_empty() {
        return;
    }
    if let Some(path) = config_file_path() {
        let _ = std::fs::write(path, dir);
    }
}

/// The directory to use for batch output when the user has no saved
/// preference yet: their home directory, falling back to the current
/// working directory if that's somehow unavailable.
pub fn default_batch_output_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Resolve the batch output directory to use on startup: the last one the
/// user picked, or their home directory if this is the first run.
pub fn initial_batch_output_dir() -> PathBuf {
    load_last_batch_output_dir().unwrap_or_else(default_batch_output_dir)
}
