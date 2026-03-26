#!/usr/bin/env node

/**
 * Circuit Breaker — PostToolUseFailure Hook
 *
 * Tracks consecutive tool failures. After 3 failures, injects a message
 * suggesting a different approach. After 5 trips in a session, escalates
 * to a hard "stop and rethink" message.
 *
 * State file: .claude/circuit-breaker-state.json
 * Counter resets when the threshold is hit (self-contained state).
 */

const fs = require('fs');
const path = require('path');
const health = require('./harness-health-util');

const PROJECT_ROOT = health.PROJECT_ROOT;
const STATE_FILE = path.join(PROJECT_ROOT, '.claude', 'circuit-breaker-state.json');
const THRESHOLD = 3;

function readState() {
  try {
    return JSON.parse(fs.readFileSync(STATE_FILE, 'utf8'));
  } catch {
    return { consecutiveFailures: 0, lifetimeTrips: 0, lastFailedTool: null, lastFailureTime: null };
  }
}

function writeState(state) {
  const dir = path.dirname(STATE_FILE);
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
  const tmpFile = STATE_FILE + '.tmp';
  fs.writeFileSync(tmpFile, JSON.stringify(state, null, 2));
  fs.renameSync(tmpFile, STATE_FILE);
}

function main() {
  let input = '';
  process.stdin.setEncoding('utf8');
  process.stdin.on('data', (chunk) => { input += chunk; });
  process.stdin.on('end', () => {
    let event;
    try {
      event = JSON.parse(input);
    } catch {
      process.exit(0);
    }

    const toolName = event.tool_name || 'unknown';
    const error = (event.error || event.tool_error || '').toString().slice(0, 200);

    health.increment('circuit-breaker', 'count');

    const state = readState();
    state.consecutiveFailures += 1;
    state.lastFailedTool = toolName;
    state.lastFailureTime = new Date().toISOString();
    state.lastError = error || null;

    writeState(state);

    if (state.consecutiveFailures >= THRESHOLD) {
      health.increment('circuit-breaker', 'trips');
      const lifetimeTrips = (state.lifetimeTrips || 0) + 1;
      // Reset consecutive counter, preserve lifetime trips
      writeState({
        consecutiveFailures: 0,
        lifetimeTrips,
        lastFailedTool: null,
        lastFailureTime: null,
        lastError: null,
      });

      const lines = [
        `[Circuit Breaker] ${THRESHOLD} consecutive tool failures detected (trip #${lifetimeTrips} this session).`,
        `Last failed tool: ${toolName}`,
        error ? `Last error: ${error}` : null,
      ];

      if (lifetimeTrips >= 5) {
        lines.push(
          `WARNING: ${lifetimeTrips} circuit breaker trips this session. You are stuck in a failure loop.`,
          `STOP trying variations of the same approach. Step back and:`,
          `  - Re-read the relevant files from scratch`,
          `  - Consider whether the approach is fundamentally wrong`,
          `  - Try a completely different strategy, not a minor variation`,
        );
      } else {
        lines.push(
          `Consider a different approach:`,
          `  - If editing: re-read the file first, the content may have changed`,
          `  - If running commands: check if a prerequisite step was missed`,
          `  - If searching: try broader/narrower patterns or different file paths`,
          `  - If the same action keeps failing: try an alternative tool or approach`,
        );
      }

      process.stdout.write(lines.filter(Boolean).join('\n'));
    }

    process.exit(0);
  });
}

main();
