# Bug Audit: live_index/ and domain/ subsystems

Date: 2026-03-20
Auditor: Claude Opus 4.6

## Bug 1: Jaccard co-change denominator inflated by duplicate file paths

**File:** `src/live_index/git_temporal.rs`, lines 314-348 vs 371-387
**Severity:** Wrong query results (incorrect co-change coupling scores)

**The bug:** `file_commit_indices` accumulates commit indices per file WITHOUT deduplication (line 326-328):
```rust
for file_path in &commit.files {
    file_commit_indices.entry(file_path.clone()).or_default().push(idx);
```

But the pair counting (line 372-374) deduplicates file paths per commit:
```rust
let mut sorted_files: Vec<&str> = commit.files.iter().map(|s| s.as_str()).collect();
sorted_files.sort_unstable();
sorted_files.dedup();
```

Later, Jaccard is computed as `shared / (count_a + count_b - shared)` where `count_a = file_commit_indices[a].len()`. If a commit lists the same file twice (e.g., git renames show both old and new paths, or merge commits), `count_a` is inflated, making the union too large and the Jaccard coefficient too small.

**Correct behavior:** Either dedup `file_commit_indices` entries per commit, or don't dedup in pair counting. They must be consistent.

**Fix:**
```rust
// After pushing idx, or build a HashSet per file per commit:
file_commit_indices.entry(file_path.clone()).or_default().push(idx);
// Should be followed by dedup after the commit loop, or use a set.
```

---

## Bug 2: `truncate_message` mixes byte length with char count

**File:** `src/live_index/git_temporal.rs`, lines 582-589
**Severity:** Wrong results for multi-byte UTF-8 commit messages

**The bug:**
```rust
fn truncate_message(msg: &str, max_len: usize) -> String {
    if msg.len() <= max_len {        // msg.len() is BYTE length
        msg.to_string()
    } else {
        let truncated: String = msg.chars().take(max_len.saturating_sub(3)).collect();
        //                           ^^^^^ CHARACTER count
        format!("{truncated}...")
    }
}
```

The guard condition uses `msg.len()` (byte count) but truncation uses `.chars().take()` (character count). For a message with multi-byte characters (e.g., CJK, emoji), the byte length exceeds the char count. Example: a 20-character CJK message might have 60 bytes. With `max_len=72`:
- `msg.len()` = 60 <= 72 -> returns full message (correct)

But consider a 25-character CJK message (75 bytes) with `max_len=72`:
- `msg.len()` = 75 > 72 -> enters truncation branch
- `msg.chars().take(69)` -> takes all 25 chars (25 < 69)
- Returns the full message + "..." even though the original was only 25 chars

The result is adding unnecessary "..." to messages that are short in character count but long in byte count.

**Correct behavior:** Use `.chars().count()` consistently, or use byte length consistently with proper UTF-8 boundary handling.

---

## Bug 3: `parent_chain` in `inspect_match` silently drops symbols at the same depth

**File:** `src/live_index/query.rs`, lines 2088-2104
**Severity:** Silent data loss in inspect_match results

**The bug:**
```rust
let mut parent_chain: Vec<EnclosingSymbolView> = file
    .symbols
    .iter()
    .filter(|s| s.line_range.0 <= target_line_0 && s.line_range.1 >= target_line_0)
    .collect::<Vec<_>>()
    .into_iter()
    .map(|s| (s.depth, s))
    .collect::<Vec<_>>()
    .into_iter()
    .collect::<std::collections::BTreeMap<_, _>>()  // DEDUP BY DEPTH!
    .into_values()
    // ...
```

The code collects `(depth, symbol)` pairs into a `BTreeMap<u32, &SymbolRecord>`. A BTreeMap deduplicates by key (depth), so if two symbols at the same depth both contain the target line, only the LAST one inserted survives. This silently drops parent chain entries.

Example: Two `depth=1` methods in different impl blocks that both contain the target line -- one will be silently dropped from the parent chain.

**Correct behavior:** Use a `BTreeMap<u32, Vec<&SymbolRecord>>` or sort by depth without dedup, or pick the innermost (narrowest range) symbol at each depth level explicitly.

---

## Bug 4: `context_bundle_view` body extraction uses inconsistent range endpoints

**File:** `src/live_index/query.rs`, lines 1773-1778
**Severity:** Wrong body content for symbols with `item_byte_range`

**The bug:**
```rust
let start = sym_rec.effective_start() as usize;  // Uses doc_byte_range or byte_range.0
let end = sym_rec.byte_range.1 as usize;          // Uses core byte_range end
```

`effective_start()` includes doc comments (returns `doc_byte_range.0` if present, else `byte_range.0`). But `end` uses `byte_range.1` (the core symbol end), NOT `item_end()` which would include the full item range. For symbols with `item_byte_range`, this means:
- Start may be pulled earlier (to include docs), but...
- End does NOT use the item's full end byte

If `item_byte_range` is set and its end extends beyond `byte_range.1` (e.g., for decorators/attributes that extend past the function body), the body will be truncated. However, the `byte_count` on line 1778 uses `end.saturating_sub(start)` which will be consistent with the truncated body.

This is an inconsistency more than a crash-causing bug, but it produces wrong body content for symbols where `item_byte_range.1 > byte_range.1`.

---

## Bug 5: `capture_find_implementations_view` reports 0-based line numbers

**File:** `src/live_index/query.rs`, line 1528
**Severity:** Off-by-one in line number output

**The bug:**
```rust
entries.push(ImplementationEntryView {
    trait_name: trait_name.clone(),
    implementor: implementor.clone(),
    file_path: file_path.clone(),
    line: reference.line_range.0,   // 0-based!
});
```

All other view captures in this file convert line numbers to 1-based for display (e.g., line 1450: `reference.line_range.0 + 1`, line 1799: `reference.line_range.0 + 1`, line 840: `sym.line_range.0 + 1`). But `find_implementations` does NOT add 1, producing a 0-based line number in an API that everywhere else uses 1-based.

**Correct behavior:** Should be `line: reference.line_range.0 + 1`.

---

## Bug 6: `trace_symbol_view` sibling filter uses wrong comparison for `symbol_line`

**File:** `src/live_index/query.rs`, lines 1990-1998
**Severity:** Wrong sibling list when `symbol_line` is provided

**The bug:**
```rust
let target_depth = file
    .symbols
    .iter()
    .find(|s| {
        s.name == name
            && kind_filter
                .map(|k| s.kind.to_string().eq_ignore_ascii_case(k))
                .unwrap_or(true)
            && symbol_line.map(|l| s.line_range.0 == l).unwrap_or(true)
    })
```

When `symbol_line` is provided, it compares `s.line_range.0 == l` (0-based == user input). But `resolve_symbol_selector` (which was already called successfully) compares `symbol.line_range.0 + 1 == symbol_line` (line 425). So the same `symbol_line` value is treated as 0-based here but 1-based in the selector. If `symbol_line=5` (1-based), the selector matches the symbol at `line_range.0 == 4`, but this depth finder looks for `line_range.0 == 5` (a different symbol).

This means the wrong depth is extracted, resulting in the wrong set of siblings being returned.

**Correct behavior:** Should be `symbol_line.map(|l| s.line_range.0 + 1 == l).unwrap_or(true)` to match the 1-based convention used by `resolve_symbol_selector`.

---

## Non-bugs examined and rejected

1. **Lock ordering in `SharedIndexHandle::update_file`**: Acquires `live` then `pre_update_symbols` -- this follows the documented lock ordering (1 then 2). Not a bug.

2. **Trigram index case sensitivity**: Both indexing and search consistently lowercase. Not a bug.

3. **Circuit breaker `should_abort` Relaxed ordering**: The circuit breaker uses `Ordering::Relaxed` for atomic operations. While this means threads may see slightly stale values, this is acceptable for a best-effort abort check and won't produce wrong index data.

4. **`build_reload_data` not tracking skipped files**: Explicitly documented as intentional (line 1108-1109).

5. **`find_enclosing_symbol` in domain/index.rs**: Correctly finds innermost by highest start line. Not a bug.

6. **Snapshot `mtime_secs` re-read from disk**: The `build_snapshot` function reads mtime from disk at serialization time (line 382-387) rather than using the stored `file.mtime_secs`. This is actually more correct (gets the current mtime, not the potentially stale one from index time).
