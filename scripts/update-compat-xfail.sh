#!/usr/bin/env bash
set -euo pipefail
MARKAST_WRITE_XFAIL=1 cargo test --test compat_snapshot -- --nocapture
