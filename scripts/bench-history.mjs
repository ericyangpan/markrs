#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..');
const defaultHistoryPath = path.join(repoRoot, 'bench', 'results', 'history.jsonl');

function valueArg(flag) {
  const idx = process.argv.indexOf(flag);
  if (idx === -1) return null;
  if (idx + 1 >= process.argv.length) {
    throw new Error(`Missing value after ${flag}`);
  }
  return process.argv[idx + 1];
}

function parsePositiveInt(raw) {
  if (!raw) return null;
  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0 || !Number.isInteger(value)) {
    throw new Error(`Expected positive integer, got ${raw}`);
  }
  return value;
}

function median(values) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 0) {
    return (sorted[mid - 1] + sorted[mid]) / 2;
  }
  return sorted[mid];
}

function getSuite(entry, engineId, suiteId) {
  const engine = entry.engines?.find((candidate) => candidate.engine === engineId);
  if (!engine) return null;
  const suite = engine.suites?.find((candidate) => candidate.id === suiteId);
  if (!suite) return null;
  const ms = suite.trimmedMeanMs ?? suite.meanMs;
  return {
    ms,
    n: Array.isArray(suite.runsMs) ? suite.runsMs.length : null,
    trimCount: suite.trimCount ?? null,
  };
}

function formatMs(ms) {
  return ms == null ? '-' : ms.toFixed(3);
}

function formatRatio(ratio) {
  return ratio == null ? '-' : `${ratio.toFixed(2)}x`;
}

function main() {
  const historyPath = valueArg('--path') ?? defaultHistoryPath;
  const suiteArg = valueArg('--suite') ?? 'full-corpus';
  const engineId = valueArg('--engine') ?? 'markast';
  const baselineId = valueArg('--baseline') ?? 'marked';
  const limit = parsePositiveInt(valueArg('--limit') ?? '20') ?? 20;
  const byCommit = process.argv.includes('--by-commit');
  const jsonOut = process.argv.includes('--json');

  if (!fs.existsSync(historyPath)) {
    throw new Error(`History file not found: ${historyPath}`);
  }

  const lines = fs.readFileSync(historyPath, 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);

  const entries = [];
  for (let i = 0; i < lines.length; i += 1) {
    try {
      entries.push(JSON.parse(lines[i]));
    } catch (error) {
      throw new Error(`Failed to parse JSON on line ${i + 1}: ${error.message}`);
    }
  }

  entries.sort((a, b) => String(a.generatedAt ?? '').localeCompare(String(b.generatedAt ?? '')));

  const suiteIds = suiteArg === 'all'
    ? Array.from(new Set(entries.flatMap((entry) => (entry.suites ?? []).map((suite) => suite.id)))).sort()
    : suiteArg.split(',').map((id) => id.trim()).filter(Boolean);

  const rows = [];
  for (const entry of entries) {
    const generatedAt = entry.generatedAt ?? '';
    const commit = entry.git?.commit?.slice(0, 7) ?? 'unknown';
    const dirty = Boolean(entry.git?.dirty);
    for (const suiteId of suiteIds) {
      const baseline = getSuite(entry, baselineId, suiteId);
      const candidate = getSuite(entry, engineId, suiteId);
      if (!baseline || !candidate) continue;
      rows.push({
        generatedAt,
        commit,
        dirty,
        suiteId,
        engineId,
        baselineId,
        engineMs: candidate.ms,
        baselineMs: baseline.ms,
        speedup: baseline.ms / candidate.ms,
        engineN: candidate.n,
        baselineN: baseline.n,
        engineTrimCount: candidate.trimCount,
        baselineTrimCount: baseline.trimCount,
      });
    }
  }

  const selected = rows.slice(Math.max(0, rows.length - limit));

  if (byCommit) {
    const grouped = new Map();
    for (const row of selected) {
      const key = `${row.commit}|${row.suiteId}`;
      const bucket = grouped.get(key) ?? [];
      bucket.push(row);
      grouped.set(key, bucket);
    }

    const groupedRows = [];
    for (const [key, bucket] of grouped.entries()) {
      const [commit, suiteId] = key.split('|');
      groupedRows.push({
        commit,
        suiteId,
        count: bucket.length,
        dirty: bucket.some((row) => row.dirty),
        latestAt: bucket.reduce((best, row) => (row.generatedAt > best ? row.generatedAt : best), bucket[0].generatedAt),
        medianSpeedup: median(bucket.map((row) => row.speedup)),
        medianEngineMs: median(bucket.map((row) => row.engineMs)),
        medianBaselineMs: median(bucket.map((row) => row.baselineMs)),
      });
    }

    groupedRows.sort((a, b) => String(a.latestAt).localeCompare(String(b.latestAt)));
    const groupedLimited = groupedRows.slice(Math.max(0, groupedRows.length - limit));

    if (jsonOut) {
      console.log(JSON.stringify(groupedLimited, null, 2));
      return;
    }

    const linesOut = [];
    linesOut.push(`History: ${path.relative(repoRoot, historyPath)} (${entries.length} entries)`);
    linesOut.push(`Suite: ${suiteArg} | Engine: ${engineId} | Baseline: ${baselineId} | Group: commit`);
    linesOut.push('');
    linesOut.push('| Latest | Commit | Dirty | Suite | Runs | Engine ms (median) | Baseline ms (median) | Speedup (median) |');
    linesOut.push('| --- | --- | --- | --- | ---: | ---: | ---: | ---: |');
    for (const row of groupedLimited) {
      linesOut.push(`| ${String(row.latestAt).slice(0, 19)} | ${row.commit} | ${row.dirty ? 'yes' : 'no'} | ${row.suiteId} | ${row.count} | ${formatMs(row.medianEngineMs)} | ${formatMs(row.medianBaselineMs)} | ${formatRatio(row.medianSpeedup)} |`);
    }
    console.log(linesOut.join('\n'));
    return;
  }

  if (jsonOut) {
    console.log(JSON.stringify(selected, null, 2));
    return;
  }

  const speeds = selected.map((row) => row.speedup);
  const speedMedian = median(speeds);
  const speedMin = speeds.length ? Math.min(...speeds) : null;
  const speedMax = speeds.length ? Math.max(...speeds) : null;

  const linesOut = [];
  linesOut.push(`History: ${path.relative(repoRoot, historyPath)} (${entries.length} entries)`);
  linesOut.push(`Suite: ${suiteArg} | Engine: ${engineId} | Baseline: ${baselineId}`);
  linesOut.push(`Speedup stats (selected rows): min=${formatRatio(speedMin)} median=${formatRatio(speedMedian)} max=${formatRatio(speedMax)}`);
  linesOut.push('');
  linesOut.push('| When | Commit | Dirty | Suite | Engine ms | Baseline ms | Speedup | n(engine) | n(base) |');
  linesOut.push('| --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: |');
  for (const row of selected) {
    linesOut.push(`| ${String(row.generatedAt).slice(0, 19)} | ${row.commit} | ${row.dirty ? 'yes' : 'no'} | ${row.suiteId} | ${formatMs(row.engineMs)} | ${formatMs(row.baselineMs)} | ${formatRatio(row.speedup)} | ${row.engineN ?? '-'} | ${row.baselineN ?? '-'} |`);
  }
  console.log(linesOut.join('\n'));
}

try {
  main();
} catch (error) {
  console.error(`[bench-history] ${error.stack || error.message}`);
  process.exit(1);
}

