use std::path::{Path, PathBuf};

/// Check if the file path has a supported image extension.
pub fn is_supported_image(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext = ext.to_lowercase();
        matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tiff" | "tif" | "ico"
        )
    } else {
        false
    }
}

/// Scan the parent directory of `current_path` for all supported images,
/// sort them alphabetically, and find the index of `current_path`.
pub fn get_image_siblings(current_path: &Path) -> (Vec<PathBuf>, usize) {
    let current_abs = current_path
        .canonicalize()
        .unwrap_or_else(|_| current_path.to_path_buf());

    let parent = match current_abs.parent() {
        Some(p) => p,
        None => return (vec![current_abs.clone()], 0),
    };

    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_supported_image(&path) {
                if let Ok(abs_path) = path.canonicalize() {
                    files.push(abs_path);
                } else {
                    files.push(path);
                }
            }
        }
    }

    // Keep extensionless images and files outside readable directory listings navigable.
    if !files.contains(&current_abs) {
        files.push(current_abs.clone());
    }
    files.sort();
    files.dedup();

    // Sort alphabetically (case-insensitive)
    files.sort_by(|a, b| {
        a.to_string_lossy()
            .to_lowercase()
            .cmp(&b.to_string_lossy().to_lowercase())
            .then_with(|| a.cmp(b))
    });

    let current_index = files.iter().position(|p| p == &current_abs).unwrap_or(0);

    (files, current_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_is_supported_image() {
        assert!(is_supported_image(Path::new("test.png")));
        assert!(is_supported_image(Path::new("test.PNG")));
        assert!(is_supported_image(Path::new("test.jpeg")));
        assert!(is_supported_image(Path::new("test.webp")));
        assert!(!is_supported_image(Path::new("test.txt")));
        assert!(!is_supported_image(Path::new("test")));
    }

    #[test]
    fn test_get_image_siblings() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("a.png");
        let file2 = dir.path().join("b.jpg");
        let file3 = dir.path().join("c.txt");
        let file4 = dir.path().join("d.webp");

        File::create(&file1).unwrap();
        File::create(&file2).unwrap();
        File::create(&file3).unwrap();
        File::create(&file4).unwrap();

        let (siblings, index) = get_image_siblings(&file2);

        // Canonical paths might differ on some OS, let's canonicalize the files for comparison
        let file1_canon = file1.canonicalize().unwrap();
        let file2_canon = file2.canonicalize().unwrap();
        let file4_canon = file4.canonicalize().unwrap();

        assert_eq!(siblings.len(), 3);
        assert_eq!(siblings[0], file1_canon);
        assert_eq!(siblings[1], file2_canon);
        assert_eq!(siblings[2], file4_canon);
        assert_eq!(index, 1);
    }
}
