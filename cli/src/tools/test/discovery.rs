use std::path::{Path, PathBuf};

/// Supported test file extensions (Deno parity).
const TEST_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mts", "mjs", "cjs", "cts"];

/// Returns true if a filename matches test file patterns:
/// - `*_test.{ext}` (e.g., `math_test.ts`)
/// - `*.test.{ext}` (e.g., `math.test.ts`)
/// - `test.{ext}` (e.g., `test.ts`)
pub fn is_test_file(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };

    for ext in TEST_EXTENSIONS {
        // *_test.{ext}
        if name.ends_with(&format!("_test.{}", ext)) {
            return true;
        }
        // *.test.{ext}
        if name.ends_with(&format!(".test.{}", ext)) {
            return true;
        }
        // test.{ext} (bare name)
        if name == format!("test.{}", ext) {
            return true;
        }
    }

    false
}

/// Returns true if the file has a supported extension (for __tests__/ directories).
fn has_supported_extension(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return false,
    };
    TEST_EXTENSIONS.contains(&ext)
}

/// Returns true if the given path is inside a `__tests__/` directory.
fn is_in_tests_dir(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map_or(false, |s| s == "__tests__")
    })
}

/// Recursively discover test files from the given paths.
/// Files are included directly; directories are walked recursively.
/// Skips `.`-prefixed directories and `node_modules`.
pub fn discover_test_files(paths: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for path_str in paths {
        let path = PathBuf::from(path_str);
        if !path.exists() {
            eprintln!("warning: path does not exist: {}", path_str);
            continue;
        }

        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            walk_dir(&path, &mut files);
        }
    }

    files.sort();
    files.dedup();
    files
}

fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            eprintln!(
                "warning: cannot read directory {}: {}",
                dir.display(),
                err
            );
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Skip hidden directories and node_modules
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }

        if path.is_dir() {
            walk_dir(&path, files);
        } else if path.is_file() {
            // Include if: matches test file patterns OR is inside __tests__/ with supported extension
            if is_test_file(&path) || (is_in_tests_dir(&path) && has_supported_extension(&path)) {
                files.push(path);
            }
        }
    }
}
