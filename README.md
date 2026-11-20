# CR-QISL experimental simulation platform

**Chronos-Resonant Quantum-Inspired State Lattice (CR-QISL)** — a *quantum-inspired classical*
distributed-state simulation: every node keeps a complex amplitude `a·e^{iθ}` per candidate state, exchanges
it asynchronously over an unreliable network, aggregates neighbours with phase interaction, damps, L1-normalises
and takes a *local* stability decision.

> **Research position.** This is **not** a physical quantum implementation and **not** an established consensus
> protocol. No safety, Byzantine-tolerance, liveness or finality guarantee is claimed or implied anywhere in this
> repository. A node in the `Stable` state has made a **local** decision; "agreement" in the telemetry is an
> **observer-level** measurement that no node can compute.
>
> **Equations.** The CR-QISL specification (v0.1.0–v0.1.2) was not available to the implementation as an explicit
> equation set. Every update rule is therefore an *implementation assumption* (`ASSUMPTION[Axx]` in the code),
> listed in [`docs/ASSUMPTIONS.md`](docs/ASSUMPTIONS.md), configurable, and kept in one replaceable place
> (`src/model.rs`). Results are properties of **this** simulator under **these** assumptions.


## Getting started

See [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) for running CI and the 120-run main experiment, downloading artifacts, reading analysis outputs, and opening the interactive viewer.

## Contents

| Path | What |
|---|---|
| `src/rng.rs` | deterministic xoshiro256** + counter-based hashing (common random numbers) |
| `src/config.rs` | every experiment parameter, defaults = initial experiment, validation |
| `src/topology.rs` | ring, Erdős–Rényi, small-world, scale-free, grid, complete; partition groups |
| `src/network.rs` | asynchronous latency (fixed/uniform/exponential, asymmetric), loss, partition/healing, byte accounting |
| `src/model.rs` | amplitude/phase equations, damping, L1 normalisation, integrity weights, validation (**assumptions**) |
| `src/node.rs` | local stability decision, `CONFLICT_PENDING`, witnesses, reconciliation, delta gating, heartbeat |
| `src/sim.rs` | tick engine, adversaries, telemetry, metrics, trace export |
| `src/metrics.rs`, `src/stats.rs`, `src/analysis.rs` | run summaries/CSV, statistics, paired analysis |
| `src/experiment.rs`, `src/main.rs` | batch runner (threads, failure capture, exports) and CLI |
| `viewer/index.html` | single-file interactive visualisation (no build step) |
| `configs/` | `initial_experiment.toml`, `quick.toml`, `scenarios/*.toml` |
| `docs/` | assumptions, limitations, metric definitions |
| `tools/`, `scripts/` | static checks, viewer smoke test, run helpers |

## Quick start

```bash
cargo test --release                                    # unit + integration tests
cargo run --release -- batch --config configs/quick.toml --out out/quick
cargo run --release -- batch --config configs/initial_experiment.toml --out out/initial   # 4 variants x 30 seeds
cargo run --release -- run --config configs/initial_experiment.toml --variant full_cr_qisl --seed 1 --trace
# then open viewer/index.html and load out/.../traces/*.trace.json
```

Without `--config` the built-in defaults (= `configs/initial_experiment.toml`) are used. `crqisl default-config`
prints the full configuration; every key is optional in a config file (missing keys use defaults) and unknown keys
are rejected, so typos are caught.

### Initial experiment

n = 100 nodes (small-world, k = 6), 2 competing states, 30 seeds (1–30), 20 % packet loss, asynchronous uniform
latency 1–5 ticks, partition at tick 60, healing at tick 140, horizon 300 ticks, variants A–D:

| id | variant | features |
|---|---|---|
| A | `weighted_gossip` | weighted neighbour averaging only |
| B | `weighted_gossip_damping` | A + damping |
| C | `phase_ablation` | B + integrity weights + conflict witnesses + reconciliation, **phase dynamics off** |
| D | `full_cr_qisl` | C + phase dynamics (complex aggregation / resonance) |

All variants share the same communication policy (delta gating, heartbeat, input validation), the same local
decision rule, and — for a given seed — the same topology, partition groups, adversary set, initial state and
**channel randomness** (loss and latency draws are keyed by seed/link/tick, not by event order).

Competing states: right after the partition starts, each side receives evidence for a different candidate
(assumption A8, scheduled `[[events]]`). Without partition-time evidence nothing makes either candidate win.

### Scenarios (`configs/scenarios/`)

`baseline`, `packet_loss`, `asymmetric_latency`, `partition_healing`, `adversarial_amplitude`, `adversarial_phase`,
`invalid_numeric` (+ `_no_validation`), `majority_adversary_breakdown`, `conflict_reconciliation` (+ `_off`),
`long_horizon`. Run all with `scripts/run_scenarios.sh`. The adversarial scenarios do **not** assert that the
integrity filter protects anything; they record what happened (`adv_target_adoption_final`, …).

## Outputs (per batch, no timestamps → reruns are byte-identical)

```
config.resolved.{json,toml}   manifest.json   runs.csv   runs.json
series/<variant>_seed<N>.csv      observer-level per-tick series
events/<variant>_seed<N>.csv      partition/heal, injections, stable/revoke, witness, conflict events
traces/<variant>_seed<N>.trace.json   per-node amplitude + phase + state history (viewer input)
analysis/{descriptive.csv, paired_tests.csv, report.md}
```

Recorded metrics are defined in [`docs/METRICS.md`](docs/METRICS.md): convergence/agreement/stability times
(right-censored and flagged), divergence before/after healing, amplitude-spread diagnostics (final and post-heal mean/min), decision oscillation, message / byte / drop /
heartbeat / witness / reconciliation counts, per-node histories. `retransmissions` and `signature_ops` are always
0 because no retransmission layer and no authentication exist.

## Statistical analysis

`analysis/report.md` (also `crqisl analyze --runs runs.csv`): median, IQR, paired Wilcoxon signed-rank
(zero differences dropped, exact for n ≤ 30 without ties, else tie- and continuity-corrected normal approximation),
matched-pairs rank-biserial correlation, Cohen's d_z, Holm–Bonferroni over all metric × variant-pair tests.
Censoring: never-occurring times are kept at the horizon in the primary analysis and dropped pairwise in a reported
sensitivity analysis. Failed runs are never silently dropped; each exclusion is listed. With the default 300-tick
horizon, strict all-node agreement after healing is often censored — use `configs/scenarios/long_horizon.toml`
and the secondary `quorum_agreement_time_after_heal` metric.

## Determinism

`Rng` = xoshiro256** seeded through SplitMix64; independent streams per purpose (topology, partition, adversaries,
amplitudes, phases). Thread count does not affect results (runs are independent; outputs are ordered by job).

## Verification status — read this

This code was written in an environment **without a Rust toolchain or network**. What was and was not verified:

* **Not run:** `cargo build`, `cargo test`, `cargo clippy`. The Rust has never been compiled by its author. Expect
  that a first build may report small compile errors or warnings; they should be local and mechanical. Please run
  `cargo test --release` first and report/fix what it shows.
* **Run:** `tools/static_check.py` (delimiters, modules, imports, associated functions, metric columns),
  `tools/check_configs.py` (all TOML files vs. the Rust config structs), `tools/viewer_smoke_test.js` (viewer logic
  against a DOM/canvas stub). Statistical routines (exact/normal Wilcoxon, `erfc`, Holm) and RNG reference
  values in the Rust unit tests were computed independently with SciPy/Python (`tools/prototype/stats_crosscheck.py`).
* **Behavioural sanity check:** the core dynamics (update rule, decision FSM, witnesses, reconciliation, gating,
  loss/latency/partition) were prototyped in Python (`tools/prototype/behavioural_port.py`, NOT part of the engine; different RNG streams) only to
  choose non-degenerate defaults and to confirm that the qualitative assertions in the integration tests are plausible.
  The integration tests that depend on dynamics (e.g. “a partition splits decisions”, “healing triggers witnesses”)
  therefore quantify over several seeds. **No scientific result is claimed from that prototype.**
* The viewer was not opened in a real browser; layout/appearance is unverified.

`scripts/verify.sh` runs everything that is available.

## License / provenance

No license is asserted by this package; add the repository's license file as appropriate.
