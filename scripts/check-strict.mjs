#!/usr/bin/env node

import { spawnSync } from 'node:child_process';

function run(command, args) {
  const rustflags = [process.env.RUSTFLAGS, '-Dwarnings'].filter(Boolean).join(' ');
  const result = spawnSync(command, args, {
    stdio: 'inherit',
    env: {
      ...process.env,
      RUSTFLAGS: rustflags,
    },
  });

  if (result.error) {
    throw result.error;
  }

  if (typeof result.status === 'number' && result.status !== 0) {
    process.exit(result.status);
  }

  if (result.signal) {
    process.kill(process.pid, result.signal);
  }
}

run('cargo', ['check']);
run('cargo', ['test', '--all-targets']);
