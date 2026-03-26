#!/usr/bin/env node

/**
 * restore-compact.js — SessionStart hook (compact matcher)
 *
 * Re-injects critical context after Claude's message compression.
 * Reads the state saved by pre-compact.js and outputs it so Claude
 * knows what was happening before the compaction.
 */

const fs = require('fs');
const path = require('path');
const health = require('./harness-health-util');

const PROJECT_ROOT = health.PROJECT_ROOT;
const STATE_FILE = path.join(PROJECT_ROOT, '.claude', 'compact-state.json');

function main() {
  if (!fs.existsSync(STATE_FILE)) {
    process.exit(0);
  }

  let state;
  try {
    state = JSON.parse(fs.readFileSync(STATE_FILE, 'utf8'));
  } catch {
    process.exit(0);
  }

  const lines = ['[Context Restored] State from before compaction:'];

  if (state.activeCampaign) {
    lines.push(`  Active campaign: ${state.activeCampaign}`);
    lines.push(`  Campaign file: .planning/campaigns/${state.activeCampaign}.md`);
  }

  if (state.activeFleetSession) {
    lines.push(`  Active fleet session: ${state.activeFleetSession}`);
    lines.push(`  Session file: .planning/fleet/session-${state.activeFleetSession}.md`);
  }

  if (state.recentContext) {
    lines.push(`  Context: ${state.recentContext}`);
  }

  if (lines.length > 1) {
    lines.push('  Read the campaign/session file to rebuild full context.');
    process.stdout.write(lines.join('\n'));
  }

  process.exit(0);
}

main();
