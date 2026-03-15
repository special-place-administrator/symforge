# Sprint 16: Correctness, Atomicity & Lifecycle — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **IMPORTANT:** Do NOT use Tokenizor MCP's `batch_rename` tool — it is broken on the installed version. Use `replace_symbol_body`, `edit_within_symbol`, and `batch_edit` instead.

**Goal:** Fix 6 correctness/lifecycle bugs: TOCTOU panic, temp file collision, CRLF preservation, splice overlap validation, SIGTERM handling, and denylist hardening.

**Architecture:** Each fix is independent — one commit per item, no cross-dependencies. Execution order: C6 → C3 → C1 → C4 → C5 → C2-lite.

**Tech Stack:** Rust, tokio (async/signals), tempfile crate (C3), libc (C5 Unix signals)

**Spec:** `docs/superpowers/specs/2026-03-15-sprint-16-correctness-lifecycle-design.md`

---

## Chunk 1: C6 — Fix `open_project_session` TOCTOU Panic

### Task 1: Add ActivationState enum and split ProjectInstance::load()

**Files:**
- Modify: `src/daemon.rs:69-80` (ProjectInstance struct — add activation_state field)
- Modify: `src/daemon.rs:842-880` (ProjectInstance::load — split into load + activate)

- [ ] **Step 1: Add ActivationState enum above ProjectInstance struct**

In `src/daemon.rs`, add before the `ProjectInstance` struct (around line 68):

```rust
/// Tracks whether a ProjectInstance has been fully activated (watcher + git temporal started).
/// Prevents two racing opens from both activating the same project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivationState {
    /// Freshly constructed, no background tasks started. Safe to discard.
    Inactive,
    /// Activation in progress (watcher + git temporal being started).
    Activating,
    /// Fully active with watcher and git temporal running.
    Active,
}
```

- [ ] **Step 2: Add activation_state field to ProjectInstance**

Add to the `ProjectInstance` struct (around line 69-80):

```rust
activation_state: ActivationState,
```

- [ ] **Step 3: Split ProjectInstance::load() into load() + activate()**

Refactor `ProjectInstance::load()` (lines 842-880) into two methods:

```rust
/// Pure construction: allocate index, parse files, build data structures.
/// No spawned tasks, no background work, no OS watchers. Safe to discard.
fn load(canonical_root: &Path) -> anyhow::Result<Self> {
    // ... existing index loading logic ...
    // ... but do NOT call start_project_watcher() or spawn_git_temporal_computation()
    // Set activation_state: ActivationState::Inactive
    // Set watcher_task: None
}

/// Post-commit activation: start watcher task and git temporal analysis.
/// Call only after write-lock re-check confirms this instance won insertion.
fn activate(&mut self) {
    assert_eq!(self.activation_state, ActivationState::Inactive);
    self.activation_state = ActivationState::Activating;
    // Move watcher spawn logic here from old load()
    // Move git temporal spawn here from old load()
    self.activation_state = ActivationState::Active;
}
```

The key is moving `start_project_watcher()` and `spawn_git_temporal_computation()` calls from `load()` into `activate()`.

- [ ] **Step 4: Compile check**

Run: `cargo check 2>&1 | head -30`
Expected: Compilation errors in `open_project_session` and tests that use `load()` — those are fixed in the next steps.

- [ ] **Step 5: Commit work-in-progress**

```bash
git add src/daemon.rs
git commit -m "refactor: split ProjectInstance::load into load + activate (C6 prep)"
```

### Task 2: Rewrite open_project_session with double-checked locking

**Files:**
- Modify: `src/daemon.rs:188-246` (open_project_session — rewrite body)

- [ ] **Step 1: Rewrite open_project_session**

Replace the body of `open_project_session` (lines 188-246) with:

```rust
pub fn open_project_session(
    &self,
    request: OpenProjectRequest,
) -> anyhow::Result<OpenProjectResponse> {
    let canonical_root = canonical_project_root(Path::new(&request.project_root))?;
    let project_id = project_key(&canonical_root);

    // Fast path: project already loaded — just add session under write lock.
    {
        let projects = self.projects.read().expect("lock poisoned");
        if projects.contains_key(&project_id) {
            drop(projects);
            // Project exists — add session under write lock and return.
            return self.register_session_for_existing_project(
                &project_id, &request, &canonical_root,
            );
        }
    }

    // Slow path: project not loaded — load unlocked, then re-check under write lock.
    let mut new_project = ProjectInstance::load(&canonical_root)?;

    let needs_activation = {
        let mut projects = self.projects.write().expect("lock poisoned");
        if projects.contains_key(&project_id) {
            // Another thread won the race — discard our loaded instance (no tasks to clean up).
            false
        } else {
            // We won — insert and mark as activating under the lock.
            new_project.activation_state = ActivationState::Activating;
            projects.insert(project_id.clone(), new_project);
            true
        }
    };

    // Activate outside the write lock if we inserted.
    if needs_activation {
        let mut projects = self.projects.write().expect("lock poisoned");
        if let Some(project) = projects.get_mut(&project_id) {
            if project.activation_state == ActivationState::Activating {
                project.activate();
            }
        }
    }

    // Register session (works whether we inserted or another thread did).
    self.register_session_for_existing_project(&project_id, &request, &canonical_root)
}
```

- [ ] **Step 2: Extract register_session_for_existing_project helper**

Add a new private method to `DaemonState`:

```rust
/// Register a session for an already-loaded project.
/// Acquires write lock on projects and sessions.
fn register_session_for_existing_project(
    &self,
    project_id: &str,
    request: &OpenProjectRequest,
    canonical_root: &Path,
) -> anyhow::Result<OpenProjectResponse> {
    let session_id = format!(
        "session-{}",
        self.next_session_id.fetch_add(1, Ordering::Relaxed)
    );
    let now = SystemTime::now();

    let (project_name, canonical_root_text, session_count) = {
        let mut projects = self.projects.write().expect("lock poisoned");
        let project = projects
            .get_mut(project_id)
            .ok_or_else(|| anyhow::anyhow!(
                "project {} was removed between check and session registration", project_id
            ))?;
        project.session_ids.insert(session_id.clone());
        (
            project.project_name.clone(),
            normalized_path_string(&project.canonical_root),
            project.session_ids.len(),
        )
    };

    let session = SessionRecord {
        session_id: session_id.clone(),
        project_id: project_id.to_string(),
        client_name: request.client_name.clone(),
        pid: request.pid,
        opened_at: now,
        last_seen_at: now,
    };
    self.sessions
        .write()
        .expect("lock poisoned")
        .insert(session_id.clone(), session);

    Ok(OpenProjectResponse {
        project_id: project_id.to_string(),
        session_id,
        project_name,
        canonical_root: canonical_root_text,
        session_count,
    })
}
```

- [ ] **Step 3: Compile and run existing tests**

Run: `cargo test --all-targets -- --test-threads=1 -q 2>&1 | tail -5`
Expected: All existing tests pass (existing tests don't exercise concurrent opens).

- [ ] **Step 4: Commit**

```bash
git add src/daemon.rs
git commit -m "fix: rewrite open_project_session with double-checked locking (C6)"
```

### Task 3: Add C6 concurrency tests

**Files:**
- Modify: `src/daemon.rs` (test module, after existing tests ~line 1570+)

- [ ] **Step 1: Write test_concurrent_open_same_project**

Add to the test module in `src/daemon.rs`:

```rust
#[tokio::test]
async fn test_concurrent_open_same_project_no_panic() {
    let _lock = env_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().to_path_buf();

    // Create a minimal valid project directory
    std::fs::create_dir_all(project_root.join("src")).unwrap();
    std::fs::write(project_root.join("src/main.rs"), "fn main() {}").unwrap();

    let state = Arc::new(DaemonState::new());
    let barrier = Arc::new(std::sync::Barrier::new(2));

    let mut handles = Vec::new();
    for i in 0..2 {
        let state = Arc::clone(&state);
        let barrier = Arc::clone(&barrier);
        let root = project_root.display().to_string();
        handles.push(tokio::task::spawn_blocking(move || {
            barrier.wait();
            state.open_project_session(OpenProjectRequest {
                project_root: root,
                client_name: format!("client-{i}"),
                pid: None,
            })
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    let successes: Vec<_> = results
        .into_iter()
        .filter_map(|r| r.ok().and_then(|r| r.ok()))
        .collect();

    // Both should succeed — same project, different sessions
    assert_eq!(successes.len(), 2, "both opens should succeed");

    // Verify: exactly 1 project instance
    let projects = state.projects.read().expect("lock poisoned");
    assert_eq!(projects.len(), 1, "exactly one project instance");

    // Verify: project has exactly 2 sessions
    let (_, project) = projects.iter().next().unwrap();
    assert_eq!(project.session_ids.len(), 2, "two sessions registered");
    assert_eq!(project.activation_state, ActivationState::Active, "project is active");
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test test_concurrent_open_same_project_no_panic -- --test-threads=1 -v`
Expected: PASS

- [ ] **Step 3: Write test_concurrent_open_different_projects**

```rust
#[tokio::test]
async fn test_concurrent_open_different_projects() {
    let _lock = env_lock();
    let tmp1 = tempfile::TempDir::new().unwrap();
    let tmp2 = tempfile::TempDir::new().unwrap();

    for tmp in [&tmp1, &tmp2] {
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();
    }

    let state = Arc::new(DaemonState::new());
    let barrier = Arc::new(std::sync::Barrier::new(2));

    let roots = vec![
        tmp1.path().display().to_string(),
        tmp2.path().display().to_string(),
    ];

    let mut handles = Vec::new();
    for (i, root) in roots.into_iter().enumerate() {
        let state = Arc::clone(&state);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::task::spawn_blocking(move || {
            barrier.wait();
            state.open_project_session(OpenProjectRequest {
                project_root: root,
                client_name: format!("client-{i}"),
                pid: None,
            })
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    let successes: Vec<_> = results
        .into_iter()
        .filter_map(|r| r.ok().and_then(|r| r.ok()))
        .collect();

    assert_eq!(successes.len(), 2, "both opens should succeed");

    let projects = state.projects.read().expect("lock poisoned");
    assert_eq!(projects.len(), 2, "two distinct project instances");
}
```

- [ ] **Step 4: Run all C6 tests**

Run: `cargo test test_concurrent_open -- --test-threads=1 -v`
Expected: Both tests PASS

- [ ] **Step 5: Write test_open_close_race_no_panic**

```rust
#[tokio::test]
async fn test_open_close_race_no_panic() {
    let _lock = env_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();

    let state = Arc::new(DaemonState::new());
    let root = tmp.path().display().to_string();

    // Open a session first
    let resp = state.open_project_session(OpenProjectRequest {
        project_root: root.clone(),
        client_name: "opener".into(),
        pid: None,
    }).unwrap();

    let barrier = Arc::new(std::sync::Barrier::new(2));

    // Race: open another session vs close the first
    let state2 = Arc::clone(&state);
    let barrier2 = barrier.clone();
    let root2 = root.clone();
    let open_handle = tokio::task::spawn_blocking(move || {
        barrier2.wait();
        state2.open_project_session(OpenProjectRequest {
            project_root: root2,
            client_name: "racer".into(),
            pid: None,
        })
    });

    let state3 = Arc::clone(&state);
    let barrier3 = barrier.clone();
    let session_id = resp.session_id.clone();
    let close_handle = tokio::task::spawn_blocking(move || {
        barrier3.wait();
        state3.close_session(&session_id)
    });

    let (open_result, close_result) = tokio::join!(open_handle, close_handle);
    // Neither should panic — both should return Ok or a handled error
    assert!(open_result.is_ok(), "open task should not panic");
    assert!(close_result.is_ok(), "close task should not panic");
}
```

- [ ] **Step 6: Write test_discarded_instance_no_leaked_tasks**

```rust
#[tokio::test]
async fn test_discarded_instance_no_leaked_tasks() {
    let _lock = env_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();

    let state = Arc::new(DaemonState::new());
    let barrier = Arc::new(std::sync::Barrier::new(3));

    // Race 3 threads opening the same project
    let mut handles = Vec::new();
    for i in 0..3 {
        let state = Arc::clone(&state);
        let barrier = Arc::clone(&barrier);
        let root = tmp.path().display().to_string();
        handles.push(tokio::task::spawn_blocking(move || {
            barrier.wait();
            state.open_project_session(OpenProjectRequest {
                project_root: root,
                client_name: format!("client-{i}"),
                pid: None,
            })
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    let successes: Vec<_> = results
        .into_iter()
        .filter_map(|r| r.ok().and_then(|r| r.ok()))
        .collect();

    assert_eq!(successes.len(), 3, "all three opens should succeed");

    // Key invariant: exactly 1 project instance, active, with 1 watcher
    let projects = state.projects.read().expect("lock poisoned");
    assert_eq!(projects.len(), 1, "exactly one project instance");
    let (_, project) = projects.iter().next().unwrap();
    assert_eq!(project.activation_state, ActivationState::Active);
    assert_eq!(project.session_ids.len(), 3, "three sessions registered");
    // Watcher task should exist exactly once (not duplicated)
    assert!(project.watcher_task.is_some(), "watcher should be started");
}
```

- [ ] **Step 7: Commit**

```bash
git add src/daemon.rs
git commit -m "test: add concurrent open_project_session tests (C6)"
```

- [ ] **Step 8: Run full test suite**

Run: `cargo test --all-targets -- --test-threads=1 -q 2>&1 | tail -5`
Expected: All tests pass (1230+ existing + new)

- [ ] **Step 7: Final C6 commit (squash if desired)**

```bash
git add src/daemon.rs
git commit -m "fix: resolve open_project_session TOCTOU panic (C6)"
```

---

## Chunk 2: C3 — Fix `atomic_write_file` Temp Filename Collision

### Task 4: Add tempfile dependency and rewrite atomic_write_file

**Files:**
- Modify: `Cargo.toml` (add tempfile dependency)
- Modify: `src/protocol/edit.rs:52-57` (atomic_write_file — rewrite)

- [ ] **Step 1: Add tempfile to Cargo.toml**

```bash
cargo add tempfile
```

- [ ] **Step 2: Verify tempfile persist behavior on Windows**

Check the tempfile crate source to confirm `persist()` uses `MoveFileExW` with
`MOVEFILE_REPLACE_EXISTING` on Windows. Look at `tempfile/src/file/imp/windows.rs`
or similar. Document the finding in a code comment.

Run: `cargo doc --open -p tempfile` or check the source directly.

- [ ] **Step 3: Rewrite atomic_write_file**

Replace `atomic_write_file` in `src/protocol/edit.rs` (lines 52-57):

```rust
pub(crate) fn atomic_write_file(path: &Path, content: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent directory")
    })?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(content)?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
```

Add at the top of the file (in the imports section):
```rust
// tempfile is used by atomic_write_file for collision-safe temp files
```

- [ ] **Step 4: Run existing atomic_write_file tests**

Run: `cargo test test_atomic_write_file -- --test-threads=1 -v`
Expected: All 3 existing tests PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/protocol/edit.rs
git commit -m "fix: use tempfile crate for collision-safe atomic writes (C3 impl)"
```

### Task 5: Add C3 concurrency and cleanup tests

**Files:**
- Modify: `src/protocol/edit.rs` (test module ~line 1561+)

- [ ] **Step 1: Write test_atomic_write_concurrent_no_hybrid**

```rust
#[test]
fn test_atomic_write_concurrent_no_hybrid() {
    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("target.txt");

    // Create initial file
    std::fs::write(&target, b"initial").unwrap();

    let payload_a = vec![b'A'; 1024 * 1024]; // 1MB of 'A'
    let payload_b = vec![b'B'; 1024 * 1024]; // 1MB of 'B'

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let target_clone = target.clone();
    let barrier_a = barrier.clone();
    let payload_a_clone = payload_a.clone();

    let handle_a = std::thread::spawn(move || {
        barrier_a.wait();
        atomic_write_file(&target_clone, &payload_a_clone)
    });

    let target_clone = target.clone();
    let barrier_b = barrier.clone();
    let payload_b_clone = payload_b.clone();

    let handle_b = std::thread::spawn(move || {
        barrier_b.wait();
        atomic_write_file(&target_clone, &payload_b_clone)
    });

    handle_a.join().unwrap().unwrap();
    handle_b.join().unwrap().unwrap();

    // Final file must be exactly one of the two payloads, never a hybrid
    let result = std::fs::read(&target).unwrap();
    assert!(
        result == payload_a || result == payload_b,
        "final file must be exactly payload A or payload B, got {} bytes starting with {:?}",
        result.len(),
        &result[..8.min(result.len())]
    );
}
```

- [ ] **Step 2: Write test_atomic_write_no_orphan_temp_files**

```rust
#[test]
fn test_atomic_write_no_orphan_temp_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("target.txt");

    atomic_write_file(&target, b"hello world").unwrap();

    // Count files in directory — should be exactly 1 (the target)
    let files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(files.len(), 1, "no orphan temp files should remain");
    assert_eq!(files[0].file_name(), "target.txt");
}
```

- [ ] **Step 3: Write test_atomic_write_error_path_cleanup (spec-required)**

```rust
#[test]
fn test_atomic_write_error_path_no_orphan() {
    // Writing to a nonexistent directory should fail without leaving temp files
    let dir = tempfile::TempDir::new().unwrap();
    let bad_target = dir.path().join("nonexistent_subdir").join("target.txt");

    let result = atomic_write_file(&bad_target, b"should fail");
    assert!(result.is_err(), "writing to nonexistent dir should fail");

    // Verify no temp files leaked in the parent TempDir
    let files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(files.len(), 0, "no orphan temp files after failed write");
}
```

- [ ] **Step 4: Run the new tests**

Run: `cargo test test_atomic_write -- --test-threads=1 -v`
Expected: All tests PASS (3 existing + 3 new)

- [ ] **Step 5: Commit**

```bash
git add src/protocol/edit.rs
git commit -m "test: add concurrent write and orphan cleanup tests (C3)"
```

- [ ] **Step 5: Squash into single C3 commit**

```bash
git add -A
git commit -m "fix: use unique temp files in atomic_write_file (C3)"
```

---

## Chunk 3: C1 — CRLF Line Ending Preservation

### Task 6: Add LineEnding detection and normalization helpers

**Files:**
- Modify: `src/protocol/edit.rs` (add new types and helpers near top, after line ~48)

- [ ] **Step 1: Add LineEnding enum and detection function**

Add after `apply_splice` (around line 46) in `src/protocol/edit.rs`:

```rust
// ---------------------------------------------------------------------------
// Line ending detection and normalization
// ---------------------------------------------------------------------------

/// Detected line ending style of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    /// Returns the byte sequence for this line ending.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        match self {
            LineEnding::Lf => b"\n",
            LineEnding::CrLf => b"\r\n",
        }
    }
}

/// Detect the dominant line ending style in file content.
/// Counts \r\n pairs vs lone \n. If \r\n > lone \n → CrLf, else Lf.
/// Empty or no-newline content defaults to Lf.
pub(crate) fn detect_line_ending(content: &[u8]) -> LineEnding {
    let mut crlf_count: usize = 0;
    let mut lf_count: usize = 0;
    let mut i = 0;
    while i < content.len() {
        if i + 1 < content.len() && content[i] == b'\r' && content[i + 1] == b'\n' {
            crlf_count += 1;
            i += 2;
        } else if content[i] == b'\n' {
            lf_count += 1;
            i += 1;
        } else {
            i += 1;
        }
    }
    if crlf_count > lf_count {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    }
}

/// Normalize line endings in generated/replacement text to match the target style.
/// 1. Convert \r\n → \n
/// 2. Convert lone \r → \n
/// 3. If target is CrLf, convert \n → \r\n
pub(crate) fn normalize_line_endings(text: &[u8], target: LineEnding) -> Vec<u8> {
    // Step 1+2: canonicalize to \n
    let mut canonical = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if i + 1 < text.len() && text[i] == b'\r' && text[i + 1] == b'\n' {
            canonical.push(b'\n');
            i += 2;
        } else if text[i] == b'\r' {
            canonical.push(b'\n');
            i += 1;
        } else {
            canonical.push(text[i]);
            i += 1;
        }
    }

    // Step 3: if CRLF, expand \n → \r\n
    match target {
        LineEnding::Lf => canonical,
        LineEnding::CrLf => {
            let mut result = Vec::with_capacity(canonical.len() * 2);
            for &byte in &canonical {
                if byte == b'\n' {
                    result.extend_from_slice(b"\r\n");
                } else {
                    result.push(byte);
                }
            }
            result
        }
    }
}
```

- [ ] **Step 2: Write unit tests for detection and normalization**

```rust
#[test]
fn test_detect_line_ending_lf() {
    assert_eq!(detect_line_ending(b"hello\nworld\n"), LineEnding::Lf);
}

#[test]
fn test_detect_line_ending_crlf() {
    assert_eq!(detect_line_ending(b"hello\r\nworld\r\n"), LineEnding::CrLf);
}

#[test]
fn test_detect_line_ending_empty() {
    assert_eq!(detect_line_ending(b""), LineEnding::Lf);
}

#[test]
fn test_detect_line_ending_dominant_count() {
    // 2 CRLF, 1 LF → CRLF wins
    assert_eq!(detect_line_ending(b"a\r\nb\r\nc\n"), LineEnding::CrLf);
    // 1 CRLF, 2 LF → LF wins
    assert_eq!(detect_line_ending(b"a\r\nb\nc\n"), LineEnding::Lf);
}

#[test]
fn test_normalize_line_endings_to_crlf() {
    let input = b"line1\nline2\nline3";
    let result = normalize_line_endings(input, LineEnding::CrLf);
    assert_eq!(result, b"line1\r\nline2\r\nline3");
}

#[test]
fn test_normalize_line_endings_to_lf() {
    let input = b"line1\r\nline2\r\nline3";
    let result = normalize_line_endings(input, LineEnding::Lf);
    assert_eq!(result, b"line1\nline2\nline3");
}

#[test]
fn test_normalize_lone_cr() {
    let input = b"line1\rline2\r";
    let result = normalize_line_endings(input, LineEnding::CrLf);
    assert_eq!(result, b"line1\r\nline2\r\n");
}
```

- [ ] **Step 3: Run the new tests**

Run: `cargo test test_detect_line_ending test_normalize_line_endings test_normalize_lone -- --test-threads=1 -v`
Expected: All 7 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/protocol/edit.rs
git commit -m "feat: add LineEnding detection and normalization helpers (C1 prep)"
```

### Task 7: Update apply_indentation and insert helpers for CRLF

**Files:**
- Modify: `src/protocol/edit.rs:150-165` (apply_indentation)
- Modify: `src/protocol/edit.rs:176-222` (build_insert_before, build_insert_after)

- [ ] **Step 1: Update apply_indentation to accept LineEnding**

Replace `apply_indentation` (lines 150-165):

```rust
pub(crate) fn apply_indentation(text: &str, indent: &[u8], line_ending: LineEnding) -> Vec<u8> {
    let newline = line_ending.as_bytes();
    let mut result = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            result.extend_from_slice(newline);
        }
        if !line.is_empty() {
            result.extend_from_slice(indent);
            result.extend_from_slice(line.as_bytes());
        }
    }
    if text.ends_with('\n') || text.ends_with("\r\n") {
        result.extend_from_slice(newline);
    }
    result
}
```

- [ ] **Step 2: Update all callers of apply_indentation**

Every caller needs to pass a `LineEnding` parameter. For each call site:

1. **build_insert_before** (~line 190): Add `line_ending: LineEnding` parameter, detect from `file_content`, pass to `apply_indentation`. Also normalize the separator bytes.
2. **build_insert_after** (~line 212): Same — add parameter, detect, pass through. Normalize `b"\n\n"` separator to use detected ending.
3. **replace_symbol_body** in `src/protocol/tools.rs` (~line 3025): Detect from file content, pass through.
4. **edit_within_symbol** in `src/protocol/tools.rs` (~line 3275): Detect from file content, pass through.
5. **execute_batch_edit** in `src/protocol/edit.rs` (~line 615): Detect once per file.
6. **execute_batch_insert** in `src/protocol/edit.rs`: Detect once per file.

For each caller, the pattern is:
```rust
let line_ending = detect_line_ending(file_content);
// ... then pass line_ending to apply_indentation and normalize any generated separators
```

- [ ] **Step 3: Update build_insert_before and build_insert_after signatures**

```rust
pub(crate) fn build_insert_before(
    file_content: &[u8],
    sym: &SymbolRecord,
    new_code: &str,
    line_ending: LineEnding,
) -> Vec<u8> {
    let indent = detect_indentation(file_content, sym.byte_range.0);
    let indented = apply_indentation(new_code, &indent, line_ending);
    let newline = line_ending.as_bytes();
    let line_start = sym.byte_range.0;

    // CRLF-aware blank line detection: check for \n\n (LF) or \r\n\r\n (CRLF)
    let prefix = &file_content[..line_start as usize];
    let already_has_blank = match line_ending {
        LineEnding::CrLf => {
            prefix.len() >= 4
                && prefix[prefix.len() - 1] == b'\n'
                && prefix[prefix.len() - 2] == b'\r'
                && prefix[prefix.len() - 3] == b'\n'
                && prefix[prefix.len() - 4] == b'\r'
        }
        LineEnding::Lf => {
            prefix.len() >= 2
                && prefix[prefix.len() - 1] == b'\n'
                && prefix[prefix.len() - 2] == b'\n'
        }
    };
    let separator = if already_has_blank { newline } else {
        // Need double newline — build from line_ending
        &[newline, newline].concat()  // Note: allocates; or use a local buffer
    };
    // ... rest of function uses separator and indented
}

pub(crate) fn build_insert_after(
    file_content: &[u8],
    sym: &SymbolRecord,
    new_code: &str,
    line_ending: LineEnding,
) -> Vec<u8> {
    let indent = detect_indentation(file_content, sym.byte_range.0);
    let indented = apply_indentation(new_code, &indent, line_ending);
    let newline = line_ending.as_bytes();
    let mut insertion = Vec::new();
    insertion.extend_from_slice(newline);
    insertion.extend_from_slice(newline);
    insertion.extend_from_slice(&indented);
    // ...
}
```

- [ ] **Step 4: Compile check — fix all callers**

Run: `cargo check 2>&1 | head -50`
Fix each compilation error by adding `line_ending` parameter at call sites.

- [ ] **Step 5: Run existing tests**

Run: `cargo test --all-targets -- --test-threads=1 -q 2>&1 | tail -5`
Expected: All existing tests pass (they use LF content, so `detect_line_ending` returns `Lf`).

- [ ] **Step 6: Commit**

```bash
git add src/protocol/edit.rs src/protocol/tools.rs
git commit -m "refactor: thread LineEnding through edit helpers (C1)"
```

### Task 8: Update build_delete and collapse_blank_lines for CRLF

**Files:**
- Modify: `src/protocol/edit.rs:230-321` (build_delete, collapse_blank_lines)

- [ ] **Step 1: Update collapse_blank_lines to be CRLF-aware**

The current implementation counts `\n` bytes. For CRLF files, it must count `\r\n` pairs:

```rust
fn collapse_blank_lines(content: &[u8], line_ending: LineEnding) -> Vec<u8> {
    let newline = line_ending.as_bytes();
    let newline_len = newline.len();
    let mut result = Vec::with_capacity(content.len());
    let mut consecutive_newlines = 0u32;
    let mut i = 0;

    while i < content.len() {
        let is_newline = match line_ending {
            LineEnding::CrLf => {
                i + 1 < content.len() && content[i] == b'\r' && content[i + 1] == b'\n'
            }
            LineEnding::Lf => content[i] == b'\n',
        };

        if is_newline {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                result.extend_from_slice(newline);
            }
            i += newline_len;
        } else {
            consecutive_newlines = 0;
            result.push(content[i]);
            i += 1;
        }
    }
    result
}
```

- [ ] **Step 2: Update build_delete for CRLF-aware trailing newline trimming**

In `build_delete` (lines 230-303), add `line_ending: LineEnding` parameter and
update all bare `\n` checks. The function has 4 spots that scan for `\n`:

```rust
pub(crate) fn build_delete(
    file_content: &[u8],
    sym: &SymbolRecord,
    line_ending: LineEnding,
) -> Vec<u8> {
    let start = sym.byte_range.0 as usize;
    let end = sym.byte_range.1 as usize;
    let newline = line_ending.as_bytes();
    let newline_len = newline.len();

    // 1. Walk backwards from symbol start to find beginning of line
    //    (skip leading whitespace on the symbol's line)
    let mut line_start = start;
    while line_start > 0 && file_content[line_start - 1] != b'\n' {
        line_start -= 1;
    }

    // 2. Walk forward from symbol end to consume trailing newline(s)
    //    CRLF-aware: check for \r\n pair, not just \n
    let mut line_end = end;
    match line_ending {
        LineEnding::CrLf => {
            // Consume one \r\n after symbol end
            if line_end + 1 < file_content.len()
                && file_content[line_end] == b'\r'
                && file_content[line_end + 1] == b'\n'
            {
                line_end += 2;
            }
        }
        LineEnding::Lf => {
            if line_end < file_content.len() && file_content[line_end] == b'\n' {
                line_end += 1;
            }
        }
    }

    // 3. Delete the range and collapse excessive blank lines
    let mut result = Vec::with_capacity(file_content.len());
    result.extend_from_slice(&file_content[..line_start]);
    result.extend_from_slice(&file_content[line_end..]);

    collapse_blank_lines(&result, line_ending)
}
```

**Key change:** The trailing newline consumption (step 2) checks for `\r\n` pair
on CRLF files instead of bare `\n`, preventing orphan `\r` bytes.

- [ ] **Step 3: Update callers of build_delete and collapse_blank_lines**

Both are called from tool handlers — detect line ending at the call site and pass through.

- [ ] **Step 4: Compile and run existing tests**

Run: `cargo test --all-targets -- --test-threads=1 -q 2>&1 | tail -5`
Expected: All existing tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/protocol/edit.rs src/protocol/tools.rs
git commit -m "fix: make build_delete and collapse_blank_lines CRLF-aware (C1)"
```

### Task 9: Add CRLF-specific tests

**Files:**
- Modify: `src/protocol/edit.rs` (test module)

- [ ] **Step 1: Write CRLF round-trip test for apply_indentation**

```rust
#[test]
fn test_apply_indentation_preserves_crlf() {
    let text = "fn foo() {\n    bar();\n}\n";
    let indent = b"    ";
    let result = apply_indentation(text, indent, LineEnding::CrLf);

    // Must contain at least one \r\n (the input has 3 newlines)
    assert!(
        result.windows(2).any(|w| w == b"\r\n"),
        "output should contain CRLF sequences"
    );

    // No bare \n allowed — every \n must be preceded by \r
    for (i, &byte) in result.iter().enumerate() {
        if byte == b'\n' {
            assert!(
                i > 0 && result[i - 1] == b'\r',
                "bare LF found at byte {i}, expected CRLF"
            );
        }
    }
}
```

- [ ] **Step 2: Write CRLF insert_before round-trip test**

```rust
#[test]
fn test_build_insert_before_crlf_preserved() {
    let file_content = b"struct Foo {\r\n}\r\n";
    let sym = make_test_symbol("Foo", SymbolKind::Struct, (0, 16), 1);
    let new_code = "/// A doc comment\nstruct Bar {}";
    let line_ending = detect_line_ending(file_content);
    assert_eq!(line_ending, LineEnding::CrLf);

    let result = build_insert_before(file_content, &sym, new_code, line_ending);
    // Verify no bare \n in output
    for (i, &byte) in result.iter().enumerate() {
        if byte == b'\n' && (i == 0 || result[i - 1] != b'\r') {
            panic!("bare LF found at byte {i} in CRLF output");
        }
    }
}
```

- [ ] **Step 3: Write CRLF delete test (no orphan \\r)**

```rust
#[test]
fn test_build_delete_crlf_no_orphan_cr() {
    let file_content = b"fn keep() {}\r\n\r\nfn remove() {}\r\n\r\nfn also_keep() {}\r\n";
    let sym = make_test_symbol("remove", SymbolKind::Function, (16, 32), 3);
    let line_ending = detect_line_ending(file_content);
    assert_eq!(line_ending, LineEnding::CrLf);

    let result = build_delete(file_content, &sym, line_ending);
    // No orphan \r (a \r not followed by \n)
    for (i, &byte) in result.iter().enumerate() {
        if byte == b'\r' && (i + 1 >= result.len() || result[i + 1] != b'\n') {
            panic!("orphan \\r at byte {i}");
        }
    }
}
```

- [ ] **Step 4: Write collapse_blank_lines CRLF test**

```rust
#[test]
fn test_collapse_blank_lines_crlf() {
    // 4 consecutive CRLF newlines (3 blank lines) → collapse to 2
    let input = b"line1\r\n\r\n\r\n\r\nline2\r\n";
    let result = collapse_blank_lines(input, LineEnding::CrLf);
    assert_eq!(result, b"line1\r\n\r\nline2\r\n");
}
```

- [ ] **Step 5: Write LF-stays-LF test**

```rust
#[test]
fn test_lf_file_stays_lf_after_edit() {
    let file_content = b"fn keep() {}\n\nfn target() {}\n";
    let line_ending = detect_line_ending(file_content);
    assert_eq!(line_ending, LineEnding::Lf);
    // Verify no \r introduced
    let sym = make_test_symbol("target", SymbolKind::Function, (14, 28), 3);
    let new_code = "fn replacement() {\n    todo!();\n}";
    let indented = apply_indentation(new_code, b"", line_ending);
    assert!(!indented.contains(&b'\r'), "no \\r should be introduced in LF file");
}
```

- [ ] **Step 6: Write batch_edit CRLF test (spec-required)**

```rust
#[test]
fn test_batch_edit_crlf_multiple_replacements() {
    // Verify that batch_edit with multiple replacements in a CRLF file
    // preserves \r\n throughout. This tests the per-file detection pattern.
    let file_content = b"fn alpha() {}\r\nfn beta() {}\r\nfn gamma() {}\r\n";
    let line_ending = detect_line_ending(file_content);
    assert_eq!(line_ending, LineEnding::CrLf);

    // Simulate two edits: normalize inserted text to CRLF
    let replacement = normalize_line_endings(b"fn new_alpha() {\n    todo!();\n}", line_ending);
    // Verify the replacement itself uses CRLF
    assert!(
        replacement.windows(2).any(|w| w == b"\r\n"),
        "replacement should use CRLF"
    );
    for (i, &byte) in replacement.iter().enumerate() {
        if byte == b'\n' {
            assert!(i > 0 && replacement[i - 1] == b'\r', "bare LF in replacement at {i}");
        }
    }
}
```

- [ ] **Step 7: Write no-mixed-endings invariant test (spec-required)**

```rust
#[test]
fn test_crlf_edit_no_mixed_endings() {
    // After editing a CRLF file, the output must not contain mixed endings
    let file_content = b"line1\r\nline2\r\nline3\r\n";
    let line_ending = detect_line_ending(file_content);
    let normalized = normalize_line_endings(b"inserted\ntext\n", line_ending);

    // Splice into file
    let result = apply_splice(file_content, (7, 14), &normalized); // replace "line2\r\n"

    // Invariant: no bare \n in a CRLF file
    for (i, &byte) in result.iter().enumerate() {
        if byte == b'\n' {
            assert!(
                i > 0 && result[i - 1] == b'\r',
                "mixed endings: bare LF at byte {i} in CRLF output"
            );
        }
    }
}
```

- [ ] **Step 8: Run all C1 tests**

Run: `cargo test test_detect_line test_normalize test_apply_indentation_preserves_crlf test_build_insert_before_crlf test_build_delete_crlf test_collapse_blank_lines_crlf test_lf_file_stays -- --test-threads=1 -v`
Expected: All PASS

- [ ] **Step 7: Run full test suite**

Run: `cargo test --all-targets -- --test-threads=1 -q 2>&1 | tail -5`
Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add src/protocol/edit.rs
git commit -m "fix: preserve CRLF line endings in surgical edits (C1)"
```

---

## Chunk 4: C4 — Splice Overlap Validation in batch_rename

### Task 10: Add validate_rename_ranges helper

**Files:**
- Modify: `src/protocol/edit.rs` (add helper near execute_batch_rename, ~line 840)

- [ ] **Step 1: Write the validation function**

Add before `execute_batch_rename`:

```rust
/// Validate rename ranges for a single file. Returns Ok(ranges) with validated,
/// non-overlapping ranges sorted descending by start offset.
/// Errors on: overlapping ranges, out-of-bounds, or text mismatch.
fn validate_rename_ranges(
    ranges: &mut Vec<(u32, u32)>,
    original: &[u8],
    old_name: &str,
    file_path: &str,
) -> Result<(), String> {
    let old_bytes = old_name.as_bytes();

    // Sort descending by (start, end) — note: current code only sorts by start
    ranges.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    ranges.dedup();

    for &(start, end) in ranges.iter() {
        if start >= end {
            return Err(format!(
                "{file_path}: invalid range ({start}, {end}): start >= end"
            ));
        }
        if end as usize > original.len() {
            return Err(format!(
                "{file_path}: range ({start}, {end}) exceeds file length {}",
                original.len()
            ));
        }
        let actual = &original[start as usize..end as usize];
        if actual != old_bytes {
            return Err(format!(
                "{file_path}: range ({start}, {end}) contains {:?}, expected {:?}",
                String::from_utf8_lossy(actual),
                old_name
            ));
        }
    }

    // Check for overlaps: ranges are descending, so prev.start >= curr.start
    for window in ranges.windows(2) {
        let prev = window[0]; // higher offset
        let curr = window[1]; // lower offset
        if curr.1 > prev.0 {
            return Err(format!(
                "{file_path}: overlapping ranges ({}, {}) and ({}, {})",
                curr.0, curr.1, prev.0, prev.1
            ));
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Write unit tests for validate_rename_ranges**

```rust
#[test]
fn test_validate_rename_ranges_exact_dedup() {
    let content = b"foo bar foo baz foo";
    let mut ranges = vec![(0, 3), (8, 11), (16, 19), (8, 11)]; // duplicate (8,11)
    validate_rename_ranges(&mut ranges, content, "foo", "test.rs").unwrap();
    assert_eq!(ranges.len(), 3, "duplicate should be deduped");
}

#[test]
fn test_validate_rename_ranges_overlap_rejected() {
    let content = b"foobarbaz";
    let mut ranges = vec![(0, 5), (3, 8)]; // overlapping
    let result = validate_rename_ranges(&mut ranges, content, "fooba", "test.rs");
    assert!(result.is_err(), "overlapping ranges must be rejected");
    assert!(result.unwrap_err().contains("overlapping"));
}

#[test]
fn test_validate_rename_ranges_contained_rejected() {
    let content = b"aaa_foo_bbb_foo_ccc";
    let mut ranges = vec![(0, 19), (4, 7)]; // contained
    let result = validate_rename_ranges(&mut ranges, content, "foo", "test.rs");
    assert!(result.is_err(), "contained ranges must be rejected");
}

#[test]
fn test_validate_rename_ranges_adjacent_allowed() {
    let content = b"foofoofoo";
    let mut ranges = vec![(0, 3), (3, 6), (6, 9)];
    validate_rename_ranges(&mut ranges, content, "foo", "test.rs").unwrap();
    assert_eq!(ranges.len(), 3);
}

#[test]
fn test_validate_rename_ranges_text_mismatch() {
    let content = b"foo bar baz";
    let mut ranges = vec![(4, 7)]; // "bar", not "foo"
    let result = validate_rename_ranges(&mut ranges, content, "foo", "test.rs");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("contains"));
}
```

- [ ] **Step 3: Run validation tests**

Run: `cargo test test_validate_rename_ranges -- --test-threads=1 -v`
Expected: All 5 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/protocol/edit.rs
git commit -m "feat: add validate_rename_ranges helper (C4 prep)"
```

### Task 11: Integrate validation into execute_batch_rename

**Files:**
- Modify: `src/protocol/edit.rs:838-901` (Phase 3 dedup + Phase 4 apply loop)

- [ ] **Step 1: Replace Phase 3 dedup with validate_rename_ranges**

In `execute_batch_rename`, replace the Phase 3 sort+dedup block (around lines 838-842):

```rust
// OLD:
// for ranges in by_file.values_mut() {
//     ranges.sort_by(|a, b| b.0.cmp(&a.0));
//     ranges.dedup();
// }

// NEW: validate each file's ranges against original content
for (path, ranges) in by_file.iter_mut() {
    let file = {
        let guard = index.read().expect("lock poisoned");
        guard
            .capture_shared_file(path)
            .ok_or_else(|| format!("File disappeared: {path}"))?
    };
    validate_rename_ranges(ranges, &file.content, &input.name, path)?;
}
```

- [ ] **Step 2: Add debug assertion in Phase 4 apply loop**

In the apply loop (around line 901), add a debug assertion:

```rust
let mut last_start: Option<u32> = None;
for range in ranges {
    debug_assert!(
        last_start.map_or(true, |prev| range.0 < prev),
        "ranges must be strictly descending: {} not < {:?}",
        range.0, last_start
    );
    new_content = apply_splice(&new_content, *range, new_name_bytes);
    last_start = Some(range.0);
}
```

- [ ] **Step 3: Update dry-run reporting to show validation stats**

In the dry-run block, add dedup/validation info:

```rust
// After validation, report accurate counts
lines.push(format!(
    "\n── Confident matches (will be applied) — {} site(s) across {} file(s) ──",
    total_confident,
    by_file.len(),
));
```

- [ ] **Step 4: Run existing batch_rename tests**

Run: `cargo test test_batch_rename -- --test-threads=1 -v`
Expected: All existing tests PASS

- [ ] **Step 5: Write test for length-changing rename with close refs**

```rust
#[test]
fn test_batch_rename_length_change_close_refs() {
    // Content with "ab" appearing 3 times close together
    let content = b"ab ab ab";
    // Ranges: (6,8), (3,5), (0,2) — already descending
    let mut ranges = vec![(0u32, 2u32), (3, 5), (6, 8)];
    validate_rename_ranges(&mut ranges, content, "ab", "test.rs").unwrap();

    // Apply in descending order: rename "ab" → "xyz" (length change: 2→3)
    let new_name = b"xyz";
    let mut result = content.to_vec();
    for range in &ranges {
        result = apply_splice(&result, *range, new_name);
    }
    assert_eq!(result, b"xyz xyz xyz");
}
```

- [ ] **Step 6: Write dry-run reporting test (spec-required)**

```rust
#[test]
fn test_validate_rename_ranges_dedup_count() {
    // Verify that post-dedup count is accurate
    let content = b"foo + foo + foo";
    let mut ranges = vec![(0u32, 3u32), (6, 9), (12, 15), (6, 9)]; // one duplicate
    validate_rename_ranges(&mut ranges, content, "foo", "test.rs").unwrap();
    assert_eq!(ranges.len(), 3, "post-dedup should have 3 unique ranges");
}
```

- [ ] **Step 7: Run all C4 tests**

Run: `cargo test test_validate_rename test_batch_rename -- --test-threads=1 -v`
Expected: All PASS

- [ ] **Step 8: Commit**

```bash
git add src/protocol/edit.rs
git commit -m "fix: validate splice overlap in batch_rename (C4)"
```

---

## Chunk 5: C5 — Daemon SIGTERM Handling

### Task 12: Add SIGTERM handling to run_daemon_until_shutdown

**Files:**
- Modify: `src/daemon.rs:1017-1023` (run_daemon_until_shutdown)

- [ ] **Step 1: Rewrite run_daemon_until_shutdown**

Replace the function body (lines 1017-1023):

```rust
pub async fn run_daemon_until_shutdown(bind_host: &str) -> anyhow::Result<()> {
    let handle = spawn_daemon(bind_host).await?;
    tracing::info!(port = handle.port, "shared daemon started");

    // Wait for either SIGINT (Ctrl+C) or SIGTERM (kill, systemd, containers).
    // Both trigger the same graceful shutdown path.
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("received SIGINT, shutting down");
            },
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM, shutting down");
            },
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
        tracing::info!("received Ctrl+C, shutting down");
    }

    let _ = handle.shutdown_tx.send(());
    Ok(())
}
```

- [ ] **Step 2: Compile check**

Run: `cargo check 2>&1 | head -20`
Expected: Compiles cleanly

- [ ] **Step 3: Run existing daemon tests**

Run: `cargo test --all-targets -- --test-threads=1 -q 2>&1 | tail -5`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/daemon.rs
git commit -m "fix: handle SIGTERM for daemon graceful shutdown (C5 signals)"
```

### Task 13: Improve terminate_process with direct signal API and idempotent semantics

**Files:**
- Modify: `src/daemon.rs:1537-1561` (terminate_process)

- [ ] **Step 1: Rewrite terminate_process**

Replace the function (lines 1537-1561):

```rust
fn terminate_process(pid: u32) -> io::Result<()> {
    #[cfg(windows)]
    {
        let status = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        // Treat "process not found" as success (idempotent termination)
        if status.success() || status.code() == Some(128) {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "taskkill exited with status {status}"
            )))
        }
    }

    #[cfg(not(windows))]
    {
        // Use direct signal API instead of shelling out to kill(1)
        let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if result == 0 {
            // Signal sent successfully — poll briefly for exit
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                // Check if process still exists (signal 0 = existence check)
                if unsafe { libc::kill(pid as i32, 0) } != 0 {
                    return Ok(()); // Process exited
                }
            }
            Ok(()) // Sent signal, process may still be shutting down
        } else {
            let errno = std::io::Error::last_os_error();
            if errno.raw_os_error() == Some(libc::ESRCH) {
                // ESRCH = no such process — already dead, treat as success
                Ok(())
            } else {
                Err(errno)
            }
        }
    }
}
```

- [ ] **Step 2: Add `libc` to dependencies if not already present**

Check: `grep 'libc' Cargo.toml`
If not present: `cargo add libc`

- [ ] **Step 3: Update stop_incompatible_recorded_daemon to clean up files on idempotent kill**

In the caller of `terminate_process` (~line 749), ensure `cleanup_daemon_files()`
is called regardless of whether the process was already dead:

```rust
// After terminate_process succeeds (including idempotent "already dead"):
cleanup_daemon_files();
```

- [ ] **Step 4: Compile and run tests**

Run: `cargo test --all-targets -- --test-threads=1 -q 2>&1 | tail -5`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/daemon.rs
git commit -m "fix: use direct signal API and idempotent termination (C5)"
```

### Task 14: Add C5 integration test (Unix only)

**Files:**
- Modify: `src/daemon.rs` (test module)

- [ ] **Step 1: Write SIGTERM test (Unix only)**

```rust
#[cfg(unix)]
#[tokio::test]
async fn test_terminate_process_idempotent_on_dead_pid() {
    // Terminating a nonexistent PID should succeed (idempotent)
    let dead_pid = 999_999u32; // Very unlikely to be a real process
    let result = terminate_process(dead_pid);
    assert!(result.is_ok(), "terminating dead PID should be idempotent, got: {result:?}");
}
```

- [ ] **Step 2: Write SIGTERM daemon integration test (spec-required, Unix only)**

```rust
#[cfg(unix)]
#[tokio::test]
async fn test_daemon_sigterm_graceful_shutdown() {
    use std::process::Command;
    use tokio::time::{timeout, Duration};

    let _lock = env_lock();

    // Spawn the daemon as a child process
    let current_exe = std::env::current_exe().unwrap();
    let mut child = Command::new(&current_exe)
        .arg("daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn daemon");

    let pid = child.id();

    // Give daemon a moment to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send SIGTERM
    unsafe { libc::kill(pid as i32, libc::SIGTERM); }

    // Wait for exit with timeout
    let exit_result = timeout(Duration::from_secs(5), async {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status,
                Ok(None) => tokio::time::sleep(Duration::from_millis(100)).await,
                Err(e) => panic!("error waiting for daemon: {e}"),
            }
        }
    }).await;

    assert!(exit_result.is_ok(), "daemon should exit within 5s of SIGTERM");
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test test_terminate_process_idempotent test_daemon_sigterm -- --test-threads=1 -v`
Expected: Both PASS

- [ ] **Step 4: Commit**

```bash
git add src/daemon.rs
git commit -m "test: add SIGTERM and idempotent termination tests (C5)"
```

- [ ] **Step 5: Squash C5 commits**

```bash
git add -A
git commit -m "fix: handle SIGTERM for daemon graceful shutdown (C5)"
```

---

## Chunk 6: C2-lite — Denylist Extension Hardening

### Task 15: Add new extensions to denylist

**Files:**
- Modify: `src/domain/index.rs:480-526` (DENYLISTED_EXTENSIONS array)

- [ ] **Step 1: Add 5 new extensions**

In `DENYLISTED_EXTENSIONS` (lines 480-526), add to the appropriate section:

```rust
// Executables and libraries
"exe", "dll", "so", "dylib", "class",
```

Add these entries alphabetically within a new comment group, or append to the
existing `// Binary` section near `"bin"`.

- [ ] **Step 2: Verify case-insensitive handling is preserved**

Check `is_denylisted_extension` (lines 528-530) still does `.to_lowercase()`:

```rust
pub fn is_denylisted_extension(ext: &str) -> bool {
    DENYLISTED_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}
```

This already handles mixed-case. No changes needed.

- [ ] **Step 3: Run existing denylist tests**

Run: `cargo test test_extension_is_denylisted test_extension_not_denylisted -- --test-threads=1 -v`
Expected: Existing tests pass (they don't test the new extensions yet)

- [ ] **Step 4: Commit**

```bash
git add src/domain/index.rs
git commit -m "fix: add exe/dll/so/dylib/class to denylist (C2-lite impl)"
```

### Task 16: Add C2-lite tests

**Files:**
- Modify: `src/domain/index.rs` (test module, ~line 759)

- [ ] **Step 1: Add new extension tests**

Update or add to the existing `test_extension_is_denylisted` test:

```rust
#[test]
fn test_new_executable_extensions_denylisted() {
    assert!(is_denylisted_extension("exe"));
    assert!(is_denylisted_extension("dll"));
    assert!(is_denylisted_extension("so"));
    assert!(is_denylisted_extension("dylib"));
    assert!(is_denylisted_extension("class"));
}

#[test]
fn test_denylist_case_insensitive() {
    assert!(is_denylisted_extension("DLL"));
    assert!(is_denylisted_extension("So"));
    assert!(is_denylisted_extension("EXE"));
    assert!(is_denylisted_extension("Dylib"));
    assert!(is_denylisted_extension("CLASS"));
}
```

- [ ] **Step 2: Add precedence test (tiny denylisted file → MetadataOnly)**

In `src/discovery/mod.rs` tests or `tests/admission_acceptance.rs`:

```rust
#[test]
fn test_tiny_denylisted_file_still_metadata_only() {
    // A 100-byte .exe file should still be MetadataOnly (denylist wins over size)
    let decision = classify_admission_by_extension("exe", 100);
    assert_eq!(decision.tier, AdmissionTier::MetadataOnly);
    assert_eq!(decision.skip_reason, Some(SkipReason::DenylistedExtension));
}
```

(Adapt to actual `classify_admission` API — may need to call the real function with a temp file.)

- [ ] **Step 3: Run all denylist tests**

Run: `cargo test test_new_executable test_denylist_case test_tiny_denylisted -- --test-threads=1 -v`
Expected: All PASS

- [ ] **Step 4: Run full test suite**

Run: `cargo test --all-targets -- --test-threads=1 -q 2>&1 | tail -5`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/domain/index.rs src/discovery/mod.rs
git commit -m "fix: add exe/dll/so/dylib/class to denylist (C2-lite)"
```

---

## Final Verification

### Task 17: Full test suite and formatting check

- [ ] **Step 1: Run full test suite**

```bash
cargo test --all-targets -- --test-threads=1
```
Expected: All tests pass (1230+ existing + ~25 new)

- [ ] **Step 2: Run formatting check**

```bash
cargo fmt -- --check
```
Expected: No formatting differences

- [ ] **Step 3: Run compilation check**

```bash
cargo check
```
Expected: Clean compilation, no warnings

- [ ] **Step 4: Review total test count increase**

```bash
cargo test --all-targets -- --test-threads=1 2>&1 | grep "test result"
```
Expected: Significant increase from 1230 baseline

- [ ] **Step 5: Final commit if any formatting fixes needed**

```bash
cargo fmt
git add -A
git commit -m "style: fix rustfmt formatting"
```
