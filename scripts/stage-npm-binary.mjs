#!/usr/bin/env node

import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { basename, join, resolve } from 'node:path';

function usage() {
  console.error('Usage: node scripts/stage-npm-binary.mjs <package-dir> <binary-path>');
  process.exit(1);
}

const packageDirArg = process.argv[2];
const binaryArg = process.argv[3];

if (!packageDirArg || !binaryArg) {
  usage();
}

const packageDir = resolve(packageDirArg);
const packageJsonPath = join(packageDir, 'package.json');

if (!existsSync(packageJsonPath)) {
  console.error(`[stage] package.json not found: ${packageJsonPath}`);
  process.exit(1);
}

const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
const binaryPath = resolve(binaryArg);

if (!existsSync(binaryPath)) {
  console.error(`[stage] binary not found: ${binaryPath}`);
  process.exit(1);
}

const isWindows = Array.isArray(packageJson.os) && packageJson.os.includes('win32');
const outputFile = isWindows ? 'markast.exe' : 'markast';
const outputPath = join(packageDir, 'bin', outputFile);

mkdirSync(join(packageDir, 'bin'), { recursive: true });
copyFileSync(binaryPath, outputPath);

if (!isWindows) {
  chmodSync(outputPath, 0o755);
}

console.log(`[stage] ${basename(binaryPath)} -> ${outputPath}`);
