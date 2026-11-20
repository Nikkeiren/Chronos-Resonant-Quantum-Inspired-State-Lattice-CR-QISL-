#!/usr/bin/env bash
# Initial experiment: 4 variants x 30 seeds, n=100, 20% loss, partition 60-140, horizon 300.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo run --release -- batch --config configs/initial_experiment.toml --out "${1:-out/initial_experiment}"
