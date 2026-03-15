#!/usr/bin/env node

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..');
const resultsDir = path.join(repoRoot, 'bench', 'results');
const resultJsonPath = path.join(resultsDir, 'latest.json');
const historyJsonlPath = path.join(resultsDir, 'history.jsonl');
const readmePath = path.join(repoRoot, 'README.md');
const writeReadme = process.argv.includes('--write-readme');
const noHistory = process.argv.includes('--no-history');
const suiteFilterArg = valueArg('--suite');
const engineFilterArg = valueArg('--engine');
const rustOnly = process.argv.includes('--rust-only');
const jsOnly = process.argv.includes('--js-only');

const warmupRunsOverride = parsePositiveInt(process.env.BENCH_WARMUP_RUNS);
const measureRunsOverride = parsePositiveInt(process.env.BENCH_MEASURE_RUNS);

const SUITES = [
  {
    id: 'readme',
    label: 'README.md',
    source: 'Project README rendered as a single document',
    warmupRuns: 10,
    measureRuns: 30,
    options: { gfm: true, breaks: false, pedantic: false },
    loadDocs: () => [fs.readFileSync(path.join(repoRoot, 'README.md'), 'utf8')],
  },
  {
    id: 'commonmark-core',
    label: 'CommonMark Core',
    source: 'Official CommonMark 0.31.2 JSON examples rendered in non-GFM mode',
    warmupRuns: 4,
    measureRuns: 10,
    options: { gfm: false, breaks: false, pedantic: false },
    loadDocs: () => collectJsonCaseDocs('third_party/marked/test/specs/commonmark/commonmark.0.31.2.json'),
  },
  {
    id: 'fixtures',
    label: 'Marked Fixtures',
    source: '`new` + `original` fixture pairs from vendored marked specs',
    warmupRuns: 4,
    measureRuns: 12,
    options: { gfm: true, breaks: false, pedantic: false },
    loadDocs: () => collectMdFixtureDocs(),
  },
  {
    id: 'full-corpus',
    label: 'Comparable Corpus',
    source: 'All 1485 comparable parser-output cases from vendored marked specs',
    warmupRuns: 4,
    measureRuns: 12,
    options: { gfm: true, breaks: false, pedantic: false },
    loadDocs: () => collectComparableCorpusDocs(),
  },
];

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

function shell(cmd, args, options = {}) {
  const output = execFileSync(cmd, args, {
    cwd: repoRoot,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    ...options,
  });
  return typeof output === 'string' ? output.trim() : '';
}

function stripMarkedFrontMatter(markdown) {
  if (!markdown.startsWith('---\n')) {
    return markdown;
  }

  const end = markdown.indexOf('\n---\n', 4);
  if (end === -1) {
    return markdown;
  }

  return markdown.slice(end + '\n---\n'.length);
}

function walkFiles(dir, ext, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walkFiles(fullPath, ext, out);
      continue;
    }
    if (entry.isFile() && fullPath.endsWith(ext)) {
      out.push(fullPath);
    }
  }
  return out;
}

function collectMdFixtureDocs() {
  const specsRoot = path.join(repoRoot, 'third_party', 'marked', 'test', 'specs');
  const docs = [];
  for (const dir of ['new', 'original']) {
    const root = path.join(specsRoot, dir);
    const files = walkFiles(root, '.md').sort();
    for (const file of files) {
      const markdown = fs.readFileSync(file, 'utf8');
      docs.push(stripMarkedFrontMatter(markdown));
    }
  }
  return docs;
}

function collectJsonCaseDocs(relPath) {
  const file = path.join(repoRoot, relPath);
  const cases = JSON.parse(fs.readFileSync(file, 'utf8'));
  return cases.map((entry) => entry.markdown);
}

function collectComparableCorpusDocs() {
  return [
    ...collectMdFixtureDocs(),
    ...collectJsonCaseDocs('third_party/marked/test/specs/commonmark/commonmark.0.31.2.json'),
    ...collectJsonCaseDocs('third_party/marked/test/specs/gfm/commonmark.0.31.2.json'),
    ...collectJsonCaseDocs('third_party/marked/test/specs/gfm/gfm.0.29.json'),
  ];
}

function buildSuites() {
  const suiteFilter = suiteFilterArg
    ? new Set(suiteFilterArg.split(',').map((id) => id.trim()).filter(Boolean))
    : null;
  const selectedSuites = suiteFilter
    ? SUITES.filter((suite) => suiteFilter.has(suite.id))
    : SUITES;

  if (suiteFilter && selectedSuites.length === 0) {
    throw new Error(`No suites matched --suite ${suiteFilterArg}`);
  }

  return selectedSuites.map((suite) => {
    const docs = suite.loadDocs();
    const inputBytes = docs.reduce((sum, doc) => sum + Buffer.byteLength(doc), 0);
    return {
      id: suite.id,
      label: suite.label,
      source: suite.source,
      warmupRuns: warmupRunsOverride ?? suite.warmupRuns,
      measureRuns: measureRunsOverride ?? suite.measureRuns,
      docs,
      docsCount: docs.length,
      inputBytes,
      options: suite.options,
    };
  });
}

function checksumString(input, seed = 2166136261) {
  let hash = seed >>> 0;
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function trimRunCount(sampleCount) {
  if (sampleCount >= 10) {
    return Math.max(1, Math.floor(sampleCount * 0.1));
  }
  if (sampleCount >= 6) {
    return 1;
  }
  return 0;
}

function summarizeRuns(runsMs, docsCount, inputBytes) {
  const sorted = [...runsMs].sort((a, b) => a - b);
  const meanMs = runsMs.reduce((sum, ms) => sum + ms, 0) / runsMs.length;
  const trimCount = trimRunCount(sorted.length);
  const trimmedRuns =
    trimCount > 0 && trimCount * 2 < sorted.length
      ? sorted.slice(trimCount, sorted.length - trimCount)
      : sorted;
  const trimmedMeanMs =
    trimmedRuns.reduce((sum, ms) => sum + ms, 0) / trimmedRuns.length;
  const medianMs =
    sorted.length % 2 === 0
      ? (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2
      : sorted[Math.floor(sorted.length / 2)];
  return {
    meanMs,
    trimmedMeanMs,
    medianMs,
    minMs: sorted[0],
    maxMs: sorted[sorted.length - 1],
    trimCount,
    docsPerSec: (docsCount * 1000) / trimmedMeanMs,
    mibPerSec: (inputBytes / (1024 * 1024) * 1000) / trimmedMeanMs,
  };
}

function createMarkedRenderer(markedModule) {
  return (markdown, options) =>
    markedModule.marked.parse(markdown, {
      gfm: options.gfm,
      breaks: options.breaks,
      pedantic: options.pedantic,
    });
}

function createRemarkRenderer(remarkModule, remarkGfmModule, remarkHtmlModule) {
  const gfmProcessor = remarkModule
    .remark()
    .use(remarkGfmModule.default)
    .use(remarkHtmlModule.default);
  const commonmarkProcessor = remarkModule
    .remark()
    .use(remarkHtmlModule.default);
  return (markdown, options) => String((options.gfm ? gfmProcessor : commonmarkProcessor).processSync(markdown));
}

function createMarkdownItRenderer(markdownItModule) {
  const MarkdownIt = markdownItModule.default;
  const gfmRenderer = new MarkdownIt({ html: true, linkify: true, breaks: false });
  const commonmarkRenderer = new MarkdownIt('commonmark', { html: true, linkify: false, breaks: false });
  return (markdown, options) => (options.gfm ? gfmRenderer : commonmarkRenderer).render(markdown);
}

function runJsEngine(engineId, renderer, suites) {
  const results = [];
  for (const suite of suites) {
    for (let i = 0; i < suite.warmupRuns; i += 1) {
      for (const doc of suite.docs) {
        renderer(doc, suite.options);
      }
    }

    const runsMs = [];
    let outputBytes = 0;
    let checksum = 0;
    for (let i = 0; i < suite.measureRuns; i += 1) {
      let runOutputBytes = 0;
      let runChecksum = 2166136261;
      const started = performance.now();
      for (const doc of suite.docs) {
        const html = renderer(doc, suite.options);
        runOutputBytes += Buffer.byteLength(html);
        runChecksum = checksumString(html, runChecksum);
      }
      const elapsed = performance.now() - started;
      outputBytes = runOutputBytes;
      checksum = runChecksum >>> 0;
      runsMs.push(elapsed);
    }

    results.push({
      id: suite.id,
      docs: suite.docsCount,
      inputBytes: suite.inputBytes,
      outputBytes,
      checksum,
      warmupRuns: suite.warmupRuns,
      measureRuns: suite.measureRuns,
      runsMs,
      ...summarizeRuns(runsMs, suite.docsCount, suite.inputBytes),
    });
  }

  return { engine: engineId, suites: results };
}

function runRustEngine(engineId, suites) {
  let exePath;
  if (engineId === 'markast') {
    shell('cargo', ['build', '--release', '--bin', 'markast-bench'], { stdio: 'inherit' });
    exePath = path.join(repoRoot, 'target', 'release', 'markast-bench');
  } else if (engineId === 'pulldown-cmark') {
    shell(
      'cargo',
      [
        'build',
        '--release',
        '--manifest-path',
        path.join('bench', 'pulldown-cmark-runner', 'Cargo.toml'),
        '--target-dir',
        path.join('target', 'pulldown-cmark-runner'),
      ],
      { stdio: 'inherit' },
    );
    exePath = path.join(repoRoot, 'target', 'pulldown-cmark-runner', 'release', 'pulldown-cmark-runner');
  } else {
    throw new Error(`Unsupported Rust engine: ${engineId}`);
  }
  const inputPath = path.join(resultsDir, 'markast-bench-input.json');
  fs.mkdirSync(resultsDir, { recursive: true });
  fs.writeFileSync(
    inputPath,
    JSON.stringify({
      suites: suites.map((suite) => ({
        id: suite.id,
        warmup_runs: suite.warmupRuns,
        measure_runs: suite.measureRuns,
        docs: suite.docs,
        options: suite.options,
      })),
    }),
  );
  const raw = shell(exePath, ['--input', inputPath]);
  const parsed = JSON.parse(raw);
  return {
    engine: parsed.engine,
    suites: parsed.suites.map((suite) => ({
      id: suite.id,
      docs: suite.docs,
      inputBytes: suite.input_bytes,
      outputBytes: suite.output_bytes,
      checksum: suite.checksum,
      warmupRuns: suite.warmup_runs,
      measureRuns: suite.measure_runs,
      runsMs: suite.runs_ms,
      ...summarizeRuns(suite.runs_ms, suite.docs, suite.input_bytes),
    })),
  };
}

function shouldRunEngine(engineId) {
  if (engineFilterArg) {
    return engineFilterArg
      .split(',')
      .map((id) => id.trim())
      .filter(Boolean)
      .includes(engineId);
  }

  const isRust = engineId === 'markast' || engineId === 'pulldown-cmark';
  if (rustOnly) return isRust;
  if (jsOnly) return !isRust;
  return true;
}

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MiB`;
}

function formatMs(ms) {
  return `${ms.toFixed(2)}`;
}

function formatSpeedup(baseMs, candidateMs) {
  return `${(baseMs / candidateMs).toFixed(2)}x`;
}

function renderMarkdownReport(report) {
  const lines = [];
  lines.push(`Benchmark date: ${report.generatedAt.slice(0, 10)}`);
  lines.push('');
  lines.push('Method: in-process render throughput on the same default-GFM corpus for all engines. Outputs are not normalized for semantic equality; this report only measures rendering speed on shared inputs. `Trimmed mean ms` drops one run from each side for 6-9 samples, or 10% from each side for 10+ samples.');
  lines.push('');
  lines.push(`Environment: ${report.environment.cpu} | ${report.environment.platform} | Node ${report.versions.node} | Rust ${report.versions.rustc}`);
  lines.push('');
  lines.push('| Suite | Docs | Input size | Warmup | Measured | Source |');
  lines.push('| --- | ---: | ---: | ---: | ---: | --- |');
  for (const suite of report.suites) {
    lines.push(`| ${suite.label} | ${suite.docsCount} | ${formatBytes(suite.inputBytes)} | ${suite.warmupRuns} | ${suite.measureRuns} | ${suite.source} |`);
  }
  lines.push('');
  lines.push('| Suite | Engine | Trimmed mean ms | Median ms | Docs/s | MiB/s | vs marked |');
  lines.push('| --- | --- | ---: | ---: | ---: | ---: | ---: |');

  const enginesPresent = new Set(report.engines.map((engine) => engine.engine));
  const engineOrder = ['markast', 'pulldown-cmark', 'marked', 'markdown-it', 'remark'].filter((engineId) => enginesPresent.has(engineId));
  for (const suite of report.suites) {
    const byEngine = new Map(report.engines.map((engine) => [engine.engine, engine.suites.find((entry) => entry.id === suite.id)]));
    const marked = byEngine.get('marked');
    for (const engineId of engineOrder) {
      const row = byEngine.get(engineId);
      if (!row) {
        continue;
      }
      const label = engineId === 'markast'
        ? 'markast (Rust)'
        : engineId === 'pulldown-cmark'
          ? 'pulldown-cmark (Rust)'
          : engineId === 'marked'
            ? 'marked (JS)'
          : engineId === 'markdown-it'
              ? 'markdown-it (JS)'
              : 'remark + gfm + html';
      const comparisonMs = row.trimmedMeanMs ?? row.meanMs;
      const markedComparisonMs = marked?.trimmedMeanMs ?? marked?.meanMs;
      const speedup = markedComparisonMs ? formatSpeedup(markedComparisonMs, comparisonMs) : '-';
      lines.push(`| ${suite.label} | ${label} | ${formatMs(comparisonMs)} | ${formatMs(row.medianMs)} | ${row.docsPerSec.toFixed(1)} | ${row.mibPerSec.toFixed(2)} | ${speedup} |`);
    }
  }
  lines.push('');
  lines.push(`Raw benchmark data: \`bench/results/latest.json\``);
  return lines.join('\n');
}

function updateReadme(markdown) {
  const start = '<!-- benchmark-report:start -->';
  const end = '<!-- benchmark-report:end -->';
  const readme = fs.readFileSync(readmePath, 'utf8');
  const startIndex = readme.indexOf(start);
  const endIndex = readme.indexOf(end);
  if (startIndex === -1 || endIndex === -1 || endIndex < startIndex) {
    throw new Error('README benchmark markers not found');
  }
  const updated = `${readme.slice(0, startIndex + start.length)}\n${markdown}\n${readme.slice(endIndex)}`;
  fs.writeFileSync(readmePath, updated);
}

function collectEnvironment() {
  return {
    platform: `${os.platform()} ${os.release()} (${os.arch()})`,
    cpu: os.cpus()[0]?.model ?? 'unknown CPU',
    memoryGiB: Number((os.totalmem() / (1024 ** 3)).toFixed(1)),
  };
}

function collectGitInfo() {
  try {
    const commit = shell('git', ['rev-parse', 'HEAD']);
    const branch = shell('git', ['rev-parse', '--abbrev-ref', 'HEAD']);
    const describe = shell('git', ['describe', '--tags', '--always', '--dirty']);
    const porcelain = shell('git', ['status', '--porcelain']);
    const dirtyFiles = porcelain ? porcelain.split('\n').filter(Boolean).length : 0;
    return {
      commit,
      branch,
      describe,
      dirty: dirtyFiles > 0,
      dirtyFiles,
    };
  } catch {
    return null;
  }
}

function collectInvocationInfo() {
  return {
    argv: process.argv.slice(2),
    filters: {
      suite: suiteFilterArg,
      engine: engineFilterArg,
      rustOnly,
      jsOnly,
      writeReadme,
    },
    overrides: {
      warmupRuns: warmupRunsOverride,
      measureRuns: measureRunsOverride,
    },
    env: {
      BENCH_WARMUP_RUNS: process.env.BENCH_WARMUP_RUNS ?? null,
      BENCH_MEASURE_RUNS: process.env.BENCH_MEASURE_RUNS ?? null,
      CI: process.env.CI ?? null,
    },
  };
}

function appendHistoryEntry(entry) {
  fs.mkdirSync(resultsDir, { recursive: true });
  fs.appendFileSync(historyJsonlPath, `${JSON.stringify(entry)}\n`, 'utf8');
}

async function main() {
  if (rustOnly && jsOnly) {
    throw new Error('Cannot combine --rust-only and --js-only');
  }

  if (writeReadme && (suiteFilterArg || engineFilterArg || rustOnly || jsOnly)) {
    throw new Error('Refusing --write-readme with suite/engine filters');
  }

  fs.mkdirSync(resultsDir, { recursive: true });
  const suites = buildSuites();

  const hasJsEngines =
    shouldRunEngine('marked') || shouldRunEngine('markdown-it') || shouldRunEngine('remark');

  const report = {
    generatedAt: new Date().toISOString(),
    environment: collectEnvironment(),
    versions: {
      markast: JSON.parse(fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8')).version,
      marked: '17.0.4',
      markdownIt: '14.1.1',
      pulldownCmark: '0.13.1',
      remark: '15.0.1',
      remarkHtml: '16.0.1',
      remarkGfm: '4.0.1',
      node: process.versions.node,
      rustc: shell('rustc', ['--version']),
    },
    suites: suites.map((suite) => ({
      id: suite.id,
      label: suite.label,
      source: suite.source,
      docsCount: suite.docsCount,
      inputBytes: suite.inputBytes,
      warmupRuns: suite.warmupRuns,
      measureRuns: suite.measureRuns,
    })),
    engines: [],
  };

  if (shouldRunEngine('markast')) {
    report.engines.push(runRustEngine('markast', suites));
  }
  if (shouldRunEngine('pulldown-cmark')) {
    report.engines.push(runRustEngine('pulldown-cmark', suites));
  }

  if (hasJsEngines) {
    const markedModule = await import('marked');
    const markdownItModule = await import('markdown-it');
    const remarkModule = await import('remark');
    const remarkGfmModule = await import('remark-gfm');
    const remarkHtmlModule = await import('remark-html');

    if (shouldRunEngine('marked')) {
      report.engines.push(runJsEngine('marked', createMarkedRenderer(markedModule), suites));
    }
    if (shouldRunEngine('markdown-it')) {
      report.engines.push(runJsEngine('markdown-it', createMarkdownItRenderer(markdownItModule), suites));
    }
    if (shouldRunEngine('remark')) {
      report.engines.push(
        runJsEngine('remark', createRemarkRenderer(remarkModule, remarkGfmModule, remarkHtmlModule), suites),
      );
    }
  }

  if (report.engines.length === 0) {
    throw new Error('No engines selected');
  }

  fs.writeFileSync(resultJsonPath, JSON.stringify(report, null, 2));
  if (!noHistory) {
    appendHistoryEntry({
      historySchema: 1,
      git: collectGitInfo(),
      invocation: collectInvocationInfo(),
      ...report,
    });
  }
  const markdown = renderMarkdownReport(report);
  console.log(markdown);
  if (writeReadme) {
    updateReadme(markdown);
  }
}

main().catch((error) => {
  console.error(`[bench] ${error.stack || error.message}`);
  process.exit(1);
});
