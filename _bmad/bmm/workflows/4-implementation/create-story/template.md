# Story {{epic_num}}.{{story_num}}: {{story_title}}

Status: ready-for-dev

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a {{role}},
I want {{action}},
so that {{benefit}}.

## Acceptance Criteria

1. [Add acceptance criteria from epics/PRD]

## Tasks / Subtasks

- [ ] Task 1 (AC: #)
  - [ ] Subtask 1.1
- [ ] Task 2 (AC: #)
  - [ ] Subtask 2.1

## Dev Notes

- Relevant architecture patterns and constraints
- Source tree components to touch
- Testing standards summary

### Self-Audit Checklist (mandatory before requesting review)

_Run this checklist after all tasks are complete. This is a blocking step — do not request review until every item is verified._

#### Generic Verification
- [ ] For every task marked `[x]`, cite the specific test that verifies it
- [ ] For every new error variant or branch, confirm a test exercises it
- [ ] For every computed value, trace it to where it surfaces (log, return value, persistence)
- [ ] For every test, verify the assertion can actually fail (no `assert!(true)`, no conditionals that always pass)

#### Epic-Specific Trust Verification
<!-- Update these items per-epic. Below are Epic 3 defaults. -->
- [ ] For every retrieval path, confirm a test exercises blob_id trust verification
- [ ] For every query, confirm a test exercises the invalidated/unhealthy rejection path
- [ ] For every "no results" path, confirm the response distinguishes empty vs missing vs stale

### Project Structure Notes

- Alignment with unified project structure (paths, modules, naming)
- Detected conflicts or variances (with rationale)

### References

- Cite all technical details with source paths and sections, e.g. [Source: docs/<file>.md#Section]

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List
