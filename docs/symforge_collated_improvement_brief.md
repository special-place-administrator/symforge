# SymForge Improvement Brief (Collated and Deduplicated)

## Purpose
This document consolidates two separate critique streams into one implementation-ready brief for improving SymForge. Repeated points have been merged, overlapping recommendations collapsed, and the result reorganized into a single action plan.

---

## Executive Summary

SymForge is already strong for symbol-oriented work in typed languages, especially C# and TypeScript. It performs well for interface discovery, call-path tracing, targeted context retrieval, and reference-oriented review workflows.

Where it falls short is repository intelligence outside traditional source code. The biggest gaps are SQL migrations, XML/MSBuild files, YAML/CI configuration, shell scripts, large text files, and cross-file causal debugging. In practice, these gaps force fallback to shell tools, raw file reads, and diffs.

### Central product rule
**Every repository file should be searchable, resolvable, explainable, and editable through one consistent interface.**

That is the threshold between “excellent code intelligence” and “complete repository intelligence.”

---

## What SymForge Already Does Well

### Strengths
- Strong symbol navigation for typed code, especially C# and TypeScript.
- Useful symbol context retrieval and caller/callee tracing.
- Efficient targeted inspection compared with broad file dumps.
- Good reference-oriented review flow from changed symbols to related code.
- Solid repository navigation in structured source trees.

### Current positioning
For application code alone, SymForge is already close to default-tool status.

---

## Core Problem Statement

SymForge treats non-code operational files as second-class citizens. That creates inconsistency in the exact areas that dominate real-world debugging:
- database migrations
- CI/CD configuration
- build and restore metadata
- shell scripts
- large repository documents

This leads to two major trust failures:
1. A file may be readable through one path but unresolved or “not found” through another.
2. Search and context behavior may appear incomplete or unreliable on large or non-symbol files.

Once that happens, an LLM stops trusting the tool and reaches for external fallbacks.

---

## Main Findings

## 1. File Resolution and Search Are Not Yet Consistent Enough

### Observed issues
- Some repo files are readable via raw file access but cannot be resolved by higher-level tools.
- Plain-text search is not reliably repo-wide for SQL, XML, YAML, Markdown, shell scripts, and CI files.
- Search misses obvious literals in migration files.
- Search failures are not sufficiently explained.

### Why this matters
This is a tool-trust issue, not just a UX issue. If a file exists and can be read, the system must never behave as though it does not exist.

### Required improvements
- Universal file registry for all repo files.
- Guaranteed path resolution even for non-indexed files.
- Repo-wide raw-text fallback search.
- Search result metadata showing whether the answer came from symbol, structured, raw-text, or combined indices.
- Better no-match diagnostics, including explicit reasons.
- Health/index coverage reporting by file type and capability.

---

## 2. Large-File Navigation Needs to Be Deterministic

### Observed issues
- Pagination over large files is unreliable.
- Chunk navigation may repeat content or return the wrong region.
- Match-local context may jump back to file start instead of centering on the requested occurrence.

### Why this matters
Large migration files, CI manifests, and docs are common failure surfaces. If file navigation is unstable, the entire investigation flow becomes unstable.

### Required improvements
- Stable paging with exact line windows or durable cursors.
- Exact nth-match or match-index navigation.
- Deterministic local context around a specific occurrence.
- Semantic-node chunking where possible instead of raw fixed slices.

---

## 3. Non-Code Files Need First-Class Structural Support

### Problem
SymForge is strongest where symbol graphs exist. Outside that world, it falls back too quickly to raw text.

### Required file-type support
#### SQL / migrations
- statement boundaries
- block boundaries
- DDL/DML classification
- object references
- literal extraction
- migration ordering
- dependency relationships
- Oracle-aware dialect handling

#### XML / MSBuild
- PropertyGroup
- ItemGroup
- PackageReference
- ProjectReference
- imports
- target frameworks
- central package versioning

#### YAML / CI
- jobs
- stages
- image/runtime
- needs/dependencies
- rules
- artifacts
- includes/extends
- variables

#### Other structured text
- JSON
- TOML
- Markdown headings/sections
- shell script entry points and side effects

### Principle
Non-code files should be represented as structured nodes, blocks, and facts, not just raw text spans.

---

## 4. SQL and Migration Intelligence Is the Highest-Value Gap

This is the most important improvement area.

### Current limitation
SQL is treated too much like text presence and not enough like executable structure.

### Missing capabilities
- SQL object index.
- Migration object timeline.
- Dependency graph between migrations.
- Detection of prerequisites and unresolved references.
- Constraint-aware reasoning.
- Classification of statement semantics.
- Oracle-specific dialect support.

### Questions SymForge should be able to answer directly
- Which migration creates a given object or seed prerequisite?
- Which later migrations depend on that object?
- What changed in a constraint or enum/check over time?
- Is this statement read-only or executable DML?
- Why does this migration fail in ordered fixture execution?

### Recommended implementation approach
Do **not** begin with full SQL execution semantics. Start with a hybrid model:
- fast lexical scanner for splitting and extraction
- targeted grammar-backed parser for high-value constructs
- semantic facts model for objects, references, dependencies, and failure surfaces

### Minimum useful SQL features
- SQL statement classifier
- migration object index
- unresolved prerequisite detection
- object history timeline
- semantic migration summary

---

## 5. CI, YAML, and Shell Reasoning Is Underpowered

### Current limitation
SymForge is still code-centric rather than pipeline-centric.

### Missing capabilities
- CI job graph.
- Stage-to-stage causal tracing.
- Runtime/image risk surfacing.
- Artifact provenance.
- Script entrypoint mapping.
- Linkage from job → script → test fixture → migration runner → failing migration.

### Required improvements
- Structural CI parsing.
- Shell fact extraction.
- Job context and dependency explainers.
- Execution-path trace across pipeline and code.
- Semantic diff support for CI and shell assets.

---

## 6. Cross-Language Causal Debugging Must Become First-Class

### Problem
Many real failures are not located in one file or one language. They cross boundaries such as:
- CI job definition
- shell entrypoint
- test fixture
- migration runner
- migration file
- database object state

### Missing capability
SymForge can inspect each part, but it cannot yet present one coherent causal chain.

### Required outcome
A single result should be able to show:
- what triggered the failing path
- what executed next
- which file introduced the problem
- which prerequisite was missing or changed
- which changed files are most likely responsible

This is more than references. It is execution causality.

---

## 7. Non-Code Semantic Diff Support Is Needed

### Current limitation
Raw `git diff` is still more useful than SymForge for many non-symbol assets.

### Required improvements
#### SQL diff
- object created/altered/dropped
- constraint or enum/check widened/tightened
- idempotency change
- fail-fast vs no-op behavior change

#### CI diff
- job added/removed
- stage changed
- image changed
- needs changed
- script path changed
- artifact behavior changed

#### Shell / docs diff
- block-level summaries
- semantic effect summaries

---

## 8. Log-Aware Failure Analysis Would Be High ROI

### Missing capabilities
- Parse pasted CI logs.
- Collapse noise while preserving the exception chain.
- Detect the first meaningful failure instead of only the terminal symptom.
- Map stack traces to repo files and changed files.
- Recognize known error signatures.

### Why this matters
Pipeline failures are often diagnosed from logs first, code second. Strong log ingestion would make SymForge much more useful in real debugging sessions.

---

## 9. Editing Must Be Symmetric Across Code and Non-Code Assets

### Current limitation
Symbol-addressed editing is strong for code, but SQL/XML/YAML work still falls back to generic patching.

### Required editing primitives
- replace in file
- replace nth match
- insert after match
- replace explicit range
- edit structured node
- edit semantic block

### Goal
SQL, XML, YAML, and CI files should be editable with the same confidence and specificity currently available for code symbols.

---

## 10. Response Shaping Needs to Be Summary-First

A repeated theme across both sources is that SymForge should return the right unit of information, not the largest amount of text.

### Required response model
Default every tool to:
1. short answer
2. compact supporting facts
3. optional expandable evidence

### Strong defaults
- no full-file dumps by default
- no large raw excerpts unless explicitly requested
- return the smallest semantically sufficient unit
- include confidence and provenance metadata

This improves both speed and token efficiency.

---

## Proposed Architecture Direction

## Shift the internal model from:
**code index**

## to:
**repository intelligence engine**

### Core design
Use a unified file catalog and multiple retrieval lanes behind a consistent interface.

### Recommended layers
1. **File catalog**
   - FileId
   - normalized path
   - type/classification
   - hash
   - size
   - timestamps
   - line index

2. **Symbol lane**
   - typed code symbols
   - references
   - callers/callees

3. **Structured lane**
   - SQL blocks and objects
   - XML/MSBuild nodes
   - YAML/CI nodes
   - JSON/TOML structures
   - Markdown sections

4. **Raw-text lane**
   - fallback index for every repo file

5. **Fact graph / relation graph**
   - creates
   - alters
   - depends on
   - runs in
   - produced by
   - fails in
   - references

### Key rule
Every query should hit the cheapest lane that can answer it correctly.

---

## Recommended Capability Set

## Foundation capabilities
- universal file catalog
- repo-wide raw-text index
- deterministic paging and match addressing
- search coverage metadata
- index coverage health reporting

## Structured understanding capabilities
- SQL parser + extractor
- XML/MSBuild semantic model
- YAML/CI semantic model
- shell fact extractor
- Markdown/JSON/TOML structural summaries

## Intelligence capabilities
- migration dependency reasoning
- package graph / restore conflict explainer
- CI job graph and execution path reasoning
- changed-file relevance ranking
- causal trace across file types
- log correlation and error-signature mapping

## Editing capabilities
- structured node editing
- block editing
- nth-match replacement
- range replacement

## UX / tooling capabilities
- summary-first response shaping
- result handles with expandable contexts
- confidence and fallback metadata
- investigation bundles for active sessions

---

## Suggested High-Value Product APIs

These came up repeatedly in different wording and can be normalized into one product surface.

### SQL / migration
- `get_sql_outline(path)`
- `trace_sql_object_history(object)`
- `find_unresolved_prerequisites(target)`
- `classify_sql_statement(statement_or_span)`
- `summarize_migration_diff(path)`
- `explain_migration_failure(target)`
- `migration_order_simulation(folder, target)`

### .NET / XML
- `get_package_graph(scope)`
- `explain_restore_conflict(package)`
- `find_package_references(name)`
- `trace_project_reference(path)`

### CI / shell / logs
- `get_ci_job_context(job)`
- `trace_ci_execution_path(job)`
- `find_job_dependencies(job)`
- `map_log_to_repo(log)`
- `explain_ci_failure(log_or_job)`

### Cross-language debugging
- `trace_execution_chain(target)`
- `rank_changed_files_by_failure_relevance(target)`
- `trace_failure(input)`
- `build_investigation_bundle(target)`

---

## Prioritized Roadmap

## Phase 1 — Fix Trust and Coverage
This phase should come first.

1. Universal file catalog.
2. Repo-wide raw-text search across all files.
3. Reliable path resolution for every file.
4. Deterministic paging and nth-match context.
5. Better no-match diagnostics and index coverage reporting.

### Why Phase 1 first
Without this, the system continues to feel internally inconsistent.

---

## Phase 2 — Make Non-Code Files First-Class
1. SQL structural outline and statement classifier.
2. XML/MSBuild semantic model.
3. YAML/CI semantic model.
4. Shell fact extraction.
5. Structured non-code editing primitives.

### Why Phase 2 next
This is the step that moves SymForge from code intelligence to repository intelligence.

---

## Phase 3 — Add Debugging Intelligence
1. Migration dependency model.
2. CI job graph.
3. Cross-language causal trace.
4. Log-to-file and stack-trace mapping.
5. Changed-file relevance ranker.

### Why Phase 3 matters
This is what makes the tool useful in real root-cause analysis, not just navigation.

---

## Phase 4 — Add Domain Explainability and Compression
1. Restore conflict explainer.
2. Migration failure explainer.
3. Semantic diff summaries.
4. Investigation bundles.
5. Summary-first adaptive response shaping.

---

## If the Priority Is Speed First

The right strategy is **layered indexing + cheap fact extraction + planner-based retrieval**.

### Recommended principles
- Treat SymForge as a layered indexer, not a generic search tool.
- Use fast, specialized indices for path, symbols, text spans, facts, and relations.
- Prefer fact lookup and graph traversal over file reads.
- Use a hybrid SQL pipeline: cheap lexical extraction first, deeper parsing second.
- Build compact causal graphs for CI, scripts, tests, and migrations.
- Return summary-first payloads with top evidence and explicit confidence.

### Speed-first milestones
1. SQL statement classifier and migration fact extraction.
2. CI job graph and shell fact extraction.
3. Cross-language causal graph.
4. Session investigation bundles.
5. Planner that only falls back to raw file reads when necessary.

### Success metric for speed-first mode
The tool should answer these quickly and in one shot:
- Why did this migration fail?
- What earlier migration should have created this prerequisite?
- What CI job actually runs this failing code path?
- What are the top three changed files most likely responsible?

---

## If the Priority Is Token Saving First

The right strategy is **precomputed abstractions + semantic compression + question-centric responses**.

### Recommended principles
- Optimize to avoid source retrieval most of the time.
- Store semantic digests, not only indexes.
- Make every tool answer in compression levels.
- Prefer graph traversal and summaries over excerpts.
- Use hierarchical summaries everywhere.
- Treat raw source as an escape hatch, not the default answer.

### Required compression concepts
- per-file semantic digest
- reasoning-ready facts
- ranked top evidence only
- answer templates for common root-cause explanations
- semantic compression caches
- reusable repository memory objects

### Token-first phases
1. Semantic digests for SQL, YAML, shell, and fixtures.
2. Multi-level summaries for every tool.
3. Migration timeline and CI job graph summaries.
4. Root-cause explanation templates and causal traces.
5. Adaptive answer shaping based on token budget.

### Success metric for token-first mode
The tool should usually answer with facts and implications, not source text.

---

## Rust Implementation Guidance

If implemented in Rust, the architecture should favor compact facts, incremental indexing, and summary-first query planning.

### Useful building blocks mentioned in the source notes
- file walking / ignore handling
- filesystem watching
- fast hashing
- memory-mapped reads
- literal and regex search
- full-text indexing
- path lookup structures
- tree-sitter for code parsing
- lightweight parsers for XML/YAML/JSON/TOML/Markdown
- compact metadata storage and fact graph storage

### Implementation principle
Do not optimize around parser completeness first. Optimize around whether the system can answer high-value debugging questions with minimal retrieval and minimal token cost.

---

## Practical Acceptance Criteria

SymForge should be considered materially improved when it can do all of the following reliably:

### Coverage and consistency
- Resolve any repo file by path.
- Search any repo file for literals or text spans.
- Explain what index lanes were used.
- Explain why a search failed.

### Large-file reliability
- Page large files without repetition.
- Jump to nth match reliably.
- Return local context around the exact occurrence requested.

### Non-code intelligence
- Summarize SQL/XML/YAML/shell files structurally.
- Edit non-code assets with first-class operations.
- Explain package restore conflicts from MSBuild files.
- Explain CI job structure and dependencies.

### Root-cause analysis
- Trace migration prerequisites.
- Classify SQL statement semantics.
- Map CI failures to relevant files and steps.
- Produce cross-language causal chains.
- Rank changed files by probable responsibility.

### Output quality
- Return summary-first responses.
- Include confidence metadata.
- Avoid file dumps unless explicitly requested.

---

## Final Recommendation

The clearest synthesis of both source critiques is this:

### SymForge should stop thinking of itself as a code intelligence tool with extra file access.
### It should become a repository intelligence engine.

If only a small number of things are implemented first, the most valuable sequence is:
1. universal file catalog and repo-wide raw-text search
2. deterministic paging and match-local navigation
3. SQL/migration intelligence
4. XML/MSBuild package and restore intelligence
5. YAML/CI intelligence
6. structured editing for non-code files
7. cross-language causal tracing and log-aware debugging
8. summary-first, confidence-aware outputs

That is the path from “excellent for source code” to “indispensable for real repository debugging.”
