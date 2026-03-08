#!/usr/bin/env bash
set -euo pipefail
MARKRS_WRITE_XFAIL=1 cargo test --test compat_snapshot -- --nocapture
