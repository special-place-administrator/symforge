#!/usr/bin/env node

/**
 * pre-compact.js — PreCompact hook
 *
 * Saves critical context state before Claude's message compression.
 * The restore-compact.js hook re-injects this on the next SessionStart.
 *
 * What it saves:
 * - Active campaign slug (if any)
 * - Active fleet session (if any)
 * - Recent intake items
 * - Any in-progress task context
 */

const fs = require('fs');
const path = require('path');
const health = require('./harness-health-util');

const PROJECT_ROOT = health.PROJECT_ROOT;
const STATE_FILE = path.join(PROJECT_ROOT, '.claude', 'compact-state.json');

function main() {
  health.increment('pre-compact', 'count');

  const state = {
    savedAt: new Date().toISOString(),
    activeCampaign: null,
    activeFleetSession: null,
    recentContext: null,
  };

  // Find active campaign
  const campaignsDir = path.join(PROJECT_ROOT, '.planning', 'campaigns');
  if (fs.existsSync(campaignsDir)) {
    try {
      const files = fs.readdirSync(campaignsDir).filter(f => f.endsWith('.md'));
      for (const file of files) {
        const content = fs.readFileSync(path.join(campaignsDir, file), 'utf8');
        if (/^Status:\s*active/mi.test(content)) {
          state.activeCampaign = file.replace('.md', '');
          // Extract the Active Context section
          const ctxMatch = content.match(/## Active Context\s*\n([\s\S]*?)(?=\n## |\n---|$)/);
          if (ctxMatch) {
            state.recentContext = ctxMatch[1].trim().slice(0, 500);
          }
          break;
        }
      }
    } catch { /* non-critical */ }
  }

  // Find active fleet session
  const fleetDir = path.join(PROJECT_ROOT, '.planning', 'fleet');
  if (fs.existsSync(fleetDir)) {
    try {
      const files = fs.readdirSync(fleetDir).filter(f => f.startsWith('session-') && f.endsWith('.md'));
      for (const file of files) {
        const content = fs.readFileSync(path.join(fleetDir, file), 'utf8');
        if (/status:\s*(active|needs-continue)/mi.test(content)) {
          state.activeFleetSession = file.replace('session-', '').replace('.md', '');
          break;
        }
      }
    } catch { /* non-critical */ }
  }

  // Write state
  try {
    const dir = path.dirname(STATE_FILE);
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
  } catch { /* non-critical */ }

  process.exit(0);
}

main();
