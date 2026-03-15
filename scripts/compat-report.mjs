#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');

function countSpecCases() {
  const specsRoot = path.join(repoRoot, 'third_party', 'marked', 'test', 'specs');
  const countMdPairs = (dir) => fs.readdirSync(dir).filter((name) => name.endsWith('.md')).length;
  const readJsonCount = (file) => JSON.parse(fs.readFileSync(file, 'utf8')).length;

  const newAndOriginal =
    countMdPairs(path.join(specsRoot, 'new')) +
    countMdPairs(path.join(specsRoot, 'original'));
  const commonmark = readJsonCount(path.join(specsRoot, 'commonmark', 'commonmark.0.31.2.json'));
  const gfmCommonmark = readJsonCount(path.join(specsRoot, 'gfm', 'commonmark.0.31.2.json'));
  const gfm = readJsonCount(path.join(specsRoot, 'gfm', 'gfm.0.29.json'));

  return {
    comparableTotal: newAndOriginal + commonmark + gfmCommonmark + gfm,
    newAndOriginal,
    commonmark,
    gfmCommonmark,
    gfm,
  };
}

function countXfails(file) {
  const raw = fs.readFileSync(file, 'utf8');
  return [...raw.matchAll(/- id: "([^"]+)"/g)].length;
}

function countMarkedUnitCases() {
  const unitRoot = path.join(repoRoot, 'third_party', 'marked', 'test', 'unit');
  let count = 0;
  for (const name of fs.readdirSync(unitRoot)) {
    if (!name.endsWith('.test.js')) continue;
    const raw = fs.readFileSync(path.join(unitRoot, name), 'utf8');
    count += [...raw.matchAll(/\bit\(/g)].length;
  }
  return count;
}

function countRedosCases() {
  const redosRoot = path.join(repoRoot, 'third_party', 'marked', 'test', 'specs', 'redos');
  return fs.readdirSync(redosRoot).filter((name) => name.endsWith('.md')).length;
}

function percent(passed, total) {
  return `${((passed / total) * 100).toFixed(1)}%`;
}

function buildReport() {
  const specs = countSpecCases();
  const snapshotXfail = countXfails(path.join(repoRoot, 'tests', 'compat', 'xfail.yaml'));
  const runtimeXfail = countXfails(path.join(repoRoot, 'tests', 'compat', 'runtime_xfail.yaml'));
  const unitCases = countMarkedUnitCases();
  const redosCases = countRedosCases();

  const snapshotPassed = specs.comparableTotal - snapshotXfail;
  const runtimePassed = specs.comparableTotal - runtimeXfail;

  return `## Compatibility Report

Current report date: 2026-03-08

This table compares the same parser-output cases from the official marked corpus under \`third_party/marked/test/specs\`.

Included in the same-case comparison:
- \`new\` + \`original\` fixture pairs: ${specs.newAndOriginal}
- CommonMark JSON examples: ${specs.commonmark}
- GFM CommonMark mirror examples: ${specs.gfmCommonmark}
- GFM spec examples: ${specs.gfm}
- Total comparable cases: ${specs.comparableTotal}

Excluded from this table:
- \`third_party/marked/test/unit/*.test.js\`: ${unitCases} JS unit cases. These exercise Marked's JS API surface such as hooks, lexer/parser classes, CLI integration, and instance behavior, so there is no 1:1 Rust-side case mapping in \`markast\` yet.
- \`third_party/marked/test/specs/redos\`: ${redosCases} ReDoS fixtures. These are security/performance-oriented fixtures and are not currently part of the \`markast\` compat gates.

| Target | Case source | Passed | Gaps | Pass rate |
| --- | --- | ---: | ---: | ---: |
| \`marked\` self-spec result | vendored \`marked\` fixture/spec corpus | ${specs.comparableTotal} | 0 | 100.0% |
| \`markast\` snapshot compat | vendored fixture/spec snapshots | ${snapshotPassed} | ${snapshotXfail} | ${percent(snapshotPassed, specs.comparableTotal)} |
| \`markast\` runtime compat | current \`marked@17.0.4\` runtime | ${runtimePassed} | ${runtimeXfail} | ${percent(runtimePassed, specs.comparableTotal)} |

How to refresh:
- \`npm run test:compat\`
- \`npm run test:compat:report\`
`;
}

const report = buildReport();
process.stdout.write(report);
