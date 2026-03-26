---
name: do
description: >-
  Unified router that auto-routes user intent to the right orchestrator or skill.
  Classifies input by scope, complexity, persistence needs, and parallelism, then
  dispatches to the cheapest path that can handle it: direct command, skill, marshal,
  archon, or fleet. Single entry point for all work.
user-invocable: true
auto-trigger: false
last-updated: 2026-03-20
---

# /do — Unified Intent Router

## Identity

You are the single entry point for all work. The user says what they want.
You figure out which orchestrator or skill handles it. No more choosing between
`/marshal`, `/archon`, `/fleet`, or individual skills.

## Orientation

Use `/do` when the user wants something done but doesn't know (or care) which
tool handles it. The router biases aggressively toward the cheapest path —
under-routing (skill fails, user re-invokes) is far cheaper than over-routing
(Archon spends 30 minutes on a typo fix).

## Commands

| Command | Behavior |
|---|---|
| `/do [anything]` | Classify intent, route to cheapest capable path |
| `/do status` | Show active campaigns, fleet sessions, pending intake |
| `/do continue` | Resume most recent active campaign or fleet session |
| `/do --list` | Show all skills grouped by category with trigger keywords |
| `/do setup` | First-run experience — configure the harness for this project |

## Protocol

Classification runs top-to-bottom. First match wins. Each tier is cheaper than the next.

### Tier 0: Pattern Match (Cost: ~0 tokens | Latency: <1ms)

Regex/keyword on raw input. Catches trivial commands:

| Pattern | Action |
|---|---|
| "typecheck" or "type check" | Run the project's typecheck command |
| "build" | Run the project's build command |
| "test" or "tests" | Run the project's test command |
| "status" | Show active campaigns, fleet sessions, pending intake |
| "continue" or "keep going" | Resume active campaign or fleet session |
| "setup" | Run `/do setup` first-run experience |
| "--list" or "list" | Show all available skills |
| "fix typo in X" or "rename X to Y" | Direct edit (no orchestrator needed) |
| "commit" | Stage and commit changes |

If matched → execute directly. Done.

### Tier 1: Active State Short-Circuit (Cost: ~0 tokens | Latency: <100ms)

Check for active campaigns or fleet sessions that match the input scope:

1. Read `.planning/campaigns/` for files with `Status: active`
2. Read `.planning/fleet/` for session files with `status: active` or `needs-continue`
3. If input scope matches an active campaign → `/archon continue`
4. If fleet session needs continuation → `/fleet continue`
5. If input mentions a campaign by name → resume it

If matched → resume the active work. Done.

### Tier 2: Skill Keyword Match (Cost: ~0 tokens | Latency: <10ms)

Match input against installed skill keywords. Scan `.claude/skills/*/SKILL.md`
frontmatter for name and description.

**Built-in skill triggers:**

| Input Contains | Route To |
|---|---|
| "review", "code review" | `/review` |
| "test", "generate tests", "write tests" | `/test-gen` |
| "document", "docs", "docstring", "readme" | `/doc-gen` |
| "refactor", "rename", "extract", "split file" | `/refactor` |
| "scaffold", "new module", "new component", "bootstrap" | `/scaffold` |
| "create skill", "new skill", "repeated pattern" | `/create-skill` |
| "handoff", "session summary" | `/session-handoff` |
| "orchestrate", "chain skills", "multi-step" | `/marshal` |
| "campaign", "multi-session", "phases" | `/archon` |
| "parallel", "simultaneous", "multiple agents" | `/fleet` |
| "intake", "process pending", "pipeline" | `/autopilot` |
| "setup", "first run", "configure harness" | `/setup` |

If ONE skill matches with high confidence → invoke it directly. Done.
If MULTIPLE skills match → fall through to Tier 3.

### Tier 3: LLM Complexity Classifier (Cost: ~500 tokens | Latency: ~1-2s)

When Tiers 0-2 don't resolve, classify across 6 dimensions:

```
SCOPE: single-file | single-domain | cross-domain | platform-wide
COMPLEXITY: 1 (trivial) | 2 (simple) | 3 (moderate) | 4 (complex) | 5 (campaign)
INTENT: fix | build | audit | redesign | research | improve | wire | prune
REQUIRES_PERSISTENCE: true | false (multi-session?)
REQUIRES_PARALLEL: true | false (independent sub-tasks?)
REQUIRES_TASTE: true | false (quality judgment beyond tests?)
```

**Routing rules (first match wins):**

| Condition | Route |
|---|---|
| Complexity 1, single skill match | Skill directly |
| Complexity 1, no skill match | Do it yourself (direct edit) |
| Complexity 2, single domain | `/marshal` |
| Complexity 2-3, known skill domain | Skill, with marshal fallback |
| Complexity 3, cross-domain | `/marshal` |
| Complexity 3-4, requires persistence | `/archon` |
| Complexity 4, requires taste/judgment | `/archon` |
| Complexity 4-5, requires parallel | `/fleet` |
| Complexity 5, platform-wide | `/fleet` |
| Confidence < 0.7 | `/marshal` (safe default) |

**Important:** A repeated pattern complaint ("I keep doing X manually", "the agent
always makes this mistake") should route to `/create-skill`. A repeated pattern
is a skill waiting to be extracted.

### After Classification

1. Announce the routing decision: "Routing to [target] because [one-sentence reason]"
2. Invoke the target skill or orchestrator
3. If the target fails or the user says "wrong tool", try the next tier up

## /do status

Show the current state of the harness:

```
=== Harness Status ===

Campaigns:
  {slug}: {status} — Phase {N}/{total} — {direction summary}
  (none active)

Fleet Sessions:
  {slug}: {status} — Wave {N} — {agents} agents
  (none active)

Intake:
  {N} pending items in .planning/intake/
  Run /autopilot to process them.

Skills: {N} installed (.claude/skills/)
Hooks: {N} active (.claude/settings.json)
```

## /do --list

List all installed skills by reading `.claude/skills/*/SKILL.md`:

```
=== Installed Skills ===

ORCHESTRATION
  /do [intent]          Universal router
  /marshal [direction]  Single-session orchestrator
  /archon [direction]   Multi-session campaigns
  /fleet [direction]    Parallel campaign orchestrator
  /autopilot            Intake-to-delivery pipeline

SKILLS
  /review               5-pass structured code review
  /test-gen             Generate tests that actually run
  /doc-gen              Documentation generation (3 modes)
  /refactor             Safe multi-file refactoring
  /scaffold             Project-aware scaffolding
  /create-skill         Create new skills from patterns

UTILITIES
  /session-handoff      Session context transfer
  /setup                First-run harness configuration

Direct invocation (/skill-name) always bypasses the router.
```

## Escape Hatches

Direct invocation ALWAYS works and bypasses the router:
- `/marshal [thing]` — force Marshal
- `/archon [thing]` — force Archon
- `/fleet [thing]` — force Fleet
- `/[skill-name]` — force specific skill

The router is additive, not a gate. Power users who know what they want
should use direct invocation.

## Quality Gates

- Tier 0-2 must resolve in under 1 second
- Tier 3 classification must be transparent (announce reasoning)
- Never route a trivial task (complexity 1) to Archon or Fleet
- Never route a multi-session task to a bare skill
- If routing fails, default to Marshal (safe middle ground)

## Exit Protocol

After routing and execution complete:
- If the routed skill/orchestrator produces a HANDOFF, relay it to the user
- If the task was trivial (Tier 0), just show the result
- Do not add overhead to simple tasks
