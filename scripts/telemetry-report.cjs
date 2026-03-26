#!/usr/bin/env node

/**
 * telemetry-report.cjs — Generate human-readable telemetry summaries.
 *
 * Usage:
 *   node scripts/telemetry-report.cjs                  Full summary
 *   node scripts/telemetry-report.cjs --last 10        Last N runs
 *   node scripts/telemetry-report.cjs --hooks          Hook timing summary
 *   node scripts/telemetry-report.cjs --compression    Discovery compression stats
 */

const fs = require('fs');
const path = require('path');

const PROJECT_ROOT = process.env.CLAUDE_PROJECT_DIR || process.cwd();
const TELEMETRY_DIR = path.join(PROJECT_ROOT, '.planning', 'telemetry');
const AGENT_LOG = path.join(TELEMETRY_DIR, 'agent-runs.jsonl');
const HOOK_LOG = path.join(TELEMETRY_DIR, 'hook-timing.jsonl');
const COMPRESSION_LOG = path.join(TELEMETRY_DIR, 'compression-stats.jsonl');

function readJsonl(file) {
  if (!fs.existsSync(file)) return [];
  return fs.readFileSync(file, 'utf8')
    .split('\n')
    .filter(Boolean)
    .map(line => {
      try { return JSON.parse(line); } catch { return null; }
    })
    .filter(Boolean);
}

function agentReport(limit) {
  const entries = readJsonl(AGENT_LOG);
  const relevant = limit ? entries.slice(-limit) : entries;

  if (relevant.length === 0) {
    console.log('No agent runs recorded yet.');
    return;
  }

  console.log('\n=== Agent Run Summary ===\n');

  // Count by event type
  const counts = {};
  for (const e of relevant) {
    counts[e.event] = (counts[e.event] || 0) + 1;
  }
  for (const [event, count] of Object.entries(counts)) {
    console.log(`  ${event}: ${count}`);
  }

  // Recent runs
  console.log('\n--- Recent Runs ---\n');
  const recent = relevant.filter(e => e.event === 'agent-complete' || e.event === 'agent-fail').slice(-10);
  for (const e of recent) {
    const duration = e.duration_ms ? `${(e.duration_ms / 1000).toFixed(1)}s` : '?';
    console.log(`  ${e.timestamp.slice(0, 16)} | ${e.agent} | ${e.status || e.event} | ${duration}`);
  }

  console.log(`\nTotal entries: ${entries.length}`);
}

function hookReport() {
  const entries = readJsonl(HOOK_LOG);

  if (entries.length === 0) {
    console.log('No hook timing data recorded yet.');
    return;
  }

  console.log('\n=== Hook Timing Summary ===\n');

  // Group by hook
  const byHook = {};
  for (const e of entries) {
    const key = e.hook || 'unknown';
    if (!byHook[key]) byHook[key] = { count: 0, totalMs: 0, metrics: {} };
    byHook[key].count++;
    if (e.duration_ms) byHook[key].totalMs += e.duration_ms;
    if (e.metric) {
      byHook[key].metrics[e.metric] = (byHook[key].metrics[e.metric] || 0) + 1;
    }
  }

  for (const [hook, data] of Object.entries(byHook)) {
    const avg = data.totalMs > 0 ? `avg ${(data.totalMs / data.count).toFixed(0)}ms` : '';
    console.log(`  ${hook}: ${data.count} events ${avg}`);
    for (const [metric, count] of Object.entries(data.metrics)) {
      console.log(`    ${metric}: ${count}`);
    }
  }
}

function compressionReport() {
  const entries = readJsonl(COMPRESSION_LOG);

  if (entries.length === 0) {
    console.log('No compression stats recorded yet.');
    return;
  }

  console.log('\n=== Discovery Compression Stats ===\n');

  let totalInput = 0, totalOutput = 0;
  for (const e of entries) {
    totalInput += e.inputChars || 0;
    totalOutput += e.outputChars || 0;
  }

  const avgRatio = totalInput > 0 ? (totalOutput / totalInput * 100).toFixed(1) : 0;
  console.log(`  Compressions: ${entries.length}`);
  console.log(`  Total input: ${totalInput} chars`);
  console.log(`  Total output: ${totalOutput} chars`);
  console.log(`  Average ratio: ${avgRatio}%`);

  console.log('\n--- Recent ---\n');
  for (const e of entries.slice(-5)) {
    console.log(`  ${e.agent || '?'}: ${e.inputChars} → ${e.outputChars} chars (${(e.ratio * 100).toFixed(1)}%)`);
  }
}

// ── CLI ──────────────────────────────────────────────────────────────────────

const args = process.argv.slice(2);

if (args.includes('--hooks')) {
  hookReport();
} else if (args.includes('--compression')) {
  compressionReport();
} else {
  const lastIdx = args.indexOf('--last');
  const limit = lastIdx >= 0 ? parseInt(args[lastIdx + 1], 10) : null;
  agentReport(limit);
}
