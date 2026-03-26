use std::io;
use std::path::{Path, PathBuf};

pub const SYMFORGE_DIR_NAME: &str = ".symforge";
pub const LEGACY_TOKENIZOR_DIR_NAME: &str = ".tokenizor";

/// Resolve the canonical symforge data directory under `base`.
///
/// If a legacy `.tokenizor/` directory exists, this attempts to migrate it in place
/// to `.symforge/`. If the rename fails, the legacy path is returned so existing
/// runtime state continues to work instead of hard-failing.
pub fn resolve_symforge_dir(base: &Path, scope: &str) -> io::Result<PathBuf> {
    let symforge_dir = base.join(SYMFORGE_DIR_NAME);
    if symforge_dir.exists() {
        return Ok(symforge_dir);
    }

    let legacy_dir = base.join(LEGACY_TOKENIZOR_DIR_NAME);
    if legacy_dir.exists() {
        match std::fs::rename(&legacy_dir, &symforge_dir) {
            Ok(()) => {
                tracing::warn!(
                    scope,
                    from = %legacy_dir.display(),
                    to = %symforge_dir.display(),
                    "migrated legacy .tokenizor directory to .symforge"
                );
                return Ok(symforge_dir);
            }
            Err(error) => {
                tracing::warn!(
                    scope,
                    from = %legacy_dir.display(),
                    to = %symforge_dir.display(),
                    %error,
                    "failed to migrate legacy .tokenizor directory; continuing with legacy path"
                );
                return Ok(legacy_dir);
            }
        }
    }

    Ok(symforge_dir)
}

/// Ensure the canonical symforge data directory exists under `base`.
pub fn ensure_symforge_dir(base: &Path, scope: &str) -> io::Result<PathBuf> {
    let dir = resolve_symforge_dir(base, scope)?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn test_resolve_symforge_dir_prefers_existing_canonical_dir() {
        let tmp = TempDir::new().unwrap();
        let symforge_dir = tmp.path().join(SYMFORGE_DIR_NAME);
        std::fs::create_dir_all(&symforge_dir).unwrap();

        let resolved = resolve_symforge_dir(tmp.path(), "test").unwrap();

        assert_eq!(resolved, symforge_dir);
    }

    #[test]
    fn test_resolve_symforge_dir_migrates_legacy_dir() {
        let tmp = TempDir::new().unwrap();
        let legacy_dir = tmp.path().join(LEGACY_TOKENIZOR_DIR_NAME);
        std::fs::create_dir_all(&legacy_dir).unwrap();
        std::fs::write(legacy_dir.join("index.bin"), b"snapshot").unwrap();

        let resolved = resolve_symforge_dir(tmp.path(), "test").unwrap();

        let symforge_dir = tmp.path().join(SYMFORGE_DIR_NAME);
        assert_eq!(resolved, symforge_dir);
        assert!(symforge_dir.exists(), "canonical directory should exist");
        assert!(
            symforge_dir.join("index.bin").exists(),
            "legacy contents should be preserved after migration"
        );
        assert!(
            !legacy_dir.exists(),
            "legacy directory should be renamed away after migration"
        );
    }

    #[test]
    fn test_ensure_symforge_dir_creates_canonical_dir_when_missing() {
        let tmp = TempDir::new().unwrap();

        let dir = ensure_symforge_dir(tmp.path(), "test").unwrap();

        assert_eq!(dir, tmp.path().join(SYMFORGE_DIR_NAME));
        assert!(dir.exists(), "canonical directory should be created");
    }
}
