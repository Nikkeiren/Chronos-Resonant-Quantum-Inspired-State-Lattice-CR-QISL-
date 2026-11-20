#!/usr/bin/env bash
# Runs every available check. Rust steps are skipped (loudly) when cargo is missing.
set -u
cd "$(dirname "$0")/.."
fail=0
python3 tools/static_check.py || fail=1
python3 tools/check_configs.py || fail=1
if command -v node >/dev/null; then node tools/viewer_smoke_test.js || fail=1; else echo "SKIPPED: node not found (viewer smoke test)"; fi
if command -v cargo >/dev/null; then
  cargo build --release --all-targets || fail=1
  cargo test --release || fail=1
  cargo run --release -- validate --config configs/initial_experiment.toml || fail=1
  for f in configs/scenarios/*.toml; do cargo run --release -- validate --config "$f" || fail=1; done
else
  echo "SKIPPED: cargo not found - Rust build and tests were NOT run"
fi
exit $fail
