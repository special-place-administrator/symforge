//! Session context tracking: records what the LLM has fetched this session
//! to enable deduplication hints and context inventory.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::Instant;

/// Tracks what symbols and files have been served to the LLM this session.
pub struct SessionContext {
    inner: Mutex<SessionInner>,
}

struct SessionInner {
    /// Symbols fetched: (path, name) → approximate tokens served
    symbols: HashMap<(String, String), u32>,
    /// Files fetched: path → approximate tokens served
    files: HashMap<String, u32>,
    /// Total tokens served this session
    total_tokens: u64,
    /// Session start time
    started_at: Instant,
}

/// A snapshot of the session context for display.
pub struct SessionSnapshot {
    pub symbols: Vec<(String, String, u32)>, // (path, name, tokens)
    pub files: Vec<(String, u32)>,            // (path, tokens)
    pub total_tokens: u64,
    pub duration_secs: u64,
}

impl SessionContext {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SessionInner {
                symbols: HashMap::new(),
                files: HashMap::new(),
                total_tokens: 0,
                started_at: Instant::now(),
            }),
        }
    }

    /// Record that a symbol was served to the LLM.
    pub fn record_symbol(&self, path: &str, name: &str, tokens: u32) {
        let mut inner = self.inner.lock();
        let key = (path.to_string(), name.to_string());
        inner.symbols.entry(key).or_insert(0);
        // Update with latest token count (symbol may have been re-fetched with different verbosity)
        inner.symbols.insert((path.to_string(), name.to_string()), tokens);
        inner.total_tokens += tokens as u64;
    }

    /// Record that a file was served to the LLM.
    pub fn record_file(&self, path: &str, tokens: u32) {
        let mut inner = self.inner.lock();
        inner.files.insert(path.to_string(), tokens);
        inner.total_tokens += tokens as u64;
    }

    /// Check if a symbol has already been fetched this session.
    pub fn has_symbol(&self, path: &str, name: &str) -> bool {
        let inner = self.inner.lock();
        inner.symbols.contains_key(&(path.to_string(), name.to_string()))
    }

    /// Check if a file has already been fetched this session.
    pub fn has_file(&self, path: &str) -> bool {
        let inner = self.inner.lock();
        inner.files.contains_key(path)
    }

    /// Take a snapshot for display.
    pub fn snapshot(&self) -> SessionSnapshot {
        let inner = self.inner.lock();
        let mut symbols: Vec<(String, String, u32)> = inner
            .symbols
            .iter()
            .map(|((p, n), t)| (p.clone(), n.clone(), *t))
            .collect();
        symbols.sort_by(|a, b| b.2.cmp(&a.2)); // sort by tokens descending

        let mut files: Vec<(String, u32)> = inner
            .files
            .iter()
            .map(|(p, t)| (p.clone(), *t))
            .collect();
        files.sort_by(|a, b| b.1.cmp(&a.1));

        SessionSnapshot {
            symbols,
            files,
            total_tokens: inner.total_tokens,
            duration_secs: inner.started_at.elapsed().as_secs(),
        }
    }
}

/// Format the session context inventory for display.
pub fn format_context_inventory(snap: &SessionSnapshot) -> String {
    let minutes = snap.duration_secs / 60;
    let total_items = snap.symbols.len() + snap.files.len();

    let mut lines = vec![format!(
        "Session Context ({} minutes, {} items, ~{} tokens total)",
        minutes, total_items, snap.total_tokens
    )];

    if !snap.symbols.is_empty() {
        lines.push(String::new());
        lines.push(format!("Symbols loaded ({}):", snap.symbols.len()));
        for (path, name, tokens) in snap.symbols.iter().take(15) {
            lines.push(format!("  {name} ({path}) — ~{tokens} tokens"));
        }
        if snap.symbols.len() > 15 {
            lines.push(format!("  ... and {} more", snap.symbols.len() - 15));
        }
    }

    if !snap.files.is_empty() {
        lines.push(String::new());
        lines.push(format!("Files loaded ({}):", snap.files.len()));
        for (path, tokens) in snap.files.iter().take(10) {
            lines.push(format!("  {path} — ~{tokens} tokens"));
        }
        if snap.files.len() > 10 {
            lines.push(format!("  ... and {} more", snap.files.len() - 10));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_query() {
        let ctx = SessionContext::new();
        assert!(!ctx.has_symbol("src/lib.rs", "main"));
        ctx.record_symbol("src/lib.rs", "main", 100);
        assert!(ctx.has_symbol("src/lib.rs", "main"));
        assert!(!ctx.has_file("src/lib.rs"));
        ctx.record_file("src/lib.rs", 500);
        assert!(ctx.has_file("src/lib.rs"));
    }

    #[test]
    fn test_snapshot() {
        let ctx = SessionContext::new();
        ctx.record_symbol("a.rs", "foo", 100);
        ctx.record_symbol("b.rs", "bar", 200);
        ctx.record_file("c.rs", 300);
        let snap = ctx.snapshot();
        assert_eq!(snap.symbols.len(), 2);
        assert_eq!(snap.files.len(), 1);
        assert_eq!(snap.total_tokens, 600);
    }

    #[test]
    fn test_format_inventory() {
        let ctx = SessionContext::new();
        ctx.record_symbol("src/lib.rs", "LiveIndex", 500);
        ctx.record_file("src/main.rs", 1000);
        let snap = ctx.snapshot();
        let output = format_context_inventory(&snap);
        assert!(output.contains("LiveIndex"));
        assert!(output.contains("src/main.rs"));
        assert!(output.contains("1500"));
    }
}
