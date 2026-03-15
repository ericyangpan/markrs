#!/usr/bin/env node

const { existsSync } = require('node:fs');
const { dirname, join } = require('node:path');
const { spawnSync } = require('node:child_process');

const packageRoot = join(__dirname, '..');
const binaryName = process.platform === 'win32' ? 'markast.exe' : 'markast';
const localBinaryPath = join(packageRoot, 'target', 'release', binaryName);

const PLATFORM_PACKAGES = {
  'darwin-arm64': 'markast-darwin-arm64',
  'darwin-x64': 'markast-darwin-x64',
  'linux-arm64': 'markast-linux-arm64-gnu',
  'linux-x64': 'markast-linux-x64-gnu',
  'win32-x64': 'markast-win32-x64-msvc',
};

function run(cmd, args) {
  const res = spawnSync(cmd, args, {
    stdio: 'inherit',
    cwd: packageRoot,
  });

  if (res.error) {
    console.error(`[markast] Failed to execute ${cmd}: ${res.error.message}`);
    process.exit(1);
  }

  if (typeof res.status === 'number') {
    process.exit(res.status);
  }

  process.exit(1);
}

function resolvePrebuiltBinary() {
  const key = `${process.platform}-${process.arch}`;
  const pkg = PLATFORM_PACKAGES[key];
  if (!pkg) {
    return null;
  }

  try {
    const packageJsonPath = require.resolve(`${pkg}/package.json`);
    const packageDir = dirname(packageJsonPath);
    const binaryPath = join(packageDir, 'bin', binaryName);
    if (existsSync(binaryPath)) {
      return binaryPath;
    }
  } catch {
    return null;
  }

  return null;
}

function failNoBinary() {
  const key = `${process.platform}-${process.arch}`;
  const expectedPkg = PLATFORM_PACKAGES[key];

  const lines = [
    `[markast] No prebuilt binary found for ${key}.`,
  ];

  if (expectedPkg) {
    lines.push(`[markast] Expected optional dependency: ${expectedPkg}`);
    lines.push('[markast] Reinstall with optional deps enabled:');
    lines.push('  npm i markast --include=optional');
  } else {
    lines.push('[markast] This platform is not supported by prebuilt releases yet.');
  }

  lines.push('[markast] For local development, set MARKAST_BUILD_FROM_SOURCE=1 to build from source.');

  console.error(lines.join('\n'));
  process.exit(1);
}

const prebuilt = resolvePrebuiltBinary();
if (prebuilt) {
  run(prebuilt, process.argv.slice(2));
}

if (existsSync(localBinaryPath)) {
  run(localBinaryPath, process.argv.slice(2));
}

if (process.env.MARKAST_BUILD_FROM_SOURCE === '1') {
  run('cargo', ['build', '--release']);
  run(localBinaryPath, process.argv.slice(2));
}

failNoBinary();
