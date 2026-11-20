#!/usr/bin/env bash
# Runs every scenario (SEEDS per variant, default 10). Output: out/scenarios/<name>/
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release
for f in configs/scenarios/*.toml; do
  name=$(basename "$f" .toml)
  echo "== $name"
  cargo run --release -q -- batch --config "$f" --seeds "${SEEDS:-10}" --out "out/scenarios/$name"
done
