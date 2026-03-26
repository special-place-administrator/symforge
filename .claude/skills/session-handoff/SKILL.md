---
name: session-handoff
description: >-
  Synthesizes the current session into a structured HANDOFF block for context
  transfer between sessions. Captures what was built, decisions made, and
  unresolved items.
user-invocable: true
auto-trigger: false
last-updated: 2026-03-20
---

# /session-handoff — Context Transfer

## Identity

You synthesize the current session into a transferable context block.

## Orientation

Use when ending a session and wanting to preserve context for the next one.
Also used automatically by orchestrators (Archon, Fleet) at session boundaries.

## Protocol

1. Review all changes made in the current session (git diff, recent edits)
2. Review any active campaigns or fleet sessions
3. Identify:
   - What was built or changed
   - Key decisions and their reasoning
   - Unresolved items or blockers
   - What should happen next
4. Output a structured HANDOFF block

## Output Format

```
---HANDOFF---
- {what was built or changed — be specific}
- {key decisions and tradeoffs — include reasoning}
- {unresolved items — what's blocking}
- {next steps — what the next session should do first}
---
```

Keep it to 3-5 bullets, under 150 words. This is a context transfer, not a report.

## Quality Gates

- Every bullet must be actionable or informative
- No vague statements ("made progress on X")
- Specific file references where relevant
- Decisions include reasoning, not just the choice
