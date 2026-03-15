#!/usr/bin/env node

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');
const specsRoot = path.join(repoRoot, 'third_party', 'marked', 'test', 'specs');
const markedPkgPath = path.join(repoRoot, 'third_party', 'marked', 'package.json');
const snapshotXfailPath = path.join(repoRoot, 'tests', 'compat', 'xfail.yaml');

function parseXfailIds(raw) {
  return [...raw.matchAll(/- id: "([^"]+)"/g)].map((match) => match[1]);
}

function normalizeInputAttrs(input) {
  return input.replace(/<input([^>]*)>/g, (_, raw) => {
    const attrs = parseAttrs(raw);
    if (attrs.length === 0) {
      return '<input>';
    }
    attrs.sort((a, b) => a[0].localeCompare(b[0]));
    return `<input${attrs.map(([name, value]) => value == null ? ` ${name}` : ` ${name}="${value}"`).join('')}>`;
  });
}

function parseAttrs(raw) {
  const chars = [...raw];
  const out = [];
  let i = 0;
  while (i < chars.length) {
    while (i < chars.length && /\s/.test(chars[i])) {
      i += 1;
    }
    if (i >= chars.length) {
      break;
    }
    const start = i;
    while (i < chars.length && !/\s|=/.test(chars[i])) {
      i += 1;
    }
    if (start === i) {
      i += 1;
      continue;
    }
    const name = chars.slice(start, i).join('');
    while (i < chars.length && /\s/.test(chars[i])) {
      i += 1;
    }
    let value = null;
    if (i < chars.length && chars[i] === '=') {
      i += 1;
      while (i < chars.length && /\s/.test(chars[i])) {
        i += 1;
      }
      if (i < chars.length && (chars[i] === '"' || chars[i] === '\'')) {
        const quote = chars[i];
        i += 1;
        const valueStart = i;
        while (i < chars.length && chars[i] !== quote) {
          i += 1;
        }
        value = chars.slice(valueStart, i).join('');
        if (i < chars.length && chars[i] === quote) {
          i += 1;
        }
      } else {
        const valueStart = i;
        while (i < chars.length && !/\s/.test(chars[i])) {
          i += 1;
        }
        value = chars.slice(valueStart, i).join('');
      }
    }
    out.push([name, value]);
  }
  return out;
}

function normalizeHtml(input) {
  return normalizeInputAttrs(input)
    .replace(/>\s+</g, '><')
    .replace(/<(br|hr|img|input)([^>]*?)\s*\/?\s*>/g, '<$1$2>')
    .replace(/<br>\s*\n+/g, '<br>')
    .replace(/([^\s>])\s+<blockquote>/g, '$1<blockquote>')
    .replace(/\r\n/g, '\n')
    .replace(/&quot;|&#34;|&#x22;/g, '"')
    .replace(/&#39;|&#x27;|&apos;/g, '\'')
    .replace(/&gt;/g, '>')
    .split('\n')
    .map((line) => line.replace(/\s+$/g, ''))
    .filter((line) => line.trim() !== '')
    .join('\n')
    .trim();
}

function shouldUseGfm(caseId) {
  if (caseId.includes('/commonmark/') || caseId.includes('/commonmark.')) {
    return false;
  }
  if (caseId.includes('_nogfm')) {
    return false;
  }
  return true;
}

function stripMarkedFrontMatter(markdown, options) {
  if (!markdown.startsWith('---\n')) {
    return markdown;
  }
  const rest = markdown.slice(4);
  const end = rest.indexOf('\n---\n');
  if (end === -1) {
    return markdown;
  }
  const header = rest.slice(0, end);
  for (const line of header.split('\n')) {
    const idx = line.indexOf(':');
    if (idx === -1) {
      continue;
    }
    const key = line.slice(0, idx).trim();
    const value = line.slice(idx + 1).trim();
    if (key === 'gfm') {
      options.gfm = value === 'true';
    }
    if (key === 'breaks') {
      options.breaks = value === 'true';
    }
    if (key === 'pedantic') {
      options.pedantic = value === 'true';
    }
  }
  return rest.slice(end + '\n---\n'.length);
}

function loadMarkedVersion() {
  const pkg = JSON.parse(fs.readFileSync(markedPkgPath, 'utf8'));
  return pkg.version;
}

function installMarked(version) {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'markast-marked-'));
  execFileSync('npm', ['init', '-y'], {
    cwd: tmpDir,
    stdio: 'ignore',
  });
  execFileSync('npm', ['install', `marked@${version}`], {
    cwd: tmpDir,
    stdio: 'ignore',
  });
  return tmpDir;
}

async function main() {
  const requestedIds = process.argv.slice(2);
  const ids = requestedIds.length > 0
    ? requestedIds
    : parseXfailIds(fs.readFileSync(snapshotXfailPath, 'utf8'));
  if (ids.length === 0) {
    console.log('no case ids to inspect');
    return;
  }

  const version = loadMarkedVersion();
  const installDir = installMarked(version);
  const markedModule = await import(pathToFileURL(path.join(installDir, 'node_modules', 'marked', 'lib', 'marked.esm.js')).href);
  const { marked } = markedModule;

  let matched = 0;
  let stale = 0;

  for (const id of ids) {
    const markdownPath = path.join(specsRoot, id);
    const htmlPath = markdownPath.replace(/\.md$/, '.html');
    if (!fs.existsSync(markdownPath) || !fs.existsSync(htmlPath)) {
      console.log(`MISSING\t${id}`);
      continue;
    }

    const markdownRaw = fs.readFileSync(markdownPath, 'utf8');
    const expected = fs.readFileSync(htmlPath, 'utf8');
    const options = {
      gfm: shouldUseGfm(id),
      breaks: false,
      pedantic: false,
      async: false,
    };
    const markdown = stripMarkedFrontMatter(markdownRaw, options);
    const runtimeHtml = marked.parse(markdown, options);
    const ok = normalizeHtml(runtimeHtml) === normalizeHtml(expected);
    if (ok) {
      matched += 1;
      console.log(`MATCH\t${id}`);
    } else {
      stale += 1;
      console.log(`STALE\t${id}`);
    }
  }

  console.log(`\nsummary: checked=${ids.length} match=${matched} stale=${stale} marked=${version} source=snapshot-xfail`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
