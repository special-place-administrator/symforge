use std::path::Path;

use ignore::WalkBuilder;

use crate::domain::LanguageId;
use crate::error::{Result, TokenizorError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredFile {
    pub relative_path: String,
    pub absolute_path: std::path::PathBuf,
    pub language: LanguageId,
}

pub fn discover_files(root: &Path) -> Result<Vec<DiscoveredFile>> {
    let mut files = Vec::new();

    for entry in WalkBuilder::new(root).build() {
        let entry = entry.map_err(|e| {
            TokenizorError::io(root.to_path_buf(), std::io::Error::new(std::io::ErrorKind::Other, e))
        })?;

        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let extension = match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => ext,
            None => continue,
        };

        let language = match LanguageId::from_extension(extension) {
            Some(lang) => lang,
            None => continue,
        };

        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        files.push(DiscoveredFile {
            relative_path: relative,
            absolute_path: path.to_path_buf(),
            language,
        });
    }

    files.sort_by(|a, b| {
        let a_normalized = a.relative_path.to_lowercase();
        let b_normalized = b.relative_path.to_lowercase();
        a_normalized.cmp(&b_normalized)
    });

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_files_finds_supported_languages() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("app.py"), "def main(): pass").unwrap();
        fs::write(dir.path().join("index.js"), "function x() {}").unwrap();
        fs::write(dir.path().join("app.ts"), "const x = 1;").unwrap();
        fs::write(dir.path().join("main.go"), "package main").unwrap();

        let files = discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 5);
    }

    #[test]
    fn test_discover_files_ignores_unsupported_extensions() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("data.csv"), "a,b,c").unwrap();
        fs::write(dir.path().join("readme.md"), "# Hello").unwrap();
        fs::write(dir.path().join("Makefile"), "all:").unwrap();

        let files = discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].language, LanguageId::Rust);
    }

    #[test]
    fn test_discover_files_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        // ignore crate requires a git repo to respect .gitignore
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".gitignore"), "ignored.rs\n").unwrap();
        fs::write(dir.path().join("kept.rs"), "fn kept() {}").unwrap();
        fs::write(dir.path().join("ignored.rs"), "fn ignored() {}").unwrap();

        let files = discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "kept.rs");
    }

    #[test]
    fn test_discover_files_deterministic_sort_order() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/zebra.rs"), "").unwrap();
        fs::write(dir.path().join("src/alpha.rs"), "").unwrap();
        fs::write(dir.path().join("main.rs"), "").unwrap();

        let files = discover_files(dir.path()).unwrap();
        let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
        assert_eq!(paths, vec!["main.rs", "src/alpha.rs", "src/zebra.rs"]);
    }

    #[test]
    fn test_discover_files_normalizes_forward_slashes() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/nested")).unwrap();
        fs::write(dir.path().join("src/nested/lib.rs"), "").unwrap();

        let files = discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert!(
            files[0].relative_path.contains('/'),
            "expected forward slashes: {}",
            files[0].relative_path
        );
        assert!(
            !files[0].relative_path.contains('\\'),
            "unexpected backslashes: {}",
            files[0].relative_path
        );
    }

    #[test]
    fn test_discover_files_maps_jsx_tsx_extensions() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("component.jsx"), "").unwrap();
        fs::write(dir.path().join("component.tsx"), "").unwrap();

        let files = discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2);

        let jsx = files.iter().find(|f| f.relative_path == "component.jsx").unwrap();
        assert_eq!(jsx.language, LanguageId::JavaScript);

        let tsx = files.iter().find(|f| f.relative_path == "component.tsx").unwrap();
        assert_eq!(tsx.language, LanguageId::TypeScript);
    }
}
