//! Edit hooks — extension points for feature tentacles.
//!
//! The 7 edit handlers in [`crate::protocol::tools`] share two side-car steps that
//! feature tentacles want to customise without forking the handler bodies:
//!
//! 1. **Path resolution** — today, every handler resolves the caller's relative path
//!    against the bound repo root (`server.capture_repo_root()` + [`crate::protocol::edit::safe_repo_path`]).
//!    The `worktree-awareness` feature tentacle wants to redirect that resolution into
//!    the caller's working-directory worktree when one is active.
//! 2. **Post-write bookkeeping** — today, there is nothing. The `frecency-ranking`
//!    feature tentacle wants to record an access event whenever an edit commits so
//!    its scoring can boost recently-touched files.
//!
//! This module defines the [`EditHook`] trait and a process-wide registry. The
//! default hook ([`DefaultEditHook`]) is pre-registered so today's behaviour is
//! preserved byte-for-byte when no feature hook is installed.
//!
//! Hooks are object-safe (`Box<dyn EditHook>`) so feature tentacles can register
//! trait objects at startup.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use parking_lot::RwLock;

/// Contextual information passed to every hook call.
#[derive(Debug, Clone, Copy)]
pub struct EditContext<'a> {
    /// Relative path as supplied by the caller (e.g. `src/lib.rs`).
    pub relative_path: &'a str,
    /// Absolute path the current repo root resolves `relative_path` to.
    /// This is the edit target in the absence of any feature hook.
    pub indexed_absolute_path: &'a Path,
    /// Repository root currently bound to the server.
    pub repo_root: &'a Path,
}

/// Extension point for feature tentacles.
///
/// Implementors may customise where an edit writes ([`Self::resolve_target_path`])
/// and react to a successful commit ([`Self::after_edit_committed`]). Both methods
/// have safe defaults so implementors can override only the surface they care about.
///
/// Implementations must be `Send + Sync` and object-safe.
pub trait EditHook: Send + Sync {
    /// Resolve the absolute path the edit should target.
    ///
    /// The default implementation returns `ctx.indexed_absolute_path` unchanged;
    /// feature tentacles may redirect (e.g. to a per-working-directory worktree).
    fn resolve_target_path(&self, ctx: &EditContext) -> Result<PathBuf, String> {
        Ok(ctx.indexed_absolute_path.to_path_buf())
    }

    /// Called after an atomic write has committed successfully.
    ///
    /// The default implementation is a no-op; feature tentacles may record the
    /// access (e.g. for frecency scoring).
    fn after_edit_committed(&self, _ctx: &EditContext, _resolved_path: &Path) {}
}

/// No-op default hook — preserves today's behaviour when no feature hook is
/// registered. Pre-registered at module-init time so the registry is never empty.
pub struct DefaultEditHook;

impl EditHook for DefaultEditHook {}

fn registry() -> &'static RwLock<Vec<Box<dyn EditHook>>> {
    static REGISTRY: OnceLock<RwLock<Vec<Box<dyn EditHook>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(vec![Box::new(DefaultEditHook) as Box<dyn EditHook>]))
}

/// Register a hook on the process-wide registry.
///
/// Registered hooks are appended; later registrations take precedence for path
/// resolution, and every registered hook is notified on commit. The pre-registered
/// [`DefaultEditHook`] sits at the bottom of the stack and acts as the fallback.
pub fn register(hook: Box<dyn EditHook>) {
    registry().write().push(hook);
}

/// Resolve the target path by walking registered hooks in reverse-registration
/// order and returning the first result.
///
/// Because [`DefaultEditHook`] is pre-registered and its default impl always
/// returns `Ok`, this function only returns `Err` when a feature hook explicitly
/// fails the resolution.
pub fn resolve(ctx: &EditContext) -> Result<PathBuf, String> {
    let reg = registry().read();
    // Walk hooks most-recently-registered first so feature hooks win over the default.
    if let Some(hook) = reg.iter().next_back() {
        return hook.resolve_target_path(ctx);
    }
    // Defensive — the registry is seeded with DefaultEditHook on first access, so this
    // branch is unreachable in practice.
    DefaultEditHook.resolve_target_path(ctx)
}

/// Invoke [`EditHook::after_edit_committed`] on every registered hook.
pub fn after_commit(ctx: &EditContext, resolved_path: &Path) {
    let reg = registry().read();
    for hook in reg.iter() {
        hook.after_edit_committed(ctx, resolved_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hook_returns_indexed_path_unchanged() {
        let repo_root = PathBuf::from("/tmp/repo");
        let abs = repo_root.join("src/lib.rs");
        let ctx = EditContext {
            relative_path: "src/lib.rs",
            indexed_absolute_path: &abs,
            repo_root: &repo_root,
        };
        let resolved = DefaultEditHook.resolve_target_path(&ctx).expect("resolves");
        assert_eq!(resolved, abs);
    }

    #[test]
    fn default_hook_after_commit_is_noop() {
        let repo_root = PathBuf::from("/tmp/repo");
        let abs = repo_root.join("src/lib.rs");
        let ctx = EditContext {
            relative_path: "src/lib.rs",
            indexed_absolute_path: &abs,
            repo_root: &repo_root,
        };
        // Should not panic or mutate anything observable.
        DefaultEditHook.after_edit_committed(&ctx, &abs);
    }

    #[test]
    fn registry_resolves_via_default_when_only_default_registered() {
        // This test relies on the registry state being the default seeding only,
        // which is true at process start. Other tests in this file do not register
        // feature hooks, so the invariant holds with `--test-threads=1`.
        let repo_root = PathBuf::from("/tmp/repo-registry");
        let abs = repo_root.join("src/lib.rs");
        let ctx = EditContext {
            relative_path: "src/lib.rs",
            indexed_absolute_path: &abs,
            repo_root: &repo_root,
        };
        let resolved = resolve(&ctx).expect("resolves");
        assert_eq!(resolved, abs);
    }
}
