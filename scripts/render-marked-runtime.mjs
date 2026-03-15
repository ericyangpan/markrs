#!/usr/bin/env node

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');
const markedPkgPath = path.join(repoRoot, 'third_party', 'marked', 'package.json');

function readMarkedVersion() {
  const pkg = JSON.parse(fs.readFileSync(markedPkgPath, 'utf8'));
  return pkg.version;
}

function installMarked(version) {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'markast-runtime-'));
  execFileSync('npm', ['init', '-y'], { cwd: tmpDir, stdio: 'ignore' });
  execFileSync('npm', ['install', `marked@${version}`], { cwd: tmpDir, stdio: 'ignore' });
  return tmpDir;
}

async function main() {
  const input = fs.readFileSync(0, 'utf8');
  const cases = JSON.parse(input);
  const version = readMarkedVersion();
  const installDir = installMarked(version);
  const mod = await import(pathToFileURL(path.join(installDir, 'node_modules', 'marked', 'lib', 'marked.esm.js')).href);
  const { marked } = mod;

  const out = cases.map((item) => ({
    id: item.id,
    html: marked.parse(item.markdown, {
      gfm: item.options.gfm,
      breaks: item.options.breaks,
      pedantic: item.options.pedantic,
      async: false,
    }),
  }));

  process.stdout.write(JSON.stringify({ markedVersion: version, cases: out }));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
