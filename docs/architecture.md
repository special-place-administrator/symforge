# Architecture

Status: First pass  
Date: 2026-03-06

## Purpose

`symforge` is a Rust-native, coding-first MCP server for repository indexing, retrieval, orchestration, and repair.

It is not a carbon copy of SymForge. The design should optimize for:
- speed
- robustness
- idempotency
- deterministic behavior
- self-healing and self-recovery
- byte-exact source retrieval
- strong edge-case handling for real codebases

## Design Position

The system should use a local-first architecture:
- Rust application and MCP surface
- in-process LiveIndex for the hot query path
- local snapshot persistence under `.symforge/`
- tree-sitter parsing and extraction in Rust

This split is deliberate.

The hot path stays in memory for speed, while restart recovery and warm startup come from local serialized state. Raw bytes need exact handling because symbol spans and later retrieval must remain correct on every platform, including Windows.

## Goals

Primary goals:
- index local folders and Git repositories reliably
- support incremental and full indexing
- provide stable symbol identities
- provide verified source retrieval by byte span
- recover from crashes and partial failures
- prevent duplicate mutation side effects via idempotency
- expose enough operational state for debugging and repair

Secondary goals:
- live progress visibility
- strong observability
- extensible memory model for future agent workflows
- eventual semantic recall support without distorting the core design

## Non-Goals

Not first-phase goals:
- exact parity with Python MCPs
- pushing the read path behind an external data service
- embedding-first retrieval
- hiding failure states behind blind retries

## High-Level Architecture

Core subsystems:
- `protocol`
  - MCP tool/resource/prompt surface
  - request validation
  - response shaping
- `application`
  - orchestration services
  - use cases
  - job lifecycle
- `domain`
  - repository, file, symbol, run, checkpoint, lease, idempotency, health models
  - core invariants
- `storage`
  - local snapshot persistence
  - byte-exact local artifacts when persistence is needed
  - snapshots and integrity verification
- `indexing`
  - discovery
  - filtering
  - hashing
  - parsing
  - extraction
  - validation
  - commit pipeline
- `parsing`
  - tree-sitter language bindings
  - language-specific extraction
- `observability`
  - logging
  - metrics
  - traces
  - health reports

## Data Ownership

### In-process runtime state

The running process should own hot operational state:
- repository registration for the current session
- loaded files and content hashes
- symbol and reference metadata
- watcher state
- health and verification state
- transient mutation coordination

This gives us:
- zero-hop query serving
- simpler concurrency control
- deterministic local behavior
- cheaper updates after file changes

### Local persistence: authoritative raw content plane

Raw file bytes and derived recovery artifacts should remain local when persisted.

Why:
- symbol retrieval depends on exact bytes
- newline or encoding normalization can corrupt spans
- large blobs are better handled outside the hot query path
- integrity verification is simpler with direct hash-addressed blobs

Suggested layout:

```text
.symforge/
  blobs/
    sha256/
      ab/
        cd/
          <fullhash>
  temp/
  quarantine/
  derived/
```

Raw content rules:
- store bytes exactly as read
- never normalize line endings
- never decode and re-encode before persistence
- compute spans against exact stored bytes
- write via temp file then atomic rename

## Stable IDs and Retrieval

Stable symbol ID format:

```text
{file_path}::{qualified_name}#{kind}
```

Symbol ID stability comes from:
- path
- qualified name
- symbol kind

Source retrieval does not come from the symbol ID alone. It comes from:
- blob hash
- byte start
- byte length
- verification hash

This distinction is critical. A stable ID can still point to bad bytes if storage is sloppy. This project must prevent that class of error by design.

## Core Domain Model

### Repository

Tracks a logical codebase.

Key fields:
- `repo_id`
- `kind` local or git
- `root_uri`
- `default_branch`
- `last_known_revision`
- `status`

### IndexRun

Tracks one full, incremental, repair, or verification pass.

Key fields:
- `run_id`
- `repo_id`
- `mode`
- `status`
- `requested_at`
- `started_at`
- `finished_at`
- `idempotency_key`
- `request_hash`
- `checkpoint_cursor`
- `error_summary`

### FileRecord

Tracks indexed file metadata.

Key fields:
- `repo_id`
- `path`
- `content_hash`
- `blob_id`
- `language`
- `size_bytes`
- `deleted`
- `last_indexed_run_id`

### SymbolRecord

Tracks extracted symbol metadata.

Key fields:
- `symbol_id`
- `repo_id`
- `file_path`
- `qualified_name`
- `kind`
- `language`
- `span_start_byte`
- `span_len_bytes`
- `content_hash`
- `signature`
- `summary`

### Lease

Tracks ownership of active background work.

Key fields:
- `lease_id`
- `run_id`
- `worker_id`
- `expires_at`
- `state`

### Checkpoint

Supports resumable work.

Key fields:
- `run_id`
- `cursor`
- `files_processed`
- `symbols_written`
- `created_at`

### IdempotencyRecord

Prevents duplicate mutation side effects.

Key fields:
- `operation`
- `idempotency_key`
- `request_hash`
- `status`
- `result_ref`
- `created_at`
- `expires_at`

### HealthEvent

Captures system degradation and repairs.

Key fields:
- `component`
- `severity`
- `message`
- `details`
- `occurred_at`

## Indexing Pipeline

The indexing pipeline should be event-driven and checkpointed.

Proposed flow:
1. Discover candidate files.
2. Apply ignore and security filtering.
3. Read exact raw bytes.
4. Compute content hash.
5. Persist raw bytes into CAS if absent.
6. Detect language.
7. Parse with tree-sitter.
8. Extract symbols and spans.
9. Verify spans against exact bytes.
10. Commit metadata transactionally.
11. Emit progress and checkpoint updates.

Properties:
- resumable
- replayable
- bounded
- backpressured
- incremental when possible
- safe under crash or partial failure

## Idempotency

Mutating operations must be idempotent.

Applies to:
- `index_folder`
- `index_repository`
- `repair_index`
- `checkpoint_now`
- future mutation tools

Model:
1. Client sends `idempotency_key`.
2. Server canonicalizes request args and computes `request_hash`.
3. First execution stores a pending idempotency record.
4. Success stores a durable result reference.
5. Retry with same key and same hash returns stored result.
6. Retry with same key and different hash fails deterministically.

This is required because MCP clients may retry after timeout, cancellation, or transport interruption.

## Recovery and Self-Healing

Self-healing should mean explicit deterministic repair.

Mechanisms:
- startup sweep for stale leases and temp files
- resume from latest checkpoint
- quarantine malformed parses or invalid spans
- scheduled repair tasks
- periodic integrity verification
- health scoring for repos and runs
- explicit repair tools

Failure handling policy:
- process crash
  - recover active runs from DB state and checkpoints
- parser failure on one file
  - quarantine file, continue run, emit health event
- missing blob
  - mark records degraded, schedule repair
- bad span verification
  - quarantine symbol rows, reparse affected file
- lease expiry
  - stop stale worker from committing, resume safely elsewhere

## Concurrency Model

Use Tokio with structured concurrency.

Recommended execution pools:
- discovery workers
- read/hash workers
- parser workers
- commit workers

Recommended control primitives:
- cancellation token per run
- bounded channels between stages
- task tracker for shutdown

Rules:
- CPU-heavy parsing must not block MCP responsiveness
- logically conflicting commits should serialize cleanly
- long-running work should return durable run IDs
- progress should be available through structured state, not logs alone

## MCP Surface

This project should support all three major MCP surfaces over time:
- tools
- resources
- prompts

### Initial tools

- `health`
- `index_folder`
- `index_repository`
- `get_index_run`
- `cancel_index_run`
- `checkpoint_now`
- `repair_index`
- `search_symbols`
- `search_text`
- `get_file_outline`
- `get_symbol`
- `get_symbols`
- `get_repo_outline`
- `invalidate_cache`

### Useful resources

- repository outline
- repository health
- run status
- symbol metadata
- failure diagnostics

### Useful prompts

- codebase audit
- architecture map
- broken-symbol diagnosis
- index failure triage

## Memory Strategy

The project should eventually support multiple memory layers.

### Authoritative memory

Use local persisted state for:
- architecture decisions
- run history
- checkpoints
- idempotency records
- repair history
- tool outcomes

### Code memory

Use structured metadata for:
- files
- symbols
- outlines
- hashes
- repository status

### Semantic memory

Optional later layer:
- embeddings for fuzzy retrieval over docs, notes, conversations, and maybe chunks

Current position:
- local persistence is not a dedicated vector database
- do not contort phase one around ANN requirements
- add semantic retrieval later if it proves useful

## Observability

This system should be debuggable in production-like conditions.

Required:
- structured logging
- counters and timing metrics
- traces for run lifecycle and indexing stages
- health summaries
- explicit degraded-state reporting

Observability questions should be answerable quickly:
- what is running
- what failed
- what was retried
- what was quarantined
- what can be resumed

## Security and Safety

Needed from the start:
- path traversal protection
- symlink policy
- binary detection
- secret and unsafe path exclusion
- repository root confinement
- bounded resource usage

Safety principle:
- if integrity is uncertain, degrade safely and report it
- never fabricate confidence about retrieved code slices

## Recommended Near-Term Milestones

### M1 Foundation

- clean crate/module layout
- error model
- config model
- health tool
- logging and tracing bootstrap
- storage traits for local snapshots and byte-exact artifacts

### M2 Durable control plane

- local snapshot boundary and recovery module
- repository and run domain types
- idempotency model
- checkpoint model
- lease model

### M3 Byte-exact storage and indexing skeleton

- local CAS implementation
- discovery and filtering pipeline
- hashing
- run orchestration
- checkpoint writes

### M4 Parsing and retrieval

- tree-sitter integration
- language support starting with Rust, Python, TS/JS
- stable symbol IDs
- verified `get_symbol`

### M5 Repair and live operations

- repair workflows
- health resources
- progress subscriptions
- integrity sweeps

## Current Strategic Decision

Keep the query path local-first and keep durability local unless a concrete scaling need proves otherwise.

That gives us:
- strong operational state without extra moving parts
- better recovery semantics through local snapshots
- a simpler memory model for the project
- room for future agentic workflows without premature infrastructure

And it avoids:
- blob misuse
- byte corruption risk
- turning the query path into a distributed systems problem

## Final Recommendation

Build SymForge as a Rust-first MCP platform with:
- local durable state under `.symforge/`
- local byte-exact persistence for raw content when needed
- tree-sitter extraction with verification before commit
- checkpointed idempotent jobs
- explicit recovery and repair tooling

That is the right base for a genuinely better coding MCP.
