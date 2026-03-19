# TODO — RTK-Style Adoption Follow-Up

Date: 2026-03-20

Owner for next session: Claude Code 4.6 Opus

## Goal

Make SymForge behave as closely as possible to the original RTK-style vision:

- SymForge should become the path of least resistance for semantic code-inspection workflows.
- Source-code reads, searches, repo orientation, prompt-context narrowing, and post-edit impact should prefer SymForge-backed paths automatically where the client allows it.
- Docs/config/raw system work must still fail open cleanly.
- Codex-specific limitations must be separated from SymForge product gaps.

## Current State

Version tested:
- installed binary: `C:\Users\poslj\.symforge\bin\symforge.exe`
- version observed: `1.6.0`

Already shipped in `1.6.0`:
- stronger workflow ownership positioning in [README.md](/C:/AI_STUFF/PROGRAMMING/symforge/README.md)
- hook workflow classifier and stronger source-workflow steering in [src/cli/hook.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/cli/hook.rs)
- sidecar workflow adapters and daemon/session exposure in [src/sidecar/handlers.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/sidecar/handlers.rs), [src/sidecar/router.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/sidecar/router.rs), [src/daemon.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/daemon.rs)
- stronger MCP/tool-surface steering in [src/protocol/tools.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/protocol/tools.rs) and [src/protocol/format.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/protocol/format.rs)
- stronger `init` rollout/guidance in [src/cli/init.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/cli/init.rs)
- Kilo fix so global npm install no longer writes workspace-local Kilo config into the npm package directory in [npm/scripts/install.js](/C:/AI_STUFF/PROGRAMMING/symforge/npm/scripts/install.js)
- hook adoption metrics surfaced through `health`

## What Was Verified

Installed/runtime basics:
- `symforge --version` returned `1.6.0`
- `codex mcp list` showed SymForge enabled from `C:\Users\poslj\.symforge\bin\symforge.exe`
- global Codex config still had the canonical allowlist and stronger project-doc fallback config
- global Codex `AGENTS.md` contained the stronger SymForge guidance block

Hook behavior:
- `symforge hook pre-tool` on a Rust source file suggested `get_file_context(...)`
- `symforge hook pre-tool` on `README.md` returned no suggestion

This is good. It shows the intended ownership split is encoded:
- source code: steer toward SymForge
- literal docs/config: do not over-route

Metrics surface:
- `health` now reports hook adoption counters correctly
- the repo-local log path is `.symforge/hook-adoption.log`
- current implementation records `routed`, `no-sidecar`, and `sidecar-error` style outcomes

## Important Constraint

True Codex end-to-end validation was attempted but blocked by a Codex usage-limit error.

That means:
- there is no fresh external Codex JSON event trace for `1.6.0`
- the remaining verdict is based on installed config, shipped runtime code, direct hook probing, and `health` metrics

Do not overclaim beyond that.

## Key Finding

SymForge is now much closer to the RTK idea for hookable clients, but **Codex is still not RTK-like** because Codex still does not expose a transparent hook/session-start enrichment interface.

That means:
- SymForge can steer Codex better
- SymForge cannot currently force Codex down the semantic path the way RTK-style interception can force shell rewrites

This is partly a client limitation, not purely a SymForge design flaw.

## Key Technical Question For Claude

Determine whether the remaining gap is:

1. mostly a **client capability ceiling** for Codex
2. mostly a **runtime bootstrap gap** between hook invocation and repo-local sidecar/session state
3. or both

## Most Important Code Paths

Hook bootstrap and routing:
- [src/cli/hook.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/cli/hook.rs)
  - `run_hook`
  - `read_port_file`
  - `read_session_file`
  - `proxy_path`
  - workflow classifier and adoption logging

Sidecar/session files:
- [src/sidecar/port_file.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/sidecar/port_file.rs)
  - `write_port_file`
  - `write_session_file`
  - `cleanup_session_file`

MCP startup modes:
- [src/main.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/main.rs)
  - `run_local_mcp_server_async`
  - `run_remote_mcp_server_async`

Daemon/session routing:
- [src/daemon.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/daemon.rs)

Init/guidance rollout:
- [src/cli/init.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/cli/init.rs)

Health/metrics presentation:
- [src/protocol/tools.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/protocol/tools.rs)
- [src/protocol/format.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/protocol/format.rs)

## Concrete Observation About Bootstrap

Direct manual hook probes for:
- `hook read`
- `hook grep`
- `hook session-start`
- `hook prompt-submit`

all fail-opened with `no-sidecar` in `health`.

That happened because `run_hook` in [src/cli/hook.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/cli/hook.rs) reads:
- `.symforge/sidecar.port`
- optional session file

and returns fail-open JSON if the repo-local sidecar port file is missing.

However, the daemon-backed MCP path in [src/main.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/main.rs) writes the repo-local sidecar/session files during `run_remote_mcp_server_async`.

That suggests one of two things:
- the manual hook probing was outside the intended lifecycle and the product is behaving correctly
- or the runtime still has a discoverability/bootstrap weakness because hooks feel dead unless the correct MCP startup path already ran

Claude should resolve this explicitly.

## Working Hypothesis

Most likely state:

- **Pre-tool steering is working as designed**
- **hook adoption accounting is working as designed**
- **true automatic workflow ownership depends on repo-local sidecar/session bootstrap**
- **Codex still cannot match RTK fully because it lacks the transparent hook path**

So the next work is probably not “rewrite everything again.” It is likely:
- clarify the intended lifecycle
- improve diagnostics/fallback when sidecar/session state is missing
- reduce ambiguity between home daemon state and repo-local hook state
- prove what is and is not possible for Codex specifically

## Tasks For Claude

### 1. Reproduce the actual intended runtime lifecycle

Do not rely only on manual `symforge hook ...` calls.

Prove the full expected path:
- client starts
- SymForge MCP server starts in the correct mode
- repo-local `.symforge/sidecar.port` and session file are created if expected
- hooks can route to the right sidecar/session endpoint

Focus especially on the distinction between:
- local MCP startup
- daemon-backed MCP startup
- plain `symforge daemon`
- manual hook invocation without an active client session

### 2. Decide whether missing repo-local sidecar/session files are a bug or expected

If expected:
- document it clearly
- make the hook fail-open output and/or `health` message more explicit

If not expected:
- fix the bootstrap path so hooks become usable whenever a real client session is active

### 3. Improve missing-sidecar diagnostics

Current fail-open behavior is safe, but still too opaque.

Add a concrete but concise diagnostic path for:
- no repo-local sidecar port
- missing session file
- sidecar HTTP failure

Potential targets:
- richer hook adoption reporting in `health`
- optional terse reason text in fail-open hook payloads
- better logging in `.symforge/hook-adoption.log`

Do not make fallback noisy enough to annoy normal users.

### 4. Evaluate daemon fallback behavior

Investigate whether hooks can safely fall back when `.symforge/sidecar.port` is absent:
- use daemon session state if available
- or establish a deterministic lookup path from home daemon metadata to repo-local session routing

Be careful here:
- do not invent nondeterministic heuristics
- do not route to the wrong project/session
- preserve fail-open correctness over “clever” behavior

### 5. Separate Codex ceiling from SymForge gaps

Produce a crisp answer for the project:
- what can be made RTK-like today for Claude/Kilo/hookable clients
- what cannot be made RTK-like for Codex until Codex exposes a real hook/session-start surface

This should end the ambiguity between:
- “SymForge still has a flaw”
- and “the client simply does not expose the required interception surface”

### 6. Add regression tests for the final decision

Minimum test areas:
- hook behavior when sidecar/session files exist
- hook behavior when they do not exist
- health/adoption reporting for `routed` vs `no-sidecar` vs `sidecar-error`
- any new daemon/session fallback logic

## Suggested Execution Plan

### Phase A — Diagnose Precisely

1. Inspect:
   - [src/cli/hook.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/cli/hook.rs)
   - [src/main.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/main.rs)
   - [src/sidecar/port_file.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/sidecar/port_file.rs)
   - [src/daemon.rs](/C:/AI_STUFF/PROGRAMMING/symforge/src/daemon.rs)
2. Reproduce the lifecycle with a real hookable client if available.
3. Record whether repo-local sidecar/session files should exist, when, and why.

### Phase B — Fix The Product Gap

If the gap is bootstrap:
- fix bootstrap
- keep behavior deterministic
- keep fail-open

If the gap is mostly diagnosability:
- improve diagnostics
- improve `health`
- improve docs

If the gap is mostly Codex client limitation:
- document that clearly
- stop trying to “fix” what SymForge cannot control

### Phase C — Verify End To End

Run real tests against:
- a hookable client path
- Codex MCP path
- workspace-local Kilo path if available

Acceptance should be based on real client behavior, not only unit tests.

## Acceptance Criteria

Call this done only if all of the following are true:

- it is clear whether missing repo-local sidecar/session files are expected or a bug
- hook fail-open outcomes are diagnosable rather than mysterious
- hookable clients actually get the intended owned-workflow behavior in a real session
- Codex limitations are clearly separated from SymForge runtime limitations
- docs accurately reflect the real lifecycle and boundaries
- tests lock in the chosen behavior

## Useful Commands

Basic checks:

```powershell
C:\Users\poslj\.symforge\bin\symforge.exe --version
codex mcp list
codex mcp get symforge
```

Hook probes:

```powershell
@'
{"tool_name":"Read","tool_input":{"file_path":"C:\\AI_STUFF\\PROGRAMMING\\symforge\\src\\cli\\hook.rs"},"cwd":"C:\\AI_STUFF\\PROGRAMMING\\symforge"}
'@ | C:\Users\poslj\.symforge\bin\symforge.exe hook pre-tool
```

Repo-local state:

```powershell
Get-ChildItem -Force .symforge
Get-Content .symforge\hook-adoption.log
```

Health:

Use the `health` SymForge MCP tool and inspect the `Hook Adoption` section.

## Final Reminder For Claude

Do not start from scratch.

Most of the RTK-style rollout already shipped. The remaining task is to determine exactly where the remaining adoption gap lives:
- client ceiling
- runtime bootstrap
- diagnostics
- or some combination

Use SymForge first for repo-local inspection.
