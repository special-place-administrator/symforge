# Sprint 16 Smoke Test — Parallelism & Batch Operations

Run this after installing the latest Tokenizor MCP locally.
Each section can be run independently. All use the tokenizor project itself as the test repo.

## Prerequisites

```bash
# 1. Build and install
cargo build --release
# 2. Index the project
tokenizor index .
# 3. Verify health
# Use MCP: health()
```

---

## 1. Concurrent Session Stress (C6 TOCTOU fix)

**Goal:** Verify multiple sessions can open the same project without panic or duplicate watchers.

```
# Open 3 sessions rapidly for the same project root in parallel.
# Each should get a unique session_id but share the same project_id.

MCP call: open_project_session({ project_root: ".", client_name: "stress-1" })
MCP call: open_project_session({ project_root: ".", client_name: "stress-2" })
MCP call: open_project_session({ project_root: ".", client_name: "stress-3" })

Expected:
- All 3 return successfully
- All share the same project_id
- session_count increments (1, 2, 3 or similar)
- No panic in daemon logs
```

---

## 2. Batch Edit — Multi-file CRLF Preservation (C1 + batch)

**Goal:** Verify batch_edit preserves line endings across multiple files.

```
# Step 1: Create two test files with CRLF endings
echo -e "fn alpha() {\r\n    todo!();\r\n}\r\n" > /tmp/test_crlf_a.rs
echo -e "fn beta() {\r\n    todo!();\r\n}\r\n" > /tmp/test_crlf_b.rs

# Step 2: Index a temp folder containing them
MCP call: index_folder({ path: "/tmp/test_crlf_project" })

# Step 3: batch_edit both files — replace todo!() with println!()
MCP call: batch_edit({
  edits: [
    { path: "test_crlf_a.rs", name: "alpha", operation: "replace", old_text: "todo!()", new_text: "println!(\"hello\")" },
    { path: "test_crlf_b.rs", name: "beta", operation: "replace", old_text: "todo!()", new_text: "println!(\"world\")" }
  ]
})

Expected:
- Both files edited successfully
- \r\n preserved throughout (no bare \n introduced)
- Verify: xxd test_crlf_a.rs | grep "0d 0a" shows CRLF pairs
```

---

## 3. Batch Insert — Parallel Symbol Insertion (C1 + batch)

**Goal:** Insert code before/after symbols across multiple files in one call.

```
MCP call: batch_insert({
  inserts: [
    { path: "src/protocol/edit.rs", name: "apply_splice", position: "before", code: "/// Splice helper for byte-range replacement." },
    { path: "src/protocol/edit.rs", name: "atomic_write_file", position: "before", code: "/// Atomically write file contents via tempfile." }
  ],
  dry_run: true
})

Expected:
- dry_run shows both insertions planned
- No actual file changes
- Indentation matches surrounding code
```

---

## 4. Batch Rename — Overlap Validation (C4)

**Goal:** Verify the new splice overlap validation catches bad ranges.

```
# Step 1: Find a symbol with multiple references
MCP call: find_references({ name: "LineEnding", path: "src/protocol/edit.rs" })

# Step 2: Dry-run rename
MCP call: batch_rename({
  path: "src/protocol/edit.rs",
  name: "LineEnding",
  new_name: "LineEndingStyle",
  dry_run: true
})

Expected:
- Shows confident matches with post-validation count
- No overlapping ranges in the output
- Uncertain matches listed separately
- References across src/protocol/tools.rs also found
```

---

## 5. Concurrent Search + Edit (Parallelism)

**Goal:** Run read and write operations concurrently without deadlock.

```
# Fire these in rapid succession (or parallel if your client supports it):

MCP call: search_text({ query: "ActivationState", limit: 20 })
MCP call: search_symbols({ name: "ProjectInstance", limit: 10 })
MCP call: get_symbol_context({ name: "open_project_session", path: "src/daemon.rs" })
MCP call: get_repo_map({ detail: "signatures" })
MCP call: find_references({ name: "atomic_write_file" })

Expected:
- All 5 return results (no timeout, no deadlock)
- Results are consistent (no partial/stale data)
- Daemon CPU doesn't spike to 100% and stay there
```

---

## 6. Explore — Concept-Driven Parallel Search

**Goal:** Exercise the explore tool which does internal parallel searches.

```
MCP call: explore({ query: "How does the circuit breaker decide when to stop parsing files?" })

Expected:
- Returns relevant results from src/live_index/store.rs
- Mentions CircuitBreakerState, threshold, should_abort
- No timeout or error
```

---

## 7. What Changed — Post-Edit Consistency

**Goal:** Verify the index stays consistent after edits.

```
# Step 1: Note current state
MCP call: what_changed({ since: "uncommitted" })

# Step 2: Make a trivial edit via MCP
MCP call: edit_within_symbol({
  path: "src/protocol/edit.rs",
  name: "detect_line_ending",
  old_text: "let mut crlf_count: usize = 0;",
  new_text: "let mut crlf_count: usize = 0; // CR1-safe",
  dry_run: true
})

# Step 3: If not dry_run, check what_changed shows the edit
MCP call: what_changed({ since: "uncommitted" })

Expected:
- what_changed accurately reports modified files
- Symbol index updated after edit
- No stale data
```

---

## 8. Atomic Write Stress (C3)

**Goal:** Verify tempfile-based atomic writes under load.

```
# This is best tested programmatically, but you can approximate with rapid edits:

# Make 5 rapid edit_within_symbol calls to the same file:
MCP call: edit_within_symbol({ path: "src/domain/index.rs", name: "is_denylisted_extension", old_text: ".to_lowercase()", new_text: ".to_lowercase()", dry_run: true })
# Repeat 4 more times rapidly

Expected:
- All succeed or cleanly report conflicts
- No orphan .tokenizor_tmp files in the directory
- File content is never corrupted
- Check: ls src/domain/ | grep tmp (should be empty)
```

---

## 9. Denylist Verification (C2-lite)

**Goal:** Confirm new extensions are denied.

```
# Create test files with new denylisted extensions
echo "binary" > /tmp/test.exe
echo "binary" > /tmp/test.dll
echo "binary" > /tmp/test.so
echo "binary" > /tmp/test.dylib
echo "binary" > /tmp/test.class

# Index a folder containing them
MCP call: index_folder({ path: "/tmp/denylist_test" })

# Check file context — should show MetadataOnly tier
MCP call: get_file_context({ path: "test.exe" })

Expected:
- All 5 files classified as MetadataOnly
- Not parsed for symbols
- Regular .rs/.ts files in same folder still parsed normally
```

---

## 10. Full Pipeline — Index + Search + Edit + Verify

**Goal:** End-to-end workflow exercising multiple Sprint 16 fixes.

```
# 1. Index
MCP call: index_folder({ path: "." })

# 2. Search for the new ActivationState enum
MCP call: search_symbols({ name: "ActivationState", kind: "enum" })

# 3. Get its context (callers, callees)
MCP call: get_symbol_context({ name: "ActivationState", path: "src/daemon.rs", sections: [] })

# 4. Find all references
MCP call: find_references({ name: "ActivationState" })

# 5. Dry-run rename
MCP call: batch_rename({ path: "src/daemon.rs", name: "ActivationState", new_name: "ProjectActivationState", dry_run: true })

# 6. Check impact
MCP call: analyze_file_impact({ path: "src/daemon.rs" })

Expected:
- Each step succeeds
- References found in daemon.rs (definition + usage sites)
- Dry-run shows all rename sites
- Impact analysis shows symbol changes
```

---

## Pass/Fail Checklist

| # | Test | Pass? |
|---|------|-------|
| 1 | Concurrent sessions — no panic | |
| 2 | Batch edit — CRLF preserved | |
| 3 | Batch insert — dry_run works | |
| 4 | Batch rename — overlap validation | |
| 5 | Concurrent reads — no deadlock | |
| 6 | Explore — parallel search | |
| 7 | What changed — post-edit consistency | |
| 8 | Atomic write stress — no orphans | |
| 9 | Denylist — new extensions denied | |
| 10 | Full pipeline — end-to-end | |
