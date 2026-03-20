# SymForge Index Safety & Multi-Tenant Coordination Plan

## Problem Statement

SymForge's in-memory index is the single source of truth for all code intelligence operations. When multiple agents (Claude Code sessions, Codex, IDE plugins) operate on the same repository concurrently, they can corrupt the index through:

1. **Concurrent file mutations** — two agents edit the same file simultaneously
2. **Watcher/edit race conditions** — the file watcher re-indexes a file while an edit tool is mid-mutation
3. **Dependency panics** — third-party code (e.g., `ignore` crate gitignore assertions) panics during index operations, leaving auxiliary indices inconsistent
4. **Stale reads** — an agent reads symbol positions from the index, but another agent has already modified the file, so byte offsets are wrong

## Current Defenses (Implemented)

### Layer 1: Insert-First Ordering
`LiveIndex::update_file` inserts the new file into the primary store (`self.files`) BEFORE updating auxiliary indices. If a panic occurs during auxiliary index updates, the file is still present in the primary store and discoverable by search/read operations.

### Layer 2: Panic Catch & Auto-Repair
`SharedIndex::{update_file, add_file, remove_file}` wrap `LiveIndex` mutations in `std::panic::catch_unwind`. On panic, `repair_file_indices(path)` is called, which:
- Scans the entire reverse index to remove ALL stale entries for the path
- Rebuilds trigram, reverse, and path indices from the primary store
- Logs the incident at ERROR level

### Layer 3: Gitignore Path Guard
`NoisePolicy::classify_path` guards against absolute paths reaching the `ignore` crate's `matched_path_or_any_parents`, which asserts `!path.has_root()`. Absolute paths (from watcher events on Windows, concurrent agents, or daemon proxy mismatches) are safely skipped.

## Proposed Architecture: Write-Serialized Index with Epoch Validation

### Design Principle
**No conflicts possible** — all mutations are serialized through a single writer channel. Reads are lock-free against a consistent snapshot. Edits validate against the epoch that was read, rejecting stale writes.

### Layer 4: Mutation Queue (Operation Serializer)

```
Agent A ──┐
Agent B ──┤──→ [MutationQueue] ──→ [Single Writer Thread] ──→ LiveIndex
Watcher ──┘         (mpsc)              (exclusive owner)
```

**Implementation:**
- Replace direct `SharedIndex::update_file` calls with `MutationQueue::enqueue(MutationOp)`
- `MutationOp` enum: `UpdateFile { path, file }`, `RemoveFile { path }`, `AddFile { path, file }`
- Single background thread owns `&mut LiveIndex`, processes ops sequentially
- Callers receive a `oneshot::Receiver<MutationResult>` for completion notification
- Queue is bounded (e.g., 1024 ops) with backpressure — callers block if full

**Benefits:**
- Zero contention — only one thread ever mutates the index
- Operations are naturally ordered — no interleaving of partial updates
- Backpressure prevents OOM from watcher event storms

### Layer 5: Epoch-Based Optimistic Concurrency Control (OCC)

Each index snapshot carries a monotonically increasing **epoch** counter. Every mutation increments the epoch.

**Edit flow:**
1. Agent reads symbol `Foo` at epoch 42 → gets byte range `(100, 200)`
2. Agent prepares edit, submits `EditOp { path, symbol, epoch: 42, ... }`
3. Writer thread checks: current epoch for `path` == 42?
   - **Yes** → apply edit, increment epoch to 43, return success
   - **No** → reject with `StaleEpoch { expected: 42, current: 43 }` → agent must re-read

**Implementation:**
- Add `epoch: u64` field to `IndexedFile` (defaults to 0, incremented on every mutation)
- Add `file_epoch: u64` field to `SharedIndex` published state
- Edit tools include `expected_epoch` in their input (optional, backward-compatible)
- When present, mutation queue validates epoch before applying

**Benefits:**
- Impossible to apply an edit based on stale symbol positions
- Zero-cost when epoch validation is not used (backward compatible)
- No locks needed for the validation check

### Layer 6: File-Level Advisory Locks

For cross-process coordination (multiple daemon instances, IDE + CLI):

**Implementation:**
- Before writing to disk, acquire a file-level advisory lock (`flock` on Unix, `LockFileEx` on Windows)
- Lock is held only during: read-file → compute-edit → write-file → release
- Non-blocking try-lock with retry: if another process holds the lock, wait up to 500ms then fail gracefully
- Lock file location: `<repo>/.symforge/locks/<path-hash>.lock`

**Benefits:**
- Cross-process safety without requiring a central coordinator
- Graceful degradation — lock failure returns an error, doesn't corrupt

### Layer 7: Write-Ahead Log (WAL)

For crash recovery and audit trail:

**Implementation:**
- Before applying a mutation, write `{ op, path, epoch, timestamp }` to `<repo>/.symforge/wal.jsonl`
- After successful mutation, mark entry as committed
- On startup, replay uncommitted WAL entries
- WAL is truncated when epoch reaches a checkpoint (e.g., every 100 mutations)

**Benefits:**
- Crash recovery without full re-index
- Audit trail for debugging multi-agent issues
- Enables future "undo last N mutations" capability

## Implementation Phases

### Phase 1: Epoch-Based OCC (Low effort, high impact)
- Add `epoch` field to `IndexedFile`
- Add `expected_epoch` to edit tool inputs
- Validate epoch in edit handlers before writing
- **Prevents:** stale-read edits from overwriting concurrent changes

### Phase 2: Mutation Queue (Medium effort, eliminates contention)
- Implement `MutationQueue` with bounded mpsc channel
- Migrate `SharedIndex::{update,add,remove}_file` to queue-based dispatch
- Watcher events go through the same queue
- **Prevents:** all interleaving and race conditions

### Phase 3: File Advisory Locks (Medium effort, cross-process safety)
- Implement platform-specific locking (`flock`/`LockFileEx`)
- Integrate into `atomic_write_file` in `edit.rs`
- **Prevents:** cross-process file corruption

### Phase 4: WAL (Higher effort, crash recovery)
- Implement append-only log with commit markers
- Replay logic on startup
- Periodic truncation/compaction
- **Prevents:** index loss on crash, enables audit trail

## Success Criteria

- No index mutation can leave the primary store inconsistent (already achieved)
- No auxiliary index can become permanently stale (already achieved via repair)
- No two mutations can interleave (Phase 2)
- No edit can apply against stale symbol positions (Phase 1)
- No cross-process write conflict can corrupt a file (Phase 3)
- No crash can lose committed mutations (Phase 4)
