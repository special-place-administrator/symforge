#!/usr/bin/env node

/**
 * intake-scanner.js — SessionStart hook
 *
 * Reports pending work items from .planning/intake/ on every new session.
 * Gives Claude awareness of what needs attention without human prompting.
 */

const fs = require('fs');
const path = require('path');
const health = require('./harness-health-util');

const PROJECT_ROOT = health.PROJECT_ROOT;
const INTAKE_DIR = path.join(PROJECT_ROOT, '.planning', 'intake');

function main() {
  health.increment('intake-scanner', 'count');

  if (!fs.existsSync(INTAKE_DIR)) {
    process.exit(0);
  }

  const files = fs.readdirSync(INTAKE_DIR).filter(f =>
    f.endsWith('.md') && !f.startsWith('_') && !f.startsWith('.')
  );

  if (files.length === 0) {
    process.exit(0);
  }

  const items = [];
  for (const file of files) {
    const content = fs.readFileSync(path.join(INTAKE_DIR, file), 'utf8');
    const titleMatch = content.match(/^title:\s*"?(.+?)"?\s*$/m);
    const statusMatch = content.match(/^status:\s*(\w+)/m);
    const status = statusMatch ? statusMatch[1] : 'pending';

    if (status === 'completed' || status === 'archived') continue;

    items.push({
      file: file.replace('.md', ''),
      title: titleMatch ? titleMatch[1] : file.replace('.md', '').replace(/-/g, ' '),
      status,
    });
  }

  if (items.length === 0) {
    process.exit(0);
  }

  const pending = items.filter(i => i.status === 'pending');
  const inProgress = items.filter(i => i.status === 'in-progress' || i.status === 'briefed');

  const lines = ['[Intake] Work items detected:'];

  if (pending.length > 0) {
    lines.push(`  ${pending.length} pending:`);
    for (const item of pending) {
      lines.push(`    → ${item.file}: "${item.title}"`);
    }
  }

  if (inProgress.length > 0) {
    lines.push(`  ${inProgress.length} in progress:`);
    for (const item of inProgress) {
      lines.push(`    → ${item.file}: "${item.title}" [${item.status}]`);
    }
  }

  lines.push('  Run /do status for details, or /autopilot to process pending items.');

  process.stdout.write(lines.join('\n'));
  process.exit(0);
}

main();
